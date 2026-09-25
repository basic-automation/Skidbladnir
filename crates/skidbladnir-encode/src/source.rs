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
	encoder::{EncodeError, RgbaImage, encode_rgba}, settings::EncodeSettings
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
		SourceFormat::Webp => decode_webp(&bytes).ok_or_else(|| SourceError::Decode { path: path.to_path_buf(), format: format.name(), detail: "libwebp rejected the file".to_owned() })?,
		SourceFormat::Png | SourceFormat::Jpeg | SourceFormat::Tiff => {
			let decoded = image::load_from_memory(&bytes).map_err(|error| SourceError::Decode { path: path.to_path_buf(), format: format.name(), detail: error.to_string() })?;
			let rgba = decoded.into_rgba8();
			(rgba.width(), rgba.height(), rgba.into_raw())
		}
	};

	Ok(SourceImage { width, height, pixels, format })
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
	let encoded = encode_rgba(settings, &image.as_rgba())?;

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

	#[test]
	fn saving_percent_handles_growth_and_empty_sources() {
		assert_eq!(Conversion { source_bytes: 1000, output_bytes: 250, width: 1, height: 1 }.saving_percent(), Some(75.0));
		assert_eq!(Conversion { source_bytes: 100, output_bytes: 150, width: 1, height: 1 }.saving_percent(), Some(-50.0));
		assert_eq!(Conversion { source_bytes: 0, output_bytes: 10, width: 1, height: 1 }.saving_percent(), None);
	}
}
