//! Reading what is already inside a WebP file.
//!
//! The app could not do this at all before: it converted images and never looked at one.
//! Being able to say "this is a 1920x1080 lossy WebP with alpha" matters for the two things
//! a user actually does with the output — checking the conversion did what they asked, and
//! deciding whether a file someone sent them is worth re-encoding.
//!
//! Everything here comes from libwebp's own `WebPGetFeatures`, which reads the bitstream
//! header only: it does not decode the image, so inspecting a large file is cheap.

use libwebp_sys::{VP8StatusCode, WEBP_DECODER_ABI_VERSION, WebPBitstreamFeatures, WebPGetFeaturesInternal};
use serde::{Deserialize, Serialize};

/// Which compression a WebP uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebpCompression {
	/// VP8: lossy.
	Lossy,
	/// VP8L: lossless.
	Lossless,
	/// Mixed or undefined, which libwebp reports for animations whose frames differ.
	Mixed,
}

/// What a WebP file says about itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebpInfo {
	/// Width in pixels.
	pub width: u32,
	/// Height in pixels.
	pub height: u32,
	/// Whether an alpha channel is present.
	pub has_alpha: bool,
	/// Whether this is an animation rather than a still image.
	///
	/// Worth surfacing because Skidbladnir **cannot** re-encode one: the encoder handles
	/// still images, so converting an animated WebP would silently drop every frame but the
	/// first.
	pub has_animation: bool,
	/// Lossy, lossless, or mixed.
	pub compression: WebpCompression,
}

/// Read a WebP's header. Returns `None` if the bytes are not a WebP libwebp can parse.
///
/// Reads the header only — no pixels are decoded, so this is cheap even for a large file.
#[must_use]
pub fn inspect_webp(bytes: &[u8]) -> Option<WebpInfo> {
	// SAFETY: `bytes` is a valid slice and `features` is live for the call. This is
	// WebPGetFeatures, whose inline form is not exported.
	let mut features = unsafe { std::mem::zeroed::<WebPBitstreamFeatures>() };
	let status = unsafe { WebPGetFeaturesInternal(bytes.as_ptr(), bytes.len(), &raw mut features, WEBP_DECODER_ABI_VERSION.cast_signed()) };
	if status != VP8StatusCode::VP8_STATUS_OK {
		return None;
	}

	Some(WebpInfo {
		width: u32::try_from(features.width).ok()?,
		height: u32::try_from(features.height).ok()?,
		has_alpha: features.has_alpha != 0,
		has_animation: features.has_animation != 0,
		compression: match features.format {
			1 => WebpCompression::Lossy,
			2 => WebpCompression::Lossless,
			_ => WebpCompression::Mixed,
		},
	})
}

#[cfg(test)]
mod tests {
	use super::{WebpCompression, inspect_webp};
	use crate::{
		encoder::{RgbaImage, encode_rgba}, settings::{EncodeSettings, Mode}
	};

	fn fixture(width: u32, height: u32, opaque: bool) -> Vec<u8> {
		let mut pixels = Vec::with_capacity((width * height * 4) as usize);
		for y in 0..height {
			for x in 0..width {
				pixels.extend_from_slice(&[u8::try_from((x * 3) % 256).unwrap_or(0), u8::try_from((y * 7) % 256).unwrap_or(0), 40, if opaque { 255 } else { u8::try_from((x * 255) / width.max(1)).unwrap_or(255) }]);
			}
		}
		pixels
	}

	#[test]
	fn reads_dimensions_and_compression_from_our_own_output() {
		let pixels = fixture(70, 50, true);
		let image = RgbaImage { width: 70, height: 50, pixels: &pixels };

		let lossy = encode_rgba(&EncodeSettings::default(), &image).expect("encode");
		let info = inspect_webp(&lossy).expect("inspect");
		assert_eq!((info.width, info.height), (70, 50));
		assert_eq!(info.compression, WebpCompression::Lossy);
		assert!(!info.has_animation);

		let lossless = encode_rgba(&EncodeSettings { mode: Mode::Lossless, ..Default::default() }, &image).expect("encode");
		let info = inspect_webp(&lossless).expect("inspect");
		assert_eq!(info.compression, WebpCompression::Lossless);
	}

	/// Alpha detection has to come from the bitstream, not from whether we passed RGBA in:
	/// a fully opaque image encoded from RGBA has no alpha channel in the output.
	#[test]
	fn alpha_is_read_from_the_bitstream() {
		let transparent = fixture(40, 40, false);
		let info = inspect_webp(&encode_rgba(&EncodeSettings::default(), &RgbaImage { width: 40, height: 40, pixels: &transparent }).expect("encode")).expect("inspect");
		assert!(info.has_alpha, "an image with a real alpha ramp must report alpha");

		let opaque = fixture(40, 40, true);
		let info = inspect_webp(&encode_rgba(&EncodeSettings::default(), &RgbaImage { width: 40, height: 40, pixels: &opaque }).expect("encode")).expect("inspect");
		assert!(!info.has_alpha, "a fully opaque image should not carry an alpha channel");
	}

	#[test]
	fn non_webp_input_is_rejected() {
		assert_eq!(inspect_webp(b"not a webp at all"), None);
		assert_eq!(inspect_webp(b""), None);
		// A RIFF container that is not WebP.
		assert_eq!(inspect_webp(b"RIFF\x00\x00\x00\x00WAVEfmt "), None);
		// A truncated WebP header.
		assert_eq!(inspect_webp(b"RIFF\x20\x00\x00\x00WEBP"), None);
	}
}
