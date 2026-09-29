//! The AVIF controls.

use serde::{Deserialize, Serialize};

use super::ValidationError;

/// The precision of the encoded AV1 data, for colour and alpha alike.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AvifBitDepth {
	/// 8 bits per channel.
	Eight,
	/// 10 bits per channel. `ravif`'s default, and better even for 8-bit sources, because
	/// the colour conversion keeps more precision.
	#[default]
	Ten,
}

/// How colour is stored inside the AV1 stream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AvifColorModel {
	/// YCbCr. The default, and what almost every AVIF uses.
	#[default]
	#[serde(rename = "ycbcr")]
	YCbCr,
	/// RGB (as GBR), which avoids the colour conversion at a large size cost.
	#[serde(rename = "rgb")]
	Rgb,
}

/// What happens to the colour of transparent pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AvifAlphaMode {
	/// Replace the colour of fully transparent pixels with something cheap to encode.
	/// The default: invisible pixels cost nothing to change.
	#[default]
	Clean,
	/// Keep the colour of transparent pixels exactly as given — the AVIF counterpart of
	/// WebP lossless's `-exact`.
	Dirty,
	/// Store premultiplied alpha.
	Premultiplied,
}

/// Every AVIF control, as `ravif` exposes them. Defaults are `ravif`'s own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AvifSettings {
	/// Colour quality, `1..=100`.
	pub quality: u8,
	/// Alpha quality, `1..=100`.
	pub alpha_quality: u8,
	/// Encoder speed, `1..=10`: 1 is slowest and smallest, 10 fastest and largest.
	pub speed: u8,
	/// Precision of the encoded data.
	pub bit_depth: AvifBitDepth,
	/// How colour is stored.
	pub color_model: AvifColorModel,
	/// What happens to the colour of transparent pixels.
	pub alpha_mode: AvifAlphaMode,
	/// Encode on every core, or on one.
	pub multi_threading: bool,
}

impl Default for AvifSettings {
	fn default() -> Self {
		Self { quality: 80, alpha_quality: 80, speed: 5, bit_depth: AvifBitDepth::Ten, color_model: AvifColorModel::YCbCr, alpha_mode: AvifAlphaMode::Clean, multi_threading: true }
	}
}

impl AvifSettings {
	/// Check every control against the range `ravif` accepts. `ravif` panics outside
	/// them, so this is what stands between a bad settings file and a crashed encode.
	///
	/// # Errors
	///
	/// Returns the first [`ValidationError`] found, checking in field order.
	pub fn validate(&self) -> Result<(), ValidationError> {
		for (field, value, min, max) in [("avif.quality", self.quality, 1, 100), ("avif.alpha_quality", self.alpha_quality, 1, 100), ("avif.speed", self.speed, 1, 10)] {
			if value < min || value > max {
				return Err(ValidationError::OutOfRange { field, value: u32::from(value), min: u32::from(min), max: u32::from(max) });
			}
		}
		Ok(())
	}
}
