//! `JxlSettingsPanel.vue`: every cjxl option that changes the file it writes for a still
//! image, one control per flag, with cjxl's own help text. -1 is cjxl's "the encoder
//! chooses" for its numeric options, and each three-way switch has "encoder decides" for
//! the flag left out.

use std::path::PathBuf;

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use skidbladnir_encode::settings::{JxlColorSpace, JxlSettings, JxlTarget, MetadataSource, Primaries, RenderingIntent, TransferFunction, Tristate, WhitePoint};

use crate::{
	app::Skid, panels::webp::to_u8
};

const TRISTATE: [(Tristate, &str, Option<&str>); 3] = [(Tristate::Default, "Encoder decides", None), (Tristate::Off, "Off", None), (Tristate::On, "On", None)];
const TARGETS: [(&str, &str, Option<&str>); 3] = [("default", "cjxl default", None), ("distance", "Distance", None), ("quality", "Quality", None)];
const RESPONSIVE: [(Option<bool>, &str, Option<&str>); 3] = [(None, "Not given", None), (Some(false), "Off (0)", None), (Some(true), "On (1)", None)];
const WHITE_POINTS: [(&str, &str, Option<&str>); 5] = [("d65", "D65", None), ("e", "E", None), ("dci", "DCI", None), ("d50", "D50", None), ("custom", "Custom xy", None)];
const INTENTS: [(RenderingIntent, &str, Option<&str>); 4] = [(RenderingIntent::Perceptual, "Perceptual", None), (RenderingIntent::Relative, "Relative", None), (RenderingIntent::Saturation, "Saturation", None), (RenderingIntent::Absolute, "Absolute", None)];
const METADATA: [(&str, &str, Option<&str>); 3] = [("keep", "From the input", None), ("strip", "Leave out", None), ("file", "From a file", None)];
const PRIMARY_LABELS: [&str; 6] = ["Red x", "Red y", "Green x", "Green y", "Blue x", "Blue y"];
const PREDICTOR_NAMES: [&str; 16] = ["zero", "left", "top", "avg0", "select", "gradient", "weighted", "topright", "topleft", "leftleft", "avg1", "avg2", "avg3", "toptop predictive average", "mix 5 and 6", "mix everything"];

const SRGB_SPACE: JxlColorSpace = JxlColorSpace { gray: false, white_point: WhitePoint::D65, primaries: Primaries::Srgb, rendering_intent: RenderingIntent::Relative, transfer_function: TransferFunction::Srgb };

/// A `USelect` item list from `(value, label)` pairs.
fn items<T: Copy>(pairs: &[(T, &str)]) -> Vec<(T, String)> {
	pairs.iter().map(|(value, label)| (*value, (*label).to_owned())).collect()
}

/// `choose(label)`: cjxl's -1.
fn choose(label: &str) -> (i8, String) {
	(-1, label.to_owned())
}

/// `numbers([...])`: each value labelled with itself.
fn numbers<T: Copy + std::fmt::Display>(values: &[T]) -> Vec<(T, String)> {
	values.iter().map(|value| (*value, value.to_string())).collect()
}

fn resampling() -> Vec<(i8, String)> {
	let mut list = vec![choose("Encoder chooses (only at very low quality)")];
	list.extend([1_i8, 2, 4, 8].iter().map(|value| (*value, format!("{value}×{value}"))));
	list
}

#[allow(clippy::too_many_lines)]
pub fn render(this: &mut Skid, window: &mut Window, cx: &mut Context<Skid>) -> AnyElement {
	let s = this.job().jxl.clone();

	// Quality.
	let target_kind = match s.target {
		JxlTarget::Default => "default",
		JxlTarget::Distance(_) => "distance",
		JxlTarget::Quality(_) => "quality",
	};
	let target = this.choice(
		"jxl-target",
		"Target",
		Some("-d / -q · mutually exclusive; cjxl's default is distance 1.0, or lossless for JPEG and GIF input"),
		&TARGETS,
		target_kind,
		false,
		false,
		false,
		|t, kind| {
			t.job().jxl.target = match kind {
				"distance" => JxlTarget::Distance(1.),
				"quality" => JxlTarget::Quality(90.),
				_ => JxlTarget::Default,
			};
		},
		window,
		cx,
	);
	let mut target_row = vec![target];
	match s.target {
		JxlTarget::Distance(distance) => target_row.push(this.slider(
			"jxl-distance",
			"Distance",
			f64::from(distance),
			0.,
			25.,
			0.1,
			Some(3),
			Some("-d · target visual distance in JND units; 0.0 = mathematically lossless, 1.0 = visually lossless"),
			false,
			None,
			|t, v| t.job().jxl.target = JxlTarget::Distance(to_f32(v)),
			window,
			cx,
		)),
		JxlTarget::Quality(quality) => target_row.push(this.slider("jxl-quality", "Quality", f64::from(quality), 0., 100., 1., Some(3), Some("-q · 100 = mathematically lossless, 90 = visually lossless"), false, None, |t, v| t.job().jxl.target = JxlTarget::Quality(to_f32(v)), window, cx)),
		JxlTarget::Default => {}
	}
	let alpha_body = s.alpha_distance.map(|alpha| this.slider("jxl-alpha-distance-value", "Alpha distance", f64::from(alpha), 0., 25., 0.1, Some(3), Some("-a · target visual distance for the alpha channel"), false, None, |t, v| t.job().jxl.alpha_distance = Some(to_f32(v)), window, cx));
	target_row.push(this.optional("jxl-alpha-distance", "Alpha distance", None, Some("-a · not given: 0.0, lossless alpha"), s.alpha_distance.is_some(), |t, on| t.job().jxl.alpha_distance = on.then_some(1.), alpha_body, window, cx));

	let effort = this.slider("jxl-effort", "Effort", f64::from(s.effort), 1., if s.allow_expert_options { 11. } else { 10. }, 1., None, Some("-e · encoder effort; higher values allow more computation"), false, None, |t, v| t.job().jxl.effort = to_u8(v), window, cx);
	let expert = this.toggle("jxl-allow-expert", "Allow effort 11", Some("--allow_expert_options · somewhat denser lossless compression at an extreme compute cost"), s.allow_expert_options, false, |t, v| t.job().jxl.allow_expert_options = v, window, cx);
	let lossless_jpeg = this.toggle("jxl-lossless-jpeg", "Transcode JPEG losslessly", Some("-j 1 · losslessly transcode JPEG data; off decodes it to pixels and reencodes. Not with a crop or resize."), s.lossless_jpeg, false, |t, v| t.job().jxl.lossless_jpeg = v, window, cx);
	let reconstruction = this.toggle("jxl-jpeg-reconstruction", "Allow JPEG reconstruction", Some("--allow_jpeg_reconstruction · store what is needed to rebuild the JPEG bit for bit"), s.allow_jpeg_reconstruction, false, |t, v| t.job().jxl.allow_jpeg_reconstruction = v, window, cx);

	let mut sections = vec![
		div()
			.pt(px(32.))
			.child(this.panel(
				"Quality",
				None,
				vec![Skid::grid(3, 16., target_row).into_any_element(), Skid::grid(3, 16., vec![effort, expert]).into_any_element(), Skid::grid(3, 16., vec![lossless_jpeg, reconstruction]).into_any_element()],
			))
			.into_any_element(),
	];

	// Mode and progression.
	let modular = this.choice("jxl-modular", "Modular mode", Some("-m · off = VarDCT, on = modular"), &TRISTATE, s.modular, false, false, false, |t, v| t.job().jxl.modular = v, window, cx);
	let progressive = this.toggle("jxl-progressive", "Progressive", Some("-p · more progressive/responsive decoding"), s.progressive, false, |t, v| t.job().jxl.progressive = v, window, cx);
	let responsive = this.choice("jxl-responsive", "Squeeze transform", Some("-R · the default is off for lossless output, on for lossy"), &RESPONSIVE, s.responsive, false, false, false, |t, v| t.job().jxl.responsive = v, window, cx);
	let group_order = this.choice("jxl-group-order", "Centre-first group order", Some("--group_order · 0 = scanline order, 1 = center-first order"), &TRISTATE, s.group_order, false, false, false, |t, v| t.job().jxl.group_order = v, window, cx);
	#[allow(clippy::cast_precision_loss)]
	let center_x = this.number("jxl-center-x", "Centre x", Some("--center_x · -1 = middle of the image"), s.center_x as f64, -1., i64::MAX as f64, 0, None, false, |t, v| t.job().jxl.center_x = to_i64(v), cx);
	#[allow(clippy::cast_precision_loss)]
	let center_y = this.number("jxl-center-y", "Centre y", Some("--center_y · -1 = middle of the image"), s.center_y as f64, -1., i64::MAX as f64, 0, None, false, |t, v| t.job().jxl.center_y = to_i64(v), cx);
	let progressive_ac = this.toggle("jxl-progressive-ac", "Progressive AC", Some("--progressive_ac · use the progressive mode for AC"), s.progressive_ac, false, |t, v| t.job().jxl.progressive_ac = v, window, cx);
	let qprogressive_ac = this.toggle("jxl-qprogressive-ac", "Quantised progressive AC", Some("--qprogressive_ac · progressive mode for AC with shift quantization"), s.qprogressive_ac, false, |t, v| t.job().jxl.qprogressive_ac = v, window, cx);
	let progressive_dc_items = {
		let mut list = vec![choose("Encoder chooses")];
		list.extend(items(&[(0_i8, "0: disable"), (1, "1: extra 64×64 pass"), (2, "2: extra 512×512 and 64×64 passes")]));
		list
	};
	let progressive_dc = this.select("jxl-progressive-dc", "Progressive DC", Some("--progressive_dc · number of progressive-DC frames"), &progressive_dc_items, s.progressive_dc, false, |t, v| t.job().jxl.progressive_dc = v, window, cx);
	sections.push(
		this.panel(
			"Mode and progression",
			None,
			vec![
				Skid::grid(3, 16., vec![modular, progressive, responsive]).into_any_element(),
				Skid::grid(3, 16., vec![group_order, center_x, center_y]).into_any_element(),
				Skid::grid(3, 16., vec![progressive_ac, qprogressive_ac, progressive_dc]).into_any_element(),
			],
		)
		.into_any_element(),
	);

	// Colour and metadata.
	let space_body = s.color_space.map(|space| colour_space(this, space, window, cx));
	let color_space = this.optional(
		"jxl-color-space",
		"Colour space of untagged input",
		Some("-x color_space · used only when the input says nothing about its colour"),
		Some("-x color_space · not given"),
		s.color_space.is_some(),
		|t, on| t.job().jxl.color_space = on.then_some(SRGB_SPACE),
		space_body,
		window,
		cx,
	);
	let icc_body = s.icc_file.as_ref().map(|path| this.file("jxl-icc-file", "ICC profile", Some("-x icc_pathname · a binary file containing an ICC profile"), &path.display().to_string(), |t, v| t.job().jxl.icc_file = Some(PathBuf::from(v)), window, cx));
	let icc = this.optional("jxl-icc", "ICC profile for untagged input", None, Some("-x icc_pathname · not given"), s.icc_file.is_some(), |t, on| t.job().jxl.icc_file = on.then(PathBuf::new), icc_body, window, cx);
	let intensity = this.number("jxl-intensity-target", "Intensity target", Some("--intensity_target · 0 = choose a sensible value based on the color encoding"), f64::from(s.intensity_target), 0., f64::from(f32::MAX), 3, Some("nits"), false, |t, v| t.job().jxl.intensity_target = to_f32(v), cx);
	let bitdepth = this.number("jxl-override-bitdepth", "Bit depth", Some("--override_bitdepth · 0 = use the input image bit depth"), f64::from(s.override_bitdepth), 0., 32., 0, None, false, |t, v| t.job().jxl.override_bitdepth = to_u8(v), cx);
	let numbers_column = div().flex().flex_col().child(intensity).child(bitdepth).into_any_element();
	let premultiply = this.select("jxl-premultiply", "Premultiplied alpha", Some("--premultiply · force premultiplied (associated) alpha"), &items(&[(-1_i8, "As the input is"), (0, "Not premultiplied"), (1, "Premultiplied")]), s.premultiply, false, |t, v| t.job().jxl.premultiply = v, window, cx);
	let keep_invisible = this.choice("jxl-keep-invisible", "Keep invisible pixels", Some("--keep_invisible · preserve colors of invisible pixels; on by default for lossless"), &TRISTATE, s.keep_invisible, false, false, false, |t, v| t.job().jxl.keep_invisible = v, window, cx);
	let exif = metadata(this, "jxl-exif", "Exif", "-x exif=FILE / -x strip=exif", &s.exif, |s| &mut s.exif, window, cx);
	let xmp = metadata(this, "jxl-xmp", "XMP", "-x xmp=FILE / -x strip=xmp", &s.xmp, |s| &mut s.xmp, window, cx);
	let jumbf = metadata(this, "jxl-jumbf", "JUMBF", "-x jumbf=FILE / -x strip=jumbf", &s.jumbf, |s| &mut s.jumbf, window, cx);
	let container = this.choice("jxl-container", "Container", Some("--container · 0 = only when needed, 1 = always"), &TRISTATE, s.container, false, false, false, |t, v| t.job().jxl.container = v, window, cx);
	let compress_boxes = this.choice("jxl-compress-boxes", "Compress metadata boxes", Some("--compress_boxes · Brotli compression for metadata boxes"), &TRISTATE, s.compress_boxes, false, false, false, |t, v| t.job().jxl.compress_boxes = v, window, cx);
	let brotli = this.slider("jxl-brotli-effort", "Brotli effort", f64::from(s.brotli_effort), 0., 11., 1., None, Some("--brotli_effort · higher values target higher density"), false, None, |t, v| t.job().jxl.brotli_effort = to_u8(v), window, cx);
	sections.push(
		this.panel(
			"Colour and metadata",
			None,
			vec![
				Skid::grid(3, 16., vec![color_space, icc, numbers_column]).into_any_element(),
				Skid::grid(3, 16., vec![premultiply, keep_invisible]).into_any_element(),
				Skid::grid(3, 16., vec![exif, xmp, jumbf]).into_any_element(),
				Skid::grid(3, 16., vec![container, compress_boxes, brotli]).into_any_element(),
			],
		)
		.into_any_element(),
	);

	// Filters and tools.
	let f = s.clone();
	sections.push(this.disclosure(
		"jxl-filters",
		"Filters and tools",
		false,
		move |this, window, cx| {
			let epf_items = {
				let mut list = vec![choose("Encoder chooses")];
				list.extend(numbers(&[0_i8, 1, 2, 3]));
				list
			};
			let epf = this.select("jxl-epf", "Edge-preserving filter", Some("--epf · edge preserving filter strength"), &epf_items, f.epf, false, |t, v| t.job().jxl.epf = v, window, cx);
			let gaborish = this.choice("jxl-gaborish", "Gaborish filter", Some("--gaborish"), &TRISTATE, f.gaborish, false, false, false, |t, v| t.job().jxl.gaborish = v, window, cx);
			let faster = this.select("jxl-faster-decoding", "Faster decoding", Some("--faster_decoding · decode speed at the expense of quality or density"), &numbers(&[0_u8, 1, 2, 3, 4]), f.faster_decoding, false, |t, v| t.job().jxl.faster_decoding = v, window, cx);
			let photon = this.number("jxl-photon-noise", "Photon noise", Some("--photon_noise_iso · emulate film or sensor noise; 100 is low, 3200 a lot"), f64::from(f.photon_noise_iso), 0., f64::from(f32::MAX), 3, Some("ISO"), false, |t, v| t.job().jxl.photon_noise_iso = to_f32(v), cx);
			let noise = this.choice("jxl-noise", "Adaptive noise", Some("--noise · --photon_noise_iso is recommended instead"), &TRISTATE, f.noise, false, false, false, |t, v| t.job().jxl.noise = v, window, cx);
			let dots = this.choice("jxl-dots", "Dots", Some("--dots · dots generation"), &TRISTATE, f.dots, false, false, false, |t, v| t.job().jxl.dots = v, window, cx);
			let patches = this.choice("jxl-patches", "Patches", Some("--patches · patches generation"), &TRISTATE, f.patches, false, false, false, |t, v| t.job().jxl.patches = v, window, cx);
			let cfl = this.choice("jxl-jpeg-cfl", "Chroma-from-luma for JPEG", Some("--jpeg_reconstruction_cfl · CFL for lossless JPEG reconstruction"), &TRISTATE, f.jpeg_reconstruction_cfl, false, false, false, |t, v| t.job().jxl.jpeg_reconstruction_cfl = v, window, cx);
			let perceptual = this.toggle("jxl-no-perceptual", "No perceptual optimisations", Some("--disable_perceptual_optimizations"), f.disable_perceptual_optimizations, false, |t, v| t.job().jxl.disable_perceptual_optimizations = v, window, cx);
			let resampling_items = resampling();
			let resampling = this.select("jxl-resampling", "Resampling", Some("--resampling · for color channels"), &resampling_items, f.resampling, false, |t, v| t.job().jxl.resampling = v, window, cx);
			let ec_resampling = this.select("jxl-ec-resampling", "Extra channel resampling", Some("--ec_resampling · for extra channels like alpha"), &resampling_items, f.ec_resampling, false, |t, v| t.job().jxl.ec_resampling = v, window, cx);
			let upsampling = this.select("jxl-upsampling", "Upsampling", Some("--upsampling_mode · decoder upsampling; nearest neighbour for pixel art"), &items(&[(-1_i8, "Non-separable (default)"), (0, "Nearest neighbour"), (1, "1")]), f.upsampling_mode, false, |t, v| t.job().jxl.upsampling_mode = v, window, cx);
			let downsampled = this.toggle("jxl-already-downsampled", "Already downsampled", Some("--already_downsampled · signal upsampling without downsampling first"), f.already_downsampled, false, |t, v| t.job().jxl.already_downsampled = v, window, cx);
			vec![
				Skid::grid(3, 16., vec![epf, gaborish, faster]).into_any_element(),
				Skid::grid(3, 16., vec![photon, noise, dots]).into_any_element(),
				Skid::grid(3, 16., vec![patches, cfl, perceptual]).into_any_element(),
				Skid::grid(3, 16., vec![resampling, ec_resampling, upsampling]).into_any_element(),
				Skid::grid(3, 16., vec![downsampled]).into_any_element(),
			]
		},
		window,
		cx,
	));

	// Modular mode.
	let m = s.clone();
	sections.push(this.disclosure(
		"jxl-modular-section",
		"Modular mode",
		false,
		move |this, window, cx| {
			let iterations = this.number("jxl-iterations", "MA tree learning", Some("-I · percentage of pixels used to learn MA trees; -1 = encoder chooses, 0 = no MA trees"), f64::from(m.iterations), -1., 100., 3, Some("%"), false, |t, v| t.job().jxl.iterations = to_f32(v), cx);
			let colorspace = this.number("jxl-modular-colorspace", "Colour transform", Some("-C · -1 = try several, 0 = none, 1..41 = fixed RCT, 6 = YCoCg"), f64::from(m.modular_colorspace), -1., 41., 0, None, false, |t, v| t.job().jxl.modular_colorspace = to_i8(v), cx);
			let group_items = {
				let mut list = vec![choose("Encoder chooses")];
				list.extend(items(&[(0_i8, "128×128"), (1, "256×256"), (2, "512×512"), (3, "1024×1024")]));
				list
			};
			let group_size = this.select("jxl-group-size", "Group size", Some("-g · modular group size"), &group_items, m.modular_group_size, false, |t, v| t.job().jxl.modular_group_size = v, window, cx);
			let predictor_items = {
				let mut list = vec![choose("Encoder chooses (14, or 15 at effort 10)")];
				list.extend(PREDICTOR_NAMES.iter().zip(0_i8..).map(|(name, value)| (value, format!("{value}: {name}"))));
				list
			};
			let predictor = this.select("jxl-predictor", "Predictor", Some("-P · predictor(s) to use"), &predictor_items, m.modular_predictor, false, |t, v| t.job().jxl.modular_predictor = v, window, cx);
			let prev_channels = this.number("jxl-prev-channels", "Previous-channel properties", Some("-E · maximum previous-channel MA tree properties; -1 = encoder chooses"), f64::from(m.modular_nb_prev_channels), -1., 11., 0, None, false, |t, v| t.job().jxl.modular_nb_prev_channels = to_i8(v), cx);
			#[allow(clippy::cast_precision_loss)]
			let palette = this.number("jxl-palette-colors", "Palette colours", Some("--modular_palette_colors · use a palette at or below this many colours; -1 = encoder chooses"), m.modular_palette_colors as f64, -1., i64::MAX as f64, 0, None, false, |t, v| t.job().jxl.modular_palette_colors = to_i64(v), cx);
			let lossy_palette = this.toggle("jxl-lossy-palette", "Lossy palette", Some("--modular_lossy_palette · use delta palette in a lossy way"), m.modular_lossy_palette, false, |t, v| t.job().jxl.modular_lossy_palette = v, window, cx);
			let pre_compact = this.number("jxl-pre-compact", "Global channel palette", Some("-X · global channel palette below this share of the range; -1 = encoder chooses"), f64::from(m.pre_compact), -1., 100., 3, Some("%"), false, |t, v| t.job().jxl.pre_compact = to_f32(v), cx);
			let post_compact = this.number("jxl-post-compact", "Per-group channel palette", Some("-Y · local channel palette below this share of the range; -1 = encoder chooses"), f64::from(m.post_compact), -1., 100., 3, Some("%"), false, |t, v| t.job().jxl.post_compact = to_f32(v), cx);
			vec![
				Skid::grid(3, 16., vec![iterations, colorspace, group_size]).into_any_element(),
				Skid::grid(3, 16., vec![predictor, prev_channels, palette]).into_any_element(),
				Skid::grid(3, 16., vec![lossy_palette, pre_compact, post_compact]).into_any_element(),
			]
		},
		window,
		cx,
	));

	// Codestream and output.
	let o = s;
	sections.push(this.disclosure(
		"jxl-codestream",
		"Codestream and output",
		false,
		move |this, window, cx| {
			let level_items = {
				let mut list = vec![choose("Encoder chooses")];
				list.extend(numbers(&[5_i8, 10]));
				list
			};
			let level = this.select("jxl-codestream-level", "Codestream level", Some("--codestream_level"), &level_items, o.codestream_level, false, |t, v| t.job().jxl.codestream_level = v, window, cx);
			let buffering_items = {
				let mut list = vec![choose("Encoder chooses")];
				list.extend(items(&[(0_i8, "0: buffer the entire image"), (1, "1: stream input for large images"), (2, "2: stream with a lower threshold"), (3, "3: deprecated")]));
				list
			};
			let buffering = this.select("jxl-buffering", "Buffering", Some("--buffering · how much input buffering libjxl uses"), &buffering_items, o.buffering, false, |t, v| t.job().jxl.buffering = v, window, cx);
			let output_items = {
				let mut list = vec![choose("Encoder decides")];
				list.extend(items(&[(0_i8, "0: buffer output internally"), (1, "1: streaming with seeking"), (2, "2: out-of-order jxlp")]));
				list
			};
			let output_mode = this.select("jxl-output-mode", "Output mode", Some("--output_mode"), &output_items, o.output_mode, false, |t, v| t.job().jxl.output_mode = v, window, cx);
			let streaming = this.toggle("jxl-streaming-output", "Streaming output", Some("--streaming_output · incremental writing of the output file"), o.streaming_output, false, |t, v| t.job().jxl.streaming_output = v, window, cx);
			let frame_index = this.toggle("jxl-frame-index", "Frame index box", Some("--frame_indexing=1 · index the frame in a frame index box"), o.frame_index_box, false, |t, v| t.job().jxl.frame_index_box = v, window, cx);
			let threads = this.toggle("jxl-mt", "Multi-threading", Some("--num_threads · off is 0, no multithreading; the output is the same"), o.multi_threading, false, |t, v| t.job().jxl.multi_threading = v, window, cx);
			vec![Skid::grid(3, 16., vec![level, buffering, output_mode]).into_any_element(), Skid::grid(3, 16., vec![streaming, frame_index, threads]).into_any_element()]
		},
		window,
		cx,
	));

	div().flex().flex_col().gap(px(32.)).children(sections).into_any_element()
}

/// The slot of the colour-space `ControlOptional`: grayscale, white point, primaries,
/// rendering intent and transfer function, each with its custom values when chosen.
#[allow(clippy::too_many_lines)]
fn colour_space(this: &mut Skid, space: JxlColorSpace, window: &Window, cx: &mut Context<Skid>) -> AnyElement {
	let mut body: Vec<AnyElement> = Vec::new();
	body.push(this.toggle("jxl-cs-gray", "Grayscale", Some("must match the image"), space.gray, false, |t, v| edit_space(t, |space| space.gray = v), window, cx));
	let white_kind = match space.white_point {
		WhitePoint::D65 => "d65",
		WhitePoint::E => "e",
		WhitePoint::Dci => "dci",
		WhitePoint::D50 => "d50",
		WhitePoint::Custom(_) => "custom",
	};
	body.push(this.choice(
		"jxl-cs-white",
		"White point",
		None,
		&WHITE_POINTS,
		white_kind,
		false,
		false,
		false,
		|t, kind| {
			edit_space(t, |space| {
				space.white_point = match kind {
					"e" => WhitePoint::E,
					"dci" => WhitePoint::Dci,
					"d50" => WhitePoint::D50,
					"custom" => WhitePoint::Custom([0.3127, 0.329]),
					_ => WhitePoint::D65,
				};
			});
		},
		window,
		cx,
	));
	if let WhitePoint::Custom(xy) = space.white_point {
		let x = this.number("jxl-cs-white-x", "White x", None, xy[0], f64::MIN, f64::MAX, 6, None, false, |t, v| edit_space(t, |space| set_white(space, 0, v)), cx);
		let y = this.number("jxl-cs-white-y", "White y", None, xy[1], f64::MIN, f64::MAX, 6, None, false, |t, v| edit_space(t, |space| set_white(space, 1, v)), cx);
		body.push(Skid::grid(2, 8., vec![x, y]).into_any_element());
	}
	if !space.gray {
		let primaries_kind = match space.primaries {
			Primaries::Srgb => "srgb",
			Primaries::Rec2100 => "rec2100",
			Primaries::P3 => "p3",
			Primaries::Adobe => "adobe",
			Primaries::ProPhoto => "proPhoto",
			Primaries::Custom(_) => "custom",
		};
		let primaries = items(&[("srgb", "sRGB / BT.709"), ("rec2100", "BT.2020 / BT.2100"), ("p3", "DCI-P3"), ("adobe", "Adobe RGB (1998)"), ("proPhoto", "ProPhoto RGB"), ("custom", "Custom xy")]);
		body.push(this.select(
			"jxl-cs-primaries",
			"Primaries",
			None,
			&primaries,
			primaries_kind,
			false,
			|t, kind| {
				edit_space(t, |space| {
					space.primaries = match kind {
						"rec2100" => Primaries::Rec2100,
						"p3" => Primaries::P3,
						"adobe" => Primaries::Adobe,
						"proPhoto" => Primaries::ProPhoto,
						"custom" => Primaries::Custom([0.64, 0.33, 0.3, 0.6, 0.15, 0.06]),
						_ => Primaries::Srgb,
					};
				});
			},
			window,
			cx,
		));
		if let Primaries::Custom(xy) = space.primaries {
			let fields = (0..6).map(|index| this.number(&format!("jxl-cs-primary-{index}"), PRIMARY_LABELS[index], None, xy[index], f64::MIN, f64::MAX, 6, None, false, move |t, v| edit_space(t, |space| set_primary(space, index, v)), cx)).collect();
			body.push(Skid::grid(2, 8., fields).into_any_element());
		}
	}
	body.push(this.choice("jxl-cs-intent", "Rendering intent", None, &INTENTS, space.rendering_intent, false, false, false, |t, v| edit_space(t, |space| space.rendering_intent = v), window, cx));
	let transfer_kind = match space.transfer_function {
		TransferFunction::Srgb => "srgb",
		TransferFunction::Bt709 => "bt709",
		TransferFunction::Linear => "linear",
		TransferFunction::Pq => "pq",
		TransferFunction::Hlg => "hlg",
		TransferFunction::Dci => "dci",
		TransferFunction::Adobe => "adobe",
		TransferFunction::ProPhoto => "proPhoto",
		TransferFunction::Gamma(_) => "gamma",
	};
	let transfers = items(&[("srgb", "sRGB"), ("bt709", "BT.709"), ("linear", "Linear"), ("pq", "PQ"), ("hlg", "HLG"), ("dci", "DCI"), ("adobe", "Adobe"), ("proPhoto", "ProPhoto"), ("gamma", "Gamma")]);
	body.push(this.select(
		"jxl-cs-transfer",
		"Transfer function",
		None,
		&transfers,
		transfer_kind,
		false,
		|t, kind| {
			edit_space(t, |space| {
				space.transfer_function = match kind {
					"bt709" => TransferFunction::Bt709,
					"linear" => TransferFunction::Linear,
					"pq" => TransferFunction::Pq,
					"hlg" => TransferFunction::Hlg,
					"dci" => TransferFunction::Dci,
					"adobe" => TransferFunction::Adobe,
					"proPhoto" => TransferFunction::ProPhoto,
					"gamma" => TransferFunction::Gamma(0.45455),
					_ => TransferFunction::Srgb,
				};
			});
		},
		window,
		cx,
	));
	if let TransferFunction::Gamma(gamma) = space.transfer_function {
		body.push(this.number(
			"jxl-cs-gamma",
			"Gamma",
			Some("the encoding exponent: 0.45455 is 2.2"),
			gamma,
			0.00001,
			1.,
			6,
			None,
			false,
			|t, v| {
				edit_space(t, |space| {
					if let TransferFunction::Gamma(gamma) = &mut space.transfer_function {
						*gamma = v;
					}
				});
			},
			cx,
		));
	}
	div().flex().flex_col().children(body).into_any_element()
}

fn edit_space(this: &mut Skid, edit: impl FnOnce(&mut JxlColorSpace)) {
	if let Some(space) = this.job().jxl.color_space.as_mut() {
		edit(space);
	}
}

fn set_white(space: &mut JxlColorSpace, index: usize, value: f64) {
	if let WhitePoint::Custom(xy) = &mut space.white_point {
		xy[index] = value;
	}
}

fn set_primary(space: &mut JxlColorSpace, index: usize, value: f64) {
	if let Primaries::Custom(xy) = &mut space.primaries {
		xy[index] = value;
	}
}

/// `ControlMetadata`: from the input, left out, or from a file, with the file chooser
/// indented under it while "From a file" is chosen.
#[allow(clippy::too_many_arguments)]
fn metadata(this: &mut Skid, id: &str, label: &str, help_text: &str, value: &MetadataSource, field: fn(&mut JxlSettings) -> &mut MetadataSource, window: &Window, cx: &mut Context<Skid>) -> AnyElement {
	let kind = match value {
		MetadataSource::Keep => "keep",
		MetadataSource::Strip => "strip",
		MetadataSource::File(_) => "file",
	};
	let choice = this.choice(
		id,
		label,
		Some(help_text),
		&METADATA,
		kind,
		false,
		false,
		false,
		move |t, kind| {
			let source = field(&mut t.job().jxl);
			*source = match kind {
				"file" => MetadataSource::File(if let MetadataSource::File(path) = source { std::mem::take(path) } else { PathBuf::new() }),
				"strip" => MetadataSource::Strip,
				_ => MetadataSource::Keep,
			};
		},
		window,
		cx,
	);
	let file = match value {
		MetadataSource::File(path) => Some(div().pl(px(20.)).child(this.file(&format!("{id}-file"), &format!("{label} file"), None, &path.display().to_string(), move |t, v| *field(&mut t.job().jxl) = MetadataSource::File(PathBuf::from(v)), window, cx))),
		_ => None,
	};
	div().flex().flex_col().min_w_0().child(choice).children(file).into_any_element()
}

#[allow(clippy::cast_possible_truncation)]
fn to_f32(value: f64) -> f32 {
	value as f32
}

#[allow(clippy::cast_possible_truncation)]
fn to_i8(value: f64) -> i8 {
	value.round().clamp(f64::from(i8::MIN), f64::from(i8::MAX)) as i8
}

#[allow(clippy::cast_possible_truncation)]
fn to_i64(value: f64) -> i64 {
	value.round() as i64
}
