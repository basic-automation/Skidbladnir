//! AVIF output, through libavif with the libaom encoder — the way `avifenc` drives them.
//!
//! libavif and libaom are built statically from the pinned submodules (`build.rs`). The
//! encode itself is `native/avif_shim.c`: `avifenc`'s still-image path, from choosing the
//! pixel format to the target-size search, ported as one C function over libavif. This
//! module does what `avifenc`'s PNG reader does before that (`apps/shared/avifpng.c`):
//! the samples at the PNG's own depth and channels, its colour chunks in `avifenc`'s order
//! of precedence, and its Exif and XMP by `avifenc`'s rules. It then hands the shim two
//! plain structs. A JPEG is read by `avifenc`'s own JPEG reader, compiled into the shim,
//! because it copies the JPEG's YCbCr planes in unconverted when the settings allow.
//!
//! `tests/avif_parity.rs` compares the result with a reference `avifenc` built from the
//! same libavif and libaom, byte for byte.
//!
//! libavif has a progress callback for neither encoder nor search, so progress moves at the
//! start and the end of a file, and a cancel mid-encode is honoured by discarding the result.

use std::{
	borrow::Cow, ffi::{CStr, CString, c_char, c_int}, ptr
};

use crate::{
	encoder::{EncodeError, transformed}, metadata::{jpeg_icc, jpeg_markers, libpng_iccp, png_chunks, png_text}, settings::{AvifSettings, CleanAperture, EncodeJob, MetadataSource, Tiling}, source::{SourceFormat, SourceImage}
};

#[repr(C)]
struct SkidAvifOption {
	key: *const c_char,
	value: *const c_char,
}

#[repr(C)]
struct SkidAvifInput {
	width: u32,
	height: u32,
	pixels: *const u8,
	rgb_depth: u32,
	channels: u32,
	raw_gray: c_int,
	has_cicp: c_int,
	cicp_primaries: u8,
	cicp_transfer: u8,
	icc: *const u8,
	icc_size: usize,
	has_srgb: c_int,
	has_gama: c_int,
	gama: f64,
	has_chrm: c_int,
	chrm: [f64; 8],
	exif: *const u8,
	exif_size: usize,
	xmp: *const u8,
	xmp_size: usize,
	exif_sets_transforms: c_int,
	jpeg: *const u8,
	jpeg_size: usize,
	ignore_icc: c_int,
	ignore_exif: c_int,
	ignore_xmp: c_int,
}

#[repr(C)]
struct SkidAvifSettings {
	jobs: c_int,
	speed: c_int,
	quality: c_int,
	quality_alpha: c_int,
	min_quantizer: c_int,
	max_quantizer: c_int,
	min_quantizer_alpha: c_int,
	max_quantizer_alpha: c_int,
	tile_rows_log2: c_int,
	tile_cols_log2: c_int,
	autotiling: c_int,
	scaling_set: c_int,
	scaling_n: u32,
	scaling_d: u32,
	lossless: c_int,
	depth: c_int,
	depth_extension: c_int,
	yuv_format: c_int,
	premultiply: c_int,
	sharpyuv: c_int,
	cicp_set: c_int,
	primaries: c_int,
	transfer: c_int,
	matrix: c_int,
	range_limited: c_int,
	target_size: c_int,
	progressive: c_int,
	grid_cols: u32,
	grid_rows: u32,
	pasp_set: c_int,
	pasp: [u32; 2],
	crop_set: c_int,
	crop: [u32; 4],
	clap_set: c_int,
	clap: [u32; 8],
	irot: c_int,
	imir: c_int,
	clli_set: c_int,
	clli: [u32; 2],
	icc_override: *const u8,
	icc_override_size: usize,
	exif_override: *const u8,
	exif_override_size: usize,
	xmp_override: *const u8,
	xmp_override_size: usize,
	advanced: *const SkidAvifOption,
	advanced_count: usize,
}

unsafe extern "C" {
	fn skid_avif_encode(input: *const SkidAvifInput, settings: *const SkidAvifSettings, out: *mut *mut u8, out_size: *mut usize, error: *mut c_char, error_size: usize) -> c_int;
	fn skid_avif_free(data: *mut u8);
	fn skid_avif_version() -> *const c_char;
	fn skid_avif_versions() -> *const c_char;
}

/// The linked libavif's version, e.g. `1.4.2`.
#[must_use]
pub fn linked_version() -> String {
	// SAFETY: libavif returns a static NUL-terminated string.
	unsafe { CStr::from_ptr(skid_avif_version()) }.to_string_lossy().into_owned()
}

/// The codecs the linked libavif has, as `avifCodecVersions` lists them, e.g.
/// `aom [enc]:v3.15.1`.
#[must_use]
pub fn linked_codecs() -> String {
	// SAFETY: the shim returns its own static buffer, filled by libavif.
	unsafe { CStr::from_ptr(skid_avif_versions()) }.to_string_lossy().into_owned()
}

/// The header in front of Exif in a JPEG `APP1`, optional in a PNG raw profile.
const EXIF: &[u8] = b"Exif\0\0";
/// The namespace in front of XMP in a JPEG `APP1`, optional in a PNG raw profile.
const XMP: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

/// What `avifenc`'s readers make of an input file, before the encoder sees it.
#[derive(Debug, Default)]
struct Input {
	/// Interleaved samples, one or two bytes each, native byte order.
	samples: Vec<u8>,
	rgb_depth: u32,
	channels: u32,
	raw_gray: bool,
	cicp: Option<(u8, u8)>,
	icc: Option<Vec<u8>>,
	srgb: bool,
	gama: Option<f64>,
	chrm: Option<[f64; 8]>,
	exif: Option<Vec<u8>>,
	xmp: Option<Vec<u8>>,
	/// A JPEG's Exif, whose orientation libavif turns into `irot`/`imir`.
	exif_sets_transforms: bool,
}

impl Input {
	/// The samples of `source` with `channels` channels (gray, gray+alpha, RGB or RGBA), at
	/// 16 bits when `deep` and the source has them.
	fn samples(source: &SourceImage, channels: u32, deep: bool) -> (Vec<u8>, u32) {
		let keep: &[usize] = match channels {
			1 => &[0],
			2 => &[0, 3],
			3 => &[0, 1, 2],
			_ => &[0, 1, 2, 3],
		};
		match (&source.deep, deep) {
			(Some(wide), true) => (wide.as_chunks::<4>().0.iter().flat_map(|pixel| keep.iter().flat_map(move |&c| pixel[c].to_ne_bytes())).collect(), 16),
			_ => (source.pixels.as_chunks::<4>().0.iter().flat_map(|pixel| keep.iter().map(move |&c| pixel[c])).collect(), 8),
		}
	}

	/// The channel count libpng hands `avifenc` after expanding palettes and `tRNS`.
	fn channels(source: &SourceImage) -> u32 {
		match (source.gray, source.has_alpha) {
			(true, false) => 1,
			(true, true) => 2,
			(false, false) => 3,
			(false, true) => 4,
		}
	}

	/// `avifPNGRead`.
	fn png(source: &SourceImage, settings: &AvifSettings) -> Self {
		let chunks = png_chunks(&source.bytes).unwrap_or_default();
		let split = chunks.iter().position(|(kind, _)| kind == b"IDAT").unwrap_or(chunks.len());
		let head = &chunks[..split];
		let ihdr = chunks.iter().find(|(kind, _)| kind == b"IHDR").map_or(&[][..], |(_, data)| *data);
		let (bit_depth, colour_type) = (ihdr.get(8).copied().unwrap_or(8), ihdr.get(9).copied().unwrap_or(6));
		let raw_gray = colour_type & 2 == 0;
		let channels = Self::channels(source);
		let (samples, rgb_depth) = Self::samples(source, channels, bit_depth == 16);
		let mut input = Self { samples, rgb_depth, channels, raw_gray, ..Self::default() };

		// The colour, unless ignored or overridden: cICP, then iCCP, then sRGB, then gAMA and
		// cHRM, each read by libpng from before the image data.
		if settings.icc == MetadataSource::Keep {
			let find = |kind: &[u8; 4]| head.iter().find(|(k, _)| k == kind).map(|(_, data)| *data);
			if let Some(&[primaries, transfer, _, _]) = find(b"cICP") {
				input.cicp = Some((primaries, transfer));
			} else if let Some(profile) = find(b"iCCP").and_then(|chunk| libpng_iccp(chunk, !raw_gray)) {
				input.icc = Some(profile);
			} else if find(b"sRGB").is_some_and(|data| data.len() == 1 && data[0] < 4) {
				input.srgb = true;
			} else {
				let fixed = |bytes: &[u8]| f64::from(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])) / 100_000.0;
				input.gama = find(b"gAMA").filter(|data| data.len() == 4).map(fixed).filter(|&gamma| gamma > 0.0);
				input.chrm = find(b"cHRM").filter(|data| data.len() == 32).map(|data| std::array::from_fn(|i| fixed(&data[i * 4..])));
			}
		}

		// avifExtractExifAndXMP: the chunks before the image data, then, for whatever is
		// still missing, all of them.
		let mut want_exif = settings.exif == MetadataSource::Keep;
		let mut want_xmp = settings.xmp == MetadataSource::Keep;
		for part in [head, &chunks[..]] {
			if !want_exif && !want_xmp {
				break;
			}
			if want_exif && let Some((_, exif)) = part.iter().find(|(kind, _)| kind == b"eXIf") {
				input.exif = (!exif.is_empty()).then(|| exif.to_vec());
				want_exif = false;
			}
			for (keyword, text) in part.iter().filter_map(|(kind, data)| png_text(*kind, data)) {
				if !want_exif && !want_xmp {
					break;
				}
				let strip = |mut bytes: Vec<u8>, header: &[u8]| {
					if bytes.len() > header.len() && bytes.starts_with(header) {
						bytes.drain(..header.len());
					}
					bytes
				};
				match keyword.as_str() {
					"Raw profile type exif" if want_exif => {
						input.exif = avifenc_raw_profile(&text).map(|bytes| strip(bytes, EXIF));
						want_exif = false;
					}
					"Raw profile type xmp" if want_xmp => {
						input.xmp = avifenc_raw_profile(&text).map(|bytes| strip(bytes, XMP));
						want_xmp = false;
					}
					"Raw profile type APP1" | "Raw profile type app1" => {
						if let Some(bytes) = avifenc_raw_profile(&text) {
							if want_exif && bytes.len() > EXIF.len() && bytes.starts_with(EXIF) {
								input.exif = Some(strip(bytes, EXIF));
								want_exif = false;
							} else if want_xmp && bytes.len() > XMP.len() && bytes.starts_with(XMP) {
								input.xmp = Some(strip(bytes, XMP));
								want_xmp = false;
							}
						}
					}
					"XML:com.adobe.xmp" if want_xmp => {
						input.xmp = (!text.is_empty()).then_some(text);
						want_xmp = false;
					}
					_ => {}
				}
			}
		}
		input
	}

	/// A cropped or resized JPEG, whose pixels no longer are the file's: they go through
	/// RGB, with the file's metadata read the way `avifjpeg.c` reads it. A JPEG as it is
	/// goes to `avifenc`'s own reader in the shim instead.
	fn jpeg(source: &SourceImage, settings: &AvifSettings) -> Self {
		let markers = jpeg_markers(&source.bytes);
		let channels = if source.gray { 1 } else { 3 };
		let (samples, rgb_depth) = Self::samples(source, channels, false);
		let keep = |kind: &MetadataSource| *kind == MetadataSource::Keep;
		let app1 = |signature: &[u8]| markers.iter().find(|(marker, data)| *marker == 0xe1 && data.len() > signature.len() && data.starts_with(signature)).map(|(_, data)| data[signature.len()..].to_vec());
		Self { samples, rgb_depth, channels, raw_gray: source.gray, icc: if keep(&settings.icc) { jpeg_icc(&markers) } else { None }, exif: if keep(&settings.exif) { app1(b"Exif\0\0") } else { None }, xmp: if keep(&settings.xmp) { app1(b"http://ns.adobe.com/xap/1.0/\0") } else { None }, exif_sets_transforms: true, ..Self::default() }
	}

	/// A format `avifenc` does not read: its samples at their own depth, and the metadata
	/// its container carries.
	fn other(source: &SourceImage, settings: &AvifSettings) -> Self {
		let metadata = crate::metadata::Metadata::read_like_cwebp(&source.bytes, source.format);
		let channels = Self::channels(source);
		let (samples, rgb_depth) = Self::samples(source, channels, true);
		let keep = |kind: &MetadataSource| *kind == MetadataSource::Keep;
		Self { samples, rgb_depth, channels, raw_gray: source.gray, icc: metadata.icc.filter(|_| keep(&settings.icc)), exif: metadata.exif.filter(|_| keep(&settings.exif)), xmp: metadata.xmp.filter(|_| keep(&settings.xmp)), ..Self::default() }
	}
}

/// `avifCopyRawProfile` and `avifHexStringToBytes`: `\n<name>\n<length>\n<hex>`, the
/// length read by `strtol`, newlines in the hex skipped, extra data after the payload
/// tolerated. `None` where `avifenc` would fail the read.
fn avifenc_raw_profile(text: &[u8]) -> Option<Vec<u8>> {
	let rest = text.strip_prefix(b"\n")?;
	let name_end = rest.iter().position(|&b| b == b'\n')?;
	let after_name = &rest[name_end + 1..];
	let length_end = after_name.iter().position(|&b| b == b'\n')?;
	let length_text = std::str::from_utf8(&after_name[..length_end]).ok()?.trim_start();
	let length: usize = length_text.parse().ok().filter(|&length: &usize| length > 0)?;
	let hex = &after_name[length_end + 1..];
	if length > hex.len() / 2 {
		return None;
	}
	let mut bytes = Vec::with_capacity(length);
	let mut i = 0;
	while i + 1 < hex.len() && bytes.len() < length {
		if hex[i] == b'\n' {
			i += 1;
			continue;
		}
		let digit = |c: u8| (c as char).to_digit(16);
		bytes.push(u8::try_from(digit(hex[i])? * 16 + digit(hex[i + 1])?).ok()?);
		i += 2;
	}
	(bytes.len() == length).then_some(bytes)
}

/// The number of worker threads `avifenc -j all` uses.
fn all_jobs() -> c_int {
	std::thread::available_parallelism().map_or(1, |n| c_int::try_from(n.get()).unwrap_or(c_int::MAX))
}

/// Encode a source to AVIF as `avifenc` would with these settings.
///
/// # Errors
///
/// Returns [`EncodeError`] if the settings are out of range or `avifenc` would refuse them
/// for this image, a metadata file cannot be read, libavif fails, or `on_progress` asked
/// to stop.
pub fn encode(job: &EncodeJob, source: &SourceImage, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Vec<u8>, EncodeError> {
	let settings = &job.avif;
	settings.validate()?;
	let source = transformed(source, job.crop, job.resize)?;
	if !on_progress(0) {
		return Err(EncodeError::Cancelled);
	}
	// A JPEG as it is on disk goes to avifenc's own reader; cropped or resized, its pixels
	// are all there is.
	let jpeg = (source.format == SourceFormat::Jpeg && !source.bytes.is_empty() && matches!(source, Cow::Borrowed(_))).then_some(source.bytes.as_slice());
	let input = match source.format {
		_ if jpeg.is_some() => Input::default(),
		_ if source.bytes.is_empty() => Input::other(&source, settings),
		SourceFormat::Png => Input::png(&source, settings),
		SourceFormat::Jpeg => Input::jpeg(&source, settings),
		_ => Input::other(&source, settings),
	};
	let output = encode_input(settings, source.width, source.height, &input, jpeg)?;
	if !on_progress(100) {
		return Err(EncodeError::Cancelled);
	}
	Ok(output)
}

fn encode_input(s: &AvifSettings, width: u32, height: u32, input: &Input, jpeg: Option<&[u8]>) -> Result<Vec<u8>, EncodeError> {
	let [icc_file, exif_file, xmp_file] = s.metadata_files().map(|path| path.map(|path| std::fs::read(path).map_err(|error| EncodeError::Avif(format!("could not read `{}`: {error}", path.display())))).transpose());
	let (icc_file, exif_file, xmp_file) = (icc_file?, exif_file?, xmp_file?);
	let options: Vec<(CString, CString)> = s.codec_options.iter().map(|option| Ok((CString::new(option.key.clone())?, CString::new(option.value.clone())?))).collect::<Result<_, std::ffi::NulError>>().map_err(|_| EncodeError::Avif("a codec option contains a NUL byte".to_owned()))?;
	let option_pointers: Vec<SkidAvifOption> = options.iter().map(|(key, value)| SkidAvifOption { key: key.as_ptr(), value: value.as_ptr() }).collect();

	let bytes = |data: Option<&Vec<u8>>| data.map_or((ptr::null(), 0), |data| (data.as_ptr(), data.len()));
	let (icc, icc_size) = bytes(input.icc.as_ref());
	let (exif, exif_size) = bytes(input.exif.as_ref());
	let (xmp, xmp_size) = bytes(input.xmp.as_ref());
	let raw = SkidAvifInput { width, height, pixels: input.samples.as_ptr(), rgb_depth: input.rgb_depth, channels: input.channels, raw_gray: c_int::from(input.raw_gray), has_cicp: c_int::from(input.cicp.is_some()), cicp_primaries: input.cicp.map_or(0, |c| c.0), cicp_transfer: input.cicp.map_or(0, |c| c.1), icc, icc_size, has_srgb: c_int::from(input.srgb), has_gama: c_int::from(input.gama.is_some()), gama: input.gama.unwrap_or(0.0), has_chrm: c_int::from(input.chrm.is_some()), chrm: input.chrm.unwrap_or([0.0; 8]), exif, exif_size, xmp, xmp_size, exif_sets_transforms: c_int::from(input.exif_sets_transforms), jpeg: jpeg.map_or(ptr::null(), <[u8]>::as_ptr), jpeg_size: jpeg.map_or(0, <[u8]>::len), ignore_icc: c_int::from(s.icc != MetadataSource::Keep), ignore_exif: c_int::from(s.exif != MetadataSource::Keep), ignore_xmp: c_int::from(s.xmp != MetadataSource::Keep) };

	let unset = |value: Option<u8>| value.map_or(-1, c_int::from);
	let (tile_rows_log2, tile_cols_log2, autotiling) = match s.tiling {
		Tiling::Automatic => (-1, -1, 0),
		Tiling::Manual { rows_log2, cols_log2 } => (c_int::from(rows_log2), c_int::from(cols_log2), 0),
	};
	let (crop_set, crop, clap_set, clap) = match s.clean_aperture {
		None => (0, [0; 4], 0, [0; 8]),
		Some(CleanAperture::Crop(values)) => (1, values, 0, [0; 8]),
		Some(CleanAperture::Raw(values)) => (0, [0; 4], 1, values),
	};
	let (icc_override, icc_override_size) = bytes(icc_file.as_ref());
	let (exif_override, exif_override_size) = bytes(exif_file.as_ref());
	let (xmp_override, xmp_override_size) = bytes(xmp_file.as_ref());
	let settings = SkidAvifSettings {
		jobs: s.jobs.map_or_else(all_jobs, |jobs| c_int::try_from(jobs.max(1)).unwrap_or(c_int::MAX)),
		speed: s.speed.map_or(-1, c_int::from),
		quality: unset(s.quality),
		quality_alpha: unset(s.quality_alpha),
		min_quantizer: s.quantizer.map_or(-1, |q| c_int::from(q.min)),
		max_quantizer: s.quantizer.map_or(-1, |q| c_int::from(q.max)),
		min_quantizer_alpha: s.alpha_quantizer.map_or(-1, |q| c_int::from(q.min)),
		max_quantizer_alpha: s.alpha_quantizer.map_or(-1, |q| c_int::from(q.max)),
		tile_rows_log2,
		tile_cols_log2,
		autotiling,
		scaling_set: c_int::from(s.scaling_mode.is_some()),
		scaling_n: s.scaling_mode.map_or(1, |f| f.numerator),
		scaling_d: s.scaling_mode.map_or(1, |f| f.denominator),
		lossless: c_int::from(s.lossless),
		depth: s.depth.map_or(0, c_int::from),
		depth_extension: s.depth_extension.map_or(0, c_int::from),
		yuv_format: s.yuv.as_libavif(),
		premultiply: c_int::from(s.premultiply),
		sharpyuv: c_int::from(s.sharp_yuv),
		cicp_set: c_int::from(s.cicp.is_some()),
		primaries: s.cicp.map_or(0, |c| c_int::from(c.primaries)),
		transfer: s.cicp.map_or(0, |c| c_int::from(c.transfer)),
		matrix: s.cicp.map_or(0, |c| c_int::from(c.matrix)),
		range_limited: c_int::from(s.limited_range),
		target_size: s.target_size.map_or(-1, |size| c_int::try_from(size).unwrap_or(c_int::MAX)),
		progressive: c_int::from(s.progressive),
		grid_cols: s.grid.map_or(0, |g| g.columns),
		grid_rows: s.grid.map_or(0, |g| g.rows),
		pasp_set: c_int::from(s.pasp.is_some()),
		pasp: s.pasp.unwrap_or([0; 2]),
		crop_set,
		crop,
		clap_set,
		clap,
		irot: s.irot.map_or(-1, c_int::from),
		imir: s.imir.map_or(-1, c_int::from),
		clli_set: c_int::from(s.clli.is_some()),
		clli: s.clli.map_or([0; 2], |c| c.map(u32::from)),
		icc_override,
		icc_override_size,
		exif_override,
		exif_override_size,
		xmp_override,
		xmp_override_size,
		advanced: option_pointers.as_ptr(),
		advanced_count: option_pointers.len(),
	};

	let mut out: *mut u8 = ptr::null_mut();
	let mut out_size = 0_usize;
	let mut error = [0 as c_char; 512];
	// SAFETY: every pointer in `raw` and `settings` addresses a live buffer of the stated
	// length for the whole call; the shim copies what it keeps. On success `out` is a
	// libavif allocation of `out_size` bytes, released below with `skid_avif_free`.
	let ok = unsafe { skid_avif_encode(&raw const raw, &raw const settings, &raw mut out, &raw mut out_size, error.as_mut_ptr(), error.len()) };
	if ok == 0 {
		// SAFETY: the shim NUL-terminates the message within the buffer.
		let message = unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy().into_owned();
		return Err(EncodeError::Avif(message));
	}
	// SAFETY: as above; copied before it is freed, exactly once.
	let file = unsafe { std::slice::from_raw_parts(out, out_size) }.to_vec();
	unsafe { skid_avif_free(out) };
	Ok(file)
}

/// Read an AVIF file's dimensions from its `ispe` (image spatial extents) property.
///
/// The encoder resolves a resize with one dimension `0`, so the only reliable way to
/// report the dimensions actually written is to read them back out of the file, as the
/// WebP path does. This is a scan for the property box rather than a full HEIF parser:
/// it is only ever pointed at files this module has just written. A grid image's first
/// `ispe` is its first cell's; the last is the whole image's, so the largest is taken.
#[must_use]
pub fn dimensions(avif: &[u8]) -> Option<(u32, u32)> {
	let mut best: Option<(u32, u32)> = None;
	let mut at = 0;
	while let Some(found) = avif[at..].windows(4).position(|window| window == b"ispe") {
		let start = at + found;
		// `ispe` is a full box: 4 bytes of version and flags, then width and height as
		// big-endian u32s.
		if let Some(body) = avif.get(start + 8..start + 16) {
			let size = (u32::from_be_bytes([body[0], body[1], body[2], body[3]]), u32::from_be_bytes([body[4], body[5], body[6], body[7]]));
			if best.is_none_or(|b| u64::from(size.0) * u64::from(size.1) > u64::from(b.0) * u64::from(b.1)) {
				best = Some(size);
			}
		}
		at = start + 4;
	}
	best
}

#[cfg(test)]
mod tests {
	use super::{avifenc_raw_profile, dimensions, encode, linked_codecs, linked_version};
	use crate::{
		encoder::{EncodeError, RgbaImage}, settings::{AvifSettings, EncodeJob, Grid, OutputFormat, Resize, YuvFormat}, source::SourceImage
	};

	fn fixture(width: u32, height: u32) -> Vec<u8> {
		let mut pixels = Vec::with_capacity((width * height * 4) as usize);
		for y in 0..height {
			for x in 0..width {
				pixels.extend_from_slice(&[u8::try_from(x * 255 / width).unwrap_or(0), u8::try_from(y * 255 / height).unwrap_or(0), 90, if x < width / 4 { 0 } else { 255 }]);
			}
		}
		pixels
	}

	/// Fast settings for tests.
	fn quick(settings: AvifSettings) -> EncodeJob {
		EncodeJob { format: OutputFormat::Avif, avif: AvifSettings { speed: Some(10), ..settings }, ..Default::default() }
	}

	fn run(job: &EncodeJob, width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, EncodeError> {
		encode(job, &SourceImage::from_rgba(&RgbaImage { width, height, pixels }), &mut |_| true)
	}

	#[test]
	fn links_libavif_with_libaom() {
		assert!(linked_version().starts_with("1."), "libavif {}", linked_version());
		assert!(linked_codecs().contains("aom"), "codecs: {}", linked_codecs());
	}

	#[test]
	fn writes_an_avif_file_with_the_right_dimensions() {
		let bytes = run(&quick(AvifSettings::default()), 40, 24, &fixture(40, 24)).expect("encode");
		assert_eq!(&bytes[4..12], b"ftypavif");
		assert_eq!(dimensions(&bytes), Some((40, 24)));
		let (width, height, _) = crate::source::decode_avif(&bytes).expect("decode our own output");
		assert_eq!((width, height), (40, 24));
	}

	#[test]
	fn applies_the_shared_resize() {
		for (resize, expected) in [(Resize::to(20, 12), (20, 12)), (Resize::to(20, 0), (20, 12)), (Resize::to(0, 12), (20, 12))] {
			let bytes = run(&EncodeJob { resize, ..quick(AvifSettings::default()) }, 40, 24, &fixture(40, 24)).expect("encode");
			assert_eq!(dimensions(&bytes), Some(expected), "{resize:?}");
		}
	}

	#[test]
	fn lower_quality_is_smaller() {
		let pixels = fixture(64, 48);
		let low = run(&quick(AvifSettings { quality: Some(20), ..Default::default() }), 64, 48, &pixels).expect("encode");
		let high = run(&quick(AvifSettings { quality: Some(95), ..Default::default() }), 64, 48, &pixels).expect("encode");
		assert!(low.len() < high.len(), "q20 {} bytes vs q95 {} bytes", low.len(), high.len());
	}

	#[test]
	fn every_depth_and_subsampling_encodes() {
		let pixels = fixture(64, 64);
		for depth in [8, 10, 12] {
			for yuv in [YuvFormat::Yuv444, YuvFormat::Yuv422, YuvFormat::Yuv420, YuvFormat::Yuv400] {
				let bytes = run(&quick(AvifSettings { depth: Some(depth), yuv, ..Default::default() }), 64, 64, &pixels).unwrap_or_else(|error| panic!("{depth}-bit {yuv:?}: {error}"));
				assert_eq!(dimensions(&bytes), Some((64, 64)));
			}
		}
	}

	#[test]
	fn lossless_is_exact() {
		let pixels = fixture(32, 16);
		let bytes = run(&quick(AvifSettings { lossless: true, ..Default::default() }), 32, 16, &pixels).expect("encode");
		let (_, _, decoded) = crate::source::decode_avif(&bytes).expect("decode");
		assert_eq!(decoded, pixels);
	}

	#[test]
	fn a_grid_is_split_and_reports_the_whole_size() {
		let bytes = run(&quick(AvifSettings { grid: Some(Grid { columns: 2, rows: 2 }), ..Default::default() }), 128, 128, &fixture(128, 128)).expect("encode");
		assert_eq!(dimensions(&bytes), Some((128, 128)));
	}

	#[test]
	fn avifenc_refusals_come_back_as_errors() {
		let pixels = fixture(8, 8);
		assert!(run(&quick(AvifSettings { lossless: true, yuv: YuvFormat::Yuv420, ..Default::default() }), 8, 8, &pixels).is_err());
		assert!(run(&quick(AvifSettings { quality: Some(101), ..Default::default() }), 8, 8, &pixels).is_err());
	}

	#[test]
	fn a_cancel_before_or_after_the_encode_returns_nothing() {
		let pixels = fixture(8, 8);
		let source = SourceImage::from_rgba(&RgbaImage { width: 8, height: 8, pixels: &pixels });
		assert_eq!(encode(&quick(AvifSettings::default()), &source, &mut |_| false), Err(EncodeError::Cancelled));
		assert_eq!(encode(&quick(AvifSettings::default()), &source, &mut |percent| percent < 100), Err(EncodeError::Cancelled));
	}

	#[test]
	fn raw_profiles_are_read_as_avifenc_reads_them() {
		assert_eq!(avifenc_raw_profile(b"\nexif\n       4\n45786966\n").as_deref(), Some(&b"Exif"[..]));
		assert_eq!(avifenc_raw_profile(b"\nexif\n4\n4578\n6966trailing").as_deref(), Some(&b"Exif"[..]), "extra data after the payload is tolerated");
		assert_eq!(avifenc_raw_profile(b"\nexif\n4\n457869"), None);
	}

	#[test]
	fn dimensions_of_garbage_is_none() {
		assert_eq!(dimensions(b"not an avif"), None);
		assert_eq!(dimensions(b"....ispe\0\0\0\0\0\0"), None, "a truncated ispe must not be read past its end");
	}
}
