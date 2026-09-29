//! The HEIC controls: every `heif-enc -e kvazaar` option that changes the file it writes
//! for a still image.
//!
//! Until 0.12 HEIC had one control, quality. The encoder was always libheif with Kvazaar;
//! these settings are now `heif-enc`'s options for it, applied in `heif-enc`'s order (see
//! `native/heic_shim.c`). A settings file from before loads unchanged, since `quality` kept
//! its name and default.
//!
//! What `heif-enc` offers that does not reach Kvazaar is not here:
//!
//! - `-L`. It asks the encoder for a `chroma` parameter Kvazaar does not have, so
//!   `heif-enc -L -e kvazaar` always exits with an error. Kvazaar's own lossless mode is
//!   reached with `-p lossless=true`, which is [`HeicSettings::lossless`].
//! - `-b`. It only applies to 16-bit PNG and TIFF, which `heif-enc` hands Kvazaar at 10 or
//!   more bits, and Kvazaar refuses them.
//! - `--enable-metadata-compression`. It needs libheif built with zlib, which libheif's
//!   default build (and so this one) is not; every value `heif-enc` then accepts writes the
//!   XMP uncompressed, as here.
//! - Sequences, several inputs, tiles from several files, `-A`, `--vvc` and the other
//!   codecs, and the experimental options. See ROADMAP.md.

use serde::{Deserialize, Serialize};

use super::ValidationError;

/// How libheif reduces chroma to 4:2:0 (`-C`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChromaDownsampling {
	/// `nearest-neighbor`.
	NearestNeighbor,
	/// `average`.
	Average,
	/// `sharp-yuv`: libwebp's sharp RGB to YUV conversion.
	SharpYuv,
}

impl ChromaDownsampling {
	/// The `heif_chroma_downsampling_algorithm`.
	#[must_use]
	pub const fn as_libheif(self) -> i32 {
		match self {
			Self::NearestNeighbor => 1,
			Self::Average => 2,
			Self::SharpYuv => 3,
		}
	}

	/// The value `heif-enc -C` takes.
	#[must_use]
	pub const fn as_heif_enc_str(self) -> &'static str {
		match self {
			Self::NearestNeighbor => "nearest-neighbor",
			Self::Average => "average",
			Self::SharpYuv => "sharp-yuv",
		}
	}
}

/// The colour description written to the file and into the HEVC stream
/// (`--color-profile`, and for `custom` the four code-point options).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "preset")]
pub enum ColorProfile {
	/// `custom`, `heif-enc`'s default: ITU-T H.273 code points as given.
	#[serde(rename_all = "camelCase")]
	Custom {
		/// `--matrix_coefficients`: 0, 1, 2 or 4 to 14. `heif-enc`'s default is 6 (BT.601).
		matrix_coefficients: u16,
		/// `--colour_primaries`: 1, 2, 4 to 12 or 22. The default is 1 (BT.709).
		colour_primaries: u16,
		/// `--transfer_characteristic`: 1, 2 or 4 to 18. The default is 13 (sRGB).
		transfer_characteristics: u16,
		/// `--full_range_flag`. The default is full range.
		full_range: bool,
	},
	/// `auto`: sRGB (matrix BT.709) for an RGB source, BT.709 for a YCbCr or gray one.
	Auto,
	/// `601`.
	Bt601,
	/// `709`.
	Bt709,
	/// `compatible`, the same code points as `709`.
	Compatible,
	/// `2020`.
	Bt2020,
}

impl Default for ColorProfile {
	fn default() -> Self {
		Self::Custom { matrix_coefficients: 6, colour_primaries: 1, transfer_characteristics: 13, full_range: true }
	}
}

impl ColorProfile {
	/// The value `heif-enc --color-profile` takes.
	#[must_use]
	pub const fn as_heif_enc_str(self) -> &'static str {
		match self {
			Self::Custom { .. } => "custom",
			Self::Auto => "auto",
			Self::Bt601 => "601",
			Self::Bt709 => "709",
			Self::Compatible => "compatible",
			Self::Bt2020 => "2020",
		}
	}
}

/// A rotation and mirroring, signalled in the file (`irot`/`imir`), not applied to the
/// pixels. The values are libheif's (and Exif's) orientation numbers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
	/// As stored.
	#[default]
	Normal,
	/// `--flip-h`.
	FlipHorizontally,
	/// `--rotate-cw 180`.
	Rotate180,
	/// `--flip-v`.
	FlipVertically,
	/// `--rotate-cw 90 --flip-h`.
	Rotate90CwThenFlipHorizontally,
	/// `--rotate-cw 90`.
	Rotate90Cw,
	/// `--rotate-cw 90 --flip-v`.
	Rotate90CwThenFlipVertically,
	/// `--rotate-cw 270`.
	Rotate270Cw,
}

impl Orientation {
	/// The `heif_orientation`, 1 to 8.
	#[must_use]
	pub const fn as_libheif(self) -> i32 {
		match self {
			Self::Normal => 1,
			Self::FlipHorizontally => 2,
			Self::Rotate180 => 3,
			Self::FlipVertically => 4,
			Self::Rotate90CwThenFlipHorizontally => 5,
			Self::Rotate90Cw => 6,
			Self::Rotate90CwThenFlipVertically => 7,
			Self::Rotate270Cw => 8,
		}
	}

	/// The `heif-enc` options that compose to this orientation, in order.
	#[must_use]
	pub const fn as_heif_enc_args(self) -> &'static [&'static str] {
		match self {
			Self::Normal => &[],
			Self::FlipHorizontally => &["--flip-h"],
			Self::Rotate180 => &["--rotate-cw", "180"],
			Self::FlipVertically => &["--flip-v"],
			Self::Rotate90CwThenFlipHorizontally => &["--rotate-cw", "90", "--flip-h"],
			Self::Rotate90Cw => &["--rotate-cw", "90"],
			Self::Rotate90CwThenFlipVertically => &["--rotate-cw", "90", "--flip-v"],
			Self::Rotate270Cw => &["--rotate-cw", "270"],
		}
	}
}

/// An omnidirectional (360°) projection, signalled with a `prfr` property
/// (`--omaf-image-projection`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OmafProjection {
	/// `equirectangular`.
	Equirectangular,
	/// `cube-map`.
	CubeMap,
}

impl OmafProjection {
	/// The `heif_omaf_image_projection`.
	#[must_use]
	pub const fn as_libheif(self) -> i32 {
		match self {
			Self::Equirectangular => 0,
			Self::CubeMap => 1,
		}
	}

	/// The value `heif-enc --omaf-image-projection` takes.
	#[must_use]
	pub const fn as_heif_enc_str(self) -> &'static str {
		match self {
			Self::Equirectangular => "equirectangular",
			Self::CubeMap => "cube-map",
		}
	}
}

/// Every HEIC control. Defaults are `heif-enc`'s.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[expect(clippy::struct_excessive_bools, reason = "each bool is one of heif-enc's switches; the struct mirrors its command line field for field")]
pub struct HeicSettings {
	/// `-q`, `0..=100`. libheif's and `heif-enc`'s default is 50.
	pub quality: u8,
	/// `-p lossless=true`: Kvazaar codes the 4:2:0 image without loss. The conversion to
	/// YCbCr 4:2:0 before it still loses, as it does in `heif-enc`.
	pub lossless: bool,
	/// Keep the alpha channel; `false` is `--no-alpha`.
	pub alpha: bool,
	/// `--premultiplied-alpha`: mark the colour as premultiplied by alpha. The pixels are
	/// not changed.
	pub premultiplied_alpha: bool,
	/// `-t`: add a thumbnail that fits this square. An image already inside it gets none.
	pub thumbnail: Option<u32>,
	/// Keep the thumbnail's alpha; `false` is `--no-thumb-alpha`.
	pub thumbnail_alpha: bool,
	/// `-C`; `None` lets libheif pick the conversion, which is not the same as `average`.
	pub chroma_downsampling: Option<ChromaDownsampling>,
	/// `--color-profile` and the code points.
	pub color_profile: ColorProfile,
	/// `--enable-two-colr-boxes`: with an ICC profile, write the code points too.
	pub two_colr_boxes: bool,
	/// `--clli`: maximum content light level and maximum picture-average light level, in
	/// cd/m².
	pub clli: Option<[u16; 2]>,
	/// `--pasp`: pixel aspect ratio, horizontal and vertical spacing.
	pub pasp: Option<[u32; 2]>,
	/// `--rotate-cw`, `--flip-h` and `--flip-v`, composed. A JPEG's own Exif orientation
	/// is applied first.
	pub orientation: Orientation,
	/// `--cut-tiles`: split the image into a grid of square tiles this size.
	pub cut_tiles: Option<u32>,
	/// `--omaf-image-projection`.
	pub omaf_projection: Option<OmafProjection>,
	/// `--pitm-description`: a description of the image, in a `udes` property. Empty for
	/// none.
	pub description: String,
	/// `--add-compatible-brand`, each exactly four bytes.
	pub compatible_brands: Vec<String>,
	/// `--unif`: one ID space for items, tracks and groups, and the `unif` brand.
	pub unif: bool,
	/// `--mini`: the compact `mini` box instead of `meta`, when the image allows it.
	pub mini: bool,
}

impl Default for HeicSettings {
	fn default() -> Self {
		Self {
			quality: 50,
			lossless: false,
			alpha: true,
			premultiplied_alpha: false,
			thumbnail: None,
			thumbnail_alpha: true,
			chroma_downsampling: None,
			color_profile: ColorProfile::default(),
			two_colr_boxes: false,
			clli: None,
			pasp: None,
			orientation: Orientation::Normal,
			cut_tiles: None,
			omaf_projection: None,
			description: String::new(),
			compatible_brands: Vec::new(),
			unif: false,
			mini: false,
		}
	}
}

impl HeicSettings {
	/// Check every value is one `heif-enc` accepts.
	///
	/// # Errors
	///
	/// Returns [`ValidationError`] for a value out of range, a code point libheif does not
	/// know, a brand that is not four bytes, or a zero thumbnail or tile size.
	pub fn validate(&self) -> Result<(), ValidationError> {
		if self.quality > 100 {
			return Err(ValidationError::OutOfRange { field: "heic.quality", value: u32::from(self.quality), min: 0, max: 100 });
		}
		if let ColorProfile::Custom { matrix_coefficients, colour_primaries, transfer_characteristics, .. } = self.color_profile {
			// heif_nclx_color_profile_set_*: the code points libheif knows.
			for (field, value, known) in [
				("heic.color_profile.matrix_coefficients", matrix_coefficients, matches!(matrix_coefficients, 0..=2 | 4..=14)),
				("heic.color_profile.colour_primaries", colour_primaries, matches!(colour_primaries, 1 | 2 | 4..=12 | 22)),
				("heic.color_profile.transfer_characteristics", transfer_characteristics, matches!(transfer_characteristics, 1 | 2 | 4..=18)),
			] {
				if !known {
					return Err(ValidationError::NotAllowed { field, value: i64::from(value) });
				}
			}
		}
		for (field, size) in [("heic.thumbnail", self.thumbnail), ("heic.cut_tiles", self.cut_tiles)] {
			if let Some(size) = size {
				// `heif-enc` reads both with atoi into an int.
				if size == 0 || size > i32::MAX.unsigned_abs() {
					return Err(ValidationError::OutOfRange { field, value: size, min: 1, max: i32::MAX.unsigned_abs() });
				}
			}
		}
		if self.compatible_brands.iter().any(|brand| brand.len() != 4) {
			return Err(ValidationError::Conflict("a compatible brand must be exactly four bytes"));
		}
		if self.description.contains('\0') {
			return Err(ValidationError::Conflict("the description cannot contain a NUL character"));
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::{ColorProfile, HeicSettings, Orientation};
	use crate::settings::ValidationError;

	#[test]
	fn a_settings_file_from_before_0_12_loads_with_its_quality() {
		let old: HeicSettings = serde_json::from_str(r#"{"quality":72}"#).expect("deserialize");
		assert_eq!(old, HeicSettings { quality: 72, ..HeicSettings::default() });
	}

	#[test]
	fn settings_round_trip_through_json() {
		let s = HeicSettings { color_profile: ColorProfile::Custom { matrix_coefficients: 9, colour_primaries: 9, transfer_characteristics: 16, full_range: false }, orientation: Orientation::Rotate90CwThenFlipVertically, compatible_brands: vec!["abcd".into()], thumbnail: Some(64), ..HeicSettings::default() };
		let back: HeicSettings = serde_json::from_value(serde_json::to_value(&s).expect("serialize")).expect("deserialize");
		assert_eq!(back, s);
		let auto: HeicSettings = serde_json::from_str(r#"{"colorProfile":{"preset":"auto"}}"#).expect("deserialize");
		assert_eq!(auto.color_profile, ColorProfile::Auto);
	}

	#[test]
	fn values_heif_enc_refuses_are_refused() {
		assert!(HeicSettings::default().validate().is_ok());
		assert!(matches!(HeicSettings { quality: 101, ..HeicSettings::default() }.validate(), Err(ValidationError::OutOfRange { .. })));
		assert!(matches!(HeicSettings { color_profile: ColorProfile::Custom { matrix_coefficients: 3, colour_primaries: 1, transfer_characteristics: 13, full_range: true }, ..HeicSettings::default() }.validate(), Err(ValidationError::NotAllowed { .. })));
		assert!(matches!(HeicSettings { color_profile: ColorProfile::Custom { matrix_coefficients: 6, colour_primaries: 13, transfer_characteristics: 13, full_range: true }, ..HeicSettings::default() }.validate(), Err(ValidationError::NotAllowed { .. })));
		assert!(matches!(HeicSettings { color_profile: ColorProfile::Custom { matrix_coefficients: 6, colour_primaries: 1, transfer_characteristics: 3, full_range: true }, ..HeicSettings::default() }.validate(), Err(ValidationError::NotAllowed { .. })));
		assert!(HeicSettings { compatible_brands: vec!["abc".into()], ..HeicSettings::default() }.validate().is_err());
		assert!(HeicSettings { thumbnail: Some(0), ..HeicSettings::default() }.validate().is_err());
		assert!(HeicSettings { cut_tiles: Some(0), ..HeicSettings::default() }.validate().is_err());
	}
}
