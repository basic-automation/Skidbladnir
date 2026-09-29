//! The one representation of an encode job.
//!
//! An [`EncodeJob`] is what the window sends, what presets and the preferences file
//! store, and what the encoder takes: the output format, the crop and resize (which every
//! format shares), and one settings struct per format.
//!
//! # The contract: every option of the reference tool
//!
//! Each format's settings are the **whole** option surface of that format's reference
//! command-line encoder, as far as it concerns one still image: `cwebp` for WebP,
//! `avifenc` for AVIF, `cjxl` for JPEG XL and `heif-enc` (with Kvazaar) for HEIC. A
//! command line for one of those tools maps onto one value of these types, and the
//! parity tests encode the same image both ways and compare the bytes. Where a tool has a
//! shorthand that rewrites several options at once (`cwebp -preset`, `cwebp -z`), the
//! shorthand is a method that sets the fields, and the fields are what is stored.
//!
//! Each format's settings stay live while the user tries another, which is why they are
//! sibling fields rather than an enum.

mod avif;
mod heic;
mod jxl;
pub(crate) mod legacy;
mod webp;

pub use avif::{AvifAlphaMode, AvifBitDepth, AvifColorModel, AvifSettings};
pub use heic::HeicSettings;
pub use jxl::{JxlColorSpace, JxlSettings, JxlTarget, MetadataSource, Primaries, RenderingIntent, TransferFunction, Tristate, WhitePoint};
use serde::{Deserialize, Serialize};
use thiserror::Error;
pub use webp::{AlphaFiltering, FilterType, ImageHint, Preset, TargetMetric, WebpMetadata, WebpSettings};

/// Which format to write.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputFormat {
	/// WebP, through libwebp. Parity-tested against `cwebp`.
	#[default]
	Webp,
	/// AVIF, through libavif with the libaom encoder. Parity-tested against `avifenc`.
	Avif,
	/// JPEG XL, through libjxl. Parity-tested against `cjxl`.
	Jxl,
	/// HEIC: HEVC in HEIF, through libheif with the Kvazaar encoder. Parity-tested against
	/// `heif-enc -e kvazaar`.
	Heic,
}

impl OutputFormat {
	/// The file extension written for this format, without the dot.
	#[must_use]
	pub const fn extension(self) -> &'static str {
		match self {
			Self::Webp => "webp",
			Self::Avif => "avif",
			Self::Jxl => "jxl",
			Self::Heic => "heic",
		}
	}

	/// The MIME type of this format's files.
	#[must_use]
	pub const fn mime_type(self) -> &'static str {
		match self {
			Self::Webp => "image/webp",
			Self::Avif => "image/avif",
			Self::Jxl => "image/jxl",
			Self::Heic => "image/heic",
		}
	}
}

/// When a resize applies, relative to the source's own size (`cwebp -resize_mode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResizeMode {
	/// Always resize (`always`, `cwebp`'s default).
	#[default]
	Always,
	/// Only ever shrink: leave an image alone when it already fits (`down_only`).
	DownOnly,
	/// Only ever enlarge (`up_only`).
	UpOnly,
}

/// An output resize, applied after any crop and before encoding.
///
/// `0` means "derive from the other dimension, preserving aspect ratio", which is
/// `cwebp -resize`'s own convention. Both zero means no resize at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resize {
	/// Target width in pixels, or `0` to derive it from the height.
	pub width: u32,
	/// Target height in pixels, or `0` to derive it from the width.
	pub height: u32,
	/// When the resize applies at all.
	pub mode: ResizeMode,
}

impl Resize {
	/// A resize to `width` × `height` that always applies.
	#[must_use]
	pub const fn to(width: u32, height: u32) -> Self {
		Self { width, height, mode: ResizeMode::Always }
	}

	/// Whether this resize does anything. Both dimensions zero means "leave it alone".
	#[must_use]
	pub const fn is_noop(self) -> bool {
		self.width == 0 && self.height == 0
	}

	/// The resize to actually apply to a `width` × `height` source, after the mode has had
	/// its say. A mode that declines returns a no-op. This is `cwebp.c`'s `ApplyResizeMode`,
	/// line for line.
	#[must_use]
	pub const fn for_source(self, width: u32, height: u32) -> Self {
		let (dst_w, dst_h) = (self.width, self.height);
		let declined = match self.mode {
			ResizeMode::Always => false,
			ResizeMode::DownOnly => (dst_w == 0 && height <= dst_h) || (dst_h == 0 && width <= dst_w) || (width <= dst_w && height <= dst_h),
			ResizeMode::UpOnly => width >= dst_w && height >= dst_h,
		};
		if declined { Self { width: 0, height: 0, mode: self.mode } } else { self }
	}
}

impl<'de> Deserialize<'de> for Resize {
	/// Reads the pre-0.12 `noEnlarge` switch as well as `mode`. `noEnlarge` was the only
	/// resize condition before `cwebp`'s modes were offered, and is `down_only`'s nearest
	/// equivalent.
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		#[derive(Default, Deserialize)]
		#[serde(rename_all = "camelCase", default)]
		struct Wire {
			width: u32,
			height: u32,
			mode: Option<ResizeMode>,
			no_enlarge: bool,
		}
		let wire = Wire::deserialize(deserializer)?;
		let mode = wire.mode.unwrap_or(if wire.no_enlarge { ResizeMode::DownOnly } else { ResizeMode::Always });
		Ok(Self { width: wire.width, height: wire.height, mode })
	}
}

/// A crop, applied to the source before anything else (`cwebp -crop`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Crop {
	/// Left edge, in source pixels.
	pub x: u32,
	/// Top edge, in source pixels.
	pub y: u32,
	/// Width of the kept rectangle.
	pub width: u32,
	/// Height of the kept rectangle.
	pub height: u32,
}

impl Crop {
	/// Whether this rectangle lies inside a `width` × `height` source and keeps at least one
	/// pixel, which is what `WebPPictureView` requires.
	#[must_use]
	pub const fn fits(self, width: u32, height: u32) -> bool {
		self.width > 0 && self.height > 0 && self.x < width && self.y < height && self.width <= width - self.x && self.height <= height - self.y
	}
}

/// A complete encode job: the output format, the shared crop and resize, and each
/// format's own settings.
///
/// This is the type that crosses the IPC boundary and is written to disk by presets and
/// the preferences file. It reads every shape an earlier version wrote — the flat shape
/// from before the per-format split, and the WebP modes from before 0.12 — so a saved
/// preset loads with every value intact, and always writes its own shape.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodeJob {
	/// Which format to write.
	pub format: OutputFormat,
	/// Crop the source first, or `None` to keep all of it. Shared by every format.
	pub crop: Option<Crop>,
	/// Resize before encoding. Shared by every format.
	pub resize: Resize,
	/// The WebP controls.
	pub webp: WebpSettings,
	/// The AVIF controls. Kept alongside the WebP ones rather than replacing them, so
	/// trying the other format does not throw away this one's tuning.
	pub avif: AvifSettings,
	/// The JPEG XL controls, kept alongside the others for the same reason.
	pub jxl: JxlSettings,
	/// The HEIC controls, likewise.
	pub heic: HeicSettings,
}

impl From<WebpSettings> for EncodeJob {
	/// A WebP job with no crop or resize.
	fn from(webp: WebpSettings) -> Self {
		Self { webp, ..Self::default() }
	}
}

impl<'de> Deserialize<'de> for EncodeJob {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		/// Both job shapes at once. `resize` is top-level in each; the WebP controls are
		/// either under `webp` (current) or flattened at the top level (before the split).
		/// `webp` wins when present.
		#[derive(Deserialize)]
		#[serde(rename_all = "camelCase")]
		struct Wire {
			#[serde(default)]
			format: OutputFormat,
			#[serde(default)]
			crop: Option<Crop>,
			#[serde(default)]
			resize: Resize,
			webp: Option<WebpSettings>,
			#[serde(default)]
			avif: AvifSettings,
			#[serde(default)]
			jxl: JxlSettings,
			#[serde(default)]
			heic: HeicSettings,
			#[serde(flatten)]
			flat: legacy::WebpWire,
		}

		let wire = Wire::deserialize(deserializer)?;
		let webp = wire.webp.unwrap_or_else(|| if wire.flat.is_empty() { WebpSettings::default() } else { wire.flat.into_settings() });
		Ok(Self { format: wire.format, crop: wire.crop, resize: wire.resize, webp, avif: wire.avif, jxl: wire.jxl, heic: wire.heic })
	}
}

impl EncodeJob {
	/// Check the settings of the format this job writes, and the shared crop.
	///
	/// A crop is only checked for being non-empty here: whether it fits depends on the
	/// source, and is checked when the source is known.
	///
	/// # Errors
	///
	/// Returns the first [`ValidationError`] found.
	pub fn validate(&self) -> Result<(), ValidationError> {
		if let Some(crop) = self.crop
			&& (crop.width == 0 || crop.height == 0)
		{
			return Err(ValidationError::EmptyCrop);
		}
		match self.format {
			OutputFormat::Webp => self.webp.validate(),
			OutputFormat::Avif => self.avif.validate(),
			OutputFormat::Jxl => self.jxl.validate(),
			OutputFormat::Heic => self.heic.validate(),
		}
	}
}

/// Why a job cannot be encoded.
///
/// The range checks each reference tool (or the library under it) enforces, so a value
/// the tool would refuse is refused here too, with a message, rather than crashing or
/// being silently clamped.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub enum ValidationError {
	/// A numeric control is outside the range the encoder accepts.
	#[error("{field} is {value}, but must be in {min}..={max}")]
	OutOfRange {
		/// The field's name, spelled as the struct field.
		field: &'static str,
		/// The rejected value.
		value: u32,
		/// Lowest accepted value.
		min: u32,
		/// Highest accepted value.
		max: u32,
	},
	/// A fractional control is outside the range the encoder accepts, or is not a number.
	#[error("{field} is {value}, but must be in {min}..={max}")]
	OutOfRangeFloat {
		/// The field's name, spelled as the struct field.
		field: &'static str,
		/// The rejected value.
		value: f32,
		/// Lowest accepted value.
		min: f32,
		/// Highest accepted value.
		max: f32,
	},
	/// A signed control is outside the range the encoder accepts.
	#[error("{field} is {value}, but must be in {min}..={max}")]
	OutOfRangeSigned {
		/// The field's name, spelled as the struct field.
		field: &'static str,
		/// The rejected value.
		value: i64,
		/// Lowest accepted value.
		min: i64,
		/// Highest accepted value.
		max: i64,
	},
	/// A control takes one of a few values, and this is not one of them.
	#[error("{field} is {value}, which is not one of the values it accepts")]
	NotAllowed {
		/// The field's name, spelled as the struct field.
		field: &'static str,
		/// The rejected value.
		value: i64,
	},
	/// Two controls that the reference tool refuses to combine.
	#[error("{0}")]
	Conflict(&'static str),
	/// A target size of zero bytes cannot be met.
	#[error("target size must be at least 1 byte")]
	ZeroTargetSize,
	/// A crop with no width or no height.
	#[error("the crop rectangle is empty")]
	EmptyCrop,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn default_job_is_an_untouched_webp_encode() {
		let job = EncodeJob::default();
		assert_eq!(job.format, OutputFormat::Webp);
		assert!(job.resize.is_noop());
		assert_eq!(job.crop, None);
		assert_eq!(job.validate(), Ok(()));
	}

	/// `cwebp.c`'s `ApplyResizeMode`.
	#[test]
	fn resize_modes_follow_cwebp() {
		let down = Resize { width: 800, height: 0, mode: ResizeMode::DownOnly };
		assert!(down.for_source(640, 480).is_noop(), "already narrower than the target");
		assert_eq!(down.for_source(1600, 1200), down);
		let box_down = Resize { width: 800, height: 600, mode: ResizeMode::DownOnly };
		assert_eq!(box_down.for_source(1000, 500), box_down, "down_only declines only when BOTH fit");
		let up = Resize { width: 800, height: 600, mode: ResizeMode::UpOnly };
		assert!(up.for_source(1600, 1200).is_noop());
		assert_eq!(up.for_source(400, 1200), up, "up_only applies when either is smaller");
		assert_eq!(Resize::to(10, 0).for_source(1, 1), Resize::to(10, 0));
	}

	/// The pre-0.12 `noEnlarge` switch becomes `down_only`.
	#[test]
	fn no_enlarge_reads_as_down_only() {
		let resize: Resize = serde_json::from_str(r#"{"width":640,"height":0,"noEnlarge":true}"#).expect("deserialize");
		assert_eq!(resize, Resize { width: 640, height: 0, mode: ResizeMode::DownOnly });
		let resize: Resize = serde_json::from_str(r#"{"width":640,"height":0,"noEnlarge":false}"#).expect("deserialize");
		assert_eq!(resize.mode, ResizeMode::Always);
	}

	#[test]
	fn crops_must_fit() {
		let crop = Crop { x: 10, y: 10, width: 20, height: 20 };
		assert!(crop.fits(30, 30));
		assert!(!crop.fits(29, 30));
		assert!(!Crop { x: 30, ..crop }.fits(30, 30));
		assert!(!Crop { width: 0, ..crop }.fits(100, 100));
		assert_eq!(EncodeJob { crop: Some(Crop { width: 0, ..crop }), ..Default::default() }.validate(), Err(ValidationError::EmptyCrop));
	}

	/// The job's own wire shape: format, crop and resize at the top, each format's
	/// controls under its own key.
	#[test]
	fn job_json_nests_each_formats_controls() {
		let json = serde_json::to_value(EncodeJob::default()).expect("job serializes");
		assert_eq!(json["format"], "webp");
		assert_eq!(json["resize"]["mode"], "always");
		assert!(json["crop"].is_null());
		assert!((json["webp"]["quality"].as_f64().expect("a number") - 75.0).abs() < f64::EPSILON);
		assert!(json.get("quality").is_none(), "the WebP controls must not also appear flattened: {json}");
		let back: EncodeJob = serde_json::from_value(json).expect("deserialize");
		assert_eq!(back, EncodeJob::default());
	}

	/// A file written before the per-format split stores the flat, moded shape. It must load
	/// with every value intact, and encode as it did.
	#[test]
	fn the_flat_shape_from_before_the_split_still_loads() {
		let legacy = r#"{"mode":"lossy","preset":null,"quality":92,"alphaQuality":80,"alphaFiltering":"fast","method":6,"segments":2,"partitionLimit":10,"sns":30,"passes":3,"filter":"strong","filterStrength":40,"filterSharpness":5,"target":{"kind":"size","value":5000},"sharpYuv":true,"lowMemory":true,"multiThreading":false,"resize":{"width":640,"height":0}}"#;
		let job: EncodeJob = serde_json::from_str(legacy).expect("the legacy shape deserializes");
		assert_eq!(job.resize, Resize::to(640, 0));
		let w = &job.webp;
		assert!((w.quality - 92.0).abs() < f32::EPSILON);
		assert_eq!((w.alpha_quality, w.alpha_filtering, w.method, w.segments, w.partition_limit, w.sns, w.passes), (80, AlphaFiltering::Fast, 6, 2, 10, 30, 3));
		assert_eq!((w.filter_type, w.autofilter, w.filter_strength, w.filter_sharpness), (FilterType::Strong, false, 40, 5));
		assert_eq!(w.target, Some(TargetMetric::Size(5000)));
		assert!(w.sharp_yuv && w.low_memory && !w.multi_threading);

		// And it is rewritten in the current shape, which reads back identically.
		let rewritten = serde_json::to_string(&job).expect("serialize");
		assert_eq!(serde_json::from_str::<EncodeJob>(&rewritten).expect("the current shape deserializes"), job);
	}

	/// The nested but still moded shape, from 0.6 to 0.11.
	#[test]
	fn the_nested_moded_shape_still_loads() {
		let job: EncodeJob = serde_json::from_str(r#"{"format":"webp","resize":{"width":0,"height":0,"noEnlarge":false},"webp":{"mode":"lossless","quality":90}}"#).expect("deserialize");
		assert!(job.webp.lossless && job.webp.exact);
		assert!((job.webp.quality - 90.0).abs() < f32::EPSILON);
	}

	/// When both shapes are present, `webp` wins: the flat keys are only a fallback.
	#[test]
	fn nested_webp_controls_win_over_flat_ones() {
		let job: EncodeJob = serde_json::from_str(r#"{"quality":10,"webp":{"quality":90}}"#).expect("deserialize");
		assert!((job.webp.quality - 90.0).abs() < f32::EPSILON);
	}

	#[test]
	fn a_partial_job_falls_back_to_defaults() {
		let job: EncodeJob = serde_json::from_str(r#"{"webp":{"lossless":true}}"#).expect("deserialize");
		assert!(job.webp.lossless);
		assert_eq!(job.webp.method, WebpSettings::default().method);
		assert!(job.resize.is_noop());
		assert_eq!(serde_json::from_str::<EncodeJob>("{}").expect("an empty object deserializes"), EncodeJob::default());
	}

	/// A job validates the format it will actually write, and only that one: a user with a
	/// half-edited WebP preset can still encode AVIF, and the reverse.
	#[test]
	fn a_job_validates_only_the_format_it_writes() {
		let bad_webp = WebpSettings { method: 9, ..Default::default() };
		assert!(EncodeJob { format: OutputFormat::Webp, webp: bad_webp.clone(), ..Default::default() }.validate().is_err());
		assert_eq!(EncodeJob { format: OutputFormat::Avif, webp: bad_webp, ..Default::default() }.validate(), Ok(()));
	}

	#[test]
	fn formats_name_their_files() {
		assert_eq!((OutputFormat::Webp.extension(), OutputFormat::Webp.mime_type()), ("webp", "image/webp"));
		assert_eq!((OutputFormat::Avif.extension(), OutputFormat::Avif.mime_type()), ("avif", "image/avif"));
		assert_eq!((OutputFormat::Jxl.extension(), OutputFormat::Jxl.mime_type()), ("jxl", "image/jxl"));
		assert_eq!((OutputFormat::Heic.extension(), OutputFormat::Heic.mime_type()), ("heic", "image/heic"));
	}
}
