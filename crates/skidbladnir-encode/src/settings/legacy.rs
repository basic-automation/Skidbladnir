//! Reading settings saved before 0.12, when the WebP controls were grouped into modes.
//!
//! Presets and the preferences file outlive the version that wrote them, so a file from
//! any earlier version must load, and must encode to **the same bytes it encoded to
//! then**. Falling back to the defaults would silently throw a user's tuning away, and
//! loading the numbers into the new fields naively would change what they produce —
//! the old modes did not send every control (the lossless mode never sent `-segments`,
//! the preset mode never sent `-alpha_q`, auto filtering never sent the strength slider).
//!
//! So the old shape is converted by replaying what the old encoder did with it:
//! [`LegacyWebpSettings::into_current`] is the pre-0.12 `build_config`, writing into the
//! new fields instead of a `WebPConfig`. The old shape is recognised by its `mode` key,
//! which it always wrote and the current shape never has.

use serde::{Deserialize, Deserializer};

use super::webp::{AlphaFiltering, FilterType, ImageHint, Preset, TargetMetric, WebpMetadata, WebpSettings};

/// The encoding mode, one of the five the pre-0.12 window offered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
	/// Ordinary lossy VP8. The only mode whose advanced controls reached the encoder.
	#[default]
	Lossy,
	/// Lossless VP8L, always with `-exact`.
	Lossless,
	/// Lossless with near-lossless preprocessing, its level taken from the quality slider.
	NearLossless,
	/// Lossy with `-jpeg_like`.
	JpegLike,
	/// One of libwebp's presets.
	Preset,
}

/// The pre-0.12 filter choice: automatic, or one of the two types by hand.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LegacyFilter {
	/// `-af`.
	#[default]
	Auto,
	/// `-nostrong` with the strength and sharpness sliders.
	Simple,
	/// `-strong` with the strength and sharpness sliders.
	Strong,
}

/// The pre-0.12 WebP settings, with the defaults they had.
#[derive(Clone, Debug, PartialEq)]
pub struct LegacyWebpSettings {
	pub mode: Mode,
	pub preset: Option<Preset>,
	pub quality: u8,
	pub alpha_quality: u8,
	pub alpha_filtering: Option<AlphaFiltering>,
	pub method: u8,
	pub segments: u8,
	pub partition_limit: u8,
	pub sns: u8,
	pub passes: u8,
	pub filter: LegacyFilter,
	pub filter_strength: u8,
	pub filter_sharpness: u8,
	pub target: Option<TargetMetric>,
	pub sharp_yuv: bool,
	pub low_memory: bool,
	pub multi_threading: bool,
}

impl Default for LegacyWebpSettings {
	fn default() -> Self {
		Self { mode: Mode::Lossy, preset: None, quality: 75, alpha_quality: 100, alpha_filtering: Some(AlphaFiltering::Best), method: 4, segments: 4, partition_limit: 0, sns: 50, passes: 6, filter: LegacyFilter::Auto, filter_strength: 20, filter_sharpness: 0, target: None, sharp_yuv: false, low_memory: false, multi_threading: true }
	}
}

impl LegacyWebpSettings {
	/// What the pre-0.12 encoder made of these settings, as current settings.
	///
	/// A line-for-line replay of the old `build_config`: start from libwebp's defaults and
	/// apply only what that mode sent. A preset mode with no preset chosen was refused
	/// before 0.12; here it simply encodes without one.
	#[must_use]
	pub fn into_current(self) -> WebpSettings {
		let lossy = self.mode == Mode::Lossy;
		let manual_filter = matches!(self.filter, LegacyFilter::Simple | LegacyFilter::Strong);
		let mut s = WebpSettings::libwebp_defaults();

		match self.mode {
			Mode::Preset => {
				if let Some(preset) = self.preset {
					s.apply_preset(preset);
				}
			}
			Mode::NearLossless => (s.near_lossless, s.lossless) = (self.quality, true),
			Mode::JpegLike => s.jpeg_like = true,
			Mode::Lossless => (s.lossless, s.exact) = (true, true),
			Mode::Lossy => {}
		}
		if lossy {
			match self.filter {
				LegacyFilter::Auto => s.autofilter = true,
				LegacyFilter::Simple => s.filter_type = FilterType::Simple,
				LegacyFilter::Strong => s.filter_type = FilterType::Strong,
			}
		}
		s.multi_threading = self.multi_threading;
		if lossy && manual_filter {
			(s.filter_strength, s.filter_sharpness) = (self.filter_strength, self.filter_sharpness);
		}
		if lossy {
			s.sharp_yuv = self.sharp_yuv;
		}
		if self.mode != Mode::Preset
			&& let Some(alpha_filtering) = self.alpha_filtering
		{
			s.alpha_filtering = alpha_filtering;
		}
		if self.mode != Mode::NearLossless {
			s.quality = f32::from(self.quality);
		}
		if matches!(self.mode, Mode::Lossy | Mode::Lossless | Mode::JpegLike) {
			(s.alpha_quality, s.method) = (self.alpha_quality, self.method);
		}
		if lossy {
			(s.low_memory, s.segments, s.partition_limit, s.target, s.passes, s.sns) = (self.low_memory, self.segments, self.partition_limit, self.target, self.passes, self.sns);
		}
		s
	}
}

/// A field that may be absent, explicitly `null`, or set — three states, because in the
/// old shape `null` meant "leave libwebp's default" where absent meant the old default.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Explicit<T> {
	/// The key was not there.
	#[default]
	Absent,
	/// The key was there, and `null`.
	Null,
	/// The key had a value.
	Value(T),
}

impl<T> Explicit<T> {
	/// The value as an `Option`, or `absent` if the key was not there.
	pub(crate) fn or(self, absent: Option<T>) -> Option<T> {
		match self {
			Self::Absent => absent,
			Self::Null => None,
			Self::Value(value) => Some(value),
		}
	}
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Explicit<T> {
	fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		Option::<T>::deserialize(deserializer).map(|value| value.map_or(Self::Null, Self::Value))
	}
}

/// A quality as the JSON has it: the old shape wrote whole numbers, the current one may
/// write fractions.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
enum Quality {
	/// A whole number, as `-q` took before 0.12.
	Whole(u8),
	/// A fraction.
	Fraction(f32),
}

impl Quality {
	fn as_f32(self) -> f32 {
		match self {
			Self::Whole(q) => f32::from(q),
			Self::Fraction(q) => q,
		}
	}

	/// The old, whole-number quality. A fraction cannot have been written by the old
	/// shape, so one is rounded to the nearest whole quality rather than refused.
	fn as_whole(self) -> Option<u8> {
		match self {
			Self::Whole(q) => Some(q),
			Self::Fraction(q) => (0..=100_u8).find(|&whole| (f32::from(whole) - q).abs() <= 0.5),
		}
	}
}

/// Every key either shape can carry. Deserialised first, then read as whichever shape it
/// is: the old one if it has a `mode`, the current one otherwise. A field missing from
/// either falls back to that shape's own default.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WebpWire {
	// Both shapes.
	quality: Option<Quality>,
	alpha_quality: Option<u8>,
	alpha_filtering: Explicit<AlphaFiltering>,
	method: Option<u8>,
	segments: Option<u8>,
	partition_limit: Option<u8>,
	sns: Option<u8>,
	passes: Option<u8>,
	filter_strength: Option<u8>,
	filter_sharpness: Option<u8>,
	target: Explicit<TargetMetric>,
	sharp_yuv: Option<bool>,
	low_memory: Option<bool>,
	multi_threading: Option<bool>,
	// The old shape only.
	mode: Option<Mode>,
	preset: Option<Preset>,
	filter: Option<LegacyFilter>,
	// The current shape only.
	lossless: Option<bool>,
	near_lossless: Option<u8>,
	exact: Option<bool>,
	alpha_compression: Option<bool>,
	image_hint: Option<ImageHint>,
	filter_type: Option<FilterType>,
	autofilter: Option<bool>,
	qmin: Option<u8>,
	qmax: Option<u8>,
	preprocessing: Option<u8>,
	jpeg_like: Option<bool>,
	keep_alpha: Option<bool>,
	blend_alpha: Explicit<u32>,
	metadata: Option<WebpMetadata>,
}

impl WebpWire {
	/// Whether this is the pre-0.12 shape.
	#[must_use]
	pub const fn is_legacy(&self) -> bool {
		self.mode.is_some()
	}

	/// Whether this carries no WebP keys at all, as the top level of a current-shape job
	/// does once its own keys are taken out.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.quality.is_none() && self.mode.is_none() && self.lossless.is_none() && self.method.is_none() && self.alpha_quality.is_none() && self.segments.is_none() && self.sns.is_none() && self.passes.is_none() && self.filter.is_none() && self.filter_type.is_none() && self.target == Explicit::Absent && self.alpha_filtering == Explicit::Absent && self.multi_threading.is_none()
	}

	/// Read as whichever shape this is.
	#[must_use]
	pub fn into_settings(self) -> WebpSettings {
		if self.is_legacy() {
			let d = LegacyWebpSettings::default();
			let quality = self.quality.and_then(Quality::as_whole).unwrap_or(d.quality);
			LegacyWebpSettings { mode: self.mode.unwrap_or_default(), preset: self.preset, quality, alpha_quality: self.alpha_quality.unwrap_or(d.alpha_quality), alpha_filtering: self.alpha_filtering.or(d.alpha_filtering), method: self.method.unwrap_or(d.method), segments: self.segments.unwrap_or(d.segments), partition_limit: self.partition_limit.unwrap_or(d.partition_limit), sns: self.sns.unwrap_or(d.sns), passes: self.passes.unwrap_or(d.passes), filter: self.filter.unwrap_or(d.filter), filter_strength: self.filter_strength.unwrap_or(d.filter_strength), filter_sharpness: self.filter_sharpness.unwrap_or(d.filter_sharpness), target: self.target.or(d.target), sharp_yuv: self.sharp_yuv.unwrap_or(d.sharp_yuv), low_memory: self.low_memory.unwrap_or(d.low_memory), multi_threading: self.multi_threading.unwrap_or(d.multi_threading) }.into_current()
		} else {
			let d = WebpSettings::default();
			WebpSettings { lossless: self.lossless.unwrap_or(d.lossless), near_lossless: self.near_lossless.unwrap_or(d.near_lossless), exact: self.exact.unwrap_or(d.exact), quality: self.quality.map_or(d.quality, Quality::as_f32), alpha_quality: self.alpha_quality.unwrap_or(d.alpha_quality), alpha_compression: self.alpha_compression.unwrap_or(d.alpha_compression), alpha_filtering: self.alpha_filtering.or(Some(d.alpha_filtering)).unwrap_or(d.alpha_filtering), method: self.method.unwrap_or(d.method), image_hint: self.image_hint.unwrap_or(d.image_hint), target: self.target.or(d.target), segments: self.segments.unwrap_or(d.segments), sns: self.sns.unwrap_or(d.sns), filter_strength: self.filter_strength.unwrap_or(d.filter_strength), filter_sharpness: self.filter_sharpness.unwrap_or(d.filter_sharpness), filter_type: self.filter_type.unwrap_or(d.filter_type), autofilter: self.autofilter.unwrap_or(d.autofilter), passes: self.passes.unwrap_or(d.passes), qmin: self.qmin.unwrap_or(d.qmin), qmax: self.qmax.unwrap_or(d.qmax), preprocessing: self.preprocessing.unwrap_or(d.preprocessing), partition_limit: self.partition_limit.unwrap_or(d.partition_limit), jpeg_like: self.jpeg_like.unwrap_or(d.jpeg_like), sharp_yuv: self.sharp_yuv.unwrap_or(d.sharp_yuv), low_memory: self.low_memory.unwrap_or(d.low_memory), multi_threading: self.multi_threading.unwrap_or(d.multi_threading), keep_alpha: self.keep_alpha.unwrap_or(d.keep_alpha), blend_alpha: self.blend_alpha.or(d.blend_alpha), metadata: self.metadata.unwrap_or(d.metadata) }
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn read(json: &str) -> WebpSettings {
		serde_json::from_str(json).expect("settings deserialize")
	}

	/// The old default and the new default are the same encode.
	#[test]
	fn the_old_default_is_the_new_default() {
		assert_eq!(LegacyWebpSettings::default().into_current(), WebpSettings::default());
		assert_eq!(read(r#"{"mode":"lossy"}"#), WebpSettings::default());
	}

	/// Each old mode sent only its own controls; the rest stay at libwebp's defaults.
	#[test]
	fn old_modes_replay_what_they_sent() {
		let lossless = read(r#"{"mode":"lossless","quality":90,"segments":2,"sns":10,"method":6}"#);
		assert!(lossless.lossless && lossless.exact, "the old lossless mode always paired -exact");
		assert_eq!((lossless.method, lossless.segments, lossless.sns), (6, 4, 50), "segments and SNS were never sent in lossless");

		let near = read(r#"{"mode":"nearLossless","quality":60,"method":6}"#);
		assert!(near.lossless);
		assert_eq!(near.near_lossless, 60, "the quality slider was the level");
		assert!((near.quality - 75.0).abs() < f32::EPSILON, "and no -q was sent");
		assert_eq!(near.method, 4, "nor -m");
		assert!(!near.exact);

		let preset = read(r#"{"mode":"preset","preset":"drawing","quality":80,"alphaQuality":10,"alphaFiltering":"best"}"#);
		assert_eq!((preset.sns, preset.filter_sharpness, preset.filter_strength), (25, 6, 10));
		assert!((preset.quality - 80.0).abs() < f32::EPSILON);
		assert_eq!(preset.alpha_quality, 100, "the preset mode sent no -alpha_q");
		assert_eq!(preset.alpha_filtering, AlphaFiltering::Fast, "nor -alpha_filter, so the preset's own stood");

		let jpeg_like = read(r#"{"mode":"jpegLike","sns":10}"#);
		assert!(jpeg_like.jpeg_like && !jpeg_like.autofilter, "the JPEG-like mode sent no -af");
		assert_eq!(jpeg_like.sns, 50);
	}

	#[test]
	fn old_filter_choices() {
		let auto = read(r#"{"mode":"lossy","filter":"auto","filterStrength":99,"filterSharpness":5}"#);
		assert!(auto.autofilter);
		assert_eq!((auto.filter_strength, auto.filter_sharpness), (60, 0), "auto filtering did not send the sliders");
		let simple = read(r#"{"mode":"lossy","filter":"simple","filterStrength":10,"filterSharpness":7}"#);
		assert_eq!((simple.filter_type, simple.autofilter, simple.filter_strength, simple.filter_sharpness), (FilterType::Simple, false, 10, 7));
	}

	/// `alphaFiltering: null` meant "leave libwebp's default"; absent meant the old default.
	#[test]
	fn null_and_absent_alpha_filtering_differ() {
		assert_eq!(read(r#"{"mode":"lossy","alphaFiltering":null}"#).alpha_filtering, AlphaFiltering::Fast);
		assert_eq!(read(r#"{"mode":"lossy"}"#).alpha_filtering, AlphaFiltering::Best);
	}

	#[test]
	fn old_targets_still_load() {
		assert_eq!(read(r#"{"mode":"lossy","target":{"kind":"psnr","value":42}}"#).target, Some(TargetMetric::Psnr(42.0)));
		assert_eq!(read(r#"{"mode":"lossless","target":{"kind":"size","value":5000}}"#).target, None, "lossless never sent a target");
	}
}
