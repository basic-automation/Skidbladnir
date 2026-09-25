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
	encoder::{EncodeError, RgbaImage, encode_rgba_with_progress}, settings::EncodeSettings
};

/// The input formats Skidbladnir accepts, matching the Electron app's accepted list.
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
		}
	}
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
	#[error("`{path}` is not a PNG, JPEG, TIFF or WebP image")]
	Unsupported {
		/// The path that failed.
		path: PathBuf,
	},
	/// The file is an animated WebP, which this encoder cannot re-encode.
	///
	/// Distinguished from a decode failure because it is not one: the file is perfectly
	/// valid, it is simply a kind of image the still-image encoder has nothing to do with.
	/// libwebp's still decoder refuses it anyway, but with a message that tells the user
	/// nothing about why.
	#[error("`{path}` is an animated WebP. Skidbladnir encodes still images, so it cannot re-encode an animation.")]
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
	let format = SourceFormat::sniff(&bytes).ok_or_else(|| SourceError::Unsupported { path: path.to_path_buf() })?;

	let (width, height, pixels) = match format {
		// libwebp decodes its own format; using a second WebP implementation here would
		// mean a WebP-to-WebP conversion round-trips through a decoder that is not the
		// reference one.
		SourceFormat::Webp => {
			// Check before decoding: libwebp's still decoder refuses an animation, but the
			// failure it reports says nothing about why, and "libwebp rejected the file" is
			// not something a user can act on.
			if crate::inspect::inspect_webp(&bytes).is_some_and(|info| info.has_animation) {
				return Err(SourceError::Animated { path: path.to_path_buf() });
			}
			decode_webp(&bytes).ok_or_else(|| SourceError::Decode { path: path.to_path_buf(), format: format.name(), detail: "libwebp rejected the file".to_owned() })?
		}
		SourceFormat::Png | SourceFormat::Jpeg | SourceFormat::Tiff => {
			let decoded = image::load_from_memory(&bytes).map_err(|error| SourceError::Decode { path: path.to_path_buf(), format: format.name(), detail: error.to_string() })?;
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
	let mut prefix = vec![0_u8; 16];
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
pub fn mirrored_output_path(output_root: &Path, relative: &Path) -> Option<PathBuf> {
	use std::path::Component;

	let parent = relative.parent().unwrap_or(Path::new(""));
	if parent.components().any(|component| !matches!(component, Component::Normal(_))) {
		return None;
	}
	let directory = output_root.join(parent);
	Some(output_path_in(directory, relative))
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

/// The output path the Electron app would derive: the input's file stem, `.webp`, in the
/// chosen directory.
///
/// Kept identical so a user's converted files land where they already expect.
#[must_use]
pub fn output_path_in(directory: PathBuf, input: &Path) -> PathBuf {
	let stem = input.file_stem().unwrap_or_else(|| OsStr::new("image"));
	// Append rather than `with_extension`, which would treat the stem's own dots as an
	// extension and turn `archive.tar.gz` into `archive.webp` instead of
	// `archive.tar.webp`. The Electron app concatenates, so this does too.
	let mut name = stem.to_os_string();
	name.push(".webp");
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
pub fn encode_file(settings: &EncodeSettings, input: &Path, output: &Path) -> Result<Conversion, ConvertError> {
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
pub fn encode_file_with_progress(settings: &EncodeSettings, input: &Path, output: &Path, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Conversion, ConvertError> {
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

	let source_bytes = fs::metadata(input).map(|meta| meta.len()).map_err(|source| SourceError::Read { path: input.to_path_buf(), source })?;
	let image = load(input)?;
	let encoded = encode_rgba_with_progress(settings, &image.as_rgba(), on_progress)?;

	// A temporary name in the destination directory, so the rename stays on one filesystem
	// and is therefore atomic.
	let staging = directory.join(format!(".skidbladnir-{}-{}.webp.part", std::process::id(), output.file_name().and_then(OsStr::to_str).unwrap_or("out")));
	fs::write(&staging, &encoded).map_err(|source| ConvertError::Write { path: staging.clone(), source })?;
	if let Err(source) = fs::rename(&staging, output) {
		let _ = fs::remove_file(&staging);
		return Err(ConvertError::Write { path: output.to_path_buf(), source });
	}

	let (width, height) = encoded_dimensions(&encoded).unwrap_or((image.width, image.height));
	Ok(Conversion { source_bytes, output_bytes: encoded.len() as u64, width, height })
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
	use crate::settings::{EncodeSettings, Mode, Resize};

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
		assert_eq!(SourceFormat::sniff(b"GIF89a......"), None);
		assert_eq!(SourceFormat::sniff(b"RIFF\x00\x00\x00\x00WAVEfmt "), None, "a RIFF container that is not WebP");
		assert_eq!(SourceFormat::sniff(b"\x89PN"), None, "a truncated header must not match");
		assert_eq!(SourceFormat::sniff(b""), None);
	}

	#[test]
	fn output_path_follows_the_electron_naming() {
		assert_eq!(output_path_in(PathBuf::from("/out"), Path::new("/in/holiday.JPG")), PathBuf::from("/out/holiday.webp"));
		assert_eq!(output_path_in(PathBuf::from("/out"), Path::new("/in/archive.tar.gz")), PathBuf::from("/out/archive.tar.webp"));
	}

	#[test]
	fn converts_a_png_and_reports_both_sizes() {
		let scratch = Scratch::new("convert");
		let input = scratch.join("source.png");
		let output = scratch.join("source.webp");
		write_png(&input, 64, 48);

		let conversion = encode_file(&EncodeSettings::default(), &input, &output).expect("conversion succeeds");
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
		let settings = EncodeSettings { resize: Resize { width: 32, height: 0 }, ..Default::default() };
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
		encode_file(&EncodeSettings::default(), &png, &webp).expect("first conversion succeeds");
		let original = fs::read(&webp).expect("read the converted file");

		// Now convert that WebP into the same directory: output_path_in derives photo.webp,
		// which is the input.
		let derived = output_path_in(scratch.0.clone(), &webp);
		assert_eq!(derived, webp, "this is the dangerous case the guard exists for");
		let error = encode_file(&EncodeSettings::default(), &webp, &derived).expect_err("must refuse");
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
		let settings = EncodeSettings { method: 9, ..Default::default() };
		assert!(encode_file(&settings, &input, &output).is_err());
		assert_eq!(fs::read(&output).expect("read the output"), b"PRECIOUS EXISTING CONTENT", "a failed encode must not truncate the destination");
	}

	#[test]
	fn leaves_no_staging_files_behind() {
		let scratch = Scratch::new("staging");
		let input = scratch.join("source.png");
		write_png(&input, 24, 24);
		encode_file(&EncodeSettings::default(), &input, &scratch.join("out.webp")).expect("conversion succeeds");
		let leftovers: Vec<_> = fs::read_dir(&scratch.0).expect("list the directory").filter_map(Result::ok).filter(|entry| entry.file_name().to_string_lossy().contains(".part")).collect();
		assert!(leftovers.is_empty(), "staging files left behind: {leftovers:?}");
	}

	#[test]
	fn a_missing_output_directory_is_refused_rather_than_created() {
		let scratch = Scratch::new("missingdir");
		let input = scratch.join("source.png");
		write_png(&input, 16, 16);
		let missing = scratch.join("nope");
		let error = encode_file(&EncodeSettings::default(), &input, &missing.join("out.webp")).expect_err("must refuse");
		assert!(matches!(error, ConvertError::MissingOutputDirectory { .. }), "got {error}");
		assert!(!missing.exists(), "the directory must not have been created");
	}

	#[test]
	fn a_non_image_is_rejected_with_a_useful_error() {
		let scratch = Scratch::new("notimage");
		let input = scratch.join("notes.txt");
		fs::write(&input, b"this is not an image").expect("write the file");
		let error = load(&input).expect_err("must refuse");
		assert!(error.to_string().contains("not a PNG, JPEG, TIFF or WebP"), "got {error}");
	}

	/// WebP input goes through libwebp itself, so a WebP -> WebP conversion does not
	/// round-trip through a second, non-reference decoder.
	#[test]
	fn webp_input_is_decoded_by_libwebp() {
		let scratch = Scratch::new("webpin");
		let png = scratch.join("source.png");
		write_png(&png, 40, 30);
		let webp = scratch.join("intermediate.webp");
		encode_file(&EncodeSettings { mode: Mode::Lossless, ..Default::default() }, &png, &webp).expect("encode to webp");

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

		let error = super::encode_file_with_progress(&EncodeSettings::default(), &input, &output, &mut |_| false).expect_err("must cancel");
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
		super::encode_file_with_progress(&EncodeSettings::default(), &input, &scratch.join("out.webp"), &mut |percent| {
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
		super::encode_file(&EncodeSettings { mode: Mode::Lossless, ..Default::default() }, &png, &webp).expect("encode");

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
		assert_eq!(super::mirrored_output_path(out, Path::new("a.png")), Some(PathBuf::from("/out/a.webp")), "the output is a WebP, not a copy of the input name");
		assert_eq!(super::mirrored_output_path(out, Path::new("nested/deeper/d.JPG")), Some(PathBuf::from("/out/nested/deeper/d.webp")));
	}

	/// The guard that matters: a relative path must never be able to write outside the
	/// directory the user chose.
	#[test]
	fn a_relative_path_cannot_escape_the_output_directory() {
		let out = Path::new("/out");
		assert_eq!(super::mirrored_output_path(out, Path::new("../escaped.png")), None);
		assert_eq!(super::mirrored_output_path(out, Path::new("nested/../../escaped.png")), None);
		assert_eq!(super::mirrored_output_path(out, Path::new("/absolute/escaped.png")), None);
	}

	/// An animated WebP must be refused with a message that explains itself.
	///
	/// libwebp's still decoder already refuses one, so the behaviour was never wrong — but
	/// it reported "libwebp rejected the file", which tells the user nothing.
	#[test]
	fn an_animated_webp_is_refused_by_name() {
		let Some(animated) = animated_fixture() else {
			// Building one needs a multi-frame encoder this crate does not link, so the
			// fixture is supplied by the environment when available.
			eprintln!("SKIPPED: set SKIDBLADNIR_ANIMATED_WEBP to an animated WebP to run this");
			return;
		};
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

	/// An animated WebP fixture from the environment, if one was provided.
	fn animated_fixture() -> Option<Vec<u8>> {
		fs::read(std::env::var_os("SKIDBLADNIR_ANIMATED_WEBP")?).ok()
	}

	#[test]
	fn saving_percent_handles_growth_and_empty_sources() {
		assert_eq!(Conversion { source_bytes: 1000, output_bytes: 250, width: 1, height: 1 }.saving_percent(), Some(75.0));
		assert_eq!(Conversion { source_bytes: 100, output_bytes: 150, width: 1, height: 1 }.saving_percent(), Some(-50.0));
		assert_eq!(Conversion { source_bytes: 0, output_bytes: 10, width: 1, height: 1 }.saving_percent(), None);
	}
}
