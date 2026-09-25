//! Encoding a preview without touching the disk.
//!
//! "Will this setting ruin the image?" is the question the encoder controls exist to
//! answer, and the Electron app could only answer it by writing a file and opening it.
//! This encodes into memory and hands the frontend two images to compare.
//!
//! Both sides come back as `data:` URLs rather than file paths, deliberately: it means the
//! preview needs **no filesystem permission in the webview at all**, and it cannot
//! accidentally show a stale file from a previous run.

use std::path::Path;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;
use skidbladnir_encode::{
	encoder::encode_rgba, settings::{EncodeSettings, Mode}, source
};

/// Above this many pixels a preview is refused rather than attempted.
///
/// Both sides of the comparison are base64 `data:` URLs held in the webview, so the cost
/// is real memory, not just time. 24 megapixels is larger than any camera output the app
/// is likely to see and still bounded.
const MAX_PREVIEW_PIXELS: u64 = 24_000_000;

/// The two images to show side by side, plus the sizes that make the trade concrete.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
	/// The source pixels, losslessly encoded so what is shown is exactly the input.
	pub original: String,
	/// The result of encoding with the current settings.
	pub encoded: String,
	/// Size of the source file on disk.
	pub source_bytes: u64,
	/// Size the encoded image would be on disk.
	pub encoded_bytes: u64,
	/// Width after any resize.
	pub width: u32,
	/// Height after any resize.
	pub height: u32,
}

/// Encode `input` with `settings` and return it beside a faithful copy of the original.
///
/// The "original" side is the source pixels encoded **losslessly**, not the source file
/// re-served: that way both sides are WebP the webview can display, and the left-hand
/// image is still pixel-exact rather than a second lossy rendering of the input.
///
/// Nothing is written to disk.
///
/// # Errors
///
/// Returns the failure as a string for display.
pub fn preview(settings: &EncodeSettings, input: &Path) -> Result<Preview, String> {
	let source_bytes = std::fs::metadata(input).map(|meta| meta.len()).map_err(|error| format!("could not read `{}`: {error}", input.display()))?;
	let image = source::load(input).map_err(|error| error.to_string())?;

	let pixels = u64::from(image.width) * u64::from(image.height);
	if pixels > MAX_PREVIEW_PIXELS {
		return Err(format!("{}x{} is too large to preview ({} megapixels; the limit is {}). Convert it and compare the files instead.", image.width, image.height, pixels / 1_000_000, MAX_PREVIEW_PIXELS / 1_000_000));
	}

	let encoded = encode_rgba(settings, &image.as_rgba()).map_err(|error| error.to_string())?;

	// The original is shown at the same dimensions as the encoded side when a resize is in
	// play, so the comparison is like for like rather than a big image next to a small one.
	let original_settings = EncodeSettings { mode: Mode::Lossless, quality: 100, resize: settings.resize, ..Default::default() };
	let original = encode_rgba(&original_settings, &image.as_rgba()).map_err(|error| error.to_string())?;

	let (width, height) = dimensions(&encoded).unwrap_or((image.width, image.height));

	Ok(Preview { original: data_url(&original), encoded: data_url(&encoded), source_bytes, encoded_bytes: encoded.len() as u64, width, height })
}

/// Wrap WebP bytes as a `data:` URL the webview can put in an `<img src>`.
fn data_url(webp: &[u8]) -> String {
	format!("data:image/webp;base64,{}", STANDARD.encode(webp))
}

/// Read the dimensions back out of an encoded WebP.
fn dimensions(webp: &[u8]) -> Option<(u32, u32)> {
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
	use std::{fs, path::PathBuf};

	use base64::Engine as _;
	use skidbladnir_encode::settings::{EncodeSettings, Resize};

	use super::preview;

	struct Scratch(PathBuf);

	impl Scratch {
		fn new(name: &str) -> Self {
			let path = std::env::temp_dir().join(format!("skidbladnir-preview-{}-{name}", std::process::id()));
			let _ = fs::remove_dir_all(&path);
			fs::create_dir_all(&path).expect("create the scratch directory");
			Self(path)
		}
	}

	impl Drop for Scratch {
		fn drop(&mut self) {
			let _ = fs::remove_dir_all(&self.0);
		}
	}

	fn write_png(path: &std::path::Path, width: u32, height: u32) {
		let mut pixels = Vec::with_capacity((width * height * 4) as usize);
		for y in 0..height {
			for x in 0..width {
				pixels.extend_from_slice(&[u8::try_from((x * 7) % 256).unwrap_or(0), u8::try_from((y * 5) % 256).unwrap_or(0), 90, 255]);
			}
		}
		image::RgbaImage::from_raw(width, height, pixels).expect("buffer matches").save_with_format(path, image::ImageFormat::Png).expect("write the fixture");
	}

	#[test]
	fn returns_two_displayable_images_and_writes_nothing() {
		let scratch = Scratch::new("basic");
		let input = scratch.join_png();
		let before: Vec<_> = fs::read_dir(&scratch.0).expect("list").filter_map(Result::ok).map(|e| e.file_name()).collect();

		let result = preview(&EncodeSettings::default(), &input).expect("preview");
		assert!(result.original.starts_with("data:image/webp;base64,"), "{}", &result.original[..40]);
		assert!(result.encoded.starts_with("data:image/webp;base64,"));
		assert!(result.encoded_bytes > 0);
		assert_eq!(result.source_bytes, fs::metadata(&input).expect("stat").len());
		assert_eq!((result.width, result.height), (64, 48));

		let after: Vec<_> = fs::read_dir(&scratch.0).expect("list").filter_map(Result::ok).map(|e| e.file_name()).collect();
		assert_eq!(before, after, "a preview must not write anything to disk");
	}

	/// A resize must apply to both sides, or the comparison is a big image next to a small
	/// one and tells the user nothing about quality.
	#[test]
	fn a_resize_applies_to_both_sides() {
		let scratch = Scratch::new("resize");
		let input = scratch.join_png();
		let result = preview(&EncodeSettings { resize: Resize { width: 32, height: 0 }, ..Default::default() }, &input).expect("preview");
		assert_eq!((result.width, result.height), (32, 24));
		// Both sides decode to the resized dimensions.
		for (label, url) in [("original", &result.original), ("encoded", &result.encoded)] {
			let bytes = super::STANDARD.decode(url.trim_start_matches("data:image/webp;base64,")).expect("valid base64");
			assert_eq!(super::dimensions(&bytes), Some((32, 24)), "{label} side was not resized");
		}
	}

	/// Lower quality must produce a visibly smaller encoded side, or the preview is not
	/// showing the user anything about their setting.
	#[test]
	fn quality_changes_the_encoded_side() {
		let scratch = Scratch::new("quality");
		let input = scratch.join_png();
		let low = preview(&EncodeSettings { quality: 5, ..Default::default() }, &input).expect("preview");
		let high = preview(&EncodeSettings { quality: 95, ..Default::default() }, &input).expect("preview");
		assert!(low.encoded_bytes < high.encoded_bytes, "q5 {} vs q95 {}", low.encoded_bytes, high.encoded_bytes);
		// The original side is the same both times: it does not depend on the settings.
		assert_eq!(low.original, high.original);
	}

	#[test]
	fn a_non_image_is_refused() {
		let scratch = Scratch::new("notimage");
		let input = scratch.0.join("notes.txt");
		fs::write(&input, b"not an image").expect("write");
		assert!(preview(&EncodeSettings::default(), &input).is_err());
	}

	impl Scratch {
		fn join(&self, name: &str) -> PathBuf {
			self.0.join(name)
		}

		fn join_png(&self) -> PathBuf {
			let path = self.join("source.png");
			write_png(&path, 64, 48);
			path
		}
	}
}
