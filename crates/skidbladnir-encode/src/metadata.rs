//! Keeping a source image's ICC profile, EXIF and XMP in the WebP written from it —
//! `cwebp -metadata`, reproduced byte for byte.
//!
//! Without this every conversion drops them, which is `cwebp`'s default and the Electron
//! app's only behaviour. Dropping the ICC profile is the one that shows: a Display P3 or
//! Adobe RGB photo converted without it is read as sRGB and its colours shift.
//!
//! Both halves follow libwebp 1.6.0 rule for rule, because `cwebp` is the reference:
//!
//! - **Reading** is `imageio`'s. JPEG: the first `APP1` `Exif` and XMP segments, and the ICC
//!   profile reassembled from its `APP2` segments by sequence number, refusing a profile
//!   whose segments are missing, duplicated or inconsistent (`cwebp` then refuses the
//!   whole file). PNG: `iCCP` (inflated), `eXIf`, the `XML:com.adobe.xmp` text chunk and
//!   `ImageMagick`'s hex "Raw profile type" text chunks. TIFF: the ICC profile and XMP tags
//!   (`cwebp` reads no EXIF from TIFF). WebP: the `ICCP`, `EXIF` and `XMP ` chunks its
//!   `VP8X` header declares. Other inputs carry nothing `cwebp` could read.
//! - **Writing** is `WriteWebPWithMetadata`'s: `RIFF`, a `VP8X` header (created, or the
//!   encoder's own with the flags added), `ICCP`, the image chunks, `EXIF`, `XMP `.
//!
//! The gate is `tests/metadata.rs`: files written this way are byte-identical to
//! `cwebp -metadata ...` on fixtures carrying each kind of metadata.

use crate::{settings::KeepMetadata, source::SourceFormat};

/// The metadata found in a source image. `None` where the file has none (or none that
/// `cwebp` would read).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
	/// The ICC colour profile.
	pub icc: Option<Vec<u8>>,
	/// The EXIF block, as the TIFF-structured bytes after JPEG's `Exif\0\0` header.
	pub exif: Option<Vec<u8>>,
	/// The XMP packet.
	pub xmp: Option<Vec<u8>>,
}

impl Metadata {
	/// Whether there is nothing to keep.
	#[must_use]
	pub const fn is_empty(&self) -> bool {
		self.icc.is_none() && self.exif.is_none() && self.xmp.is_none()
	}
}

/// Read the metadata of the image file in `bytes`, as `cwebp`'s readers do.
///
/// # Errors
///
/// A description of the malformed metadata, where `cwebp` would refuse the file for it:
/// an ICC profile split inconsistently across JPEG segments, or a PNG raw profile that is
/// not the hex it claims to be.
pub fn extract(bytes: &[u8]) -> Result<Metadata, String> {
	match SourceFormat::sniff(bytes) {
		Some(SourceFormat::Jpeg) => from_jpeg(bytes),
		Some(SourceFormat::Png) => from_png(bytes),
		Some(SourceFormat::Tiff) => Ok(from_tiff(bytes)),
		Some(SourceFormat::Webp) => Ok(from_webp(bytes)),
		_ => Ok(Metadata::default()),
	}
}

/// JPEG, as `imageio/jpegdec.c` reads it: markers up to the start of scan.
fn from_jpeg(bytes: &[u8]) -> Result<Metadata, String> {
	const APP1: u8 = 0xe1;
	const APP2: u8 = 0xe2;
	const EXIF: &[u8] = b"Exif\0\0";
	const XMP: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
	const ICC: &[u8] = b"ICC_PROFILE\0";

	let mut metadata = Metadata::default();
	// (sequence number, bytes) for each ICC segment, and the count they all declare.
	let mut icc_segments: Vec<(u8, &[u8])> = Vec::new();
	let mut icc_count = 0;
	for (marker, data) in jpeg_segments(bytes) {
		// A signature alone, with nothing after it, is not a payload (`data_length >
		// signature_length`).
		if marker == APP1 && data.len() > EXIF.len() && data.starts_with(EXIF) {
			metadata.exif.get_or_insert_with(|| data[EXIF.len()..].to_vec());
		} else if marker == APP1 && data.len() > XMP.len() && data.starts_with(XMP) {
			metadata.xmp.get_or_insert_with(|| data[XMP.len()..].to_vec());
		} else if marker == APP2 && data.len() > ICC.len() + 2 && data.starts_with(ICC) {
			let (sequence, count, segment) = (data[ICC.len()], data[ICC.len() + 1], &data[ICC.len() + 2..]);
			if sequence == 0 || count == 0 {
				return Err("its ICC profile has a segment numbered or counted 0".to_owned());
			}
			if icc_count == 0 {
				icc_count = count;
			} else if icc_count != count {
				return Err("its ICC profile's segments disagree on how many there are".to_owned());
			}
			if icc_segments.iter().any(|&(seen, _)| seen == sequence) {
				return Err(format!("its ICC profile has segment {sequence} twice"));
			}
			icc_segments.push((sequence, segment));
		}
	}
	if !icc_segments.is_empty() {
		let highest = icc_segments.iter().map(|&(sequence, _)| sequence).max().unwrap_or(0);
		if usize::from(highest) != icc_segments.len() || usize::from(icc_count) != icc_segments.len() {
			return Err("its ICC profile is missing segments".to_owned());
		}
		icc_segments.sort_by_key(|&(sequence, _)| sequence);
		metadata.icc = Some(icc_segments.into_iter().flat_map(|(_, segment)| segment.iter().copied()).collect());
	}
	Ok(metadata)
}

/// The `(marker, payload)` of each segment of a JPEG before its first scan — the markers
/// libjpeg has saved by the time `jpeg_read_header` returns.
fn jpeg_segments(bytes: &[u8]) -> impl Iterator<Item = (u8, &[u8])> {
	let mut at = 2; // past SOI
	std::iter::from_fn(move || {
		loop {
			// Any number of 0xFF fill bytes may precede a marker.
			while bytes.get(at) == Some(&0xff) && bytes.get(at + 1) == Some(&0xff) {
				at += 1;
			}
			if bytes.get(at) != Some(&0xff) {
				return None;
			}
			let marker = *bytes.get(at + 1)?;
			match marker {
				// Start of scan, end of image: the header is over.
				0xda | 0xd9 => return None,
				// Standalone markers carry no length.
				0x01 | 0xd0..=0xd7 => at += 2,
				_ => {
					let length = usize::from(u16::from_be_bytes([*bytes.get(at + 2)?, *bytes.get(at + 3)?]));
					let data = bytes.get(at + 4..(at + 2).checked_add(length)?)?;
					at += 2 + length;
					return Some((marker, data));
				}
			}
		}
	})
}

/// PNG, as `imageio/pngdec.c` reads it from the chunks before the image data.
fn from_png(bytes: &[u8]) -> Result<Metadata, String> {
	let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
	let Ok(reader) = decoder.read_info() else { return Ok(Metadata::default()) };
	let info = reader.info();
	let mut metadata = Metadata { icc: info.icc_profile.as_ref().map(|icc| icc.to_vec()), exif: info.exif_metadata.as_ref().map(|exif| exif.to_vec()), xmp: None };

	// The text chunks, whichever of the three kinds each is. `cwebp` takes the first of
	// each name; eXIf, where present, wins over a raw EXIF profile.
	let mut texts: Vec<(String, String)> = info.uncompressed_latin1_text.iter().map(|chunk| (chunk.keyword.clone(), chunk.text.clone())).collect();
	for chunk in &info.compressed_latin1_text {
		texts.push((chunk.keyword.clone(), chunk.get_text().map_err(|error| format!("a compressed text chunk is damaged: {error}"))?));
	}
	for chunk in &info.utf8_text {
		texts.push((chunk.keyword.clone(), chunk.get_text().map_err(|error| format!("a compressed text chunk is damaged: {error}"))?));
	}
	let had_exif = metadata.exif.is_some();
	let (mut raw_exif, mut xmp) = (None, None);
	for (keyword, text) in &texts {
		match keyword.as_str() {
			"Raw profile type exif" | "Raw profile type APP1" | "Raw profile type app1" if raw_exif.is_none() => raw_exif = Some(raw_profile(text).ok_or_else(|| format!("its \"{keyword}\" text is not a raw profile"))?),
			"Raw profile type xmp" if xmp.is_none() => xmp = Some(raw_profile(text).ok_or_else(|| format!("its \"{keyword}\" text is not a raw profile"))?),
			"XML:com.adobe.xmp" if xmp.is_none() && !text.is_empty() => xmp = Some(text.clone().into_bytes()),
			_ => {}
		}
	}
	if !had_exif {
		metadata.exif = raw_exif;
	}
	metadata.xmp = xmp;
	Ok(metadata)
}

/// An `ImageMagick` "raw profile": `\n<name>\n<length>\n<hex, broken by newlines>`, read as
/// `ProcessRawProfile` reads it.
fn raw_profile(text: &str) -> Option<Vec<u8>> {
	let rest = text.strip_prefix('\n')?;
	let (_name, rest) = rest.split_once('\n')?;
	let rest = rest.trim_start_matches([' ', '\t']);
	let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
	let length: usize = rest[..digits].parse().ok()?;
	let hex = rest[digits..].strip_prefix('\n')?;
	let mut out = Vec::with_capacity(length);
	let mut chars = hex.bytes().filter(|&c| c != b'\n');
	while out.len() < length {
		let pair = [chars.next()?, chars.next()?];
		out.push(u8::from_str_radix(std::str::from_utf8(&pair).ok()?, 16).ok()?);
	}
	Some(out)
}

/// TIFF, as `imageio/tiffdec.c` reads it: the ICC profile and XMP tags. (`cwebp` warns
/// that it cannot extract EXIF from a TIFF and keeps none.)
fn from_tiff(bytes: &[u8]) -> Metadata {
	use image::ImageDecoder;
	let Ok(mut decoder) = image::codecs::tiff::TiffDecoder::new(std::io::Cursor::new(bytes)) else { return Metadata::default() };
	let non_empty = |value: Option<Vec<u8>>| value.filter(|bytes| !bytes.is_empty());
	Metadata { icc: non_empty(decoder.icc_profile().ok().flatten()), exif: None, xmp: non_empty(decoder.xmp_metadata().ok().flatten()) }
}

/// WebP, as `imageio/webpdec.c` reads it: each chunk the `VP8X` flags declare, the first
/// of each kind.
fn from_webp(bytes: &[u8]) -> Metadata {
	let mut metadata = Metadata::default();
	let Some(vp8x) = webp_chunks(bytes).find(|(fourcc, _)| fourcc == b"VP8X") else { return metadata };
	let flags = vp8x.1.first().copied().unwrap_or(0);
	for (fourcc, body) in webp_chunks(bytes) {
		let slot = match &fourcc {
			b"ICCP" if flags & ICCP_FLAG != 0 => &mut metadata.icc,
			b"EXIF" if flags & EXIF_FLAG != 0 => &mut metadata.exif,
			b"XMP " if flags & XMP_FLAG != 0 => &mut metadata.xmp,
			_ => continue,
		};
		slot.get_or_insert_with(|| body.to_vec());
	}
	metadata
}

/// The `(fourcc, payload)` of each top-level chunk in a WebP file.
fn webp_chunks(bytes: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
	let mut at = 12;
	std::iter::from_fn(move || {
		let fourcc: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
		let size = usize::try_from(u32::from_le_bytes(bytes.get(at + 4..at + 8)?.try_into().ok()?)).ok()?;
		let body = bytes.get(at + 8..(at + 8).checked_add(size)?)?;
		at += 8 + size + (size & 1);
		Some((fourcc, body))
	})
}

/// The still-image WebP `encoded` from the image file `source` under `job`, with the
/// metadata `job` asks to keep added. Unchanged unless the job writes WebP and keeps
/// something — which is also every default job, so the parity gate is untouched.
///
/// # Errors
///
/// When the source's metadata is malformed where `cwebp` would refuse it, or the
/// metadata cannot be added (see [`extract`] and [`attach`]).
pub fn keep(job: &crate::EncodeJob, source: &[u8], encoded: Vec<u8>) -> Result<Vec<u8>, String> {
	if job.format != crate::OutputFormat::Webp || !job.metadata.any() {
		return Ok(encoded);
	}
	let metadata = extract(source)?;
	if metadata.is_empty() {
		return Ok(encoded);
	}
	let info = crate::inspect::inspect_webp(&encoded).ok_or("the encoder's output is not a WebP file")?;
	attach(encoded, info.width, info.height, &metadata, job.metadata)
}

const ICCP_FLAG: u8 = 0x20;
const ALPHA_FLAG: u8 = 0x10;
const EXIF_FLAG: u8 = 0x08;
const XMP_FLAG: u8 = 0x04;

/// Add the metadata `keep` asks for, where `metadata` has it, to the encoded WebP `webp`
/// of a `width` × `height` image, exactly as `cwebp`'s `WriteWebPWithMetadata` does.
/// Returns `webp` unchanged when there is nothing to add.
///
/// # Errors
///
/// When `webp` is not a WebP file, or the metadata would take it past the container's
/// 4 GiB limit.
pub fn attach(webp: Vec<u8>, width: u32, height: u32, metadata: &Metadata, keep: KeepMetadata) -> Result<Vec<u8>, String> {
	let (icc, exif, xmp) = (pick(keep.icc, metadata.icc.as_deref()), pick(keep.exif, metadata.exif.as_deref()), pick(keep.xmp, metadata.xmp.as_deref()));
	let chunk_size = |payload: Option<&[u8]>| payload.map_or(0, |bytes| 8 + bytes.len() + (bytes.len() & 1));
	let metadata_size = chunk_size(icc) + chunk_size(exif) + chunk_size(xmp);
	if metadata_size == 0 {
		return Ok(webp);
	}
	if webp.len() < 20 || &webp[0..4] != b"RIFF" || &webp[8..12] != b"WEBP" {
		return Err("the encoder's output is not a WebP file".to_owned());
	}
	let flags = (if icc.is_some() { ICCP_FLAG } else { 0 }) | (if exif.is_some() { EXIF_FLAG } else { 0 }) | (if xmp.is_some() { XMP_FLAG } else { 0 });

	let has_vp8x = &webp[12..16] == b"VP8X";
	let riff_size = u32::try_from(webp.len() - 8 + if has_vp8x { 0 } else { 18 } + metadata_size).map_err(|_| "adding the metadata would exceed the WebP size limit".to_owned())?;
	let mut out = Vec::with_capacity(webp.len() + 18 + metadata_size);
	out.extend_from_slice(b"RIFF");
	out.extend_from_slice(&riff_size.to_le_bytes());
	out.extend_from_slice(b"WEBP");
	let image = if has_vp8x {
		let mut vp8x = webp.get(12..30).ok_or("the encoder's VP8X header is truncated")?.to_vec();
		vp8x[8] |= flags;
		out.extend_from_slice(&vp8x);
		&webp[30..]
	} else {
		// A simple-format file: its alpha, if any, is only in the VP8L header (bit 28 of
		// the 32 bits after the signature byte), and VP8X must now declare it.
		let alpha = &webp[12..16] == b"VP8L" && webp.get(24).is_some_and(|byte| byte & 0x10 != 0);
		out.extend_from_slice(b"VP8X\x0a\0\0\0");
		out.extend_from_slice(&u32::from(flags | if alpha { ALPHA_FLAG } else { 0 }).to_le_bytes());
		out.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
		out.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
		&webp[12..]
	};
	push_chunk(&mut out, *b"ICCP", icc);
	out.extend_from_slice(image);
	push_chunk(&mut out, *b"EXIF", exif);
	push_chunk(&mut out, *b"XMP ", xmp);
	Ok(out)
}

/// `payload` if it is wanted and not empty (`UpdateFlagsAndSize`'s test).
fn pick(wanted: bool, payload: Option<&[u8]>) -> Option<&[u8]> {
	payload.filter(|bytes| wanted && !bytes.is_empty())
}

/// Append a metadata chunk, padded to an even length as RIFF requires; nothing for `None`.
fn push_chunk(out: &mut Vec<u8>, fourcc: [u8; 4], payload: Option<&[u8]>) {
	let Some(payload) = payload else { return };
	out.extend_from_slice(&fourcc);
	out.extend_from_slice(&u32::try_from(payload.len()).unwrap_or(u32::MAX).to_le_bytes());
	out.extend_from_slice(payload);
	if payload.len() & 1 == 1 {
		out.push(0);
	}
}

#[cfg(test)]
mod tests {
	use super::{extract, raw_profile};

	/// `ImageMagick`'s layout, including its space-padded length and line breaks.
	#[test]
	fn raw_profiles_decode_as_imagemagick_writes_them() {
		assert_eq!(raw_profile("\nexif\n       3\n0aff\n10\n"), Some(vec![0x0a, 0xff, 0x10]));
		assert_eq!(raw_profile("\nexif\n4\n0aff10\n"), None, "shorter than it claims");
		assert_eq!(raw_profile("exif\n1\n00\n"), None, "must start with a newline");
		assert_eq!(raw_profile("\nexif\n1\nzz\n"), None, "not hex");
	}

	/// Files with no metadata, or that are not images at all, have none — never an error.
	#[test]
	fn nothing_to_find_is_not_an_error() {
		assert!(extract(b"").expect("empty").is_empty());
		assert!(extract(b"\xff\xd8\xff\xd9").expect("a bare JPEG").is_empty());
		assert!(extract(b"\xff\xd8\xff\xe1\xff\xff").expect("a truncated segment").is_empty());
		assert!(extract(b"RIFF\0\0\0\0WEBPVP8X").expect("a truncated WebP").is_empty());
		assert!(extract(b"GIF89a").expect("a GIF").is_empty());
	}
}
