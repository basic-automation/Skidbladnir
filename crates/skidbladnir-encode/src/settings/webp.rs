//! The WebP controls: every `cwebp` option that changes the file it writes.
//!
//! # One field per flag, not modes
//!
//! Until 0.14 these were grouped into five modes — lossy, lossless, near-lossless,
//! JPEG-like and preset — which is how the Electron app offered them. The grouping made
//! `cwebp` results unreachable: `-near_lossless 60 -q 90 -m 6` (the near-lossless level
//! was the quality slider, so effort and method could not be set), `-lossless` without
//! `-exact`, `-jpeg_like -sns 30`, `-preset photo -f 40`, anything in lossless that is not
//! the defaults for alpha. `cwebp` has no modes: every flag is a field of libwebp's
//! `WebPConfig`, or an instruction about the picture, and they combine freely.
//!
//! So this is now that surface, field for field. [`WebpSettings`] carries each `WebPConfig`
//! field `cwebp` can set, with the range `WebPValidateConfig` accepts, and the picture
//! options (`-noalpha`, `-blend_alpha`, `-metadata`). A `cwebp` command line maps onto
//! exactly one value of it, and the parity test proves the reverse: the same value, sent
//! through [`crate::cwebp_args`], makes the real `cwebp` write the same bytes.
//!
//! Two `cwebp` flags are *not* fields, because they are not settings but shorthands:
//! `-preset` re-initialises the whole config to one of libwebp's six presets, and `-z`
//! replaces the method and quality with one of ten lossless levels. Both are
//! [`WebpSettings::apply_preset`] and [`WebpSettings::apply_lossless_preset`]: the window
//! offers them as buttons that set the controls, which is what they do on the command line
//! too. Everything they can produce is therefore reachable through the fields.
//!
//! Settings saved before 0.14 still load: see [`super::legacy`].

use serde::{Deserialize, Serialize};

use super::ValidationError;

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

/// The in-loop deblocking filter type (`-strong` / `-nostrong`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterType {
	/// The simple filter (`-nostrong`): luma only.
	Simple,
	/// The strong filter (`-strong`): luma and chroma. libwebp's default.
	#[default]
	Strong,
}

/// The predictive filtering method for the alpha plane (`-alpha_filter`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlphaFiltering {
	/// No alpha filtering (`none`).
	Off,
	/// libwebp's own default (`fast`).
	#[default]
	Fast,
	/// Try every predictor and keep the best (`best`).
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

	/// The `WebPConfig::alpha_filtering` value.
	#[must_use]
	pub const fn as_config(self) -> i32 {
		match self {
			Self::Off => 0,
			Self::Fast => 1,
			Self::Best => 2,
		}
	}
}

/// A hint about the kind of image, which steers the lossless encoder (`-hint`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageHint {
	/// No hint: libwebp's default.
	#[default]
	Default,
	/// Digital picture, like a portrait or an inner shot (`picture`).
	Picture,
	/// Outdoor photograph, with natural lighting (`photo`).
	Photo,
	/// Discrete tone image, such as a graph or map (`graph`).
	Graph,
}

impl ImageHint {
	/// The string `cwebp -hint` expects, or `None` for the default, which has no spelling.
	#[must_use]
	pub const fn as_cwebp_str(self) -> Option<&'static str> {
		match self {
			Self::Default => None,
			Self::Picture => Some("picture"),
			Self::Photo => Some("photo"),
			Self::Graph => Some("graph"),
		}
	}

	/// The `WebPImageHint` value.
	#[must_use]
	pub const fn as_config(self) -> i32 {
		match self {
			Self::Default => 0,
			Self::Picture => 1,
			Self::Photo => 2,
			Self::Graph => 3,
		}
	}
}

/// A size- or distortion-targeted encode, which overrides the quality setting.
///
/// libwebp takes both fields, but gives `target_size` precedence, so a job carries at most
/// one and the window offers them as one choice.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum TargetMetric {
	/// Aim for this output size in bytes (`-size`).
	Size(u32),
	/// Aim for at least this PSNR in dB (`-psnr`). Fractional, as `cwebp` reads it.
	Psnr(f32),
}

/// Which metadata to copy from the source into the WebP (`-metadata`).
///
/// `cwebp` copies none by default, so neither does this; turning one on copies it when the
/// source has it, and does nothing when it does not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WebpMetadata {
	/// Exif (`exif`).
	pub exif: bool,
	/// The ICC colour profile (`icc`).
	pub icc: bool,
	/// XMP (`xmp`).
	pub xmp: bool,
}

impl WebpMetadata {
	/// Whether any metadata is to be copied.
	#[must_use]
	pub const fn any(self) -> bool {
		self.exif || self.icc || self.xmp
	}
}

/// The animation encoder's own options, from `img2webp` and `gif2webp`. They apply only
/// when the source is an animation (an animated WebP, or any GIF), and change nothing for a
/// still image.
///
/// Every option left at its default is the tool's own default, so an untouched job still
/// matches `img2webp` (animated WebP) and `gif2webp` (GIF) exactly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WebpAnimation {
	/// Search harder for the smallest file; slower, and spaces no keyframes (`-min_size`).
	pub minimize_size: bool,
	/// Encode each frame lossy or lossless, whichever is smaller (`-mixed`).
	pub allow_mixed: bool,
	/// Minimum distance between keyframes (`-kmin`); `None` is the tool's default. Any
	/// integer, as the tools take it: libwebp corrects an inconsistent pair itself.
	pub kmin: Option<i32>,
	/// Maximum distance between keyframes (`-kmax`); `None` is the tool's default, `0`
	/// places no keyframes, `1` makes every frame one.
	pub kmax: Option<i32>,
	/// How many times the animation plays, `0` for forever (`img2webp -loop`); `None` keeps
	/// the source's.
	pub loop_count: Option<u16>,
	/// Read a GIF's loop count as Chrome once did — repeats as plays, and no loop extension
	/// as forever (`gif2webp -loop_compatibility`). GIF input only.
	pub loop_compatibility: bool,
}

/// Every WebP control: the `cwebp` surface less the crop and resize, which belong to the
/// [`super::EncodeJob`] because every output format shares them.
///
/// Field names follow `WebPConfig`, and the doc comment of each names the `cwebp` flag.
/// [`WebpSettings::default`] reproduces what the app encoded by default before 0.14, so an
/// untouched window still writes the same file it always did.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[expect(clippy::struct_excessive_bools, reason = "each bool is one of cwebp's on/off flags; the struct mirrors its command line field for field")]
pub struct WebpSettings {
	/// Encode losslessly (`-lossless`).
	pub lossless: bool,
	/// Near-lossless preprocessing level, `0..=100`, where `100` is off (`-near_lossless`).
	/// Applies to lossless encodes only; `cwebp -near_lossless` also turns lossless on.
	pub near_lossless: u8,
	/// Keep the RGB values under fully transparent pixels (`-exact`).
	pub exact: bool,
	/// Quality, `0..=100`, fractional as `cwebp -q` reads it. For a lossless encode this is
	/// compression effort rather than fidelity.
	pub quality: f32,
	/// Alpha channel quality, `0..=100` (`-alpha_q`).
	pub alpha_quality: u8,
	/// Compress the alpha plane losslessly (`-alpha_method 1`, the default) or not at all
	/// (`-alpha_method 0`).
	pub alpha_compression: bool,
	/// Predictive filtering for the alpha plane (`-alpha_filter`).
	pub alpha_filtering: AlphaFiltering,
	/// Compression method, `0..=6`: the quality/speed trade-off (`-m`).
	pub method: u8,
	/// What kind of image this is, for the lossless encoder (`-hint`).
	pub image_hint: ImageHint,
	/// A size or PSNR target that overrides `quality` (`-size` / `-psnr`).
	pub target: Option<TargetMetric>,
	/// Maximum number of segments, `1..=4` (`-segments`).
	pub segments: u8,
	/// Spatial noise shaping strength, `0..=100` (`-sns`).
	pub sns: u8,
	/// Deblocking filter strength, `0..=100` (`-f`). Replaced per segment when
	/// [`WebpSettings::autofilter`] is on.
	pub filter_strength: u8,
	/// Deblocking filter sharpness, `0..=7`, where `0` is sharpest (`-sharpness`).
	pub filter_sharpness: u8,
	/// The deblocking filter type (`-strong` / `-nostrong`).
	pub filter_type: FilterType,
	/// Let the encoder choose the filter strength (`-af`).
	pub autofilter: bool,
	/// Entropy-analysis passes, `1..=10` (`-pass`).
	pub passes: u8,
	/// Lowest quantizer quality the encoder may use, `0..=100` (`-qrange`, first value).
	pub qmin: u8,
	/// Highest quantizer quality the encoder may use, `0..=100` (`-qrange`, second value).
	pub qmax: u8,
	/// Preprocessing filter bits, `0..=7` (`-pre`): 1 smooths segments, 2 dithers
	/// pseudo-randomly, 4 uses the sharp RGB to YUV conversion.
	pub preprocessing: u8,
	/// Quality degradation allowed to fit the 512k prediction-mode limit, `0..=100`
	/// (`-partition_limit`).
	pub partition_limit: u8,
	/// Remap the lossy parameters to approximate JPEG's size/quality curve (`-jpeg_like`).
	pub jpeg_like: bool,
	/// Use the sharper, slower RGB to YUV conversion (`-sharp_yuv`).
	pub sharp_yuv: bool,
	/// Trade CPU for a smaller memory footprint (`-low_memory`).
	pub low_memory: bool,
	/// Encode multi-threaded (`-mt`).
	pub multi_threading: bool,
	/// Keep the alpha channel. Off discards it (`-noalpha`).
	pub keep_alpha: bool,
	/// Flatten the image onto this background colour, `0xRRGGBB`, before encoding
	/// (`-blend_alpha`). The result is opaque.
	pub blend_alpha: Option<u32>,
	/// Which of the source's metadata to copy (`-metadata`).
	pub metadata: WebpMetadata,
	/// The animation encoder's options, for an animated source.
	pub animation: WebpAnimation,
}

impl Default for WebpSettings {
	/// The app's default encode since the Electron app: `-af -mt -alpha_filter best -q 75
	/// -alpha_q 100 -m 4 -segments 4 -partition_limit 0 -pass 6 -sns 50`, with every flag it
	/// does not mention at libwebp's own default.
	fn default() -> Self {
		Self::libwebp_defaults().with_app_defaults()
	}
}

impl WebpSettings {
	/// libwebp's `WebPConfigInit` values with the picture options at `cwebp`'s defaults:
	/// what `cwebp` encodes when given nothing but `-q 75`.
	#[must_use]
	pub const fn libwebp_defaults() -> Self {
		Self { lossless: false, near_lossless: 100, exact: false, quality: 75.0, alpha_quality: 100, alpha_compression: true, alpha_filtering: AlphaFiltering::Fast, method: 4, image_hint: ImageHint::Default, target: None, segments: 4, sns: 50, filter_strength: 60, filter_sharpness: 0, filter_type: FilterType::Strong, autofilter: false, passes: 1, qmin: 0, qmax: 100, preprocessing: 0, partition_limit: 0, jpeg_like: false, sharp_yuv: false, low_memory: false, multi_threading: false, keep_alpha: true, blend_alpha: None, metadata: WebpMetadata { exif: false, icc: false, xmp: false }, animation: WebpAnimation { minimize_size: false, allow_mixed: false, kmin: None, kmax: None, loop_count: None, loop_compatibility: false } }
	}

	/// Start again from what `cwebp` encodes when given no options at all —
	/// [`Self::libwebp_defaults`] — keeping only multi-threading, which changes how fast the
	/// file is written but not a byte of it.
	pub fn reset_to_cwebp_defaults(&mut self) {
		*self = Self { multi_threading: self.multi_threading, ..Self::libwebp_defaults() };
	}

	/// The Electron app's choices on top of libwebp's defaults.
	const fn with_app_defaults(self) -> Self {
		Self { autofilter: true, multi_threading: true, alpha_filtering: AlphaFiltering::Best, passes: 6, ..self }
	}

	/// Re-initialise every encoder field to a libwebp preset, keeping the quality — exactly
	/// what `cwebp -preset` does at the point it appears on the command line. The picture
	/// options (alpha, blending, metadata) and the animation encoder's are not part of the
	/// encoder config and are left alone, as is multi-threading, which changes how fast the file is written but not a
	/// byte of it.
	pub fn apply_preset(&mut self, preset: Preset) {
		let kept = (self.quality, self.multi_threading, self.keep_alpha, self.blend_alpha, self.metadata, self.animation);
		*self = Self::libwebp_defaults();
		(self.quality, self.multi_threading, self.keep_alpha, self.blend_alpha, self.metadata, self.animation) = kept;
		// config_enc.c, WebPConfigInitInternal's preset switch.
		match preset {
			Preset::Default => {}
			Preset::Picture => (self.sns, self.filter_sharpness, self.filter_strength) = (80, 4, 35),
			Preset::Photo => (self.sns, self.filter_sharpness, self.filter_strength, self.preprocessing) = (80, 3, 30, 2),
			Preset::Drawing => (self.sns, self.filter_sharpness, self.filter_strength) = (25, 6, 10),
			Preset::Icon => (self.sns, self.filter_strength) = (0, 0),
			Preset::Text => (self.sns, self.filter_strength, self.segments) = (0, 0, 2),
		}
	}

	/// Apply one of libwebp's ten lossless levels, `0..=9` (`cwebp -z`): lossless on, and
	/// the method and quality that level stands for. `0` is fastest, `9` slowest and
	/// smallest; `cwebp`'s own default is `6`.
	///
	/// # Errors
	///
	/// Returns [`ValidationError::OutOfRange`] for a level above 9.
	pub fn apply_lossless_preset(&mut self, level: u8) -> Result<(), ValidationError> {
		// config_enc.c, kLosslessPresets.
		const LEVELS: [(u8, f32); 10] = [(0, 0.0), (1, 20.0), (2, 25.0), (3, 30.0), (3, 50.0), (4, 50.0), (4, 75.0), (4, 90.0), (5, 90.0), (6, 100.0)];
		let &(method, quality) = LEVELS.get(usize::from(level)).ok_or(ValidationError::OutOfRange { field: "webp.lossless_preset", value: u32::from(level), min: 0, max: 9 })?;
		(self.lossless, self.method, self.quality) = (true, method, quality);
		Ok(())
	}

	/// Check every control against the range `WebPValidateConfig` and `cwebp` accept.
	///
	/// Every range is checked whatever else is set, so a value round-tripped through IPC or
	/// a saved preset is never silently out of range once another control changes.
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

		range("near_lossless", self.near_lossless, 0, 100)?;
		if !(0.0..=100.0).contains(&self.quality) {
			return Err(ValidationError::OutOfRangeFloat { field: "quality", value: self.quality, min: 0.0, max: 100.0 });
		}
		range("alpha_quality", self.alpha_quality, 0, 100)?;
		range("method", self.method, 0, 6)?;
		range("segments", self.segments, 1, 4)?;
		range("sns", self.sns, 0, 100)?;
		range("filter_strength", self.filter_strength, 0, 100)?;
		range("filter_sharpness", self.filter_sharpness, 0, 7)?;
		range("passes", self.passes, 1, 10)?;
		range("qmin", self.qmin, 0, 100)?;
		range("qmax", self.qmax, self.qmin, 100)?;
		range("preprocessing", self.preprocessing, 0, 7)?;
		range("partition_limit", self.partition_limit, 0, 100)?;

		match self.target {
			Some(TargetMetric::Size(0)) => return Err(ValidationError::ZeroTargetSize),
			Some(TargetMetric::Psnr(psnr)) if !(psnr > 0.0 && psnr <= 10_000.0) => {
				return Err(ValidationError::OutOfRangeFloat { field: "target", value: psnr, min: 0.0, max: 10_000.0 });
			}
			_ => {}
		}
		if let Some(colour) = self.blend_alpha
			&& colour > 0x00ff_ffff
		{
			return Err(ValidationError::OutOfRange { field: "blend_alpha", value: colour, min: 0, max: 0x00ff_ffff });
		}

		Ok(())
	}
}

/// Accept the pre-0.14 shape as well as this one. See [`super::legacy`].
impl<'de> Deserialize<'de> for WebpSettings {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		super::legacy::WebpWire::deserialize(deserializer).map(super::legacy::WebpWire::into_settings)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The default is the Electron app's default encode, flag for flag, so a window nobody
	/// has touched writes the same file after 0.14 as before it.
	#[test]
	fn default_is_the_apps_historic_encode() {
		let s = WebpSettings::default();
		assert!(!s.lossless);
		assert!((s.quality - 75.0).abs() < f32::EPSILON);
		assert_eq!((s.alpha_quality, s.alpha_filtering, s.method, s.segments, s.partition_limit, s.passes, s.sns), (100, AlphaFiltering::Best, 4, 4, 0, 6, 50));
		assert!(s.autofilter, "the Electron app sent -af by default");
		assert!(s.multi_threading, "the Electron app hardcoded -mt");
		assert_eq!(s.target, None, "the Electron app's `-size 1` default was a bug and is not reproduced");
		assert_eq!((s.near_lossless, s.exact, s.keep_alpha, s.blend_alpha, s.metadata.any()), (100, false, true, None, false));
		assert_eq!(s.validate(), Ok(()));
	}

	#[test]
	fn presets_match_config_enc_c() {
		let mut s = WebpSettings { quality: 42.0, sns: 1, preprocessing: 5, lossless: true, keep_alpha: false, ..WebpSettings::default() };
		s.apply_preset(Preset::Photo);
		assert!((s.quality - 42.0).abs() < f32::EPSILON, "a preset keeps the quality");
		assert_eq!((s.sns, s.filter_sharpness, s.filter_strength, s.preprocessing), (80, 3, 30, 2));
		assert!(!s.lossless, "a preset re-initialises the whole encoder config");
		assert!(!s.keep_alpha, "a preset does not touch the picture options");
		s.apply_preset(Preset::Text);
		assert_eq!((s.sns, s.filter_strength, s.segments, s.preprocessing), (0, 0, 2, 0));
	}

	#[test]
	fn lossless_presets_match_config_enc_c() {
		let mut s = WebpSettings::default();
		s.apply_lossless_preset(9).expect("level 9");
		assert!(s.lossless);
		assert_eq!(s.method, 6);
		assert!((s.quality - 100.0).abs() < f32::EPSILON);
		s.apply_lossless_preset(0).expect("level 0");
		assert_eq!(s.method, 0);
		assert!(s.apply_lossless_preset(10).is_err());
	}

	#[test]
	fn out_of_range_controls_are_rejected() {
		let d = WebpSettings::default;
		for (s, field) in [(WebpSettings { alpha_quality: 101, ..d() }, "alpha_quality"), (WebpSettings { method: 7, ..d() }, "method"), (WebpSettings { segments: 0, ..d() }, "segments"), (WebpSettings { segments: 5, ..d() }, "segments"), (WebpSettings { sns: 101, ..d() }, "sns"), (WebpSettings { passes: 11, ..d() }, "passes"), (WebpSettings { filter_sharpness: 8, ..d() }, "filter_sharpness"), (WebpSettings { preprocessing: 8, ..d() }, "preprocessing"), (WebpSettings { qmin: 60, qmax: 50, ..d() }, "qmax"), (WebpSettings { near_lossless: 101, ..d() }, "near_lossless")] {
			assert!(matches!(s.validate(), Err(ValidationError::OutOfRange { field: f, .. }) if f == field), "{field}: {:?}", s.validate());
		}
		assert!(WebpSettings { quality: 100.5, ..d() }.validate().is_err());
		assert!(WebpSettings { quality: f32::NAN, ..d() }.validate().is_err());
		assert!(WebpSettings { target: Some(TargetMetric::Psnr(0.0)), ..d() }.validate().is_err());
		assert_eq!(WebpSettings { target: Some(TargetMetric::Size(0)), ..d() }.validate(), Err(ValidationError::ZeroTargetSize));
		assert!(WebpSettings { blend_alpha: Some(0x0100_0000), ..d() }.validate().is_err());
		assert_eq!(WebpSettings { quality: 99.5, target: Some(TargetMetric::Psnr(42.5)), blend_alpha: Some(0x00ff_ffff), ..d() }.validate(), Ok(()));
	}

	#[test]
	fn json_uses_camel_case_field_names() {
		let json = serde_json::to_value(WebpSettings::default()).expect("serialize");
		for key in ["lossless", "nearLossless", "alphaQuality", "alphaCompression", "alphaFiltering", "imageHint", "filterStrength", "filterSharpness", "filterType", "autofilter", "partitionLimit", "jpegLike", "sharpYuv", "lowMemory", "multiThreading", "keepAlpha", "blendAlpha", "metadata"] {
			assert!(json.get(key).is_some(), "missing `{key}` in {json}");
		}
		assert!(json.get("mode").is_none(), "the current shape has no modes: {json}");
		let back: WebpSettings = serde_json::from_value(json).expect("deserialize");
		assert_eq!(back, WebpSettings::default());
	}
}
