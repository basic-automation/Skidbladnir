//! The HEIC controls: every `heif-enc` option that changes the file it writes for a still
//! image, and, in the GPL edition, x265's own.
//!
//! The encoder depends on the edition. The standard edition's is Kvazaar, and these
//! settings are `heif-enc -e kvazaar`'s options, applied in `heif-enc`'s order (see
//! `native/heic_shim.c`) and parity-tested against it. The GPL edition's is x265 (the
//! `x265` feature): the same options, and x265's own controls on top ([`HeicSettings`]'s
//! `chroma` to `sao`), which are what `heif-enc -e x265 -p` sets. A settings file from an
//! earlier version loads unchanged: every field kept its name and default.
//!
//! What `heif-enc` offers that does not reach Kvazaar is not here:
//!
//! - `-L`. It asks the encoder for a `chroma` parameter Kvazaar does not have, so
//!   `heif-enc -L -e kvazaar` always exits with an error. Kvazaar's own lossless mode is
//!   reached with `-p lossless=true`, which is [`HeicSettings::lossless`] in the standard
//!   edition; in the GPL edition it is `-L`.
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

/// Whether this build encodes HEIC with x265 — the GPL edition, the `x265` feature — rather
/// than Kvazaar. Settings only x265 can honour are refused without it.
pub const HEIC_X265: bool = cfg!(feature = "x265");

/// How much of a HEIC's colour detail x265 keeps: the chroma subsampling (`-p chroma`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeicChroma {
	/// Colour at half the resolution in both directions. What cameras and phones write,
	/// and what every HEIC reader decodes. Kvazaar's only choice.
	#[default]
	#[serde(rename = "420")]
	Yuv420,
	/// Colour at half the horizontal resolution.
	#[serde(rename = "422")]
	Yuv422,
	/// Colour at full resolution: sharp coloured edges, text and pixel art survive, at a
	/// larger size. Not every HEIC reader decodes it.
	#[serde(rename = "444")]
	Yuv444,
}

impl HeicChroma {
	/// The value of libheif's `chroma` encoder parameter.
	#[must_use]
	pub const fn parameter(self) -> &'static str {
		match self {
			Self::Yuv420 => "420",
			Self::Yuv422 => "422",
			Self::Yuv444 => "444",
		}
	}
}

/// The precision of the encoded HEVC data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HeicBitDepth {
	/// 8 bits per channel, HEVC Main. Kvazaar's only choice, and the default because every
	/// reader decodes it.
	#[default]
	Eight,
	/// 10 bits per channel, HEVC Main 10, as phones write their HDR photos. An 8-bit source
	/// is widened to 10 bits first, so libheif's colour conversion runs at 10 bits and
	/// smooth gradients band less; `heif-enc` has no way to ask for that.
	Ten,
}

/// x265's speed presets, fastest first (`-p preset`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HeicPreset {
	/// The fastest.
	Ultrafast,
	/// Nearly the fastest.
	Superfast,
	/// Very fast.
	Veryfast,
	/// Faster than `fast`.
	Faster,
	/// Fast.
	Fast,
	/// x265's own default for video.
	Medium,
	/// libheif's default for still images.
	#[default]
	Slow,
	/// Slower.
	Slower,
	/// Very slow.
	Veryslow,
	/// The slowest, and exhaustive.
	Placebo,
}

impl HeicPreset {
	/// Every preset, fastest first.
	pub const ALL: [Self; 10] = [Self::Ultrafast, Self::Superfast, Self::Veryfast, Self::Faster, Self::Fast, Self::Medium, Self::Slow, Self::Slower, Self::Veryslow, Self::Placebo];

	/// The name x265 and libheif's `preset` parameter use.
	#[must_use]
	pub const fn parameter(self) -> &'static str {
		match self {
			Self::Ultrafast => "ultrafast",
			Self::Superfast => "superfast",
			Self::Veryfast => "veryfast",
			Self::Faster => "faster",
			Self::Fast => "fast",
			Self::Medium => "medium",
			Self::Slow => "slow",
			Self::Slower => "slower",
			Self::Veryslow => "veryslow",
			Self::Placebo => "placebo",
		}
	}
}

/// What x265 tunes its decisions for (`-p tune`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HeicTune {
	/// The best PSNR, at the cost of the psycho-visual tuning below.
	Psnr,
	/// The best SSIM. libheif's default.
	#[default]
	Ssim,
	/// Keep film grain and noise rather than smoothing it away.
	Grain,
	/// Cheaper to decode.
	Fastdecode,
}

impl HeicTune {
	/// The name x265 and libheif's `tune` parameter use.
	#[must_use]
	pub const fn parameter(self) -> &'static str {
		match self {
			Self::Psnr => "psnr",
			Self::Ssim => "ssim",
			Self::Grain => "grain",
			Self::Fastdecode => "fastdecode",
		}
	}
}

/// x265's adaptive quantisation: how bits move between flat and detailed areas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HeicAqMode {
	/// None: every area is quantised alike.
	Off,
	/// By each block's variance. What libheif sets.
	#[default]
	Variance,
	/// By variance, with the strength set per image.
	AutoVariance,
	/// Auto-variance, biased to spend more on dark areas.
	AutoVarianceDark,
	/// Auto-variance, guided by edges.
	AutoVarianceEdge,
}

impl HeicAqMode {
	/// x265's `aq-mode` number.
	#[must_use]
	pub const fn parameter(self) -> &'static str {
		match self {
			Self::Off => "0",
			Self::Variance => "1",
			Self::AutoVariance => "2",
			Self::AutoVarianceDark => "3",
			Self::AutoVarianceEdge => "4",
		}
	}
}

/// Which of the input's metadata goes into the file. `heif-enc` always copies all three it
/// finds; turning one off is the same as removing it from the input first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HeicMetadata {
	/// The ICC colour profile.
	pub icc: bool,
	/// Exif.
	pub exif: bool,
	/// XMP.
	pub xmp: bool,
}

impl Default for HeicMetadata {
	fn default() -> Self {
		Self { icc: true, exif: true, xmp: true }
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
	/// Which of the input's metadata to keep.
	pub metadata: HeicMetadata,
	/// x265 (GPL edition): chroma subsampling, `-p chroma`. Kvazaar writes 4:2:0 only.
	pub chroma: HeicChroma,
	/// x265 (GPL edition): precision of the encoded data. Kvazaar writes 8 bits only.
	pub bit_depth: HeicBitDepth,
	/// x265 (GPL edition): speed preset, `-p preset`.
	pub preset: HeicPreset,
	/// x265 (GPL edition): what to tune for, `-p tune`.
	pub tune: HeicTune,
	/// x265 (GPL edition): how deep intra transform units split, `1..=4`,
	/// `-p tu-intra-depth`. libheif sets 2.
	pub tu_intra_depth: u8,
	/// x265 (GPL edition): adaptive quantisation, `-p x265:aq-mode`.
	pub aq_mode: HeicAqMode,
	/// x265 (GPL edition): its strength, in tenths, `0..=30` (0.0 to 3.0),
	/// `-p x265:aq-strength`. x265's default is 1.0.
	pub aq_strength: u8,
	/// x265 (GPL edition): psycho-visual rate-distortion, in tenths, `0..=50`,
	/// `-p x265:psy-rd`. libheif sets 1.0.
	pub psy_rd: u8,
	/// x265 (GPL edition): psycho-visual quantisation, in tenths, `0..=500`,
	/// `-p x265:psy-rdoq`. libheif sets 1.0.
	pub psy_rdoq: u16,
	/// x265 (GPL edition): the in-loop deblocking filter, `-p x265:deblock`.
	pub deblock: bool,
	/// x265 (GPL edition): deblocking strength offset (`tC`), `-6..=6`.
	pub deblock_strength: i8,
	/// x265 (GPL edition): deblocking threshold offset (`beta`), `-6..=6`.
	pub deblock_threshold: i8,
	/// x265 (GPL edition): sample adaptive offset, `-p x265:sao`.
	pub sao: bool,
}

impl Default for HeicSettings {
	fn default() -> Self {
		Self { quality: 50, lossless: false, alpha: true, premultiplied_alpha: false, thumbnail: None, thumbnail_alpha: true, chroma_downsampling: None, color_profile: ColorProfile::default(), two_colr_boxes: false, clli: None, pasp: None, orientation: Orientation::Normal, cut_tiles: None, omaf_projection: None, description: String::new(), compatible_brands: Vec::new(), unif: false, mini: false, metadata: HeicMetadata { icc: true, exif: true, xmp: true }, chroma: HeicChroma::Yuv420, bit_depth: HeicBitDepth::Eight, preset: HeicPreset::Slow, tune: HeicTune::Ssim, tu_intra_depth: 2, aq_mode: HeicAqMode::Variance, aq_strength: 10, psy_rd: 10, psy_rdoq: 10, deblock: true, deblock_strength: 0, deblock_threshold: 0, sao: true }
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
			for (field, value, known) in [("heic.color_profile.matrix_coefficients", matrix_coefficients, matches!(matrix_coefficients, 0..=2 | 4..=14)), ("heic.color_profile.colour_primaries", colour_primaries, matches!(colour_primaries, 1 | 2 | 4..=12 | 22)), ("heic.color_profile.transfer_characteristics", transfer_characteristics, matches!(transfer_characteristics, 1 | 2 | 4..=18))] {
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
		for (field, value, min, max) in [("heic.tu_intra_depth", self.tu_intra_depth, 1, 4), ("heic.aq_strength", self.aq_strength, 0, 30), ("heic.psy_rd", self.psy_rd, 0, 50)] {
			if value < min || value > max {
				return Err(ValidationError::OutOfRange { field, value: u32::from(value), min: u32::from(min), max: u32::from(max) });
			}
		}
		if self.psy_rdoq > 500 {
			return Err(ValidationError::OutOfRange { field: "heic.psy_rdoq", value: u32::from(self.psy_rdoq), min: 0, max: 500 });
		}
		for (field, value) in [("heic.deblock_strength", self.deblock_strength), ("heic.deblock_threshold", self.deblock_threshold)] {
			if !(-6..=6).contains(&value) {
				return Err(ValidationError::OutOfRangeSigned { field, value: i64::from(value), min: -6, max: 6 });
			}
		}
		if !HEIC_X265 {
			if self.chroma != HeicChroma::Yuv420 {
				return Err(ValidationError::NeedsX265 { field: "heic.chroma" });
			}
			if self.bit_depth != HeicBitDepth::Eight {
				return Err(ValidationError::NeedsX265 { field: "heic.bit_depth" });
			}
		}
		Ok(())
	}

	/// These settings with anything this edition cannot write put back to what it can.
	///
	/// In the GPL edition that is nothing. In the standard edition chroma and bit depth
	/// return to Kvazaar's 8-bit 4:2:0, which is what lets a preset or preferences file saved
	/// by the GPL edition still open there rather than being thrown away. The x265 tuning
	/// rides along untouched; Kvazaar ignores it.
	#[must_use]
	pub fn for_this_edition(self) -> Self {
		if HEIC_X265 {
			return self;
		}
		Self { chroma: HeicChroma::Yuv420, bit_depth: HeicBitDepth::Eight, ..self }
	}
}

#[cfg(test)]
mod tests {
	use super::{ColorProfile, HEIC_X265, HeicAqMode, HeicBitDepth, HeicChroma, HeicMetadata, HeicPreset, HeicSettings, HeicTune, Orientation};
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

	/// The x265 defaults are what libheif sets for x265 itself.
	#[test]
	fn x265_defaults_are_libheifs() {
		let s = HeicSettings::default();
		assert_eq!((s.quality, s.lossless, s.chroma, s.bit_depth), (50, false, HeicChroma::Yuv420, HeicBitDepth::Eight));
		assert_eq!((s.preset, s.tune, s.tu_intra_depth), (HeicPreset::Slow, HeicTune::Ssim, 2));
		assert_eq!((s.aq_mode, s.aq_strength, s.psy_rd, s.psy_rdoq), (HeicAqMode::Variance, 10, 10, 10));
		assert_eq!((s.deblock, s.deblock_strength, s.deblock_threshold, s.sao), (true, 0, 0, true));
		assert_eq!(s.metadata, HeicMetadata { icc: true, exif: true, xmp: true }, "heif-enc copies all three");
	}

	/// 0.13's GPL-edition settings, as its window wrote them, still load.
	#[test]
	fn a_gpl_edition_file_from_0_13_loads() {
		let json = r#"{"quality":70,"lossless":true,"chroma":"444","bitDepth":"ten","preset":"veryslow","tune":"grain","tuIntraDepth":4,"aqMode":"autoVarianceEdge","aqStrength":15,"psyRd":20,"psyRdoq":50,"deblock":false,"deblockStrength":-2,"deblockThreshold":3,"sao":false}"#;
		let s: HeicSettings = serde_json::from_str(json).expect("parse");
		assert_eq!(s, HeicSettings { quality: 70, lossless: true, chroma: HeicChroma::Yuv444, bit_depth: HeicBitDepth::Ten, preset: HeicPreset::Veryslow, tune: HeicTune::Grain, tu_intra_depth: 4, aq_mode: HeicAqMode::AutoVarianceEdge, aq_strength: 15, psy_rd: 20, psy_rdoq: 50, deblock: false, deblock_strength: -2, deblock_threshold: 3, sao: false, ..HeicSettings::default() });
	}

	#[test]
	fn x265_ranges_are_enforced_in_both_editions() {
		for (settings, field, value, min, max) in [(HeicSettings { tu_intra_depth: 0, ..HeicSettings::default() }, "heic.tu_intra_depth", 0, 1, 4), (HeicSettings { tu_intra_depth: 5, ..HeicSettings::default() }, "heic.tu_intra_depth", 5, 1, 4), (HeicSettings { aq_strength: 31, ..HeicSettings::default() }, "heic.aq_strength", 31, 0, 30), (HeicSettings { psy_rd: 51, ..HeicSettings::default() }, "heic.psy_rd", 51, 0, 50), (HeicSettings { psy_rdoq: 501, ..HeicSettings::default() }, "heic.psy_rdoq", 501, 0, 500)] {
			assert_eq!(settings.validate(), Err(ValidationError::OutOfRange { field, value, min, max }));
		}
		assert_eq!(HeicSettings { deblock_strength: -7, ..HeicSettings::default() }.validate(), Err(ValidationError::OutOfRangeSigned { field: "heic.deblock_strength", value: -7, min: -6, max: 6 }));
	}

	/// What only x265 can write is refused by the standard edition, and put back to what
	/// Kvazaar writes when read from disk. Lossless is not x265's alone: Kvazaar has its own.
	#[test]
	fn what_only_x265_writes_follows_the_edition() {
		let x265 = HeicSettings { lossless: true, chroma: HeicChroma::Yuv444, bit_depth: HeicBitDepth::Ten, preset: HeicPreset::Placebo, ..HeicSettings::default() };
		if HEIC_X265 {
			assert_eq!(x265.validate(), Ok(()));
			assert_eq!(x265.clone().for_this_edition(), x265);
		} else {
			assert_eq!(x265.validate(), Err(ValidationError::NeedsX265 { field: "heic.chroma" }));
			assert_eq!(HeicSettings { bit_depth: HeicBitDepth::Ten, ..HeicSettings::default() }.validate(), Err(ValidationError::NeedsX265 { field: "heic.bit_depth" }));
			let kept = x265.for_this_edition();
			assert_eq!(kept, HeicSettings { lossless: true, preset: HeicPreset::Placebo, ..HeicSettings::default() });
			assert_eq!(kept.validate(), Ok(()));
		}
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
