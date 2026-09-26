//! The one representation of an encode job.
//!
//! An [`EncodeJob`] is what the window sends, what presets and the preferences file
//! store, and what the encoder takes: the output format, the resize (which every format
//! shares), and one settings struct per format. There is one format today, WebP, whose
//! [`WebpSettings`] mirror field-for-field what the Electron UI collects in `index.html`
//! and what `main.js` turns into a `cwebp` command line. Ranges and defaults are the
//! Electron `<input>` attributes, not invented ones, so a user of the old app finds the
//! same numbers in the new one.
//!
//! The split exists so a second format can be added without a UI that shows WebP's
//! `sns` for an encoder that has no such thing (ROADMAP.md Phase 7). Each format's
//! settings stay live while the user tries another, which is why they are sibling
//! fields rather than an enum.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The encoding mode, one of the five radio buttons in the Electron UI's mode group.
///
/// The mode is not just a flag: it decides which other controls apply at all. The
/// Electron UI shows the lossy option groups only for [`Mode::Lossy`], and
/// [`Mode::NearLossless`] ignores every control except the quality slider (which it
/// reinterprets as the near-lossless level). That behaviour is real and is reproduced
/// by the encoder, not merely by hiding UI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
	/// Ordinary lossy VP8 encoding. The Electron UI's default, and the only mode whose
	/// advanced option groups are reachable.
	#[default]
	Lossy,
	/// Lossless VP8L. The Electron app pairs this with `-exact`, preserving RGB under
	/// fully transparent pixels, so the port does too.
	Lossless,
	/// Near-lossless preprocessing, whose level comes from the quality slider.
	NearLossless,
	/// Remap the lossy parameters to approximate JPEG's size/quality curve (`-jpeg_like`).
	JpegLike,
	/// Use one of libwebp's named parameter presets; see [`Preset`].
	Preset,
}

impl Mode {
	/// Whether this mode reaches the advanced lossy controls (segments, SNS, filtering,
	/// target size/PSNR, passes, partition limit, sharp YUV, low memory).
	///
	/// Only [`Mode::Lossy`] does. The Electron UI enforces this by sending those values
	/// over IPC for the lossy mode alone.
	#[must_use]
	pub const fn uses_lossy_options(self) -> bool {
		matches!(self, Self::Lossy)
	}
}

/// A named libwebp parameter preset (`cwebp -preset`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Preset {
	/// Balanced default.
	#[default]
	Default,
	/// Digital photography, outdoor and natural lighting.
	Photo,
	/// Indoor portrait photography, softer detail.
	Picture,
	/// Hand- or line-drawn artwork with high contrast edges.
	Drawing,
	/// Small, colourful images.
	Icon,
	/// Text-like images.
	Text,
}

impl Preset {
	/// The string `cwebp -preset` expects.
	#[must_use]
	pub const fn as_cwebp_str(self) -> &'static str {
		match self {
			Self::Default => "default",
			Self::Photo => "photo",
			Self::Picture => "picture",
			Self::Drawing => "drawing",
			Self::Icon => "icon",
			Self::Text => "text",
		}
	}
}

/// The in-loop deblocking filter selection.
///
/// This maps to two separate libwebp concepts that `cwebp` spells as three flags:
/// [`FilterType::Auto`] is `-af` (let the encoder pick the strength), while
/// [`FilterType::Simple`] and [`FilterType::Strong`] are `-nostrong` / `-strong` and
/// hand the strength and sharpness controls to the user.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterType {
	/// Auto-adjust the filter strength (`-af`).
	#[default]
	Auto,
	/// The simple filter (`-nostrong`).
	Simple,
	/// The strong filter (`-strong`).
	Strong,
}

impl FilterType {
	/// Whether the user's filter strength and sharpness values apply.
	///
	/// They do not under [`FilterType::Auto`]: the Electron UI disables both sliders and
	/// does not send them, because the encoder is choosing the strength itself.
	#[must_use]
	pub const fn uses_manual_strength(self) -> bool {
		matches!(self, Self::Simple | Self::Strong)
	}
}

/// The predictive filtering method for the alpha plane (`-alpha_filter`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlphaFiltering {
	/// No alpha filtering (`none`).
	Off,
	/// libwebp's own default (`fast`).
	Fast,
	/// Try every predictor and keep the best (`best`). What the Electron app asks for.
	Best,
}

impl AlphaFiltering {
	/// The string `cwebp -alpha_filter` expects.
	#[must_use]
	pub const fn as_cwebp_str(self) -> &'static str {
		match self {
			Self::Off => "none",
			Self::Fast => "fast",
			Self::Best => "best",
		}
	}
}

/// A size- or distortion-targeted encode, which overrides the quality setting.
///
/// These are mutually exclusive in libwebp and in the Electron UI, which offers them as
/// two radio buttons, so they are one enum rather than two optional fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum TargetMetric {
	/// Aim for this output size in bytes (`-size`).
	Size(u32),
	/// Aim for at least this PSNR in dB (`-psnr`).
	Psnr(u32),
}

/// An output resize, applied before encoding.
///
/// `0` means "derive from the other dimension, preserving aspect ratio", which is
/// `cwebp -resize`'s own convention. Both zero means no resize at all, and in that case
/// no `-resize` is emitted — matching the Electron UI, which only sends the resize when
/// at least one dimension is non-zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resize {
	/// Target width in pixels, or `0` to derive it from the height.
	pub width: u32,
	/// Target height in pixels, or `0` to derive it from the width.
	pub height: u32,
}

impl Resize {
	/// Whether this resize does anything. Both dimensions zero means "leave it alone".
	#[must_use]
	pub const fn is_noop(self) -> bool {
		self.width == 0 && self.height == 0
	}
}

/// Every WebP control the GUI exposes: the `cwebp` surface less the resize, which
/// belongs to the [`EncodeJob`] because every output format shares it.
///
/// [`WebpSettings::default`] is the Electron UI's own load-time state, so a
/// default-constructed value encodes what the old app encodes when the user touches
/// nothing but the file pickers — with one deliberate exception, documented on
/// [`WebpSettings::target`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WebpSettings {
	/// Which of the five modes to encode in.
	pub mode: Mode,
	/// The named preset, required when `mode` is [`Mode::Preset`] and ignored otherwise.
	///
	/// Optional rather than defaulted because the Electron UI's preset `<select>` starts
	/// on a "Select a Preset" placeholder and refuses to convert until the user picks a
	/// real one; `None` is that placeholder state.
	pub preset: Option<Preset>,
	/// Quality, `0..=100`. For [`Mode::Lossless`] this is a compression *effort* knob
	/// rather than a fidelity one, and for [`Mode::NearLossless`] it is the
	/// near-lossless level.
	pub quality: u8,
	/// Alpha channel quality, `0..=100`.
	pub alpha_quality: u8,
	/// Predictive filtering for the alpha plane, or `None` to leave libwebp's default.
	///
	/// The Electron app asks for [`AlphaFiltering::Best`] on every encode except a
	/// preset one, where it deliberately emits nothing so the preset's own choice stands.
	pub alpha_filtering: Option<AlphaFiltering>,
	/// Compression method, `0..=6`: the quality/speed trade-off (`-m`).
	pub method: u8,
	/// Maximum number of segments, `1..=4`.
	pub segments: u8,
	/// Quality degradation allowed to fit the 512k prediction-mode limit, `0..=100`.
	pub partition_limit: u8,
	/// Spatial noise shaping strength, `0..=100`.
	pub sns: u8,
	/// Entropy-analysis passes, `1..=10`.
	pub passes: u8,
	/// Which deblocking filter to use.
	pub filter: FilterType,
	/// Deblocking filter strength, `0..=100`. Applies only when
	/// [`FilterType::uses_manual_strength`].
	pub filter_strength: u8,
	/// Deblocking filter sharpness, `0..=7`, where `0` is sharpest. Applies only when
	/// [`FilterType::uses_manual_strength`].
	pub filter_sharpness: u8,
	/// A size or PSNR target that overrides `quality`, or `None` to encode to `quality`.
	///
	/// **This defaults to `None`, which is a deliberate divergence from the Electron
	/// app.** That app defaults its target-size radio to selected and its size field to
	/// `1`, so it appends `-size 1` to every lossy encode the user does not explicitly
	/// reconfigure, overriding the quality slider and producing a minimal file. That is a
	/// bug, not a control, and it is not carried across. The *control* survives; its
	/// broken default does not.
	pub target: Option<TargetMetric>,
	/// Use the sharper, slower RGB to YUV conversion (`-sharp_yuv`).
	pub sharp_yuv: bool,
	/// Trade CPU for a smaller memory footprint (`-low_memory`).
	pub low_memory: bool,
	/// Encode multi-threaded (`-mt`). On by default, as the Electron app hardcodes it.
	pub multi_threading: bool,
}

impl Default for WebpSettings {
	fn default() -> Self {
		Self { mode: Mode::Lossy, preset: None, quality: 75, alpha_quality: 100, alpha_filtering: Some(AlphaFiltering::Best), method: 4, segments: 4, partition_limit: 0, sns: 50, passes: 6, filter: FilterType::Auto, filter_strength: 20, filter_sharpness: 0, target: None, sharp_yuv: false, low_memory: false, multi_threading: true }
	}
}

/// Which format to write.
///
/// One variant today. It exists now, before a second format does, so that the settings
/// files and the wire format already carry it and adding AVIF does not change their shape
/// a second time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputFormat {
	/// WebP, through libwebp.
	#[default]
	Webp,
}

/// A complete encode job: the output format, the shared resize, and each format's own
/// settings.
///
/// This is the type that crosses the IPC boundary and is written to disk by presets and
/// the preferences file. It reads **both** its own shape and the flat shape that
/// `EncodeSettings` had before this split — `{ "mode": ..., "quality": ..., "resize": ... }`
/// — so a preset or preferences file saved by an earlier version still loads with every
/// value intact rather than silently falling back to the defaults. It always writes its
/// own shape.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodeJob {
	/// Which format to write.
	pub format: OutputFormat,
	/// Resize before encoding. Shared by every format.
	pub resize: Resize,
	/// The WebP controls.
	pub webp: WebpSettings,
}

impl From<WebpSettings> for EncodeJob {
	/// A WebP job with no resize.
	fn from(webp: WebpSettings) -> Self {
		Self { webp, ..Self::default() }
	}
}

impl<'de> Deserialize<'de> for EncodeJob {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		/// Both shapes at once. `resize` is top-level in each, so it needs no special
		/// case; the WebP controls are either under `webp` (current) or flattened at the
		/// top level (before the split). `webp` wins when present.
		#[derive(Deserialize)]
		#[serde(rename_all = "camelCase")]
		struct Wire {
			#[serde(default)]
			format: OutputFormat,
			#[serde(default)]
			resize: Resize,
			webp: Option<WebpSettings>,
			#[serde(flatten)]
			flat: WebpSettings,
		}

		let wire = Wire::deserialize(deserializer)?;
		Ok(Self { format: wire.format, resize: wire.resize, webp: wire.webp.unwrap_or(wire.flat) })
	}
}

impl EncodeJob {
	/// Check the settings of the format this job writes.
	///
	/// # Errors
	///
	/// Returns the first [`ValidationError`] found.
	pub fn validate(&self) -> Result<(), ValidationError> {
		match self.format {
			OutputFormat::Webp => self.webp.validate(),
		}
	}
}

/// Why a set of [`WebpSettings`] cannot be encoded.
///
/// These are the errors the Electron UI shows, plus the range checks its `<input>`
/// attributes enforce in the browser and nothing enforces over IPC.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
	/// A numeric control is outside the range libwebp accepts.
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
	/// The mode is [`Mode::Preset`] but no preset was chosen.
	#[error("mode is `preset` but no preset was selected")]
	MissingPreset,
	/// A target size of zero bytes cannot be met.
	#[error("target size must be at least 1 byte")]
	ZeroTargetSize,
}

impl WebpSettings {
	/// Check every control against the range libwebp accepts.
	///
	/// Ranges are validated for all modes, not only the mode that reads them, so that a
	/// settings value round-tripped through IPC or a saved preset is never silently
	/// out of range once the user switches modes.
	///
	/// # Errors
	///
	/// Returns the first [`ValidationError`] found, checking in field order.
	pub fn validate(&self) -> Result<(), ValidationError> {
		fn range(field: &'static str, value: u8, min: u8, max: u8) -> Result<(), ValidationError> {
			if value < min || value > max {
				return Err(ValidationError::OutOfRange { field, value: u32::from(value), min: u32::from(min), max: u32::from(max) });
			}
			Ok(())
		}

		if self.mode == Mode::Preset && self.preset.is_none() {
			return Err(ValidationError::MissingPreset);
		}

		range("quality", self.quality, 0, 100)?;
		range("alpha_quality", self.alpha_quality, 0, 100)?;
		range("method", self.method, 0, 6)?;
		range("segments", self.segments, 1, 4)?;
		range("partition_limit", self.partition_limit, 0, 100)?;
		range("sns", self.sns, 0, 100)?;
		range("passes", self.passes, 1, 10)?;
		range("filter_strength", self.filter_strength, 0, 100)?;
		range("filter_sharpness", self.filter_sharpness, 0, 7)?;

		match self.target {
			Some(TargetMetric::Size(0)) => return Err(ValidationError::ZeroTargetSize),
			Some(TargetMetric::Psnr(psnr)) if !(1..=10_000).contains(&psnr) => {
				return Err(ValidationError::OutOfRange { field: "target", value: psnr, min: 1, max: 10_000 });
			}
			_ => {}
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The defaults are the Electron UI's `<input>` attributes. If one of these numbers
	/// changes, a user of the old app gets a different file out of the new one for the
	/// same clicks, so they are pinned here rather than left to drift.
	#[test]
	fn defaults_match_the_electron_ui() {
		let s = WebpSettings::default();
		assert_eq!(s.mode, Mode::Lossy);
		assert_eq!(s.quality, 75);
		assert_eq!(s.alpha_quality, 100);
		assert_eq!(s.method, 4);
		assert_eq!(s.segments, 4);
		assert_eq!(s.partition_limit, 0);
		assert_eq!(s.sns, 50);
		assert_eq!(s.passes, 6);
		assert_eq!(s.filter, FilterType::Auto);
		assert_eq!(s.filter_strength, 20);
		assert_eq!(s.filter_sharpness, 0);
		assert_eq!(s.alpha_filtering, Some(AlphaFiltering::Best));
		assert!(s.multi_threading, "the Electron app hardcodes -mt on every encode");
		assert!(EncodeJob::default().resize.is_noop());
		assert_eq!(EncodeJob::default().format, OutputFormat::Webp);
	}

	/// The one place the port deliberately disagrees with the Electron app.
	#[test]
	fn default_has_no_size_target() {
		assert_eq!(WebpSettings::default().target, None, "the Electron app's `-size 1` default is a bug and is not reproduced");
	}

	#[test]
	fn default_settings_validate() {
		assert_eq!(WebpSettings::default().validate(), Ok(()));
	}

	#[test]
	fn preset_mode_needs_a_preset() {
		let mut s = WebpSettings { mode: Mode::Preset, ..Default::default() };
		assert_eq!(s.validate(), Err(ValidationError::MissingPreset));
		s.preset = Some(Preset::Photo);
		assert_eq!(s.validate(), Ok(()));
	}

	#[test]
	fn out_of_range_controls_are_rejected() {
		let cases: Vec<(WebpSettings, &'static str, u32, u32, u32)> = vec![(WebpSettings { quality: 101, ..Default::default() }, "quality", 101, 0, 100), (WebpSettings { alpha_quality: 101, ..Default::default() }, "alpha_quality", 101, 0, 100), (WebpSettings { method: 7, ..Default::default() }, "method", 7, 0, 6), (WebpSettings { segments: 0, ..Default::default() }, "segments", 0, 1, 4), (WebpSettings { segments: 5, ..Default::default() }, "segments", 5, 1, 4), (WebpSettings { partition_limit: 101, ..Default::default() }, "partition_limit", 101, 0, 100), (WebpSettings { sns: 101, ..Default::default() }, "sns", 101, 0, 100), (WebpSettings { passes: 0, ..Default::default() }, "passes", 0, 1, 10), (WebpSettings { passes: 11, ..Default::default() }, "passes", 11, 1, 10), (WebpSettings { filter_strength: 101, ..Default::default() }, "filter_strength", 101, 0, 100), (WebpSettings { filter_sharpness: 8, ..Default::default() }, "filter_sharpness", 8, 0, 7)];
		for (settings, field, value, min, max) in cases {
			assert_eq!(settings.validate(), Err(ValidationError::OutOfRange { field, value, min, max }), "expected {field} = {value} to be rejected");
		}
	}

	#[test]
	fn boundary_values_are_accepted() {
		let extremes = WebpSettings { quality: 100, alpha_quality: 0, method: 6, segments: 1, partition_limit: 100, sns: 100, passes: 10, filter_strength: 100, filter_sharpness: 7, ..Default::default() };
		assert_eq!(extremes.validate(), Ok(()));
		let floors = WebpSettings { quality: 0, method: 0, segments: 4, partition_limit: 0, sns: 0, passes: 1, filter_strength: 0, filter_sharpness: 0, ..Default::default() };
		assert_eq!(floors.validate(), Ok(()));
	}

	#[test]
	fn target_bounds_are_checked() {
		assert_eq!(WebpSettings { target: Some(TargetMetric::Size(0)), ..Default::default() }.validate(), Err(ValidationError::ZeroTargetSize));
		assert_eq!(WebpSettings { target: Some(TargetMetric::Psnr(0)), ..Default::default() }.validate(), Err(ValidationError::OutOfRange { field: "target", value: 0, min: 1, max: 10_000 }));
		assert_eq!(WebpSettings { target: Some(TargetMetric::Psnr(10_001)), ..Default::default() }.validate(), Err(ValidationError::OutOfRange { field: "target", value: 10_001, min: 1, max: 10_000 }));
		assert_eq!(WebpSettings { target: Some(TargetMetric::Size(1)), ..Default::default() }.validate(), Ok(()));
		assert_eq!(WebpSettings { target: Some(TargetMetric::Psnr(10_000)), ..Default::default() }.validate(), Ok(()));
	}

	#[test]
	fn only_lossy_mode_reaches_the_advanced_controls() {
		assert!(Mode::Lossy.uses_lossy_options());
		for mode in [Mode::Lossless, Mode::NearLossless, Mode::JpegLike, Mode::Preset] {
			assert!(!mode.uses_lossy_options(), "{mode:?} must not reach the lossy option groups");
		}
	}

	#[test]
	fn auto_filter_hides_the_manual_strength_controls() {
		assert!(!FilterType::Auto.uses_manual_strength());
		assert!(FilterType::Simple.uses_manual_strength());
		assert!(FilterType::Strong.uses_manual_strength());
	}

	/// The settings cross the Tauri IPC boundary as JSON, so the wire shape is part of
	/// the contract with the frontend and is pinned here.
	#[test]
	fn round_trips_through_json() {
		let settings = EncodeJob { resize: Resize { width: 800, height: 0 }, webp: WebpSettings { mode: Mode::NearLossless, preset: Some(Preset::Drawing), target: Some(TargetMetric::Psnr(42)), sharp_yuv: true, ..Default::default() }, ..Default::default() };
		let json = serde_json::to_string(&settings).expect("settings serialize");
		assert_eq!(serde_json::from_str::<EncodeJob>(&json).expect("settings deserialize"), settings);
	}

	#[test]
	fn json_uses_camel_case_field_names() {
		let json = serde_json::to_value(WebpSettings::default()).expect("settings serialize");
		let object = json.as_object().expect("settings serialize to an object");
		for key in ["mode", "alphaQuality", "alphaFiltering", "partitionLimit", "filterStrength", "filterSharpness", "sharpYuv", "lowMemory", "multiThreading"] {
			assert!(object.contains_key(key), "missing `{key}` in {json}");
		}
	}

	/// A frontend that omits a field must get the default rather than a deserialize error.
	#[test]
	fn partial_json_falls_back_to_defaults() {
		let settings: WebpSettings = serde_json::from_str(r#"{"mode":"lossless","quality":90}"#).expect("partial settings deserialize");
		assert_eq!(settings.mode, Mode::Lossless);
		assert_eq!(settings.quality, 90);
		assert_eq!(settings.method, WebpSettings::default().method);
	}

	#[test]
	fn preset_and_alpha_filter_strings_are_what_cwebp_expects() {
		assert_eq!(Preset::Default.as_cwebp_str(), "default");
		assert_eq!(Preset::Photo.as_cwebp_str(), "photo");
		assert_eq!(Preset::Picture.as_cwebp_str(), "picture");
		assert_eq!(Preset::Drawing.as_cwebp_str(), "drawing");
		assert_eq!(Preset::Icon.as_cwebp_str(), "icon");
		assert_eq!(Preset::Text.as_cwebp_str(), "text");
		assert_eq!(AlphaFiltering::Off.as_cwebp_str(), "none");
		assert_eq!(AlphaFiltering::Fast.as_cwebp_str(), "fast");
		assert_eq!(AlphaFiltering::Best.as_cwebp_str(), "best");
	}

	#[test]
	fn resize_noop_detection() {
		assert!(Resize { width: 0, height: 0 }.is_noop());
		assert!(!Resize { width: 800, height: 0 }.is_noop());
		assert!(!Resize { width: 0, height: 600 }.is_noop());
	}

	/// The job's own wire shape, which the frontend now sends and the settings files now
	/// store: format and resize at the top, the WebP controls under `webp`.
	#[test]
	fn job_json_nests_the_webp_controls() {
		let json = serde_json::to_value(EncodeJob::default()).expect("job serializes");
		assert_eq!(json["format"], "webp");
		assert_eq!(json["resize"]["width"], 0);
		assert_eq!(json["webp"]["quality"], 75);
		assert!(json.get("quality").is_none(), "the WebP controls must not also appear flattened: {json}");
	}

	/// A preset or preferences file written before the split stores the old flat
	/// `EncodeSettings` shape. It must load with every value intact — falling back to the
	/// defaults would silently discard a user's saved settings.
	#[test]
	fn the_flat_shape_from_before_the_split_still_loads() {
		let legacy = r#"{"mode":"lossless","preset":null,"quality":92,"alphaQuality":80,"alphaFiltering":"fast","method":6,"segments":2,"partitionLimit":10,"sns":30,"passes":3,"filter":"strong","filterStrength":40,"filterSharpness":5,"target":{"kind":"size","value":5000},"sharpYuv":true,"lowMemory":true,"multiThreading":false,"resize":{"width":640,"height":0}}"#;
		let job: EncodeJob = serde_json::from_str(legacy).expect("the legacy shape deserializes");
		let expected = EncodeJob { format: OutputFormat::Webp, resize: Resize { width: 640, height: 0 }, webp: WebpSettings { mode: Mode::Lossless, preset: None, quality: 92, alpha_quality: 80, alpha_filtering: Some(AlphaFiltering::Fast), method: 6, segments: 2, partition_limit: 10, sns: 30, passes: 3, filter: FilterType::Strong, filter_strength: 40, filter_sharpness: 5, target: Some(TargetMetric::Size(5000)), sharp_yuv: true, low_memory: true, multi_threading: false } };
		assert_eq!(job, expected);

		// And it is rewritten in the current shape, which reads back identically.
		let rewritten = serde_json::to_string(&job).expect("serialize");
		assert_eq!(serde_json::from_str::<EncodeJob>(&rewritten).expect("the current shape deserializes"), expected);
	}

	/// When both shapes are present, `webp` wins: the flat keys are only a fallback.
	#[test]
	fn nested_webp_controls_win_over_flat_ones() {
		let job: EncodeJob = serde_json::from_str(r#"{"quality":10,"webp":{"quality":90}}"#).expect("deserialize");
		assert_eq!(job.webp.quality, 90);
	}

	#[test]
	fn a_partial_job_falls_back_to_defaults() {
		let job: EncodeJob = serde_json::from_str(r#"{"webp":{"mode":"lossless"}}"#).expect("deserialize");
		assert_eq!(job.webp.mode, Mode::Lossless);
		assert_eq!(job.webp.quality, WebpSettings::default().quality);
		assert!(job.resize.is_noop());
		assert_eq!(serde_json::from_str::<EncodeJob>("{}").expect("an empty object deserializes"), EncodeJob::default());
	}

	#[test]
	fn a_job_validates_its_formats_settings() {
		assert_eq!(EncodeJob::default().validate(), Ok(()));
		assert_eq!(EncodeJob::from(WebpSettings { mode: Mode::Preset, ..Default::default() }).validate(), Err(ValidationError::MissingPreset));
	}
}
