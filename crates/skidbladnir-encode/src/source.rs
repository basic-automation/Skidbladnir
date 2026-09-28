//! Reading the user's image files, and writing the converted result back safely.
//!
//! The encoder in [`crate::encoder`] deals only in RGBA pixels. This module is what turns
//! a path the user picked into those pixels, and what puts the result on disk.
//!
//! # The file-safety rules, and why they are here rather than in the UI
//!
//! Skidbladnir writes image files to disk, so the failure that actually matters is not a
//! wrong encoder flag — it is destroying someone's original. Two rules are enforced at
//! this layer, where they cannot be forgotten by a caller:
//!
//! 1. **Never write over the source.** Converting `photo.webp` into the folder it already
//!    lives in produces the output name `photo.webp`, which is the input. The Electron app
//!    accepts WebP input and derives exactly that name, so it will silently overwrite the
//!    original. [`encode_file`] refuses instead.
//! 2. **Never truncate an existing file on a failed encode.** The output is written to a
//!    temporary file beside the destination and renamed into place only once the encode
//!    has fully succeeded, so an error leaves whatever was there untouched.

use std::{
	ffi::OsStr, fs, io, path::{Path, PathBuf}
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
	encoder::{EncodeError, RgbaImage, encode_rgba_with_progress}, settings::{EncodeJob, OutputFormat}
};

/// The input formats Skidbladnir accepts: the Electron app's list, plus AVIF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceFormat {
	/// PNG.
	Png,
	/// JPEG.
	Jpeg,
	/// TIFF, either byte order.
	Tiff,
	/// WebP, decoded by libwebp itself rather than a third-party decoder.
	Webp,
	/// AVIF, decoded by `avif-decode` (rav1d). Still images only.
	Avif,
	/// JPEG XL, as a bare codestream or in its ISO-BMFF container, decoded by `jxl-oxide`.
	Jxl,
	/// HEIC, decoded by libheif with libde265. The primary image only.
	Heic,
}

impl SourceFormat {
	/// Identify a format from the leading bytes of a file.
	///
	/// Content sniffing rather than the extension: the Electron app trusts the extension,
	/// so a mislabelled file reaches the encoder and fails there with a less useful error.
	#[must_use]
	pub fn sniff(bytes: &[u8]) -> Option<Self> {
		match bytes {
			[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, ..] => Some(Self::Png),
			[0xff, 0xd8, 0xff, ..] => Some(Self::Jpeg),
			[b'I', b'I', 42, 0, ..] | [b'M', b'M', 0, 42, ..] => Some(Self::Tiff),
			[b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some(Self::Webp),
			[_, _, _, _, b'f', b't', b'y', b'p', ..] if is_avif_ftyp(bytes) => Some(Self::Avif),
			[_, _, _, _, b'f', b't', b'y', b'p', ..] if is_heic_ftyp(bytes) => Some(Self::Heic),
			[0xff, 0x0a, ..] | [0, 0, 0, 0x0c, b'J', b'X', b'L', b' ', 0x0d, 0x0a, 0x87, 0x0a, ..] => Some(Self::Jxl),
			_ => None,
		}
	}

	/// A human-readable name, for error messages and the UI.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Png => "PNG",
			Self::Jpeg => "JPEG",
			Self::Tiff => "TIFF",
			Self::Webp => "WebP",
			Self::Avif => "AVIF",
			Self::Jxl => "JPEG XL",
			Self::Heic => "HEIC",
		}
	}
}

/// Whether an ISO-BMFF `ftyp` box names one of `brands`, as its major brand or among the
/// compatible brands that fit in `bytes`. AVIF and HEIC share the container, so the brand
/// is what tells them apart.
fn ftyp_has_brand(bytes: &[u8], brands: &[&[u8; 4]]) -> bool {
	let Some(declared) = bytes.get(0..4).and_then(|len| len.try_into().ok()).map(u32::from_be_bytes) else { return false };
	let end = usize::try_from(declared).unwrap_or(usize::MAX).min(bytes.len());
	// Major brand at 8..12, minor version at 12..16, compatible brands from 16 on.
	let wanted = |brand: &[u8]| brands.iter().any(|candidate| brand == candidate.as_slice());
	bytes.get(8..12).is_some_and(wanted) || bytes.get(16..end).unwrap_or_default().as_chunks::<4>().0.iter().any(|brand| wanted(brand))
}

fn is_avif_ftyp(bytes: &[u8]) -> bool {
	ftyp_has_brand(bytes, &[b"avif", b"avis"])
}

/// HEVC-in-HEIF brands: still images, image collections and their sequences.
fn is_heic_ftyp(bytes: &[u8]) -> bool {
	ftyp_has_brand(bytes, &[b"heic", b"heix", b"heim", b"heis", b"hevc", b"hevx"])
}

/// An image decoded into the 8-bit RGBA the encoder wants.
#[derive(Clone, Debug)]
pub struct SourceImage {
	/// Width in pixels.
	pub width: u32,
	/// Height in pixels.
	pub height: u32,
	/// Tightly packed RGBA bytes.
	pub pixels: Vec<u8>,
	/// The format it was decoded from.
	pub format: SourceFormat,
}

impl SourceImage {
	/// Borrow this image in the shape [`encode_rgba`] takes.
	#[must_use]
	pub fn as_rgba(&self) -> RgbaImage<'_> {
		RgbaImage { width: self.width, height: self.height, pixels: &self.pixels }
	}
}

/// Why an input file could not be turned into pixels.
#[derive(Debug, Error)]
pub enum SourceError {
	/// The file could not be read.
	#[error("could not read `{path}`: {source}")]
	Read {
		/// The path that failed.
		path: PathBuf,
		/// The underlying I/O error.
		source: io::Error,
	},
	/// The leading bytes match none of the accepted formats.
	#[error("`{path}` is not a PNG, JPEG, TIFF, WebP, AVIF, JPEG XL or HEIC image")]
	Unsupported {
		/// The path that failed.
		path: PathBuf,
	},
	/// The file is an animated WebP, asked for somewhere only a still image will do.
	///
	/// Converting one to WebP works — [`encode_file`] re-encodes every frame. What cannot
	/// take one is anything that wants a single picture: [`load`], and AVIF, JPEG XL and
	/// HEIC output, which are written as stills only. Refusing there, by name, is what stops an animation
	/// being silently cut down to its first frame. libwebp's still decoder refuses it
	/// anyway, but with a message that tells the user nothing about why.
	#[error("`{path}` is an animated WebP. It converts to an animated WebP, but AVIF, JPEG XL and HEIC output take still images only.")]
	Animated {
		/// The path that failed.
		path: PathBuf,
	},
	/// The file claims a supported format but could not be decoded.
	#[error("could not decode `{path}` as {format}: {detail}")]
	Decode {
		/// The path that failed.
		path: PathBuf,
		/// The sniffed format.
		format: &'static str,
		/// What the decoder said.
		detail: String,
	},
}

/// Decode an image file into RGBA pixels.
///
/// # Errors
///
/// Returns [`SourceError`] if the file cannot be read, is not one of the accepted
/// formats, or fails to decode.
pub fn load(path: &Path) -> Result<SourceImage, SourceError> {
	let bytes = fs::read(path).map_err(|source| SourceError::Read { path: path.to_path_buf(), source })?;
	decode(path, &bytes)
}

/// Whether `bytes` are an animated WebP, from the header alone.
fn is_animated_webp(bytes: &[u8]) -> bool {
	SourceFormat::sniff(bytes) == Some(SourceFormat::Webp) && crate::inspect::inspect_webp(bytes).is_some_and(|info| info.has_animation)
}

/// Decode the bytes of the file at `path` into RGBA pixels; `path` is for error messages.
fn decode(path: &Path, bytes: &[u8]) -> Result<SourceImage, SourceError> {
	let format = SourceFormat::sniff(bytes).ok_or_else(|| SourceError::Unsupported { path: path.to_path_buf() })?;

	let (width, height, pixels) = match format {
		// libwebp decodes its own format; using a second WebP implementation here would
		// mean a WebP-to-WebP conversion round-trips through a decoder that is not the
		// reference one.
		SourceFormat::Webp => {
			// Check before decoding: libwebp's still decoder refuses an animation, but the
			// failure it reports says nothing about why, and "libwebp rejected the file" is
			// not something a user can act on.
			if is_animated_webp(bytes) {
				return Err(SourceError::Animated { path: path.to_path_buf() });
			}
			decode_webp(bytes).ok_or_else(|| SourceError::Decode { path: path.to_path_buf(), format: format.name(), detail: "libwebp rejected the file".to_owned() })?
		}
		SourceFormat::Avif => decode_avif(bytes).map_err(|detail| SourceError::Decode { path: path.to_path_buf(), format: format.name(), detail })?,
		SourceFormat::Heic => crate::heic::decode(bytes).map_err(|detail| SourceError::Decode { path: path.to_path_buf(), format: format.name(), detail })?,
		SourceFormat::Jxl => crate::jxl::decode(bytes).map_err(|detail| SourceError::Decode { path: path.to_path_buf(), format: format.name(), detail })?,
		SourceFormat::Png | SourceFormat::Jpeg | SourceFormat::Tiff => {
			let decoded = image::load_from_memory(bytes).map_err(|error| SourceError::Decode { path: path.to_path_buf(), format: format.name(), detail: error.to_string() })?;
			let rgba = to_rgba8(decoded);
			(rgba.width(), rgba.height(), rgba.into_raw())
		}
	};

	Ok(SourceImage { width, height, pixels, format })
}

/// Reduce a decoded image to 8-bit RGBA the way `cwebp` does.
///
/// For 8-bit sources this is just `into_rgba8`. For **16-bit** sources it is not, and the
/// difference is visible in the encoder's output: `cwebp` reads PNGs through libpng with
/// `png_set_strip_16`, which **discards the low byte** of each sample, while the `image`
/// crate scales the value into 8 bits, which rounds differently. Measured, not assumed —
/// before this, a 16-bit RGBA fixture encoded to 216 bytes here against `cwebp`'s 212, and
/// 16-bit RGB to 168 against 166. Truncating to match makes them identical.
///
/// 8-bit grayscale, grayscale+alpha and palette PNGs were already byte-identical and are
/// left to `into_rgba8`.
fn to_rgba8(decoded: image::DynamicImage) -> image::RgbaImage {
	use image::DynamicImage::{ImageLuma16, ImageLumaA16, ImageRgb16, ImageRgba16};

	if !matches!(decoded, ImageLuma16(_) | ImageLumaA16(_) | ImageRgb16(_) | ImageRgba16(_)) {
		return decoded.into_rgba8();
	}

	let wide = decoded.into_rgba16();
	let (width, height) = (wide.width(), wide.height());
	// `sample >> 8` of a u16 is always within u8, so this cannot lose anything further.
	let narrowed: Vec<u8> = wide.into_raw().into_iter().map(|sample| u8::try_from(sample >> 8).unwrap_or(u8::MAX)).collect();
	image::RgbaImage::from_raw(width, height, narrowed).unwrap_or_else(|| image::RgbaImage::new(width, height))
}

/// Decode an AVIF file to `(width, height, rgba)`.
///
/// 16-bit output (from a 10- or 12-bit AVIF) is reduced by dropping the low byte, the
/// same reduction `to_rgba8` makes for 16-bit PNG, so a deep source is narrowed the same
/// way whatever container it came in.
///
/// # Errors
///
/// Returns the decoder's message if the file is not a decodable AVIF still.
pub fn decode_avif(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
	use avif_decode::{Decoder, Image};

	// The parser underneath is not panic-free on hostile input: a malformed box trips a
	// `debug_assert` in `avif-parse` (found by this crate's corrupt-file test), and a user's
	// damaged file must come back as an error, never take the conversion down. Release
	// builds unwind (no `panic = "abort"` anywhere in the workspace), so this holds there too.
	let decoded = std::panic::catch_unwind(|| Decoder::from_avif(bytes).and_then(Decoder::to_image));
	let image = match decoded {
		Ok(result) => result.map_err(|error| error.to_string())?,
		Err(_) => return Err("the AVIF decoder could not read this file".to_owned()),
	};
	let narrow = |value: u16| u8::try_from(value >> 8).unwrap_or(u8::MAX);
	let (width, height, pixels): (usize, usize, Vec<u8>) = match image {
		Image::Rgb8(img) => (img.width(), img.height(), img.pixels().flat_map(|p| [p.r, p.g, p.b, 255]).collect()),
		Image::Rgba8(img) => (img.width(), img.height(), img.pixels().flat_map(|p| [p.r, p.g, p.b, p.a]).collect()),
		Image::Gray8(img) => (
			img.width(),
			img.height(),
			img.pixels()
				.flat_map(|p| {
					let v = p.value();
					[v, v, v, 255]
				})
				.collect(),
		),
		Image::Rgb16(img) => (img.width(), img.height(), img.pixels().flat_map(|p| [narrow(p.r), narrow(p.g), narrow(p.b), 255]).collect()),
		Image::Rgba16(img) => (img.width(), img.height(), img.pixels().flat_map(|p| [narrow(p.r), narrow(p.g), narrow(p.b), narrow(p.a)]).collect()),
		Image::Gray16(img) => (
			img.width(),
			img.height(),
			img.pixels()
				.flat_map(|p| {
					let v = narrow(p.value());
					[v, v, v, 255]
				})
				.collect(),
		),
	};
	let (Ok(width), Ok(height)) = (u32::try_from(width), u32::try_from(height)) else {
		return Err(format!("{width}x{height} is too large"));
	};
	Ok((width, height, pixels))
}

/// Decode a WebP file with libwebp, returning `(width, height, rgba)`.
fn decode_webp(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
	let mut width = 0;
	let mut height = 0;
	// SAFETY: `bytes` is a valid slice, and the out-parameters are live for the call.
	let decoded = unsafe { libwebp_sys::WebPDecodeRGBA(bytes.as_ptr(), bytes.len(), &raw mut width, &raw mut height) };
	if decoded.is_null() {
		return None;
	}
	let (width, height) = (u32::try_from(width).ok()?, u32::try_from(height).ok()?);
	let len = (width as usize).checked_mul(height as usize)?.checked_mul(4)?;
	// SAFETY: libwebp allocated exactly width * height * 4 bytes.
	let pixels = unsafe { std::slice::from_raw_parts(decoded, len) }.to_vec();
	// SAFETY: the pointer came from libwebp and is freed exactly once.
	unsafe { libwebp_sys::WebPFree(decoded.cast()) };
	Some((width, height, pixels))
}

/// What is known about a path the user offered, without decoding the whole image.
///
/// Dropping a folder of mixed files on the window should tell the user which ones the app
/// can actually take, and that answer has to come from the same place the loader's answer
/// comes from — otherwise the UI accepts a file the encoder then rejects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathInspection {
	/// The path that was offered.
	pub path: PathBuf,
	/// Whether Skidbladnir can read it.
	pub supported: bool,
	/// The detected format's display name, or `None` if it is not an accepted image.
	pub format: Option<&'static str>,
	/// For a WebP, what its header says. `None` for every other format.
	///
	/// Carried here so the UI can warn before converting: an **animated** WebP cannot be
	/// re-encoded by this app, and converting one would silently keep only the first frame.
	pub webp: Option<crate::inspect::WebpInfo>,
}

/// Identify which of these paths are images Skidbladnir can read.
///
/// Reads only the first few bytes of each file, so dropping a large selection on the
/// window does not read every one of them off disk in full. Anything unreadable — a
/// directory, a broken link, a file without permission — is simply reported unsupported
/// rather than raised as an error: the user dropped it, they did not ask for it to be
/// diagnosed.
#[must_use]
pub fn inspect_paths(paths: &[PathBuf]) -> Vec<PathInspection> {
	paths.iter()
		.map(|path| {
			let format = read_prefix(path).as_deref().and_then(SourceFormat::sniff);
			// Only WebP gets a second read, and only of its header — WebPGetFeatures does not
			// decode, so this stays cheap even for a large selection.
			let webp = (format == Some(SourceFormat::Webp)).then(|| fs::read(path).ok().and_then(|bytes| crate::inspect::inspect_webp(&bytes))).flatten();
			PathInspection { path: path.clone(), supported: format.is_some(), format: format.map(SourceFormat::name), webp }
		})
		.collect()
}

/// Read the leading bytes of a file, enough for every signature [`SourceFormat::sniff`]
/// checks. Returns `None` for anything that cannot be read as a file.
fn read_prefix(path: &Path) -> Option<Vec<u8>> {
	use std::io::Read as _;

	if !path.is_file() {
		return None;
	}
	let mut file = fs::File::open(path).ok()?;
	// 64 bytes: enough for every fixed signature, and for an ISO-BMFF `ftyp` box's
	// compatible-brand list, which is where an AVIF may declare itself.
	let mut prefix = vec![0_u8; 64];
	let read = file.read(&mut prefix).ok()?;
	prefix.truncate(read);
	Some(prefix)
}

/// How deep a recursive scan will go.
///
/// A bound rather than unlimited recursion: a pathological tree (or a symlink arrangement
/// this walker does not follow but a future one might) should not be able to hang the
/// window.
const MAX_SCAN_DEPTH: usize = 32;

/// The most files a single scan will return.
///
/// Dropping a home directory on the window should produce a refusal, not a hundred
/// thousand entries the UI then tries to render.
const MAX_SCAN_FILES: usize = 10_000;

/// An image found by scanning a directory, and where it sat relative to the scan root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundImage {
	/// The absolute path of the image.
	pub path: PathBuf,
	/// Its path relative to the directory that was scanned, used to mirror the structure
	/// into the output directory.
	pub relative: PathBuf,
}

/// Find the images in `root`, optionally descending into subdirectories.
///
/// Returns paths sorted, so a batch converts in a predictable order rather than whatever
/// order the filesystem hands back.
///
/// **Symlinks are not followed.** A symlinked directory can point at its own ancestor, and
/// following one turns a scan into an infinite walk; a symlinked *file* can point outside
/// the tree the user thought they selected. Neither is worth the convenience.
///
/// Files are identified by content, as everywhere else, so a `.txt` that is really a PNG is
/// found and a `.png` that is really text is not.
#[must_use]
pub fn scan_directory(root: &Path, recursive: bool) -> Vec<FoundImage> {
	let mut found = Vec::new();
	let mut queue = vec![(root.to_path_buf(), 0_usize)];

	while let Some((directory, depth)) = queue.pop() {
		let Ok(entries) = fs::read_dir(&directory) else { continue };
		let mut children: Vec<PathBuf> = entries.filter_map(Result::ok).map(|entry| entry.path()).collect();
		// Sorted, and reversed for the stack, so the output order is deterministic.
		children.sort();
		for path in children.into_iter().rev() {
			if found.len() >= MAX_SCAN_FILES {
				break;
			}
			// symlink_metadata does not traverse the link, which is the point.
			let Ok(meta) = fs::symlink_metadata(&path) else { continue };
			if meta.file_type().is_symlink() {
				continue;
			}
			if meta.is_dir() {
				if recursive && depth < MAX_SCAN_DEPTH {
					queue.push((path, depth + 1));
				}
				continue;
			}
			if read_prefix(&path).as_deref().and_then(SourceFormat::sniff).is_some() {
				let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
				found.push(FoundImage { path, relative });
			}
		}
	}

	found.sort_by(|a, b| a.path.cmp(&b.path));
	found
}

/// Where a scanned image should be written, mirroring its position under the scan root.
///
/// Returns `None` if the relative path would escape `output_root` — a `..` component, an
/// absolute path, or a Windows path prefix. That cannot arise from [`scan_directory`],
/// whose relatives are built by `strip_prefix`, but this function is public and the
/// consequence of getting it wrong is writing a file somewhere the user did not choose.
#[must_use]
pub fn mirrored_output_path(output_root: &Path, relative: &Path, format: OutputFormat) -> Option<PathBuf> {
	use std::path::Component;

	let parent = relative.parent().unwrap_or(Path::new(""));
	if parent.components().any(|component| !matches!(component, Component::Normal(_))) {
		return None;
	}
	let directory = output_root.join(parent);
	Some(output_path_in(directory, relative, format))
}

/// What a completed conversion did, for the UI's before/after readout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversion {
	/// Size of the input file on disk, in bytes.
	pub source_bytes: u64,
	/// Size of the written WebP, in bytes.
	pub output_bytes: u64,
	/// Width of the encoded image, after any resize.
	pub width: u32,
	/// Height of the encoded image, after any resize.
	pub height: u32,
}

impl Conversion {
	/// The saving as a percentage of the original, negative if the output grew.
	///
	/// Returns `None` for a zero-byte source, where a ratio means nothing.
	#[must_use]
	pub fn saving_percent(self) -> Option<f64> {
		if self.source_bytes == 0 {
			return None;
		}
		// Done in integer arithmetic rather than by casting to f64: file sizes are u64, and
		// a percentage is the only thing that needs to be fractional. i128 cannot overflow
		// for any pair of u64 sizes.
		let source = i128::from(self.source_bytes);
		let output = i128::from(self.output_bytes);
		// Hundredths of a percent, narrowed to i32 before it meets a float: `f64::from` on an
		// i32 is exact, so no precision is lost anywhere in the calculation.
		let hundredths = (source - output) * 10_000 / source;
		let hundredths = i32::try_from(hundredths.clamp(i128::from(i32::MIN), i128::from(i32::MAX))).unwrap_or(0);
		Some(f64::from(hundredths) / 100.0)
	}
}

/// Why a conversion did not complete.
#[derive(Debug, Error)]
pub enum ConvertError {
	/// The input could not be read or decoded.
	#[error(transparent)]
	Source(#[from] SourceError),
	/// The encoder refused the settings or the image.
	#[error(transparent)]
	Encode(#[from] EncodeError),
	/// The output path is the input path. Refused rather than destroying the original.
	#[error("refusing to overwrite the source image `{path}`: choose a different output")]
	WouldOverwriteSource {
		/// The path that is both input and output.
		path: PathBuf,
	},
	/// The directory the output would go in does not exist.
	///
	/// Not created automatically: writing into a directory the user did not choose is the
	/// other half of not surprising them about where their files went.
	#[error("the output directory `{path}` does not exist")]
	MissingOutputDirectory {
		/// The directory that is missing.
		path: PathBuf,
	},
	/// Writing the output failed.
	#[error("could not write `{path}`: {source}")]
	Write {
		/// The path that failed.
		path: PathBuf,
		/// The underlying I/O error.
		source: io::Error,
	},
}

/// The output path the Electron app would derive: the input's file stem plus the format's
/// extension, in the chosen directory.
///
/// For WebP this is identical to the Electron app, so a user's converted files land where
/// they already expect; AVIF follows the same rule with `.avif`.
#[must_use]
pub fn output_path_in(directory: PathBuf, input: &Path, format: OutputFormat) -> PathBuf {
	let stem = input.file_stem().unwrap_or_else(|| OsStr::new("image"));
	// Append rather than `with_extension`, which would treat the stem's own dots as an
	// extension and turn `archive.tar.gz` into `archive.webp` instead of
	// `archive.tar.webp`. The Electron app concatenates, so this does too.
	let mut name = stem.to_os_string();
	name.push(".");
	name.push(format.extension());
	let mut path = directory;
	path.push(name);
	path
}

/// Convert an image file to WebP, writing the result to `output`.
///
/// The write is staged through a temporary file in the destination directory and renamed
/// into place, so a failure part-way cannot leave a truncated or half-written image where
/// a good one used to be.
///
/// # Errors
///
/// Returns [`ConvertError`] if the input cannot be read or decoded, the settings or image
/// are invalid, the output would overwrite the source, the output directory does not
/// exist, or the write fails.
pub fn encode_file(settings: &EncodeJob, input: &Path, output: &Path) -> Result<Conversion, ConvertError> {
	encode_file_with_progress(settings, input, output, &mut |_| true)
}

/// Convert an image file, reporting encoder progress and allowing the caller to stop.
///
/// `on_progress` receives 0..=100 and returns `false` to cancel. A cancelled conversion
/// writes nothing at all: the encode fails before the staging file is created, so the
/// destination is untouched and stopping a batch cannot leave a half-converted image.
///
/// # Errors
///
/// As [`encode_file`], plus a cancellation surfaced as [`ConvertError::Encode`] wrapping
/// [`EncodeError::Cancelled`].
pub fn encode_file_with_progress(settings: &EncodeJob, input: &Path, output: &Path, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Conversion, ConvertError> {
	let directory = output.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
	if !directory.is_dir() {
		return Err(ConvertError::MissingOutputDirectory { path: directory.to_path_buf() });
	}

	// Compare resolved paths, so `dir/photo.webp` and `dir/./photo.webp` are recognised as
	// the same file. An output that does not exist yet cannot be the input.
	if let (Ok(resolved_input), Ok(resolved_output)) = (input.canonicalize(), output.canonicalize())
		&& resolved_input == resolved_output
	{
		return Err(ConvertError::WouldOverwriteSource { path: input.to_path_buf() });
	}

	let bytes = fs::read(input).map_err(|source| SourceError::Read { path: input.to_path_buf(), source })?;
	let source_bytes = bytes.len() as u64;
	let transcoded = if settings.format == OutputFormat::Jxl && settings.jxl.lossless_jpeg { transcode_jpeg(settings, &bytes, on_progress)? } else { None };
	// An animation goes through the animation encoder whole. Only WebP output holds one
	// here, so any other format is refused rather than quietly keeping the first frame.
	let (encoded, fallback) = if let Some(encoded) = transcoded {
		(encoded, (0, 0))
	} else if is_animated_webp(&bytes) {
		if settings.format != OutputFormat::Webp {
			return Err(SourceError::Animated { path: input.to_path_buf() }.into());
		}
		let animation = crate::animation::decode(&bytes).ok_or_else(|| SourceError::Decode { path: input.to_path_buf(), format: SourceFormat::Webp.name(), detail: "libwebp could not read the animation".to_owned() })?;
		(crate::animation::encode(&settings.webp, settings.resize, &animation, on_progress)?, (animation.width, animation.height))
	} else {
		let image = decode(input, &bytes)?;
		(encode_rgba_with_progress(settings, &image.as_rgba(), on_progress)?, (image.width, image.height))
	};

	// A temporary name in the destination directory, so the rename stays on one filesystem
	// and is therefore atomic.
	let staging = directory.join(format!(".skidbladnir-{}-{}.{}.part", std::process::id(), output.file_name().and_then(OsStr::to_str).unwrap_or("out"), settings.format.extension()));
	fs::write(&staging, &encoded).map_err(|source| ConvertError::Write { path: staging.clone(), source })?;
	if let Err(source) = fs::rename(&staging, output) {
		let _ = fs::remove_file(&staging);
		return Err(ConvertError::Write { path: output.to_path_buf(), source });
	}

	let (width, height) = match settings.format {
		OutputFormat::Webp => encoded_dimensions(&encoded),
		OutputFormat::Avif => crate::avif::dimensions(&encoded),
		OutputFormat::Jxl => crate::jxl::dimensions(&encoded),
		OutputFormat::Heic => crate::heic::dimensions(&encoded),
	}
	.unwrap_or(fallback);
	Ok(Conversion { source_bytes, output_bytes: encoded.len() as u64, width, height })
}

/// The lossless JPEG XL recompression of `bytes`, when the job asks for it and it applies:
/// the job writes JPEG XL with [`JxlSettings::lossless_jpeg`] on, the input is a JPEG, and
/// no resize takes effect for it (a resize needs pixels, and recompression never decodes
/// any). `Ok(None)` means "encode the pixels instead", including when libjxl declines a
/// particular JPEG.
///
/// # Errors
///
/// Returns [`EncodeError`] if libjxl fails outright or the encode was cancelled.
///
/// [`JxlSettings::lossless_jpeg`]: crate::settings::JxlSettings::lossless_jpeg
pub fn transcode_jpeg(settings: &EncodeJob, bytes: &[u8], on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Option<Vec<u8>>, EncodeError> {
	if settings.format != OutputFormat::Jxl || !settings.jxl.lossless_jpeg || SourceFormat::sniff(bytes) != Some(SourceFormat::Jpeg) {
		return Ok(None);
	}
	let Ok((width, height)) = image::ImageReader::with_format(io::Cursor::new(bytes), image::ImageFormat::Jpeg).into_dimensions() else { return Ok(None) };
	if !settings.resize.for_source(width, height).is_noop() {
		return Ok(None);
	}
	crate::jxl::recompress_jpeg(&settings.jxl, bytes, on_progress)
}

/// Read the dimensions back out of an encoded WebP, which is the only way to learn what a
/// resize actually produced when a dimension was given as zero.
fn encoded_dimensions(webp: &[u8]) -> Option<(u32, u32)> {
	let mut width = 0;
	let mut height = 0;
	// SAFETY: `webp` is a valid slice and the out-parameters are live for the call.
	if unsafe { libwebp_sys::WebPGetInfo(webp.as_ptr(), webp.len(), &raw mut width, &raw mut height) } == 0 {
		return None;
	}
	Some((u32::try_from(width).ok()?, u32::try_from(height).ok()?))
}

#[cfg(test)]
mod tests {
	use std::{
		fs, path::{Path, PathBuf}
	};

	use super::{Conversion, ConvertError, SourceFormat, encode_file, load, output_path_in};
	use crate::settings::{AvifSettings, EncodeJob, Mode, OutputFormat, Resize, WebpSettings};

	/// A scratch directory that cleans itself up. Every test in this module writes only
	/// inside one of these — nothing here touches a path outside the temp directory.
	struct Scratch(PathBuf);

	impl Scratch {
		fn new(name: &str) -> Self {
			let path = std::env::temp_dir().join(format!("skidbladnir-source-{}-{name}", std::process::id()));
			let _ = fs::remove_dir_all(&path);
			fs::create_dir_all(&path).expect("create the scratch directory");
			Self(path)
		}

		fn join(&self, name: &str) -> PathBuf {
			self.0.join(name)
		}
	}

	impl Drop for Scratch {
		fn drop(&mut self) {
			let _ = fs::remove_dir_all(&self.0);
		}
	}

	/// Write a minimal but real RGBA PNG, so the decoder has something legitimate to read.
	fn write_png(path: &Path, width: u32, height: u32) {
		let mut pixels = Vec::with_capacity((width * height * 4) as usize);
		for y in 0..height {
			for x in 0..width {
				pixels.extend_from_slice(&[u8::try_from((x * 5) % 256).unwrap_or(0), u8::try_from((y * 3) % 256).unwrap_or(0), 128, if (x + y) % 5 == 0 { 0 } else { 255 }]);
			}
		}
		let buffer = image::RgbaImage::from_raw(width, height, pixels).expect("buffer matches the dimensions");
		buffer.save_with_format(path, image::ImageFormat::Png).expect("write the PNG fixture");
	}

	#[test]
	fn sniffs_the_accepted_formats() {
		assert_eq!(SourceFormat::sniff(b"\x89PNG\r\n\x1a\n....."), Some(SourceFormat::Png));
		assert_eq!(SourceFormat::sniff(b"\xff\xd8\xff\xe0...."), Some(SourceFormat::Jpeg));
		assert_eq!(SourceFormat::sniff(b"II\x2a\x00...."), Some(SourceFormat::Tiff));
		assert_eq!(SourceFormat::sniff(b"MM\x00\x2a...."), Some(SourceFormat::Tiff));
		assert_eq!(SourceFormat::sniff(b"RIFF\x00\x00\x00\x00WEBPVP8 "), Some(SourceFormat::Webp));
		assert_eq!(SourceFormat::sniff(b"\0\0\0\x1cftypavif\0\0\0\0avifmif1miaf"), Some(SourceFormat::Avif));
		assert_eq!(SourceFormat::sniff(b"\0\0\0\x1cftypmif1\0\0\0\0mif1avifmiaf"), Some(SourceFormat::Avif), "AVIF declared as a compatible brand");
		assert_eq!(SourceFormat::sniff(b"\0\0\0\x18ftypheic\0\0\0\0mif1heic"), Some(SourceFormat::Heic), "HEIC shares the container and is told apart by its brand");
		assert_eq!(SourceFormat::sniff(b"\0\0\0\x18ftypmif1\0\0\0\0mif1heic"), Some(SourceFormat::Heic), "HEIC declared as a compatible brand");
		assert_eq!(SourceFormat::sniff(b"\0\0\0\x10ftypmif1\0\0\0\0avif"), None, "a brand past the end of the ftyp box does not count");
		assert_eq!(SourceFormat::sniff(b"\xff\x0a\x00\x00...."), Some(SourceFormat::Jxl), "a bare JPEG XL codestream");
		assert_eq!(SourceFormat::sniff(b"\0\0\0\x0cJXL \r\n\x87\n\0\0"), Some(SourceFormat::Jxl), "a JPEG XL container");
		assert_eq!(SourceFormat::sniff(b"GIF89a......"), None);
		assert_eq!(SourceFormat::sniff(b"RIFF\x00\x00\x00\x00WAVEfmt "), None, "a RIFF container that is not WebP");
		assert_eq!(SourceFormat::sniff(b"\x89PN"), None, "a truncated header must not match");
		assert_eq!(SourceFormat::sniff(b""), None);
	}

	#[test]
	fn output_path_follows_the_electron_naming() {
		assert_eq!(output_path_in(PathBuf::from("/out"), Path::new("/in/holiday.JPG"), OutputFormat::Webp), PathBuf::from("/out/holiday.webp"));
		assert_eq!(output_path_in(PathBuf::from("/out"), Path::new("/in/archive.tar.gz"), OutputFormat::Webp), PathBuf::from("/out/archive.tar.webp"));
		assert_eq!(output_path_in(PathBuf::from("/out"), Path::new("/in/holiday.JPG"), OutputFormat::Avif), PathBuf::from("/out/holiday.avif"));
	}

	#[test]
	fn converts_a_png_and_reports_both_sizes() {
		let scratch = Scratch::new("convert");
		let input = scratch.join("source.png");
		let output = scratch.join("source.webp");
		write_png(&input, 64, 48);

		let conversion = encode_file(&EncodeJob::default(), &input, &output).expect("conversion succeeds");
		assert!(output.is_file(), "the output file must exist");
		assert_eq!(conversion.output_bytes, fs::metadata(&output).expect("stat the output").len(), "the reported output size must be the file on disk");
		assert_eq!(conversion.source_bytes, fs::metadata(&input).expect("stat the input").len());
		assert_eq!((conversion.width, conversion.height), (64, 48));
		assert_eq!(&fs::read(&output).expect("read the output")[0..4], b"RIFF");
	}

	/// A resize with one dimension zero is only resolved by the encoder, so the reported
	/// dimensions must come from the encoded file rather than from the request.
	#[test]
	fn reports_the_dimensions_a_resize_actually_produced() {
		let scratch = Scratch::new("resize");
		let input = scratch.join("source.png");
		write_png(&input, 64, 48);
		let settings = EncodeJob { resize: Resize { width: 32, height: 0, no_enlarge: false }, ..Default::default() };
		let conversion = encode_file(&settings, &input, &scratch.join("out.webp")).expect("conversion succeeds");
		assert_eq!((conversion.width, conversion.height), (32, 24), "height must be derived, not echoed back as 0");
	}

	/// The rule that matters most: converting a WebP into its own directory derives the
	/// input's own name, and must not destroy it.
	#[test]
	fn refuses_to_overwrite_the_source_image() {
		let scratch = Scratch::new("overwrite");
		let png = scratch.join("photo.png");
		write_png(&png, 32, 32);
		let webp = scratch.join("photo.webp");
		encode_file(&EncodeJob::default(), &png, &webp).expect("first conversion succeeds");
		let original = fs::read(&webp).expect("read the converted file");

		// Now convert that WebP into the same directory: output_path_in derives photo.webp,
		// which is the input.
		let derived = output_path_in(scratch.0.clone(), &webp, OutputFormat::Webp);
		assert_eq!(derived, webp, "this is the dangerous case the guard exists for");
		let error = encode_file(&EncodeJob::default(), &webp, &derived).expect_err("must refuse");
		assert!(matches!(error, ConvertError::WouldOverwriteSource { .. }), "got {error}");
		assert_eq!(fs::read(&webp).expect("read the file again"), original, "the source must be byte-identical after the refusal");
	}

	/// A failed encode must not damage a file that is already there.
	#[test]
	fn a_failed_conversion_leaves_an_existing_output_untouched() {
		let scratch = Scratch::new("failure");
		let input = scratch.join("source.png");
		write_png(&input, 16, 16);
		let output = scratch.join("existing.webp");
		fs::write(&output, b"PRECIOUS EXISTING CONTENT").expect("seed the output file");

		// Invalid settings: the encode fails before anything is written.
		let settings = EncodeJob::from(WebpSettings { method: 9, ..Default::default() });
		assert!(encode_file(&settings, &input, &output).is_err());
		assert_eq!(fs::read(&output).expect("read the output"), b"PRECIOUS EXISTING CONTENT", "a failed encode must not truncate the destination");
	}

	#[test]
	fn leaves_no_staging_files_behind() {
		let scratch = Scratch::new("staging");
		let input = scratch.join("source.png");
		write_png(&input, 24, 24);
		encode_file(&EncodeJob::default(), &input, &scratch.join("out.webp")).expect("conversion succeeds");
		let leftovers: Vec<_> = fs::read_dir(&scratch.0).expect("list the directory").filter_map(Result::ok).filter(|entry| entry.file_name().to_string_lossy().contains(".part")).collect();
		assert!(leftovers.is_empty(), "staging files left behind: {leftovers:?}");
	}

	#[test]
	fn a_missing_output_directory_is_refused_rather_than_created() {
		let scratch = Scratch::new("missingdir");
		let input = scratch.join("source.png");
		write_png(&input, 16, 16);
		let missing = scratch.join("nope");
		let error = encode_file(&EncodeJob::default(), &input, &missing.join("out.webp")).expect_err("must refuse");
		assert!(matches!(error, ConvertError::MissingOutputDirectory { .. }), "got {error}");
		assert!(!missing.exists(), "the directory must not have been created");
	}

	/// A JPEG going to JPEG XL is recompressed, not re-encoded: the file rebuilds the
	/// original JPEG exactly. With a resize in effect there is no JPEG left to keep, so
	/// the pixels are encoded instead and the dimensions follow the resize.
	#[test]
	fn a_jpeg_to_jpeg_xl_is_recompressed_losslessly_unless_resized() {
		let scratch = Scratch::new("jxl-jpeg");
		let input = scratch.join("photo.jpg");
		let rgb: Vec<u8> = (0..40 * 30).flat_map(|i: u32| [u8::try_from(i % 256).unwrap_or(0), 90, u8::try_from(i / 5 % 256).unwrap_or(0)]).collect();
		let mut jpeg = Vec::new();
		image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 80).encode(&rgb, 40, 30, image::ExtendedColorType::Rgb8).expect("write a JPEG");
		fs::write(&input, &jpeg).expect("write the input");

		let job = EncodeJob { format: OutputFormat::Jxl, ..Default::default() };
		let output = scratch.join("photo.jxl");
		let conversion = encode_file(&job, &input, &output).expect("convert");
		assert_eq!((conversion.width, conversion.height), (40, 30));
		let written = fs::read(&output).expect("read the output");
		let mut rebuilt = Vec::new();
		jxl_oxide::JxlImage::builder().read(written.as_slice()).expect("parse").reconstruct_jpeg(&mut rebuilt).expect("the file carries the JPEG");
		assert_eq!(rebuilt, jpeg);

		let resized = EncodeJob { resize: Resize { width: 20, height: 0, no_enlarge: false }, ..job };
		let conversion = encode_file(&resized, &input, &output).expect("convert resized");
		assert_eq!((conversion.width, conversion.height), (20, 15));
	}

	#[test]
	fn a_non_image_is_rejected_with_a_useful_error() {
		let scratch = Scratch::new("notimage");
		let input = scratch.join("notes.txt");
		fs::write(&input, b"this is not an image").expect("write the file");
		let error = load(&input).expect_err("must refuse");
		assert!(error.to_string().contains("not a PNG, JPEG, TIFF, WebP, AVIF, JPEG XL or HEIC"), "got {error}");
	}

	/// WebP input goes through libwebp itself, so a WebP -> WebP conversion does not
	/// round-trip through a second, non-reference decoder.
	#[test]
	fn webp_input_is_decoded_by_libwebp() {
		let scratch = Scratch::new("webpin");
		let png = scratch.join("source.png");
		write_png(&png, 40, 30);
		let webp = scratch.join("intermediate.webp");
		encode_file(&EncodeJob::from(WebpSettings { mode: Mode::Lossless, ..Default::default() }), &png, &webp).expect("encode to webp");

		let loaded = load(&webp).expect("decode the webp");
		assert_eq!(loaded.format, SourceFormat::Webp);
		assert_eq!((loaded.width, loaded.height), (40, 30));
		let original = load(&png).expect("decode the png");
		assert_eq!(loaded.pixels, original.pixels, "a lossless round trip must return the exact pixels");
	}

	/// Cancelling must leave the destination exactly as it was, staging file included.
	#[test]
	fn a_cancelled_conversion_writes_nothing() {
		let scratch = Scratch::new("cancel");
		let input = scratch.join("source.png");
		write_png(&input, 64, 64);
		let output = scratch.join("out.webp");

		let error = super::encode_file_with_progress(&EncodeJob::default(), &input, &output, &mut |_| false).expect_err("must cancel");
		assert!(matches!(error, ConvertError::Encode(crate::encoder::EncodeError::Cancelled)), "got {error}");
		assert!(!output.exists(), "a cancelled conversion must not create the output");
		let leftovers: Vec<_> = fs::read_dir(&scratch.0).expect("list").filter_map(Result::ok).filter(|e| e.file_name().to_string_lossy().contains(".part")).collect();
		assert!(leftovers.is_empty(), "staging files left behind: {leftovers:?}");
	}

	#[test]
	fn progress_reaches_the_caller_through_the_file_layer() {
		let scratch = Scratch::new("fileprogress");
		let input = scratch.join("source.png");
		write_png(&input, 96, 64);
		let mut seen = Vec::new();
		super::encode_file_with_progress(&EncodeJob::default(), &input, &scratch.join("out.webp"), &mut |percent| {
			seen.push(percent);
			true
		})
		.expect("converts");
		assert!(!seen.is_empty(), "no progress reached the caller");
	}

	#[test]
	fn inspect_paths_separates_images_from_everything_else() {
		let scratch = Scratch::new("inspect");
		let png = scratch.join("real.png");
		write_png(&png, 8, 8);
		let text = scratch.join("notes.txt");
		fs::write(&text, b"not an image at all").expect("write the text file");
		// A file whose extension lies: PNG bytes named .txt, and text named .png.
		let mislabelled_image = scratch.join("actually-a-png.txt");
		fs::copy(&png, &mislabelled_image).expect("copy the png");
		let mislabelled_text = scratch.join("actually-text.png");
		fs::write(&mislabelled_text, b"still not an image").expect("write the fake png");
		let directory = scratch.join("a-folder");
		fs::create_dir_all(&directory).expect("create the directory");
		let missing = scratch.join("does-not-exist.png");

		let inspected = super::inspect_paths(&[png, text, mislabelled_image, mislabelled_text, directory, missing]);
		let supported: Vec<bool> = inspected.iter().map(|entry| entry.supported).collect();
		assert_eq!(supported, vec![true, false, true, false, false, false], "{inspected:#?}");
		assert_eq!(inspected[0].format, Some("PNG"));
		assert_eq!(inspected[0].webp, None, "only a WebP carries WebP details");
		assert_eq!(inspected[2].format, Some("PNG"), "a PNG named .txt is still a PNG");
		assert_eq!(inspected[3].format, None, "text named .png is still not an image");
	}

	/// A tiny file must not be mistaken for an image just because it is short.
	/// A WebP path reports its header details, which is what lets the UI warn before
	/// converting rather than after.
	#[test]
	fn inspect_paths_reports_webp_details() {
		let scratch = Scratch::new("webpdetails");
		let png = scratch.join("source.png");
		write_png(&png, 40, 30);
		let webp = scratch.join("out.webp");
		super::encode_file(&EncodeJob::from(WebpSettings { mode: Mode::Lossless, ..Default::default() }), &png, &webp).expect("encode");

		let inspected = super::inspect_paths(&[webp]);
		let details = inspected[0].webp.expect("a WebP must report its header");
		assert_eq!((details.width, details.height), (40, 30));
		assert_eq!(details.compression, crate::inspect::WebpCompression::Lossless);
		assert!(!details.has_animation, "a still image is not an animation");
	}

	#[test]
	fn inspect_paths_handles_files_shorter_than_a_signature() {
		let scratch = Scratch::new("shortfile");
		let tiny = scratch.join("tiny.png");
		fs::write(&tiny, b"\x89PN").expect("write the tiny file");
		let empty = scratch.join("empty.png");
		fs::write(&empty, b"").expect("write the empty file");
		let inspected = super::inspect_paths(&[tiny, empty]);
		assert!(inspected.iter().all(|entry| !entry.supported), "{inspected:#?}");
	}

	#[test]
	fn scans_a_directory_flat_and_recursively() {
		let scratch = Scratch::new("scan");
		write_png(&scratch.join("b.png"), 8, 8);
		write_png(&scratch.join("a.png"), 8, 8);
		fs::write(scratch.join("notes.txt"), b"not an image").expect("write");
		let nested = scratch.join("nested");
		fs::create_dir_all(nested.join("deeper")).expect("mkdir");
		write_png(&nested.join("c.png"), 8, 8);
		write_png(&nested.join("deeper").join("d.png"), 8, 8);

		let flat = super::scan_directory(&scratch.0, false);
		assert_eq!(flat.iter().map(|f| f.relative.to_string_lossy().into_owned()).collect::<Vec<_>>(), vec!["a.png", "b.png"], "a flat scan must not descend");

		let deep = super::scan_directory(&scratch.0, true);
		let relatives: Vec<String> = deep.iter().map(|f| f.relative.to_string_lossy().replace('\\', "/")).collect();
		assert_eq!(relatives, vec!["a.png", "b.png", "nested/c.png", "nested/deeper/d.png"], "sorted and recursive");
	}

	/// A symlinked directory can point at its own ancestor; following one turns a scan into
	/// an endless walk.
	#[cfg(unix)]
	#[test]
	fn symlinks_are_not_followed() {
		let scratch = Scratch::new("symlink");
		write_png(&scratch.join("real.png"), 8, 8);
		let loop_dir = scratch.join("loop");
		std::os::unix::fs::symlink(&scratch.0, &loop_dir).expect("create the loop");
		std::os::unix::fs::symlink(scratch.join("real.png"), scratch.join("link.png")).expect("link a file");

		let found = super::scan_directory(&scratch.0, true);
		assert_eq!(found.len(), 1, "only the real file: {found:#?}");
		assert_eq!(found[0].relative.to_string_lossy(), "real.png");
	}

	#[test]
	fn mirrors_the_structure_into_the_output_directory() {
		let out = Path::new("/out");
		assert_eq!(super::mirrored_output_path(out, Path::new("a.png"), OutputFormat::Webp), Some(PathBuf::from("/out/a.webp")), "the output is a WebP, not a copy of the input name");
		assert_eq!(super::mirrored_output_path(out, Path::new("nested/deeper/d.JPG"), OutputFormat::Webp), Some(PathBuf::from("/out/nested/deeper/d.webp")));
	}

	/// The guard that matters: a relative path must never be able to write outside the
	/// directory the user chose.
	#[test]
	fn a_relative_path_cannot_escape_the_output_directory() {
		let out = Path::new("/out");
		assert_eq!(super::mirrored_output_path(out, Path::new("../escaped.png"), OutputFormat::Webp), None);
		assert_eq!(super::mirrored_output_path(out, Path::new("nested/../../escaped.png"), OutputFormat::Webp), None);
		assert_eq!(super::mirrored_output_path(out, Path::new("/absolute/escaped.png"), OutputFormat::Webp), None);
	}

	/// Loading an animated WebP as a still must be refused with a message that explains
	/// itself, rather than handing back one frame as if it were the whole image.
	///
	/// libwebp's still decoder already refuses one, but it reports "libwebp rejected the
	/// file", which tells the user nothing.
	#[test]
	fn an_animated_webp_is_refused_as_a_still_by_name() {
		let animated = animated_fixture().expect("libwebp built the animated fixture");
		assert!(crate::inspect::inspect_webp(&animated).is_some_and(|info| info.has_animation), "the fixture must really be an animation");
		let scratch = Scratch::new("animated");
		let path = scratch.join("animation.webp");
		fs::write(&path, &animated).expect("write the fixture");

		let error = super::load(&path).expect_err("an animation must be refused");
		assert!(matches!(error, super::SourceError::Animated { .. }), "got {error}");
		assert!(error.to_string().contains("animated WebP"), "the message must say why: {error}");

		// And the UI-facing inspection agrees, so the warning and the refusal cannot drift.
		let inspected = super::inspect_paths(&[path]);
		assert!(inspected[0].webp.expect("webp details").has_animation);
	}

	/// Converting an animated WebP to WebP re-encodes every frame, with their timing.
	#[test]
	fn an_animated_webp_converts_to_an_animated_webp() {
		let animated = animated_fixture().expect("libwebp built the animated fixture");
		let scratch = Scratch::new("animated-converts");
		let input = scratch.join("animation.webp");
		fs::write(&input, &animated).expect("write the fixture");
		let output = scratch.join("out.webp");

		let job = EncodeJob { resize: Resize { width: 16, height: 0, no_enlarge: false }, ..EncodeJob::default() };
		let conversion = encode_file(&job, &input, &output).expect("an animation must convert to WebP");
		assert_eq!((conversion.width, conversion.height), (16, 12), "the reported size is the resized canvas");

		let written = fs::read(&output).expect("read the output");
		let decoded = crate::animation::decode(&written).expect("the output must be an animation libwebp reads");
		assert_eq!(decoded.frames.len(), 2, "no frame may be dropped");
		assert_eq!(decoded.frames.iter().map(|frame| frame.duration_ms).collect::<Vec<_>>(), [100, 100], "the timing carries over");
		assert_eq!(conversion.output_bytes, written.len() as u64);
	}

	/// Asking for AVIF from an animation is refused by name and writes nothing, rather than
	/// keeping only the first frame.
	#[test]
	fn an_animated_webp_is_not_cut_down_to_an_avif_still() {
		let animated = animated_fixture().expect("libwebp built the animated fixture");
		let scratch = Scratch::new("animated-avif");
		let input = scratch.join("animation.webp");
		fs::write(&input, &animated).expect("write the fixture");
		let output = scratch.join("out.avif");

		let job = EncodeJob { format: OutputFormat::Avif, ..EncodeJob::default() };
		let error = encode_file(&job, &input, &output).expect_err("AVIF output of an animation must be refused");
		assert!(matches!(error, ConvertError::Source(super::SourceError::Animated { .. })), "got {error}");
		assert!(error.to_string().contains("AVIF"), "the message must say what cannot take it: {error}");
		assert!(!output.exists(), "a refused conversion writes nothing");
		assert_eq!(fs::read_dir(&scratch.0).expect("list").count(), 1, "not even a staging file");
	}

	/// Build a real two-frame animated WebP with libwebp's own animation encoder.
	///
	/// Built rather than taken from the environment, so the test always runs. A test that
	/// skips unless someone remembered to set a variable is a test that never runs in CI,
	/// and this one guards a refusal a user actually depends on.
	fn animated_fixture() -> Option<Vec<u8>> {
		use libwebp_sys::{WEBP_ENCODER_ABI_VERSION, WEBP_MUX_ABI_VERSION, WebPAnimEncoderAdd, WebPAnimEncoderAssemble, WebPAnimEncoderDelete, WebPAnimEncoderNewInternal, WebPAnimEncoderOptions, WebPAnimEncoderOptionsInitInternal, WebPConfig, WebPConfigInitInternal, WebPData, WebPDataClear, WebPPicture, WebPPictureFree, WebPPictureImportRGBA, WebPPictureInitInternal, WebPPreset};

		let (width, height) = (32_i32, 24_i32);

		// SAFETY: every out-parameter below is a live local for the duration of the call,
		// each picture is freed after it is added, and the encoder and WebPData are released
		// before returning.
		unsafe {
			let mut options = std::mem::zeroed::<WebPAnimEncoderOptions>();
			if WebPAnimEncoderOptionsInitInternal(&raw mut options, WEBP_MUX_ABI_VERSION.cast_signed()) == 0 {
				return None;
			}
			let encoder = WebPAnimEncoderNewInternal(width, height, &raw const options, WEBP_MUX_ABI_VERSION.cast_signed());
			if encoder.is_null() {
				return None;
			}

			let mut config = std::mem::zeroed::<WebPConfig>();
			if WebPConfigInitInternal(&raw mut config, WebPPreset::WEBP_PRESET_DEFAULT, 75.0, WEBP_ENCODER_ABI_VERSION.cast_signed()) == 0 {
				WebPAnimEncoderDelete(encoder);
				return None;
			}

			for (index, timestamp) in [(0_u8, 0_i32), (1, 100)] {
				let pixels: Vec<u8> = (0..width * height).flat_map(|i| [u8::try_from(i % 256).unwrap_or(0), index.wrapping_mul(90), 60, 255]).collect();
				let mut picture = std::mem::zeroed::<WebPPicture>();
				if WebPPictureInitInternal(&raw mut picture, WEBP_ENCODER_ABI_VERSION.cast_signed()) == 0 {
					WebPAnimEncoderDelete(encoder);
					return None;
				}
				picture.use_argb = 1;
				picture.width = width;
				picture.height = height;
				let imported = WebPPictureImportRGBA(&raw mut picture, pixels.as_ptr(), width * 4) != 0;
				let added = imported && WebPAnimEncoderAdd(encoder, &raw mut picture, timestamp, &raw const config) != 0;
				WebPPictureFree(&raw mut picture);
				if !added {
					WebPAnimEncoderDelete(encoder);
					return None;
				}
			}
			// A final null frame closes the last frame's duration.
			WebPAnimEncoderAdd(encoder, std::ptr::null_mut(), 200, std::ptr::null());

			let mut data = std::mem::zeroed::<WebPData>();
			let assembled = WebPAnimEncoderAssemble(encoder, &raw mut data) != 0;
			WebPAnimEncoderDelete(encoder);
			if !assembled || data.bytes.is_null() {
				return None;
			}
			let bytes = std::slice::from_raw_parts(data.bytes, data.size).to_vec();
			WebPDataClear(&mut data);
			Some(bytes)
		}
	}

	#[test]
	fn saving_percent_handles_growth_and_empty_sources() {
		assert_eq!(Conversion { source_bytes: 1000, output_bytes: 250, width: 1, height: 1 }.saving_percent(), Some(75.0));
		assert_eq!(Conversion { source_bytes: 100, output_bytes: 150, width: 1, height: 1 }.saving_percent(), Some(-50.0));
		assert_eq!(Conversion { source_bytes: 0, output_bytes: 10, width: 1, height: 1 }.saving_percent(), None);
	}

	/// The file pipeline writes AVIF when asked, under an `.avif` name, and reports the
	/// dimensions read back out of the file.
	#[test]
	fn converts_to_avif_through_the_file_layer() {
		let scratch = Scratch::new("avif");
		let input = scratch.join("in.png");
		write_png(&input, 48, 32);
		let output = output_path_in(scratch.0.clone(), &input, OutputFormat::Avif);
		assert!(output.ends_with("in.avif"));
		let job = EncodeJob { format: OutputFormat::Avif, resize: Resize { width: 24, height: 0, no_enlarge: false }, avif: AvifSettings { speed: 10, ..Default::default() }, ..Default::default() };
		let conversion = encode_file(&job, &input, &output).expect("AVIF conversion succeeds");
		assert_eq!((conversion.width, conversion.height), (24, 16));
		let bytes = std::fs::read(&output).expect("read the output");
		assert_eq!(&bytes[4..12], b"ftypavif");
		assert_eq!(conversion.output_bytes, bytes.len() as u64);
		assert_eq!(std::fs::read_dir(&scratch.0).expect("list").count(), 2, "no staging file may be left behind");
	}

	/// AVIF input through the whole file layer: sniffed by content, decoded, re-encoded.
	#[test]
	fn reads_avif_input() {
		let scratch = Scratch::new("avif-in");
		let png = scratch.join("in.png");
		write_png(&png, 48, 32);
		let avif = scratch.join("in.avif");
		let job = EncodeJob { format: OutputFormat::Avif, avif: AvifSettings { speed: 10, ..Default::default() }, ..Default::default() };
		encode_file(&job, &png, &avif).expect("write an AVIF to read back");

		let loaded = load(&avif).expect("an AVIF must load");
		assert_eq!(loaded.format, SourceFormat::Avif);
		assert_eq!((loaded.width, loaded.height), (48, 32));

		let inspected = super::inspect_paths(std::slice::from_ref(&avif));
		assert!(inspected[0].supported, "the window must accept a dropped AVIF");
		assert_eq!(inspected[0].format, Some("AVIF"));

		// AVIF to WebP, and AVIF to AVIF into another folder.
		encode_file(&EncodeJob::default(), &avif, &scratch.join("from-avif.webp")).expect("AVIF converts to WebP");
		std::fs::create_dir_all(scratch.join("out")).expect("create out");
		encode_file(&job, &avif, &scratch.join("out").join("in.avif")).expect("AVIF re-encodes to AVIF elsewhere");
	}

	/// Converting an AVIF to AVIF in its own folder derives its own name, and must be
	/// refused exactly as WebP-to-WebP is.
	#[test]
	fn avif_to_avif_in_place_is_refused() {
		let scratch = Scratch::new("avif-same");
		let png = scratch.join("photo.png");
		write_png(&png, 16, 16);
		let job = EncodeJob { format: OutputFormat::Avif, avif: AvifSettings { speed: 10, ..Default::default() }, ..Default::default() };
		let avif = scratch.join("photo.avif");
		encode_file(&job, &png, &avif).expect("write the AVIF");
		let before = std::fs::read(&avif).expect("read it");
		let derived = output_path_in(scratch.0.clone(), &avif, OutputFormat::Avif);
		let error = encode_file(&job, &avif, &derived).expect_err("must refuse");
		assert!(matches!(error, ConvertError::WouldOverwriteSource { .. }), "{error:?}");
		assert_eq!(std::fs::read(&avif).expect("read it again"), before, "the source must be untouched");
	}

	/// A file that claims AVIF but is not one fails with a decode error naming the format.
	#[test]
	fn a_corrupt_avif_is_a_decode_error() {
		let scratch = Scratch::new("avif-bad");
		let path = scratch.join("bad.avif");
		std::fs::write(&path, b"\0\0\0\x14ftypavif\0\0\0\0avifgarbage-not-a-box").expect("write");
		let error = load(&path).expect_err("must not decode");
		assert!(error.to_string().contains("AVIF"), "{error}");
	}
}
