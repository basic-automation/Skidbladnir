//! The HEIC controls.

use serde::{Deserialize, Serialize};

use super::ValidationError;

/// Every HEIC control. Kvazaar takes 4:2:0 input only, so there is no true lossless mode
/// to offer; quality is the whole surface libheif exposes for it. 50 is libheif's own
/// default, the one `heif-enc` uses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HeicSettings {
	/// Quality, `0..=100`.
	pub quality: u8,
}

impl Default for HeicSettings {
	fn default() -> Self {
		Self { quality: 50 }
	}
}

impl HeicSettings {
	/// Check the quality is in libheif's range.
	///
	/// # Errors
	///
	/// Returns [`ValidationError::OutOfRange`] for a quality above 100.
	pub fn validate(&self) -> Result<(), ValidationError> {
		if self.quality > 100 {
			return Err(ValidationError::OutOfRange { field: "heic.quality", value: u32::from(self.quality), min: 0, max: 100 });
		}
		Ok(())
	}
}
