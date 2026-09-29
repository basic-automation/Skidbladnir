//! The metadata a source image carries: its ICC profile, Exif and XMP.
//!
//! Each reference tool reads these out of its input with its own rules, and those rules
//! decide the bytes it writes, so they are reproduced here per tool rather than
//! approximated once. [`Metadata::read_like_cwebp`] is `cwebp`'s: the readers in libwebp's
//! `imageio/` directory, whose results `cwebp -metadata` copies into the WebP.

use std::io::Read as _;

use crate::source::SourceFormat;

/// The metadata payloads of an image, as a tool extracted them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
	/// The ICC colour profile.
	pub icc: Option<Vec<u8>>,
	/// Exif, in the form the tool keeps it (for `cwebp`, starting at the TIFF header).
	pub exif: Option<Vec<u8>>,
	/// XMP, as the packet's bytes.
	pub xmp: Option<Vec<u8>>,
}

impl Metadata {
	/// Read metadata the way `cwebp` does for this input format. Malformed metadata is
	/// skipped rather than failing the read; where `cwebp -metadata` would refuse the file
	/// instead, [`Self::cwebp_refusal`] says so, and the WebP path refuses too.
	#[must_use]
	pub fn read_like_cwebp(bytes: &[u8], format: SourceFormat) -> Self {
		match format {
			SourceFormat::Png => png_like_cwebp(bytes),
			SourceFormat::Jpeg => jpeg_like_cwebp(bytes),
			SourceFormat::Tiff => tiff_like_cwebp(bytes),
			SourceFormat::Webp => webp_chunks(bytes),
			// `cwebp` cannot read these at all. Their decoders' own metadata is used. (A GIF
			// goes to WebP through the animation encoder, as `gif2webp`, never through here.)
			SourceFormat::Avif | SourceFormat::Jxl | SourceFormat::Heic | SourceFormat::Gif => Self::default(),
		}
	}

	/// Why `cwebp -metadata` would refuse this file, if it would: its readers extract every
	/// kind of metadata whenever any is asked for, and fail the whole read on an ICC profile
	/// split inconsistently across JPEG segments (`StoreICCP`) or a PNG "raw profile" text
	/// chunk that is not valid hex (`ProcessRawProfile`). Converting anyway would silently
	/// drop what was asked for, so the WebP path refuses too, by name.
	#[must_use]
	pub fn cwebp_refusal(bytes: &[u8], format: SourceFormat) -> Option<&'static str> {
		match format {
			SourceFormat::Jpeg => {
				let markers = jpeg_markers(bytes);
				let segments = markers.iter().any(|(marker, data)| *marker == 0xe2 && data.len() > JPEG_ICC_SIGNATURE.len() + 2 && data.starts_with(JPEG_ICC_SIGNATURE));
				(segments && jpeg_icc(&markers).is_none()).then_some("its ICC profile is split across JPEG segments inconsistently, which cwebp refuses too")
			}
			SourceFormat::Png => png_read(bytes).1.then_some("a PNG raw-profile text chunk is not valid hex, which cwebp refuses too"),
			_ => None,
		}
	}
}

/// `imageio/pngdec.c`, `ExtractMetadataFromPNG`: the chunks before the image data first,
/// then those after it. Within each: `eXIf`; then the text chunks `cwebp` recognises, the
/// first of a kind winning; then `iCCP`.
fn png_like_cwebp(bytes: &[u8]) -> Metadata {
	png_read(bytes).0
}

/// [`png_like_cwebp`], and whether a raw profile it tried to read was malformed, which
/// fails `cwebp`'s read.
fn png_read(bytes: &[u8]) -> (Metadata, bool) {
	let mut metadata = Metadata::default();
	let mut malformed = false;
	let Some(chunks) = png_chunks(bytes) else { return (metadata, malformed) };
	let (head, tail): (Vec<_>, Vec<_>) = {
		let split = chunks.iter().position(|(kind, _)| kind == b"IDAT").unwrap_or(chunks.len());
		(chunks[..split].to_vec(), chunks[split..].to_vec())
	};
	for part in [head, tail] {
		if let Some((_, exif)) = part.iter().find(|(kind, _)| kind == b"eXIf") {
			metadata.exif = Some(exif.to_vec());
		}
		for (keyword, text) in part.iter().filter_map(|(kind, data)| png_text(*kind, data)) {
			let (slot, raw_profile) = match keyword.as_str() {
				"Raw profile type exif" | "Raw profile type APP1" | "Raw profile type app1" => (&mut metadata.exif, true),
				"Raw profile type xmp" => (&mut metadata.xmp, true),
				"XML:com.adobe.xmp" => (&mut metadata.xmp, false),
				_ => continue,
			};
			if slot.is_some() {
				continue;
			}
			*slot = if raw_profile { raw_profile_bytes(&text) } else { Some(text) };
			malformed |= slot.is_none();
		}
		if let Some((_, iccp)) = part.iter().find(|(kind, _)| kind == b"iCCP")
			&& let Some(profile) = libpng_iccp(iccp, png_is_colour(&chunks))
		{
			metadata.icc = Some(profile);
		}
	}
	(metadata, malformed)
}

/// Whether a PNG's colour type is a colour one (RGB, palette or RGBA) rather than gray.
pub(crate) fn png_is_colour(chunks: &[([u8; 4], &[u8])]) -> bool {
	chunks.iter().find(|(kind, _)| kind == b"IHDR").and_then(|(_, data)| data.get(9)).is_some_and(|colour_type| colour_type & 2 != 0)
}

/// The ICC profile in an `iCCP` chunk, if libpng 1.6 would hand it to its caller.
///
/// Every reference tool reads PNG through libpng, and libpng drops a profile it does not
/// like with only a warning, so the tool then writes no profile at all. These are its
/// checks, in `png_handle_iCCP`'s order: the chunk must be at least 92 bytes (81 read for
/// the keyword, then at least an LZ77 stream's minimum), the keyword 1..=79 bytes and the
/// compression method 0; then `png_icc_check_length`, `png_icc_check_header` (declared
/// length, tag count, rendering intent, `acsp` signature, a colour space that matches the
/// image, the device class, the PCS) and `png_icc_check_tag_table`; and the profile must
/// inflate to at least its declared length. Warnings are not failures.
pub(crate) fn libpng_iccp(chunk: &[u8], colour: bool) -> Option<Vec<u8>> {
	const LZ77_MIN: usize = 2 + 5 + 4;
	if chunk.len() < 81 + LZ77_MIN {
		return None;
	}
	let keyword = chunk.iter().take(80).position(|&b| b == 0)?;
	if !(1..=79).contains(&keyword) || chunk.get(keyword + 1) != Some(&0) {
		return None;
	}
	let profile = inflate(&chunk[keyword + 2..])?;
	let be32 = |at: usize| profile.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
	let length = be32(0)?;
	let declared = usize::try_from(length).ok()?;
	if declared < 132 || profile.len() < declared {
		return None;
	}
	let tag_count = be32(128)?;
	let major_version = profile[8];
	let colour_space = be32(16)?;
	let class = be32(12)?;
	let pcs = be32(20)?;
	let valid_header = !(major_version > 3 && length & 3 != 0)
		&& tag_count <= 357_913_930
		&& u64::from(length) >= 132 + 12 * u64::from(tag_count)
		&& be32(64)? < 0xffff
		&& be32(36)? == u32::from_be_bytes(*b"acsp")
		&& match &colour_space.to_be_bytes() {
			b"RGB " => colour,
			b"GRAY" => !colour,
			_ => false,
		}
		&& !matches!(&class.to_be_bytes(), b"abst" | b"link")
		&& matches!(&pcs.to_be_bytes(), b"XYZ " | b"Lab ");
	if !valid_header {
		return None;
	}
	for tag in 0..tag_count as usize {
		let start = be32(132 + tag * 12 + 4)?;
		let tag_length = be32(132 + tag * 12 + 8)?;
		if start > length || tag_length > length - start {
			return None;
		}
	}
	Some(profile[..declared].to_vec())
}

/// Every chunk of a PNG, as `(type, data)`, or `None` if it is not a PNG.
pub(crate) fn png_chunks(bytes: &[u8]) -> Option<Vec<([u8; 4], &[u8])>> {
	let mut rest = bytes.strip_prefix(b"\x89PNG\r\n\x1a\n")?;
	let mut chunks = Vec::new();
	while rest.len() >= 12 {
		let length = u32::from_be_bytes(rest[0..4].try_into().ok()?) as usize;
		let kind: [u8; 4] = rest[4..8].try_into().ok()?;
		let data = rest.get(8..8 + length)?;
		chunks.push((kind, data));
		rest = rest.get(12 + length..)?;
		if &kind == b"IEND" {
			break;
		}
	}
	Some(chunks)
}

/// A PNG text chunk's keyword and text, decompressed if it was compressed. `tEXt` and
/// `zTXt` are Latin-1 and `iTXt` UTF-8; the text is kept as bytes either way, which is what
/// libpng hands `cwebp`.
pub(crate) fn png_text(kind: [u8; 4], data: &[u8]) -> Option<(String, Vec<u8>)> {
	let nul = data.iter().position(|&b| b == 0)?;
	let keyword = String::from_utf8_lossy(&data[..nul]).into_owned();
	let body = &data[nul + 1..];
	let text = match &kind {
		b"tEXt" => body.to_vec(),
		// Compression method, then zlib data.
		b"zTXt" => inflate(body.get(1..)?)?,
		b"iTXt" => {
			// Compression flag, compression method, language tag NUL, translated keyword NUL.
			let (&flag, rest) = body.split_first()?;
			let rest = rest.get(1..)?;
			let rest = &rest[rest.iter().position(|&b| b == 0)? + 1..];
			let rest = &rest[rest.iter().position(|&b| b == 0)? + 1..];
			if flag == 1 { inflate(rest)? } else { rest.to_vec() }
		}
		_ => return None,
	};
	Some((keyword, text))
}

/// Decompress a zlib stream.
fn inflate(data: &[u8]) -> Option<Vec<u8>> {
	let mut out = Vec::new();
	flate2::read::ZlibDecoder::new(data).read_to_end(&mut out).ok()?;
	Some(out)
}

/// `ImageMagick`'s "raw profile" text: `\n<name>\n<length>\n<hex, wrapped>`.
/// `imageio/pngdec.c`, `ProcessRawProfile` and `HexStringToBytes`.
pub(crate) fn raw_profile_bytes(text: &[u8]) -> Option<Vec<u8>> {
	let rest = text.strip_prefix(b"\n")?;
	let rest = &rest[rest.iter().position(|&b| b == b'\n')? + 1..];
	// strtol: skip leading whitespace, read digits; the next character must be the newline.
	let start = rest.iter().position(|b| !b.is_ascii_whitespace())?;
	let digits = rest[start..].iter().take_while(|b| b.is_ascii_digit()).count();
	let length: usize = std::str::from_utf8(&rest[start..start + digits]).ok()?.parse().ok()?;
	let hex = rest.get(start + digits..)?.strip_prefix(b"\n")?;
	// Newlines in the hex are skipped; any other non-hex character is an error.
	let mut out = Vec::with_capacity(length);
	let mut nibbles = hex.iter().filter(|&&b| b != b'\n');
	while out.len() < length {
		let (high, low) = (nibbles.next()?, nibbles.next()?);
		let value = |b: u8| (b as char).to_digit(16);
		out.push(u8::try_from(value(*high)? * 16 + value(*low)?).ok()?);
	}
	Some(out)
}

/// The `APPn` markers of a JPEG, in file order, up to the start of scan: `(marker, data)`.
pub(crate) fn jpeg_markers(bytes: &[u8]) -> Vec<(u8, &[u8])> {
	let mut markers = Vec::new();
	let Some(mut rest) = bytes.strip_prefix(&[0xff, 0xd8]) else { return markers };
	// Fill bytes: any number of 0xFF before the marker code.
	while let Some(skip) = rest.iter().position(|&b| b != 0xff) {
		if skip == 0 {
			break;
		}
		let marker = rest[skip];
		rest = &rest[skip + 1..];
		// Markers without a length.
		if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
			continue;
		}
		if marker == 0xd9 || marker == 0xda {
			break;
		}
		let Some(length) = rest.get(0..2).map(|l| usize::from(u16::from_be_bytes([l[0], l[1]]))) else { break };
		let Some(data) = rest.get(2..length.max(2)) else { break };
		markers.push((marker, data));
		rest = &rest[length.max(2)..];
	}
	markers
}

/// `imageio/jpegdec.c`, `ExtractMetadataFromJPEG`: the ICC profile reassembled from its
/// APP2 segments; Exif and XMP from the first APP1 of each kind, without their signatures.
fn jpeg_like_cwebp(bytes: &[u8]) -> Metadata {
	let markers = jpeg_markers(bytes);
	let mut metadata = Metadata { icc: jpeg_icc(&markers), ..Metadata::default() };
	for (marker, data) in &markers {
		if *marker != 0xe1 {
			continue;
		}
		// The signatures `cwebp` matches, and how many bytes of each it strips.
		for (signature, slot) in [(&b"Exif\0\0"[..], &mut metadata.exif), (&b"http://ns.adobe.com/xap/1.0/\0"[..], &mut metadata.xmp)] {
			if slot.is_none() && data.len() > signature.len() && data.starts_with(signature) {
				*slot = Some(data[signature.len()..].to_vec());
			}
		}
	}
	metadata
}

/// What starts an ICC profile's `APP2` segment.
const JPEG_ICC_SIGNATURE: &[u8] = b"ICC_PROFILE\0";

/// An ICC profile split across `ICC_PROFILE` APP2 segments, put back together in sequence
/// order. `None` if there is none, or the segments are inconsistent (`StoreICCP`'s checks).
pub(crate) fn jpeg_icc(markers: &[(u8, &[u8])]) -> Option<Vec<u8>> {
	const SIGNATURE: &[u8] = JPEG_ICC_SIGNATURE;
	let mut segments: Vec<(u8, &[u8])> = Vec::new();
	let mut expected_count = 0;
	for (marker, data) in markers {
		if *marker != 0xe2 || data.len() <= SIGNATURE.len() + 2 || !data.starts_with(SIGNATURE) {
			continue;
		}
		let (seq, count) = (data[SIGNATURE.len()], data[SIGNATURE.len() + 1]);
		if seq == 0 || count == 0 || (expected_count != 0 && expected_count != count) || segments.iter().any(|(s, _)| *s == seq) {
			return None;
		}
		expected_count = count;
		segments.push((seq, &data[SIGNATURE.len() + 2..]));
	}
	if segments.is_empty() || segments.len() != usize::from(expected_count) {
		return None;
	}
	segments.sort_by_key(|(seq, _)| *seq);
	if segments.iter().enumerate().any(|(i, (seq, _))| usize::from(*seq) != i + 1) {
		return None;
	}
	Some(segments.into_iter().flat_map(|(_, data)| data.iter().copied()).collect())
}

/// `imageio/tiffdec.c`: the ICC profile and XMP packet tags. `cwebp` does not read Exif
/// out of a TIFF.
fn tiff_like_cwebp(bytes: &[u8]) -> Metadata {
	use tiff::{decoder::Decoder, tags::Tag};
	let Ok(mut decoder) = Decoder::new(std::io::Cursor::new(bytes)) else { return Metadata::default() };
	let mut tag = |code: u16| decoder.get_tag_u8_vec(Tag::Unknown(code)).ok().filter(|v| !v.is_empty());
	Metadata { icc: tag(34675), exif: None, xmp: tag(700) }
}

/// The `ICCP`, `EXIF` and `XMP ` chunks of a WebP, which is what `imageio/webpdec.c` reads
/// through the demuxer.
pub(crate) fn webp_chunks(bytes: &[u8]) -> Metadata {
	let mut metadata = Metadata::default();
	let Some(mut rest) = bytes.get(12..).filter(|_| bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP")) else { return metadata };
	while rest.len() >= 8 {
		let kind = &rest[0..4];
		let size = u32::from_le_bytes([rest[4], rest[5], rest[6], rest[7]]) as usize;
		let Some(data) = rest.get(8..8 + size) else { break };
		match kind {
			b"ICCP" => metadata.icc = Some(data.to_vec()),
			b"EXIF" => metadata.exif = Some(data.to_vec()),
			b"XMP " => metadata.xmp = Some(data.to_vec()),
			_ => {}
		}
		rest = rest.get(8 + size + (size & 1)..).unwrap_or_default();
	}
	metadata
}

#[cfg(test)]
pub(crate) mod tests {
	use super::*;

	fn chunk(kind: [u8; 4], data: &[u8]) -> Vec<u8> {
		let mut out = u32::try_from(data.len()).expect("small").to_be_bytes().to_vec();
		out.extend_from_slice(&kind);
		out.extend_from_slice(data);
		out.extend_from_slice(&[0; 4]);
		out
	}

	fn zlib(data: &[u8]) -> Vec<u8> {
		use std::io::Write as _;
		let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
		encoder.write_all(data).expect("compress");
		encoder.finish().expect("compress")
	}

	/// An RGB or GRAY display profile with one tag and enough incompressible data that its
	/// `iCCP` chunk clears libpng's minimum size.
	pub(crate) fn test_icc(colour_space: [u8; 4]) -> Vec<u8> {
		let mut icc = vec![0_u8; 132 + 12 + 200];
		icc[0..4].copy_from_slice(&344_u32.to_be_bytes());
		icc[8..12].copy_from_slice(&0x0210_0000_u32.to_be_bytes());
		icc[12..16].copy_from_slice(b"mntr");
		icc[16..20].copy_from_slice(&colour_space);
		icc[20..24].copy_from_slice(b"XYZ ");
		icc[36..40].copy_from_slice(b"acsp");
		icc[68..80].copy_from_slice(&[0, 0, 0xf6, 0xd6, 0, 1, 0, 0, 0, 0, 0xd3, 0x2d]);
		icc[128..132].copy_from_slice(&1_u32.to_be_bytes());
		icc[132..136].copy_from_slice(b"desc");
		icc[136..140].copy_from_slice(&144_u32.to_be_bytes());
		icc[140..144].copy_from_slice(&200_u32.to_be_bytes());
		let mut state = 0x1234_5678_u32;
		for byte in &mut icc[144..] {
			state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
			*byte = state.to_be_bytes()[0];
		}
		icc
	}

	#[test]
	fn libpng_icc_rules() {
		let chunk = |profile: &[u8]| [b"icc".as_slice(), &[0, 0], &zlib(profile)].concat();
		assert!(libpng_iccp(&chunk(&test_icc(*b"RGB ")), true).is_some());
		assert!(libpng_iccp(&chunk(&test_icc(*b"RGB ")), false).is_none(), "an RGB profile on a gray PNG");
		assert!(libpng_iccp(&chunk(&test_icc(*b"GRAY")), false).is_some());
		assert!(libpng_iccp(&chunk(&test_icc(*b"CMYK")), true).is_none());
		let mut bad = test_icc(*b"RGB ");
		bad[36] = b'x';
		assert!(libpng_iccp(&chunk(&bad), true).is_none(), "no acsp signature");
		assert!(libpng_iccp(&chunk(&[0; 132]), true).is_none(), "a tiny chunk is too short for libpng");
	}

	#[test]
	fn raw_profiles_decode_like_imagemagick_writes_them() {
		assert_eq!(raw_profile_bytes(b"\nexif\n       4\n4578\n6966\n").as_deref(), Some(&b"Exif"[..]));
		assert_eq!(raw_profile_bytes(b"exif\n4\n45786966"), None, "must start with a newline");
		assert_eq!(raw_profile_bytes(b"\nexif\n4\n4578zz66"), None, "non-hex is an error");
	}

	#[test]
	fn png_metadata_follows_cwebp_precedence() {
		let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
		png.extend(chunk(*b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0]));
		png.extend(chunk(*b"iCCP", &[b"icc".as_slice(), &[0, 0], &zlib(&test_icc(*b"RGB "))].concat()));
		png.extend(chunk(*b"eXIf", b"MM\0*exif"));
		png.extend(chunk(*b"tEXt", b"Raw profile type exif\0\nexif\n2\nabcd\n"));
		png.extend(chunk(*b"iTXt", &[b"XML:com.adobe.xmp\0".as_slice(), &[0, 0, 0, 0], b"<x:xmpmeta/>"].concat()));
		png.extend(chunk(*b"IDAT", &[]));
		png.extend(chunk(*b"tEXt", b"Raw profile type xmp\0\nxmp\n2\nbeef\n"));
		png.extend(chunk(*b"IEND", &[]));
		let metadata = Metadata::read_like_cwebp(&png, SourceFormat::Png);
		assert_eq!(metadata.icc, Some(test_icc(*b"RGB ")));
		assert_eq!(metadata.exif.as_deref(), Some(&b"MM\0*exif"[..]), "eXIf beats the raw profile");
		assert_eq!(metadata.xmp.as_deref(), Some(&b"<x:xmpmeta/>"[..]), "the head's XMP beats the tail's");
	}

	#[test]
	fn jpeg_metadata_strips_the_signatures_and_reassembles_icc() {
		let segment = |marker: u8, data: &[u8]| [&[0xff, marker][..], &u16::try_from(data.len() + 2).expect("small").to_be_bytes(), data].concat();
		let jpeg = [&[0xff, 0xd8][..], &segment(0xe1, b"Exif\0\0II*\0"), &segment(0xe2, b"ICC_PROFILE\0\x02\x02WORLD"), &segment(0xe2, b"ICC_PROFILE\0\x01\x02HELLO"), &segment(0xe1, b"http://ns.adobe.com/xap/1.0/\0<xmp/>"), &[0xff, 0xda, 0, 2]].concat();
		let metadata = Metadata::read_like_cwebp(&jpeg, SourceFormat::Jpeg);
		assert_eq!(metadata.exif.as_deref(), Some(&b"II*\0"[..]));
		assert_eq!(metadata.icc.as_deref(), Some(&b"HELLOWORLD"[..]));
		assert_eq!(metadata.xmp.as_deref(), Some(&b"<xmp/>"[..]));
	}

	#[test]
	fn webp_chunks_are_read() {
		let mut webp = b"RIFF\0\0\0\0WEBP".to_vec();
		for (kind, data) in [(b"VP8X", &b"0123456789"[..]), (b"ICCP", b"icc"), (b"VP8L", b"x"), (b"EXIF", b"exif"), (b"XMP ", b"xmp")] {
			webp.extend_from_slice(kind);
			webp.extend_from_slice(&u32::try_from(data.len()).expect("small").to_le_bytes());
			webp.extend_from_slice(data);
			if data.len() % 2 == 1 {
				webp.push(0);
			}
		}
		let metadata = Metadata::read_like_cwebp(&webp, SourceFormat::Webp);
		assert_eq!((metadata.icc.as_deref(), metadata.exif.as_deref(), metadata.xmp.as_deref()), (Some(&b"icc"[..]), Some(&b"exif"[..]), Some(&b"xmp"[..])));
	}
}
