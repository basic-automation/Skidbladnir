//! AVIF output, through `ravif`.
//!
//! Unlike WebP there is no reference CLI whose bytes this has to reproduce, so the gate is
//! different: `tests/avif_reference.rs` decodes what this writes with libavif's own
//! `avifdec` and checks the pixels that come back. See ROADMAP.md Phase 7 for why `ravif`
//! rather than `libavif-sys`.
//!
//! Two things differ from the WebP path and are worth knowing:
//!
//! - **The resize is libwebp's.** [`crate::encoder::rescale_rgba`] runs the same rescaler
//!   the WebP path uses, so a resize gives the same dimensions whichever format is chosen.
//! - **An AVIF encode cannot be interrupted.** `ravif` has no progress hook, so the
//!   progress callback is asked once before the encode starts and once when it finishes.
//!   Cancelling is honoured at either point, and a cancelled encode returns nothing — but
//!   one already running goes on to the end.

use ravif::{AlphaColorMode, BitDepth, ColorModel, Encoder, Img, RGBA8};

use crate::{
	encoder::{EncodeError, RgbaImage, rescale_rgba}, settings::{AvifAlphaMode, AvifBitDepth, AvifColorModel, AvifSettings, Resize}
};

/// Encode RGBA pixels to an AVIF file.
///
/// # Errors
///
/// Returns [`EncodeError`] if the settings are out of range, the buffer does not match its
/// dimensions, the resize fails, `ravif` fails, or `on_progress` asked to stop.
pub fn encode(settings: &AvifSettings, resize: Resize, image: &RgbaImage<'_>, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Vec<u8>, EncodeError> {
	// `ravif` panics on an out-of-range quality or speed; validating first turns that into
	// an error the user can read.
	settings.validate()?;
	let (width, height, pixels) = rescale_rgba(image, resize)?;

	if !on_progress(0) {
		return Err(EncodeError::Cancelled);
	}

	let pixels: Vec<RGBA8> = pixels.as_chunks::<4>().0.iter().map(|&[r, g, b, a]| RGBA8::new(r, g, b, a)).collect();
	let config = Encoder::new()
		.with_quality(f32::from(settings.quality))
		.with_alpha_quality(f32::from(settings.alpha_quality))
		.with_speed(settings.speed)
		.with_bit_depth(match settings.bit_depth {
			AvifBitDepth::Eight => BitDepth::Eight,
			AvifBitDepth::Ten => BitDepth::Ten,
		})
		.with_internal_color_model(match settings.color_model {
			AvifColorModel::YCbCr => ColorModel::YCbCr,
			AvifColorModel::Rgb => ColorModel::RGB,
		})
		.with_alpha_color_mode(match settings.alpha_mode {
			AvifAlphaMode::Clean => AlphaColorMode::UnassociatedClean,
			AvifAlphaMode::Dirty => AlphaColorMode::UnassociatedDirty,
			AvifAlphaMode::Premultiplied => AlphaColorMode::Premultiplied,
		})
		.with_num_threads(if settings.multi_threading { None } else { Some(1) });
	let encoded = config.encode_rgba(Img::new(pixels.as_slice(), width as usize, height as usize)).map_err(|error| EncodeError::Avif(error.to_string()))?;

	// A cancel that arrived during the encode is still honoured: nothing is returned, so
	// nothing is written.
	if !on_progress(100) {
		return Err(EncodeError::Cancelled);
	}
	Ok(encoded.avif_file)
}

/// Read an AVIF file's dimensions from its `ispe` (image spatial extents) property.
///
/// The encoder resolves a resize with one dimension `0`, so the only reliable way to
/// report the dimensions actually written is to read them back out of the file, as the
/// WebP path does. This is a scan for the property box rather than a full HEIF parser:
/// it is only ever pointed at files this module has just written.
#[must_use]
pub fn dimensions(avif: &[u8]) -> Option<(u32, u32)> {
	let at = avif.windows(4).position(|window| window == b"ispe")?;
	// `ispe` is a full box: 4 bytes of version and flags, then width and height as
	// big-endian u32s.
	let body = avif.get(at + 8..at + 16)?;
	let width = u32::from_be_bytes(body[0..4].try_into().ok()?);
	let height = u32::from_be_bytes(body[4..8].try_into().ok()?);
	Some((width, height))
}

#[cfg(test)]
mod tests {
	use super::{dimensions, encode};
	use crate::{
		encoder::{EncodeError, RgbaImage}, settings::{AvifBitDepth, AvifSettings, Resize}
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

	/// Fast settings for tests: speed 10 is the quickest rav1e preset.
	fn quick() -> AvifSettings {
		AvifSettings { speed: 10, ..Default::default() }
	}

	#[test]
	fn writes_an_avif_file_with_the_right_dimensions() {
		let pixels = fixture(40, 24);
		let bytes = encode(&quick(), Resize::default(), &RgbaImage { width: 40, height: 24, pixels: &pixels }, &mut |_| true).expect("encode");
		// ISO-BMFF: a size, then `ftyp`, then the `avif` brand.
		assert_eq!(&bytes[4..12], b"ftypavif");
		assert_eq!(dimensions(&bytes), Some((40, 24)));
	}

	/// The resize is libwebp's, so a `0` is derived from the aspect ratio exactly as it is
	/// for WebP.
	#[test]
	fn applies_the_shared_resize() {
		let pixels = fixture(40, 24);
		for (resize, expected) in [(Resize { width: 20, height: 12 }, (20, 12)), (Resize { width: 20, height: 0 }, (20, 12)), (Resize { width: 0, height: 12 }, (20, 12))] {
			let bytes = encode(&quick(), resize, &RgbaImage { width: 40, height: 24, pixels: &pixels }, &mut |_| true).expect("encode");
			assert_eq!(dimensions(&bytes), Some(expected), "{resize:?}");
		}
	}

	#[test]
	fn lower_quality_is_smaller() {
		let pixels = fixture(64, 48);
		let image = RgbaImage { width: 64, height: 48, pixels: &pixels };
		let low = encode(&AvifSettings { quality: 20, ..quick() }, Resize::default(), &image, &mut |_| true).expect("encode");
		let high = encode(&AvifSettings { quality: 95, ..quick() }, Resize::default(), &image, &mut |_| true).expect("encode");
		assert!(low.len() < high.len(), "q20 {} bytes vs q95 {} bytes", low.len(), high.len());
	}

	#[test]
	fn eight_bit_depth_encodes() {
		let pixels = fixture(16, 16);
		let bytes = encode(&AvifSettings { bit_depth: AvifBitDepth::Eight, ..quick() }, Resize::default(), &RgbaImage { width: 16, height: 16, pixels: &pixels }, &mut |_| true).expect("encode");
		assert_eq!(dimensions(&bytes), Some((16, 16)));
	}

	/// `ravif` panics on these; they must come back as an error instead.
	#[test]
	fn out_of_range_settings_are_an_error_not_a_panic() {
		let pixels = fixture(8, 8);
		let error = encode(&AvifSettings { speed: 0, ..Default::default() }, Resize::default(), &RgbaImage { width: 8, height: 8, pixels: &pixels }, &mut |_| true).expect_err("speed 0 is out of range");
		assert!(matches!(error, EncodeError::Settings(_)), "{error:?}");
	}

	#[test]
	fn a_cancel_before_or_after_the_encode_returns_nothing() {
		let pixels = fixture(8, 8);
		let image = RgbaImage { width: 8, height: 8, pixels: &pixels };
		assert_eq!(encode(&quick(), Resize::default(), &image, &mut |_| false), Err(EncodeError::Cancelled));
		assert_eq!(encode(&quick(), Resize::default(), &image, &mut |percent| percent < 100), Err(EncodeError::Cancelled));
	}

	#[test]
	fn a_malformed_buffer_is_refused() {
		let error = encode(&quick(), Resize::default(), &RgbaImage { width: 4, height: 4, pixels: &[0; 10] }, &mut |_| true).expect_err("short buffer");
		assert!(matches!(error, EncodeError::MalformedImage { .. }), "{error:?}");
	}

	#[test]
	fn dimensions_of_garbage_is_none() {
		assert_eq!(dimensions(b"not an avif"), None);
		assert_eq!(dimensions(b"....ispe\0\0\0\0\0\0"), None, "a truncated ispe must not be read past its end");
	}
}
