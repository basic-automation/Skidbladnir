//! The JPEG XL controls.

use serde::{Deserialize, Serialize};

use super::ValidationError;

/// Every JPEG XL control. Defaults are `cjxl`'s own: quality 90 is distance 1.0, which
/// libjxl calls visually lossless, at effort 7, with JPEG input recompressed losslessly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JxlSettings {
	/// Quality, `0..=100`, mapped to libjxl's distance by its own
	/// `JxlEncoderDistanceFromQuality`. Ignored when [`JxlSettings::lossless`] is set.
	pub quality: u8,
	/// Encoder effort, `1..=10`: 1 is fastest, 10 slowest and smallest.
	pub effort: u8,
	/// Encode the pixels exactly, as modular lossless.
	pub lossless: bool,
	/// Recompress a JPEG input losslessly instead of re-encoding its pixels: about a fifth
	/// smaller, and the original JPEG can be rebuilt from the JPEG XL bit for bit. Applies
	/// only to JPEG input with no resize in effect; anything else is encoded from pixels.
	pub lossless_jpeg: bool,
	/// Encode on every core, or on one.
	pub multi_threading: bool,
}

impl Default for JxlSettings {
	fn default() -> Self {
		Self { quality: 90, effort: 7, lossless: false, lossless_jpeg: true, multi_threading: true }
	}
}

impl JxlSettings {
	/// Check every control against the range libjxl accepts.
	///
	/// # Errors
	///
	/// Returns the first [`ValidationError`] found, checking in field order.
	pub fn validate(&self) -> Result<(), ValidationError> {
		for (field, value, min, max) in [("jxl.quality", self.quality, 0, 100), ("jxl.effort", self.effort, 1, 10)] {
			if value < min || value > max {
				return Err(ValidationError::OutOfRange { field, value: u32::from(value), min: u32::from(min), max: u32::from(max) });
			}
		}
		Ok(())
	}
}
