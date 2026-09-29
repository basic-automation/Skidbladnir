//! The AVIF controls: every `avifenc` option that changes the file it writes for a still
//! image, with libaom as the AV1 encoder.
//!
//! Until 0.12 AVIF went through `ravif`, whose seven controls reached a fraction of what
//! `avifenc` can write: no 12-bit, no 4:2:0, 4:2:2 or 4:0:0, no true lossless, no colour
//! signalling, tiling, grids, target size or codec tuning. The encoder is now libavif with
//! libaom, `avifenc`'s own library and default codec, and these settings are `avifenc`'s
//! options. An option `avifenc` treats as "not given" is `None` here, because several of
//! its defaults depend on what else was given (alpha quality follows colour quality until
//! it is set; `--target-size` searches only the qualities that were not set).
//!
//! Not here, because they are about more than one image: sequences and their timing
//! (`--fps`, `--duration`, `-k`, `--repetition-count`), `--layered` from several inputs, and
//! grids from several files. Not here either: `-c` (only libaom is built in), and the
//! gain-map options, which need libavif built with libxml2. See ROADMAP.md.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{ValidationError, jxl::MetadataSource};

/// The YUV layout of the encoded image (`-y`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum YuvFormat {
	/// `auto`: 4:0:0 for a grayscale source, 4:4:4 otherwise.
	#[default]
	Auto,
	/// `444`.
	Yuv444,
	/// `422`.
	Yuv422,
	/// `420`.
	Yuv420,
	/// `400`: grayscale.
	Yuv400,
}

impl YuvFormat {
	/// The `avifPixelFormat`, or -1 for automatic.
	#[must_use]
	pub const fn as_libavif(self) -> i32 {
		match self {
			Self::Auto => -1,
			Self::Yuv444 => 1,
			Self::Yuv422 => 2,
			Self::Yuv420 => 3,
			Self::Yuv400 => 4,
		}
	}

	/// The value `avifenc -y` takes.
	#[must_use]
	pub const fn as_avifenc_str(self) -> &'static str {
		match self {
			Self::Auto => "auto",
			Self::Yuv444 => "444",
			Self::Yuv422 => "422",
			Self::Yuv420 => "420",
			Self::Yuv400 => "400",
		}
	}
}

/// Colour signalling as ITU-T H.273 code points (`--cicp P/T/M`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cicp {
	/// Colour primaries, e.g. 1 for BT.709/sRGB, 9 for BT.2020, 12 for Display P3.
	pub primaries: u16,
	/// Transfer characteristics, e.g. 13 for sRGB, 16 for PQ, 18 for HLG.
	pub transfer: u16,
	/// Matrix coefficients, e.g. 6 for BT.601, 1 for BT.709, 0 for identity (RGB).
	pub matrix: u16,
}

/// A pair of quantizer bounds, `0..=63` (`--min`/`--max`, `--minalpha`/`--maxalpha`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuantizerRange {
	/// The lowest quantizer, the best quality.
	pub min: u8,
	/// The highest quantizer, the worst quality.
	pub max: u8,
}

/// How the image is split into AV1 tiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Tiling {
	/// `--autotiling`, `avifenc`'s default: libavif chooses from the image size.
	#[default]
	Automatic,
	/// `--tilerowslog2` / `--tilecolslog2`, each `0..=6`.
	Manual {
		/// log2 of the number of tile rows.
		rows_log2: u8,
		/// log2 of the number of tile columns.
		cols_log2: u8,
	},
}

/// A fraction, for `--scaling-mode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fraction {
	/// Numerator.
	pub numerator: u32,
	/// Denominator.
	pub denominator: u32,
}

/// A grid of `columns` × `rows` cells the image is split into (`-g`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Grid {
	/// Cells across, `1..=256`.
	pub columns: u32,
	/// Cells down, `1..=256`.
	pub rows: u32,
}

/// The clean aperture: a crop signalled in the file, not applied to the pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "values")]
pub enum CleanAperture {
	/// `--crop x,y,width,height`, converted by libavif.
	Crop([u32; 4]),
	/// `--clap`: width, height, horizontal and vertical offset, each as numerator and
	/// denominator.
	Raw([u32; 8]),
}

/// A codec-specific option passed straight to libaom (`-a key=value`), e.g. `tune=ssim`,
/// `sharpness=3`, `color:aq-mode=1`, `alpha:end-usage=q`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodecOption {
	/// The key, with an optional `color:`/`c:` or `alpha:`/`a:` prefix.
	pub key: String,
	/// The value; empty for a bare key.
	pub value: String,
}

/// Every AVIF control. Defaults are `avifenc`'s.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[expect(clippy::struct_excessive_bools, reason = "each bool is one of avifenc's switches; the struct mirrors its command line field for field")]
pub struct AvifSettings {
	/// `-q`, `0..=100`, where 100 is lossless; `None` for `avifenc`'s 60.
	pub quality: Option<u8>,
	/// `--qalpha`, `0..=100`; `None` to follow the colour quality.
	pub quality_alpha: Option<u8>,
	/// `-s`, `0..=10`, 0 slowest; `None` for `-s default`, libaom's own. `avifenc`'s is 6.
	pub speed: Option<u8>,
	/// `-l`: set the defaults for lossless (quality 100, identity matrix, full range).
	pub lossless: bool,
	/// `-d`: 8, 10 or 12 bits; `None` for 8 from an 8-bit source, 12 from a deeper one.
	pub depth: Option<u8>,
	/// The second value of `-d D,E`: 4 or 8 bits of a hidden extension image, which with the
	/// primary reaches 16 bits (8,8, 12,4 and 12,8).
	pub depth_extension: Option<u8>,
	/// `-y`.
	pub yuv: YuvFormat,
	/// `-p`: premultiply the colour by alpha, and signal it.
	pub premultiply: bool,
	/// `--sharpyuv`: sharp RGB to YUV 4:2:0 conversion.
	pub sharp_yuv: bool,
	/// `--cicp`: colour signalling given explicitly.
	pub cicp: Option<Cicp>,
	/// `-r limited`: limited-range YUV. Full range otherwise.
	pub limited_range: bool,
	/// `--target-size`: aim for this many bytes, searching the qualities not set.
	pub target_size: Option<u32>,
	/// `--progressive`: a two-layer image, a small first layer then the full one.
	pub progressive: bool,
	/// `-g`: split into a grid of cells.
	pub grid: Option<Grid>,
	/// `--min`/`--max`: colour quantizer bounds (deprecated in `avifenc`, still honoured).
	pub quantizer: Option<QuantizerRange>,
	/// `--minalpha`/`--maxalpha`.
	pub alpha_quantizer: Option<QuantizerRange>,
	/// `--autotiling` or `--tilerowslog2`/`--tilecolslog2`.
	pub tiling: Tiling,
	/// `--scaling-mode`: encode at this fraction of the size.
	pub scaling_mode: Option<Fraction>,
	/// `-a`: libaom options, in order.
	pub codec_options: Vec<CodecOption>,
	/// `--pasp h,v`: pixel aspect ratio.
	pub pasp: Option<[u32; 2]>,
	/// `--crop` / `--clap`.
	pub clean_aperture: Option<CleanAperture>,
	/// `--irot`, `0..=3`: rotation by 90° anticlockwise steps.
	pub irot: Option<u8>,
	/// `--imir`: 0 mirrors top-to-bottom, 1 left-to-right.
	pub imir: Option<u8>,
	/// `--clli MaxCLL,MaxPALL`.
	pub clli: Option<[u16; 2]>,
	/// The ICC profile: from the input, `--ignore-icc`, or `--icc <file>`.
	pub icc: MetadataSource,
	/// Exif: from the input, `--ignore-exif`, or `--exif <file>`.
	pub exif: MetadataSource,
	/// XMP: from the input, `--ignore-xmp`, or `--xmp <file>`.
	pub xmp: MetadataSource,
	/// `-j`: worker threads, `None` for `all`.
	pub jobs: Option<u32>,
}

impl Default for AvifSettings {
	fn default() -> Self {
		Self { quality: None, quality_alpha: None, speed: Some(6), lossless: false, depth: None, depth_extension: None, yuv: YuvFormat::Auto, premultiply: false, sharp_yuv: false, cicp: None, limited_range: false, target_size: None, progressive: false, grid: None, quantizer: None, alpha_quantizer: None, tiling: Tiling::Automatic, scaling_mode: None, codec_options: Vec::new(), pasp: None, clean_aperture: None, irot: None, imir: None, clli: None, icc: MetadataSource::Keep, exif: MetadataSource::Keep, xmp: MetadataSource::Keep, jobs: None }
	}
}

impl AvifSettings {
	/// Check every control against what `avifenc` accepts.
	///
	/// # Errors
	///
	/// Returns the first [`ValidationError`] found.
	pub fn validate(&self) -> Result<(), ValidationError> {
		fn range(field: &'static str, value: u32, min: u32, max: u32) -> Result<(), ValidationError> {
			if value < min || value > max {
				return Err(ValidationError::OutOfRange { field, value, min, max });
			}
			Ok(())
		}
		for (field, value) in [("avif.quality", self.quality), ("avif.quality_alpha", self.quality_alpha)] {
			if let Some(value) = value {
				range(field, u32::from(value), 0, 100)?;
			}
		}
		if let Some(speed) = self.speed {
			range("avif.speed", u32::from(speed), 0, 10)?;
		}
		match (self.depth, self.depth_extension) {
			(None | Some(8 | 10 | 12), None) | (Some(8), Some(8)) | (Some(12), Some(4 | 8)) => {}
			(Some(depth), None) => return Err(ValidationError::NotAllowed { field: "avif.depth", value: i64::from(depth) }),
			(_, Some(extension)) => return Err(ValidationError::NotAllowed { field: "avif.depth_extension", value: i64::from(extension) }),
		}
		if let Some(size) = self.target_size {
			range("avif.target_size", size, 1, u32::MAX)?;
		}
		if let Some(grid) = self.grid {
			range("avif.grid.columns", grid.columns, 1, 256)?;
			range("avif.grid.rows", grid.rows, 1, 256)?;
		}
		for (field, quantizer) in [("avif.quantizer", self.quantizer), ("avif.alpha_quantizer", self.alpha_quantizer)] {
			if let Some(q) = quantizer {
				range(field, u32::from(q.min), 0, 63)?;
				range(field, u32::from(q.max), 0, 63)?;
			}
		}
		if let Tiling::Manual { rows_log2, cols_log2 } = self.tiling {
			range("avif.tiling.rows_log2", u32::from(rows_log2), 0, 6)?;
			range("avif.tiling.cols_log2", u32::from(cols_log2), 0, 6)?;
		}
		if let Some(fraction) = self.scaling_mode {
			range("avif.scaling_mode.numerator", fraction.numerator, 0, i32::MAX.unsigned_abs())?;
			range("avif.scaling_mode.denominator", fraction.denominator, 1, i32::MAX.unsigned_abs())?;
		}
		if let Some(irot) = self.irot {
			range("avif.irot", u32::from(irot), 0, 3)?;
		}
		if let Some(imir) = self.imir {
			range("avif.imir", u32::from(imir), 0, 1)?;
		}
		if self.progressive && self.grid.is_some() {
			return Err(ValidationError::Conflict("avifenc cannot make a progressive grid image"));
		}
		if self.codec_options.iter().any(|option| option.key.is_empty()) {
			return Err(ValidationError::Conflict("a codec option needs a key"));
		}
		Ok(())
	}

	/// The files the metadata options read, in order: ICC, Exif, XMP.
	#[must_use]
	pub fn metadata_files(&self) -> [Option<&PathBuf>; 3] {
		fn file(source: &MetadataSource) -> Option<&PathBuf> {
			if let MetadataSource::File(path) = source { Some(path) } else { None }
		}
		[file(&self.icc), file(&self.exif), file(&self.xmp)]
	}
}

/// The pre-0.12 `ravif` settings, as they were written.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Ravif {
	quality: Option<u8>,
	alpha_quality: Option<u8>,
	speed: Option<u8>,
	bit_depth: Option<String>,
	color_model: Option<String>,
	alpha_mode: Option<String>,
	multi_threading: Option<bool>,
}

/// The current shape, strictly: an unknown key means it is not this shape.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[expect(clippy::struct_excessive_bools, reason = "the wire form of AvifSettings, which mirrors avifenc's switches")]
struct Current {
	#[serde(default)]
	quality: Option<u8>,
	#[serde(default)]
	quality_alpha: Option<u8>,
	#[serde(default = "default_speed")]
	speed: Option<u8>,
	#[serde(default)]
	lossless: bool,
	#[serde(default)]
	depth: Option<u8>,
	#[serde(default)]
	depth_extension: Option<u8>,
	#[serde(default)]
	yuv: YuvFormat,
	#[serde(default)]
	premultiply: bool,
	#[serde(default)]
	sharp_yuv: bool,
	#[serde(default)]
	cicp: Option<Cicp>,
	#[serde(default)]
	limited_range: bool,
	#[serde(default)]
	target_size: Option<u32>,
	#[serde(default)]
	progressive: bool,
	#[serde(default)]
	grid: Option<Grid>,
	#[serde(default)]
	quantizer: Option<QuantizerRange>,
	#[serde(default)]
	alpha_quantizer: Option<QuantizerRange>,
	#[serde(default)]
	tiling: Tiling,
	#[serde(default)]
	scaling_mode: Option<Fraction>,
	#[serde(default)]
	codec_options: Vec<CodecOption>,
	#[serde(default)]
	pasp: Option<[u32; 2]>,
	#[serde(default)]
	clean_aperture: Option<CleanAperture>,
	#[serde(default)]
	irot: Option<u8>,
	#[serde(default)]
	imir: Option<u8>,
	#[serde(default)]
	clli: Option<[u16; 2]>,
	#[serde(default)]
	icc: MetadataSource,
	#[serde(default)]
	exif: MetadataSource,
	#[serde(default)]
	xmp: MetadataSource,
	#[serde(default)]
	jobs: Option<u32>,
}

#[expect(clippy::unnecessary_wraps, reason = "serde's `default = ...` needs this signature")]
const fn default_speed() -> Option<u8> {
	Some(6)
}

/// Accept the pre-0.12 `ravif` shape as well as this one. `ravif`'s controls map onto
/// `avifenc`'s where they have an equivalent; an untouched `ravif` default becomes
/// `avifenc`'s default rather than being carried across as though it were a choice.
impl<'de> Deserialize<'de> for AvifSettings {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		#[derive(Deserialize)]
		#[serde(rename_all = "camelCase", untagged)]
		enum Wire {
			Current(Box<Current>),
			Ravif(Ravif),
		}
		Ok(match Wire::deserialize(deserializer)? {
			Wire::Current(c) => Self { quality: c.quality, quality_alpha: c.quality_alpha, speed: c.speed, lossless: c.lossless, depth: c.depth, depth_extension: c.depth_extension, yuv: c.yuv, premultiply: c.premultiply, sharp_yuv: c.sharp_yuv, cicp: c.cicp, limited_range: c.limited_range, target_size: c.target_size, progressive: c.progressive, grid: c.grid, quantizer: c.quantizer, alpha_quantizer: c.alpha_quantizer, tiling: c.tiling, scaling_mode: c.scaling_mode, codec_options: c.codec_options, pasp: c.pasp, clean_aperture: c.clean_aperture, irot: c.irot, imir: c.imir, clli: c.clli, icc: c.icc, exif: c.exif, xmp: c.xmp, jobs: c.jobs },
			Wire::Ravif(old) => {
				let untouched = old.quality.unwrap_or(80) == 80 && old.alpha_quality.unwrap_or(80) == 80 && old.speed.unwrap_or(5) == 5 && old.bit_depth.as_deref().unwrap_or("ten") == "ten" && old.color_model.as_deref().unwrap_or("ycbcr") == "ycbcr" && old.alpha_mode.as_deref().unwrap_or("clean") == "clean";
				if untouched {
					Self { jobs: old.multi_threading.filter(|on| !on).map(|_| 1), ..Self::default() }
				} else {
					Self {
						quality: old.quality.or(Some(80)),
						quality_alpha: old.alpha_quality.or(Some(80)),
						speed: old.speed.or(Some(5)),
						depth: Some(if old.bit_depth.as_deref() == Some("eight") { 8 } else { 10 }),
						// ravif's RGB model is AV1's identity matrix, which needs 4:4:4.
						cicp: (old.color_model.as_deref() == Some("rgb")).then_some(Cicp { primaries: 1, transfer: 13, matrix: 0 }),
						premultiply: old.alpha_mode.as_deref() == Some("premultiplied"),
						jobs: old.multi_threading.filter(|on| !on).map(|_| 1),
						..Self::default()
					}
				}
			}
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn defaults_are_avifencs() {
		let s = AvifSettings::default();
		assert_eq!((s.quality, s.quality_alpha, s.speed, s.depth, s.yuv), (None, None, Some(6), None, YuvFormat::Auto));
		assert_eq!(s.validate(), Ok(()));
		let back: AvifSettings = serde_json::from_value(serde_json::to_value(&s).expect("serialize")).expect("deserialize");
		assert_eq!(back, s);
	}

	#[test]
	fn ravif_settings_still_load() {
		let untouched: AvifSettings = serde_json::from_str(r#"{"quality":80,"alphaQuality":80,"speed":5,"bitDepth":"ten","colorModel":"ycbcr","alphaMode":"clean","multiThreading":true}"#).expect("deserialize");
		assert_eq!(untouched, AvifSettings::default(), "an untouched ravif default becomes avifenc's default");
		let tuned: AvifSettings = serde_json::from_str(r#"{"quality":55,"alphaQuality":90,"speed":3,"bitDepth":"eight","colorModel":"rgb","alphaMode":"premultiplied","multiThreading":false}"#).expect("deserialize");
		assert_eq!((tuned.quality, tuned.quality_alpha, tuned.speed, tuned.depth, tuned.jobs), (Some(55), Some(90), Some(3), Some(8), Some(1)));
		assert_eq!(tuned.cicp.map(|c| c.matrix), Some(0));
		assert!(tuned.premultiply);
	}

	#[test]
	fn ranges_are_avifencs() {
		let d = AvifSettings::default;
		assert!(AvifSettings { quality: Some(101), ..d() }.validate().is_err());
		assert!(AvifSettings { speed: Some(11), ..d() }.validate().is_err());
		assert!(AvifSettings { depth: Some(9), ..d() }.validate().is_err());
		assert!(AvifSettings { depth: Some(10), depth_extension: Some(8), ..d() }.validate().is_err(), "10,8 is not a sample transform avifenc knows");
		assert_eq!(AvifSettings { depth: Some(12), depth_extension: Some(4), ..d() }.validate(), Ok(()));
		assert!(AvifSettings { grid: Some(Grid { columns: 0, rows: 2 }), ..d() }.validate().is_err());
		assert!(AvifSettings { tiling: Tiling::Manual { rows_log2: 7, cols_log2: 0 }, ..d() }.validate().is_err());
		assert!(AvifSettings { progressive: true, grid: Some(Grid { columns: 2, rows: 2 }), ..d() }.validate().is_err());
	}
}
