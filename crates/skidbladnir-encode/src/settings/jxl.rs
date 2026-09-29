//! The JPEG XL controls: every `cjxl` option that changes the file it writes for a still
//! image.
//!
//! `cjxl` hands libjxl a list of frame options, and most of its flags are one entry in
//! that list, with `-1` meaning "the encoder chooses". The fields below keep that shape —
//! `cjxl`'s own defaults, its tri-state switches as [`Tristate`], `-1` where `cjxl` has
//! `-1` — so the list built from them is exactly the list `cjxl` builds. Only `-p`, which
//! rewrites several other options, is interpreted, the way `cjxl` interprets it.
//!
//! What `cjxl` offers that is not here is about more than one image: animation from GIF
//! and APNG, frame indexing past the first frame, and streaming PPM input. See
//! ROADMAP.md.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{ValidationError, legacy::Explicit};

/// A switch with a third, "let the encoder decide", position (`cjxl`'s `Override`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tristate {
	/// The flag is not given: the encoder decides.
	#[default]
	Default,
	/// `=0`.
	Off,
	/// `=1`.
	On,
}

impl Tristate {
	/// `cjxl`'s value for the frame option, or `None` when it adds none.
	#[must_use]
	pub const fn as_option(self) -> Option<i64> {
		match self {
			Self::Default => None,
			Self::Off => Some(0),
			Self::On => Some(1),
		}
	}
}

/// How much loss is allowed: `-d`, `-q`, or neither.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum JxlTarget {
	/// Neither flag: `cjxl`'s default, distance 1.0 (visually lossless), or 0.0
	/// (mathematically lossless) for a JPEG decoded to pixels.
	Default,
	/// `-d`: the Butteraugli distance, `0.0..=25.0`, where 0 is lossless.
	Distance(f32),
	/// `-q`: quality, `0.0..=100.0`, mapped to a distance by libjxl, where 100 is lossless.
	Quality(f32),
}

/// Where a piece of metadata comes from (`cjxl -x`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "path")]
pub enum MetadataSource {
	/// From the input, as `cjxl` reads it.
	#[default]
	Keep,
	/// Left out (`-x strip=<kind>`).
	Strip,
	/// From this file instead (`-x <kind>=<file>`).
	File(PathBuf),
}

/// A white point for [`JxlColorSpace`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "xy")]
pub enum WhitePoint {
	/// `D65`.
	D65,
	/// Illuminant E, `EER`.
	E,
	/// The DCI-P3 white, `DCI`.
	Dci,
	/// `D50`.
	D50,
	/// Custom CIE xy.
	Custom([f64; 2]),
}

/// Colour primaries for [`JxlColorSpace`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "xy")]
pub enum Primaries {
	/// sRGB / BT.709, `SRG`.
	Srgb,
	/// BT.2020 / BT.2100, `202`.
	Rec2100,
	/// DCI-P3, `DCI`.
	P3,
	/// Adobe RGB (1998), `Ado`.
	Adobe,
	/// `ProPhoto` RGB, `Pro`.
	ProPhoto,
	/// Custom red, green and blue CIE xy.
	Custom([f64; 6]),
}

/// A rendering intent for [`JxlColorSpace`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderingIntent {
	/// `Per`.
	Perceptual,
	/// `Rel`.
	#[default]
	Relative,
	/// `Sat`.
	Saturation,
	/// `Abs`.
	Absolute,
}

/// A transfer function for [`JxlColorSpace`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "gamma")]
pub enum TransferFunction {
	/// `SRG`.
	Srgb,
	/// BT.709, `709`.
	Bt709,
	/// `Lin`.
	Linear,
	/// SMPTE ST 2084, `PeQ`.
	Pq,
	/// Hybrid log-gamma, `HLG`.
	Hlg,
	/// DCI gamma 2.6, `DCI`.
	Dci,
	/// Adobe RGB's gamma, `Ado`.
	Adobe,
	/// `ProPhoto`'s gamma, `Pro`.
	ProPhoto,
	/// A pure gamma, as the encoding exponent (`g0.45455` is roughly 2.2).
	Gamma(f64),
}

/// A colour space to tag untagged pixels with (`cjxl -x color_space=...`).
///
/// `cjxl` honours it only when the input says nothing about its own colour — a PNG with
/// none of `cICP`, `iCCP`, `sRGB`, `gAMA` or `cHRM` — and never for JPEG.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JxlColorSpace {
	/// Grayscale rather than RGB. Must match the image.
	pub gray: bool,
	/// The white point.
	pub white_point: WhitePoint,
	/// The primaries. Ignored for grayscale, which has none.
	pub primaries: Primaries,
	/// The rendering intent.
	pub rendering_intent: RenderingIntent,
	/// The transfer function.
	pub transfer_function: TransferFunction,
}

impl JxlColorSpace {
	/// The `cjxl -x color_space=` string for this colour space, in libjxl's
	/// `ColorEncoding` description grammar.
	#[must_use]
	pub fn description(&self) -> String {
		let white_point = match self.white_point {
			WhitePoint::D65 => "D65".to_owned(),
			WhitePoint::E => "EER".to_owned(),
			WhitePoint::Dci => "DCI".to_owned(),
			WhitePoint::D50 => "D50".to_owned(),
			WhitePoint::Custom([x, y]) => format!("{x};{y}"),
		};
		let primaries = match self.primaries {
			Primaries::Srgb => "SRG".to_owned(),
			Primaries::Rec2100 => "202".to_owned(),
			Primaries::P3 => "DCI".to_owned(),
			Primaries::Adobe => "Ado".to_owned(),
			Primaries::ProPhoto => "Pro".to_owned(),
			Primaries::Custom(xy) => xy.iter().map(ToString::to_string).collect::<Vec<_>>().join(";"),
		};
		let intent = match self.rendering_intent {
			RenderingIntent::Perceptual => "Per",
			RenderingIntent::Relative => "Rel",
			RenderingIntent::Saturation => "Sat",
			RenderingIntent::Absolute => "Abs",
		};
		let transfer = match self.transfer_function {
			TransferFunction::Srgb => "SRG".to_owned(),
			TransferFunction::Bt709 => "709".to_owned(),
			TransferFunction::Linear => "Lin".to_owned(),
			TransferFunction::Pq => "PeQ".to_owned(),
			TransferFunction::Hlg => "HLG".to_owned(),
			TransferFunction::Dci => "DCI".to_owned(),
			TransferFunction::Adobe => "Ado".to_owned(),
			TransferFunction::ProPhoto => "Pro".to_owned(),
			TransferFunction::Gamma(gamma) => format!("g{gamma}"),
		};
		if self.gray { format!("Gra_{white_point}_{intent}_{transfer}") } else { format!("RGB_{white_point}_{primaries}_{intent}_{transfer}") }
	}
}

/// Every JPEG XL control. Defaults are `cjxl`'s, except the target: quality 90, which is
/// distance 1.0 for every input rather than lossless for a JPEG decoded to pixels.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[expect(clippy::struct_excessive_bools, reason = "each bool is one of cjxl's on/off flags; the struct mirrors its command line field for field")]
pub struct JxlSettings {
	/// `-d` / `-q`.
	pub target: JxlTarget,
	/// `-a`: the alpha channel's distance, or `None` for `cjxl`'s 0 (lossless alpha).
	pub alpha_distance: Option<f32>,
	/// `-e`: effort, `1..=10`, or 11 with [`JxlSettings::allow_expert_options`].
	pub effort: u8,
	/// `--allow_expert_options`: permits effort 11.
	pub allow_expert_options: bool,
	/// `--brotli_effort`, `0..=11`: how hard metadata boxes are compressed.
	pub brotli_effort: u8,
	/// `-p`: more progressive decoding. Turns on progressive AC, one extra DC pass,
	/// centre-first group order, the Squeeze transform and turns off patches, each unless
	/// that option is set by hand.
	pub progressive: bool,
	/// `--group_order`: scanline (off) or centre-first (on).
	pub group_order: Tristate,
	/// `--container`: whether to wrap the codestream in the ISO-BMFF container.
	pub container: Tristate,
	/// `--compress_boxes`: compress metadata boxes with Brotli.
	pub compress_boxes: Tristate,
	/// `-m`: `VarDCT` (off) or modular (on) mode.
	pub modular: Tristate,
	/// `-j`: recompress a JPEG losslessly rather than re-encoding its pixels. Applies to
	/// JPEG input with no crop or resize; anything else is encoded from pixels.
	pub lossless_jpeg: bool,
	/// `--photon_noise_iso`: film grain to add, as an ISO speed; 0 adds none.
	pub photon_noise_iso: f32,
	/// `--intensity_target`: the image's peak brightness in nits; 0 chooses.
	pub intensity_target: f32,
	/// `--allow_jpeg_reconstruction`: keep what is needed to rebuild a recompressed JPEG
	/// bit for bit.
	pub allow_jpeg_reconstruction: bool,
	/// `--codestream_level`: -1 chooses, or 5 or 10.
	pub codestream_level: i8,
	/// `--buffering`, `-1..=3`: how much input libjxl buffers.
	pub buffering: i8,
	/// `--faster_decoding`, `0..=4`.
	pub faster_decoding: u8,
	/// `--premultiply`: -1 as the input is, 0 or 1 to force.
	pub premultiply: i8,
	/// `--keep_invisible`: preserve the colour of invisible pixels.
	pub keep_invisible: Tristate,
	/// `--center_x`: -1 for the middle, else the column centre-first ordering starts at.
	pub center_x: i64,
	/// `--center_y`: -1 for the middle, else the row centre-first ordering starts at.
	pub center_y: i64,
	/// `--progressive_ac`.
	pub progressive_ac: bool,
	/// `--qprogressive_ac`.
	pub qprogressive_ac: bool,
	/// `--progressive_dc`, `-1..=2`.
	pub progressive_dc: i8,
	/// `--resampling`: -1, 1, 2, 4 or 8.
	pub resampling: i8,
	/// `--ec_resampling`: -1, 1, 2, 4 or 8.
	pub ec_resampling: i8,
	/// `--already_downsampled`: the input is already downsampled by
	/// [`JxlSettings::resampling`]; signal the upsampling without downsampling again.
	pub already_downsampled: bool,
	/// `--upsampling_mode`: -1, or 0 for nearest neighbour.
	pub upsampling_mode: i8,
	/// `--epf`, `-1..=3`: edge-preserving filter strength.
	pub epf: i8,
	/// `--gaborish`.
	pub gaborish: Tristate,
	/// `--override_bitdepth`: 0 keeps the input's.
	pub override_bitdepth: u8,
	/// `--noise`.
	pub noise: Tristate,
	/// `--jpeg_reconstruction_cfl`.
	pub jpeg_reconstruction_cfl: Tristate,
	/// `--dots`.
	pub dots: Tristate,
	/// `--patches`.
	pub patches: Tristate,
	/// `--frame_indexing=1`: write a frame index box for the one frame.
	pub frame_index_box: bool,
	/// `--disable_perceptual_optimizations`.
	pub disable_perceptual_optimizations: bool,
	/// `--output_mode`, `-1..=2`.
	pub output_mode: i8,
	/// `--streaming_output`: write through libjxl's output processor.
	pub streaming_output: bool,
	/// `-I`: percentage of pixels used to learn MA trees, `-1..=100`.
	pub iterations: f32,
	/// `-C`: modular colour transform, `-1..=41`.
	pub modular_colorspace: i8,
	/// `-g`: modular group size, `-1..=3`.
	pub modular_group_size: i8,
	/// `-P`: modular predictor, `-1..=15`.
	pub modular_predictor: i8,
	/// `-E`: previous-channel properties for MA trees, `-1..=11`.
	pub modular_nb_prev_channels: i8,
	/// `--modular_palette_colors`: -1, or the colour count below which a palette is used.
	pub modular_palette_colors: i64,
	/// `--modular_lossy_palette`.
	pub modular_lossy_palette: bool,
	/// `-X`: global channel palette threshold, `-1..=100` percent.
	pub pre_compact: f32,
	/// `-Y`: per-group channel palette threshold, `-1..=100` percent.
	pub post_compact: f32,
	/// `-R`: the Squeeze transform, or `None` when not given.
	pub responsive: Option<bool>,
	/// Encode on every core (`--num_threads` left at its default), or on one
	/// (`--num_threads=0`). libjxl's output does not depend on it.
	pub multi_threading: bool,
	/// `-x color_space=`: tag untagged input with this colour space.
	pub color_space: Option<JxlColorSpace>,
	/// `-x icc_pathname=`: use this ICC profile for untagged input.
	pub icc_file: Option<PathBuf>,
	/// `-x exif=` / `-x strip=exif`.
	pub exif: MetadataSource,
	/// `-x xmp=` / `-x strip=xmp`.
	pub xmp: MetadataSource,
	/// `-x jumbf=` / `-x strip=jumbf`.
	pub jumbf: MetadataSource,
}

impl Default for JxlSettings {
	fn default() -> Self {
		Self { target: JxlTarget::Quality(90.0), alpha_distance: None, effort: 7, allow_expert_options: false, brotli_effort: 9, progressive: false, group_order: Tristate::Default, container: Tristate::Default, compress_boxes: Tristate::Default, modular: Tristate::Default, lossless_jpeg: true, photon_noise_iso: 0.0, intensity_target: 0.0, allow_jpeg_reconstruction: true, codestream_level: -1, buffering: -1, faster_decoding: 0, premultiply: -1, keep_invisible: Tristate::Default, center_x: -1, center_y: -1, progressive_ac: false, qprogressive_ac: false, progressive_dc: -1, resampling: -1, ec_resampling: -1, already_downsampled: false, upsampling_mode: -1, epf: -1, gaborish: Tristate::Default, override_bitdepth: 0, noise: Tristate::Default, jpeg_reconstruction_cfl: Tristate::Default, dots: Tristate::Default, patches: Tristate::Default, frame_index_box: false, disable_perceptual_optimizations: false, output_mode: -1, streaming_output: false, iterations: -1.0, modular_colorspace: -1, modular_group_size: -1, modular_predictor: -1, modular_nb_prev_channels: -1, modular_palette_colors: -1, modular_lossy_palette: false, pre_compact: -1.0, post_compact: -1.0, responsive: None, multi_threading: true, color_space: None, icc_file: None, exif: MetadataSource::Keep, xmp: MetadataSource::Keep, jumbf: MetadataSource::Keep }
	}
}

impl JxlSettings {
	/// Check every control against the ranges `cjxl` accepts.
	///
	/// # Errors
	///
	/// Returns the first [`ValidationError`] found.
	pub fn validate(&self) -> Result<(), ValidationError> {
		fn signed(field: &'static str, value: i64, min: i64, max: i64) -> Result<(), ValidationError> {
			if value < min || value > max {
				return Err(ValidationError::OutOfRangeSigned { field, value, min, max });
			}
			Ok(())
		}
		fn float(field: &'static str, value: f32, min: f32, max: f32) -> Result<(), ValidationError> {
			if !(min..=max).contains(&value) {
				return Err(ValidationError::OutOfRangeFloat { field, value, min, max });
			}
			Ok(())
		}
		fn one_of(field: &'static str, value: i8, allowed: &[i8]) -> Result<(), ValidationError> {
			if !allowed.contains(&value) {
				return Err(ValidationError::NotAllowed { field, value: i64::from(value) });
			}
			Ok(())
		}

		match self.target {
			JxlTarget::Default => {}
			JxlTarget::Distance(distance) => float("jxl.distance", distance, 0.0, 25.0)?,
			JxlTarget::Quality(quality) => float("jxl.quality", quality, 0.0, 100.0)?,
		}
		if let Some(alpha) = self.alpha_distance {
			float("jxl.alpha_distance", alpha, 0.0, 25.0)?;
		}
		signed("jxl.effort", i64::from(self.effort), 1, if self.allow_expert_options { 11 } else { 10 })?;
		signed("jxl.brotli_effort", i64::from(self.brotli_effort), 0, 11)?;
		float("jxl.photon_noise_iso", self.photon_noise_iso, 0.0, f32::MAX)?;
		float("jxl.intensity_target", self.intensity_target, 0.0, f32::MAX)?;
		one_of("jxl.codestream_level", self.codestream_level, &[-1, 5, 10])?;
		signed("jxl.buffering", i64::from(self.buffering), -1, 3)?;
		signed("jxl.faster_decoding", i64::from(self.faster_decoding), 0, 4)?;
		one_of("jxl.premultiply", self.premultiply, &[-1, 0, 1])?;
		signed("jxl.center_x", self.center_x, -1, i64::from(u32::MAX))?;
		signed("jxl.center_y", self.center_y, -1, i64::from(u32::MAX))?;
		if (self.center_x != -1 || self.center_y != -1) && self.effective_group_order() != Tristate::On {
			return Err(ValidationError::Conflict("setting the centre for centre-first group order needs centre-first group order"));
		}
		signed("jxl.progressive_dc", i64::from(self.progressive_dc), -1, 2)?;
		one_of("jxl.resampling", self.resampling, &[-1, 1, 2, 4, 8])?;
		one_of("jxl.ec_resampling", self.ec_resampling, &[-1, 1, 2, 4, 8])?;
		one_of("jxl.upsampling_mode", self.upsampling_mode, &[-1, 0, 1])?;
		signed("jxl.epf", i64::from(self.epf), -1, 3)?;
		signed("jxl.override_bitdepth", i64::from(self.override_bitdepth), 0, 32)?;
		signed("jxl.output_mode", i64::from(self.output_mode), -1, 2)?;
		float("jxl.iterations", self.iterations, -1.0, 100.0)?;
		signed("jxl.modular_colorspace", i64::from(self.modular_colorspace), -1, 41)?;
		signed("jxl.modular_group_size", i64::from(self.modular_group_size), -1, 3)?;
		signed("jxl.modular_predictor", i64::from(self.modular_predictor), -1, 15)?;
		signed("jxl.modular_nb_prev_channels", i64::from(self.modular_nb_prev_channels), -1, 11)?;
		signed("jxl.modular_palette_colors", self.modular_palette_colors, -1, i64::MAX)?;
		float("jxl.pre_compact", self.pre_compact, -1.0, 100.0)?;
		float("jxl.post_compact", self.post_compact, -1.0, 100.0)?;
		Ok(())
	}

	/// The group order once `-p` has had its say.
	#[must_use]
	pub const fn effective_group_order(&self) -> Tristate {
		if self.progressive && matches!(self.group_order, Tristate::Default) { Tristate::On } else { self.group_order }
	}
}

/// Accept the pre-0.14 shape as well as this one: a whole-number `quality` and a
/// `lossless` switch, which was `-d 0`.
impl<'de> Deserialize<'de> for JxlSettings {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		#[derive(Deserialize)]
		#[serde(rename_all = "camelCase")]
		struct Legacy {
			quality: Option<f32>,
			lossless: Option<bool>,
		}
		#[derive(Deserialize)]
		#[serde(rename_all = "camelCase")]
		struct Wire {
			#[serde(flatten)]
			legacy: Legacy,
			#[serde(flatten)]
			current: JxlWire,
		}
		let wire = Wire::deserialize(deserializer)?;
		let had_target = wire.current.target.is_some();
		let mut settings = wire.current.into_settings();
		if !had_target {
			if wire.legacy.lossless == Some(true) {
				settings.target = JxlTarget::Distance(0.0);
			} else if let Some(quality) = wire.legacy.quality {
				settings.target = JxlTarget::Quality(quality);
			}
		}
		Ok(settings)
	}
}

/// Every current field, each optional so a partial object falls back field by field.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct JxlWire {
	target: Option<JxlTarget>,
	alpha_distance: Explicit<f32>,
	effort: Option<u8>,
	allow_expert_options: Option<bool>,
	brotli_effort: Option<u8>,
	progressive: Option<bool>,
	group_order: Option<Tristate>,
	container: Option<Tristate>,
	compress_boxes: Option<Tristate>,
	modular: Option<Tristate>,
	lossless_jpeg: Option<bool>,
	photon_noise_iso: Option<f32>,
	intensity_target: Option<f32>,
	allow_jpeg_reconstruction: Option<bool>,
	codestream_level: Option<i8>,
	buffering: Option<i8>,
	faster_decoding: Option<u8>,
	premultiply: Option<i8>,
	keep_invisible: Option<Tristate>,
	center_x: Option<i64>,
	center_y: Option<i64>,
	progressive_ac: Option<bool>,
	qprogressive_ac: Option<bool>,
	progressive_dc: Option<i8>,
	resampling: Option<i8>,
	ec_resampling: Option<i8>,
	already_downsampled: Option<bool>,
	upsampling_mode: Option<i8>,
	epf: Option<i8>,
	gaborish: Option<Tristate>,
	override_bitdepth: Option<u8>,
	noise: Option<Tristate>,
	jpeg_reconstruction_cfl: Option<Tristate>,
	dots: Option<Tristate>,
	patches: Option<Tristate>,
	frame_index_box: Option<bool>,
	disable_perceptual_optimizations: Option<bool>,
	output_mode: Option<i8>,
	streaming_output: Option<bool>,
	iterations: Option<f32>,
	modular_colorspace: Option<i8>,
	modular_group_size: Option<i8>,
	modular_predictor: Option<i8>,
	modular_nb_prev_channels: Option<i8>,
	modular_palette_colors: Option<i64>,
	modular_lossy_palette: Option<bool>,
	pre_compact: Option<f32>,
	post_compact: Option<f32>,
	responsive: Explicit<bool>,
	multi_threading: Option<bool>,
	color_space: Explicit<JxlColorSpace>,
	icc_file: Explicit<PathBuf>,
	exif: Option<MetadataSource>,
	xmp: Option<MetadataSource>,
	jumbf: Option<MetadataSource>,
}

impl JxlWire {
	fn into_settings(self) -> JxlSettings {
		let d = JxlSettings::default();
		JxlSettings {
			target: self.target.unwrap_or(d.target),
			alpha_distance: self.alpha_distance.or(d.alpha_distance),
			effort: self.effort.unwrap_or(d.effort),
			allow_expert_options: self.allow_expert_options.unwrap_or(d.allow_expert_options),
			brotli_effort: self.brotli_effort.unwrap_or(d.brotli_effort),
			progressive: self.progressive.unwrap_or(d.progressive),
			group_order: self.group_order.unwrap_or(d.group_order),
			container: self.container.unwrap_or(d.container),
			compress_boxes: self.compress_boxes.unwrap_or(d.compress_boxes),
			modular: self.modular.unwrap_or(d.modular),
			lossless_jpeg: self.lossless_jpeg.unwrap_or(d.lossless_jpeg),
			photon_noise_iso: self.photon_noise_iso.unwrap_or(d.photon_noise_iso),
			intensity_target: self.intensity_target.unwrap_or(d.intensity_target),
			allow_jpeg_reconstruction: self.allow_jpeg_reconstruction.unwrap_or(d.allow_jpeg_reconstruction),
			codestream_level: self.codestream_level.unwrap_or(d.codestream_level),
			buffering: self.buffering.unwrap_or(d.buffering),
			faster_decoding: self.faster_decoding.unwrap_or(d.faster_decoding),
			premultiply: self.premultiply.unwrap_or(d.premultiply),
			keep_invisible: self.keep_invisible.unwrap_or(d.keep_invisible),
			center_x: self.center_x.unwrap_or(d.center_x),
			center_y: self.center_y.unwrap_or(d.center_y),
			progressive_ac: self.progressive_ac.unwrap_or(d.progressive_ac),
			qprogressive_ac: self.qprogressive_ac.unwrap_or(d.qprogressive_ac),
			progressive_dc: self.progressive_dc.unwrap_or(d.progressive_dc),
			resampling: self.resampling.unwrap_or(d.resampling),
			ec_resampling: self.ec_resampling.unwrap_or(d.ec_resampling),
			already_downsampled: self.already_downsampled.unwrap_or(d.already_downsampled),
			upsampling_mode: self.upsampling_mode.unwrap_or(d.upsampling_mode),
			epf: self.epf.unwrap_or(d.epf),
			gaborish: self.gaborish.unwrap_or(d.gaborish),
			override_bitdepth: self.override_bitdepth.unwrap_or(d.override_bitdepth),
			noise: self.noise.unwrap_or(d.noise),
			jpeg_reconstruction_cfl: self.jpeg_reconstruction_cfl.unwrap_or(d.jpeg_reconstruction_cfl),
			dots: self.dots.unwrap_or(d.dots),
			patches: self.patches.unwrap_or(d.patches),
			frame_index_box: self.frame_index_box.unwrap_or(d.frame_index_box),
			disable_perceptual_optimizations: self.disable_perceptual_optimizations.unwrap_or(d.disable_perceptual_optimizations),
			output_mode: self.output_mode.unwrap_or(d.output_mode),
			streaming_output: self.streaming_output.unwrap_or(d.streaming_output),
			iterations: self.iterations.unwrap_or(d.iterations),
			modular_colorspace: self.modular_colorspace.unwrap_or(d.modular_colorspace),
			modular_group_size: self.modular_group_size.unwrap_or(d.modular_group_size),
			modular_predictor: self.modular_predictor.unwrap_or(d.modular_predictor),
			modular_nb_prev_channels: self.modular_nb_prev_channels.unwrap_or(d.modular_nb_prev_channels),
			modular_palette_colors: self.modular_palette_colors.unwrap_or(d.modular_palette_colors),
			modular_lossy_palette: self.modular_lossy_palette.unwrap_or(d.modular_lossy_palette),
			pre_compact: self.pre_compact.unwrap_or(d.pre_compact),
			post_compact: self.post_compact.unwrap_or(d.post_compact),
			responsive: self.responsive.or(d.responsive),
			multi_threading: self.multi_threading.unwrap_or(d.multi_threading),
			color_space: self.color_space.or(d.color_space),
			icc_file: self.icc_file.or(d.icc_file),
			exif: self.exif.unwrap_or(d.exif),
			xmp: self.xmp.unwrap_or(d.xmp),
			jumbf: self.jumbf.unwrap_or(d.jumbf),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn defaults_are_cjxls() {
		let s = JxlSettings::default();
		assert_eq!(s.target, JxlTarget::Quality(90.0));
		assert_eq!((s.effort, s.brotli_effort, s.faster_decoding), (7, 9, 0));
		assert!(s.lossless_jpeg && s.allow_jpeg_reconstruction);
		assert_eq!(s.validate(), Ok(()));
	}

	#[test]
	fn old_settings_still_load() {
		let old: JxlSettings = serde_json::from_str(r#"{"quality":80,"effort":3,"lossless":false,"losslessJpeg":false,"multiThreading":true}"#).expect("deserialize");
		assert_eq!((old.target, old.effort, old.lossless_jpeg), (JxlTarget::Quality(80.0), 3, false));
		let lossless: JxlSettings = serde_json::from_str(r#"{"quality":80,"lossless":true}"#).expect("deserialize");
		assert_eq!(lossless.target, JxlTarget::Distance(0.0), "the old lossless switch was -d 0");
		let round: JxlSettings = serde_json::from_value(serde_json::to_value(JxlSettings::default()).expect("serialize")).expect("deserialize");
		assert_eq!(round, JxlSettings::default());
	}

	#[test]
	fn ranges_are_cjxls() {
		let d = JxlSettings::default;
		assert!(JxlSettings { effort: 11, ..d() }.validate().is_err());
		assert_eq!(JxlSettings { effort: 11, allow_expert_options: true, ..d() }.validate(), Ok(()));
		assert!(JxlSettings { target: JxlTarget::Distance(25.5), ..d() }.validate().is_err());
		assert!(JxlSettings { resampling: 3, ..d() }.validate().is_err());
		assert!(JxlSettings { codestream_level: 7, ..d() }.validate().is_err());
		assert!(JxlSettings { center_x: 10, ..d() }.validate().is_err(), "centre needs centre-first order");
		assert_eq!(JxlSettings { center_x: 10, progressive: true, ..d() }.validate(), Ok(()), "which -p implies");
	}

	#[test]
	fn colour_space_descriptions_follow_libjxls_grammar() {
		let srgb = JxlColorSpace { gray: false, white_point: WhitePoint::D65, primaries: Primaries::Srgb, rendering_intent: RenderingIntent::Relative, transfer_function: TransferFunction::Srgb };
		assert_eq!(srgb.description(), "RGB_D65_SRG_Rel_SRG");
		let gray = JxlColorSpace { gray: true, transfer_function: TransferFunction::Gamma(0.45455), rendering_intent: RenderingIntent::Perceptual, ..srgb };
		assert_eq!(gray.description(), "Gra_D65_Per_g0.45455");
		let custom = JxlColorSpace { white_point: WhitePoint::Custom([0.3127, 0.329]), primaries: Primaries::Custom([0.64, 0.33, 0.3, 0.6, 0.15, 0.06]), ..srgb };
		assert_eq!(custom.description(), "RGB_0.3127;0.329_0.64;0.33;0.3;0.6;0.15;0.06_Rel_SRG");
	}
}
