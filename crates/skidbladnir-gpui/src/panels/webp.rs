//! `WebpSettingsPanel.vue`: every cwebp option that changes the file it writes, one control
//! per flag, with cwebp's own help text.

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use skidbladnir_encode::settings::{AlphaFiltering, FilterType, FrameChange, ImageHint, Preset, TargetMetric};

use crate::{
	app::Skid, controls::{Press, Ring, button_soft, field, field_label, help}, theme::{ERROR, Type, c}
};

const FILTER_TYPES: [(FilterType, &str, Option<&str>); 2] = [(FilterType::Strong, "Strong", None), (FilterType::Simple, "Simple", None)];
const ALPHA_FILTERS: [(AlphaFiltering, &str, Option<&str>); 3] = [(AlphaFiltering::Off, "None", None), (AlphaFiltering::Fast, "Fast", None), (AlphaFiltering::Best, "Best", None)];
const HINTS: [(ImageHint, &str, Option<&str>); 4] = [(ImageHint::Default, "None", None), (ImageHint::Photo, "Photo", None), (ImageHint::Picture, "Picture", None), (ImageHint::Graph, "Graph", None)];

#[derive(Clone, Copy, PartialEq)]
enum Target {
	Quality,
	Size,
	Psnr,
}

#[allow(clippy::too_many_lines)]
pub fn render(this: &mut Skid, window: &mut Window, cx: &mut Context<Skid>) -> AnyElement {
	let s = this.job().webp.clone();

	// Compression.
	let lossless = this.toggle("webp-lossless", "Lossless", Some("-lossless · encode image losslessly"), s.lossless, false, |t, v| t.job().webp.lossless = v, window, cx);
	let quality = this.slider(
		"webp-quality",
		if s.lossless { "Compression effort" } else { "Quality" },
		f64::from(s.quality),
		0.,
		100.,
		1.,
		Some(3),
		Some(if s.lossless { "-q · 0 is fastest and largest, 100 slowest and smallest; not fidelity" } else { "-q · quality factor (0:small..100:big)" }),
		false,
		None,
		#[allow(clippy::cast_possible_truncation)]
		|t, v| t.job().webp.quality = v as f32,
		window,
		cx,
	);
	let method = this.slider("webp-method", "Compression method", f64::from(s.method), 0., 6., 1., None, Some("-m · compression method (0=fast, 6=slowest)"), false, None, |t, v| t.job().webp.method = to_u8(v), window, cx);
	let target = match s.target {
		None => Target::Quality,
		Some(TargetMetric::Size(_)) => Target::Size,
		Some(TargetMetric::Psnr(_)) => Target::Psnr,
	};
	let aim = this.choice(
		"webp-target",
		"Aim for",
		Some("-size / -psnr · a target overrides the quality"),
		&[(Target::Quality, "Quality", None), (Target::Size, "File size", None), (Target::Psnr, "PSNR", None)],
		target,
		false,
		false,
		false,
		|t, kind| {
			t.job().webp.target = match kind {
				Target::Quality => None,
				Target::Size => Some(TargetMetric::Size(51200)),
				Target::Psnr => Some(TargetMetric::Psnr(42.)),
			};
		},
		window,
		cx,
	);
	let mut target_row = vec![aim];
	match s.target {
		Some(TargetMetric::Size(bytes)) => target_row.push(this.number("webp-target-size", "Target size", Some("-size · target size (in bytes)"), f64::from(bytes), 1., f64::from(u32::MAX), 0, Some("bytes"), false, |t, v| t.job().webp.target = Some(TargetMetric::Size(to_u32(v))), cx)),
		Some(TargetMetric::Psnr(psnr)) => target_row.push(this.number(
			"webp-target-psnr",
			"Target PSNR",
			Some("-psnr · target PSNR (in dB, typically 42)"),
			f64::from(psnr),
			0.001,
			10000.,
			3,
			Some("dB"),
			false,
			#[allow(clippy::cast_possible_truncation)]
			|t, v| t.job().webp.target = Some(TargetMetric::Psnr(v as f32)),
			cx,
		)),
		None => {}
	}

	let presets: Vec<(Preset, String)> = [(Preset::Default, "default"), (Preset::Photo, "photo"), (Preset::Picture, "picture"), (Preset::Drawing, "drawing"), (Preset::Icon, "icon"), (Preset::Text, "text")].into_iter().map(|(value, label)| (value, label.to_owned())).collect();
	let preset_picker = this.select_box("webp-preset", "libwebp preset", &presets, this.webp_preset, false, |t, v| t.webp_preset = v, window, cx);
	let levels: Vec<(u8, String)> = (0..=9)
		.map(|level| {
			(
				level,
				format!(
					"-z {level}{}",
					if level == 0 {
						" (fastest)"
					} else if level == 9 {
						" (slowest)"
					} else {
						""
					}
				),
			)
		})
		.collect();
	let level_picker = this.select_box("webp-level", "Lossless level", &levels, this.lossless_level, false, |t, v| t.lossless_level = v, window, cx);
	let preset_field = field()
		.child(field_label("libwebp preset"))
		.child(div().flex().items_center().gap(px(8.)).child(preset_picker.flex_1().min_w_0()).child(button_soft("webp-preset-apply", "Apply", false).press(
			this,
			"webp-preset-apply",
			"Apply",
			Ring::Neutral,
			9.,
			|t, _, cx| {
				t.webp_preset_error.clear();
				let preset = t.webp_preset;
				t.job().webp.apply_preset(preset);
				t.changed(cx);
			},
			window,
			cx,
		)))
		.child(help("-preset · resets every encoder control to the preset, keeping the quality"))
		.into_any_element();
	let level_field = field()
		.child(field_label("Lossless level"))
		.child(div().flex().items_center().gap(px(8.)).child(level_picker.flex_1().min_w_0()).child(button_soft("webp-level-apply", "Apply", false).press(
			this,
			"webp-level-apply",
			"Apply",
			Ring::Neutral,
			9.,
			|t, _, cx| {
				t.webp_preset_error.clear();
				let level = t.lossless_level;
				if let Err(error) = t.job().webp.apply_lossless_preset(level) {
					t.webp_preset_error = error.to_string();
				}
				t.changed(cx);
			},
			window,
			cx,
		)))
		.child(help("-z · lossless, with the method and effort of the level"))
		.into_any_element();

	let defaults_field = field()
		.child(field_label("cwebp's defaults"))
		.child(div().flex().items_center().gap(px(8.)).child(button_soft("webp-cwebp-defaults", "Start from cwebp's defaults", false).press(
			this,
			"webp-cwebp-defaults",
			"Start from cwebp's defaults",
			Ring::Neutral,
			9.,
			|t, _, cx| {
				t.webp_preset_error.clear();
				t.job().webp.reset_to_cwebp_defaults();
				t.changed(cx);
			},
			window,
			cx,
		)))
		.child(help("no options · every control as a plain cwebp sets it, so the file matches cwebp with no flags"))
		.into_any_element();
	let mut compression = vec![Skid::grid(3, 16., vec![lossless, quality, method]).into_any_element(), Skid::grid(3, 16., target_row).into_any_element(), Skid::grid(3, 16., vec![preset_field, level_field, defaults_field]).into_any_element()];
	if !this.webp_preset_error.is_empty() {
		compression.push(div().px(px(10.)).xs().text_color(c(ERROR)).child(this.webp_preset_error.clone()).into_any_element());
	}
	let mut sections = vec![div().pt(px(32.)).child(this.panel("Compression", None, compression)).into_any_element()];

	if s.lossless {
		let near = this.slider("webp-near-lossless", "Near-lossless", f64::from(s.near_lossless), 0., 100., 1., None, Some("-near_lossless · use near-lossless image preprocessing (0..100=off)"), false, None, |t, v| t.job().webp.near_lossless = to_u8(v), window, cx);
		let hint = this.choice("webp-hint", "Image hint", Some("-hint · specify image characteristics hint"), &HINTS, s.image_hint, false, false, false, |t, v| t.job().webp.image_hint = v, window, cx);
		sections.push(this.panel("Lossless", None, vec![Skid::grid(3, 16., vec![near, hint]).into_any_element()]).into_any_element());
	}

	// Transparency.
	let discard = this.toggle("webp-noalpha", "Discard transparency", Some("-noalpha · discard any transparency information"), !s.keep_alpha, false, |t, discard| t.job().webp.keep_alpha = !discard, window, cx);
	let exact = this.toggle("webp-exact", "Exact", Some("-exact · preserve RGB values in transparent area"), s.exact, false, |t, v| t.job().webp.exact = v, window, cx);
	let blend_body = s.blend_alpha.map(|colour| this.color("webp-blend", "Background", colour, |t, v| t.job().webp.blend_alpha = Some(v), window, cx));
	let blend = this.optional("webp-blend", "Blend onto a background", Some("-blend_alpha · blend colors against background color"), Some("-blend_alpha · not given: transparency is kept"), s.blend_alpha.is_some(), |t, on| t.job().webp.blend_alpha = on.then_some(0x00ff_ffff), blend_body, window, cx);
	let alpha_quality = this.slider("webp-alpha-q", "Alpha quality", f64::from(s.alpha_quality), 0., 100., 1., None, Some("-alpha_q · transparency-compression quality (0..100)"), false, None, |t, v| t.job().webp.alpha_quality = to_u8(v), window, cx);
	let alpha_method = this.choice("webp-alpha-method", "Alpha compression", Some("-alpha_method · transparency-compression method (0..1)"), &[(true, "Lossless", None), (false, "None", None)], s.alpha_compression, false, false, false, |t, v| t.job().webp.alpha_compression = v, window, cx);
	let alpha_filter = this.choice("webp-alpha-filter", "Alpha filter", Some("-alpha_filter · predictive filtering for alpha plane"), &ALPHA_FILTERS, s.alpha_filtering, false, false, false, |t, v| t.job().webp.alpha_filtering = v, window, cx);
	sections.push(this.panel("Transparency", None, vec![Skid::grid(3, 16., vec![discard, exact, blend]).into_any_element(), Skid::grid(3, 16., vec![alpha_quality, alpha_method, alpha_filter]).into_any_element()]).into_any_element());

	// Lossy tuning, folded away while lossless.
	sections.push(this.disclosure(
		"webp-lossy",
		"Lossy tuning",
		!s.lossless,
		move |this, window, cx| {
			let sns = this.slider("webp-sns", "Spatial noise shaping", f64::from(s.sns), 0., 100., 1., None, Some("-sns · spatial noise shaping (0:off, 100:max)"), false, None, |t, v| t.job().webp.sns = to_u8(v), window, cx);
			let segments = this.slider("webp-segments", "Segments", f64::from(s.segments), 1., 4., 1., None, Some("-segments · number of segments to use (1..4)"), false, None, |t, v| t.job().webp.segments = to_u8(v), window, cx);
			let passes = this.slider("webp-pass", "Analysis passes", f64::from(s.passes), 1., 10., 1., None, Some("-pass · analysis pass number (1..10)"), false, None, |t, v| t.job().webp.passes = to_u8(v), window, cx);
			let partition = this.slider("webp-partition", "Partition limit", f64::from(s.partition_limit), 0., 100., 1., None, Some("-partition_limit · limit quality to fit the 512k limit on the first partition (0=no degradation ... 100=full)"), false, None, |t, v| t.job().webp.partition_limit = to_u8(v), window, cx);
			let filter_type = this.choice("webp-filter-type", "Filter type", Some("-strong / -nostrong · strong or simple deblocking filter"), &FILTER_TYPES, s.filter_type, false, false, false, |t, v| t.job().webp.filter_type = v, window, cx);
			let autofilter = this.toggle("webp-af", "Auto filter", Some("-af · auto-adjust filter strength"), s.autofilter, false, |t, v| t.job().webp.autofilter = v, window, cx);
			let strength = this.slider("webp-f", "Filter strength", f64::from(s.filter_strength), 0., 100., 1., None, Some("-f · filter strength (0=off..100); -af adjusts it per segment"), false, None, |t, v| t.job().webp.filter_strength = to_u8(v), window, cx);
			let sharpness = this.slider("webp-sharpness", "Filter sharpness", f64::from(s.filter_sharpness), 0., 7., 1., None, Some("-sharpness · filter sharpness (0:most .. 7:least sharp)"), false, None, |t, v| t.job().webp.filter_sharpness = to_u8(v), window, cx);
			let qmin = this.slider("webp-qmin", "Minimum quality", f64::from(s.qmin), 0., 100., 1., None, Some("-qrange · the permissible quality range, low end"), false, None, |t, v| t.job().webp.qmin = to_u8(v), window, cx);
			let qmax = this.slider("webp-qmax", "Maximum quality", f64::from(s.qmax), 0., 100., 1., None, Some("-qrange · the permissible quality range, high end"), false, None, |t, v| t.job().webp.qmax = to_u8(v), window, cx);
			let pre = this.slider("webp-pre", "Pre-processing", f64::from(s.preprocessing), 0., 7., 1., None, Some("-pre · pre-processing filter: 1 smooths segments, 2 dithers"), false, None, |t, v| t.job().webp.preprocessing = to_u8(v), window, cx);
			let sharp_yuv = this.toggle("webp-sharp-yuv", "Sharp YUV", Some("-sharp_yuv · use sharper (and slower) RGB->YUV conversion"), s.sharp_yuv, false, |t, v| t.job().webp.sharp_yuv = v, window, cx);
			let jpeg_like = this.toggle("webp-jpeg-like", "JPEG-like", Some("-jpeg_like · roughly match expected JPEG size"), s.jpeg_like, false, |t, v| t.job().webp.jpeg_like = v, window, cx);
			vec![Skid::grid(4, 12., vec![sns, segments, passes, partition]).into_any_element(), Skid::grid(4, 12., vec![filter_type, autofilter, strength, sharpness]).into_any_element(), Skid::grid(4, 12., vec![qmin, qmax, pre]).into_any_element(), Skid::grid(4, 12., vec![sharp_yuv, jpeg_like]).into_any_element()]
		},
		window,
		cx,
	));

	let exif = this.toggle("webp-exif", "Copy Exif", Some("-metadata exif · copy Exif from the input if present"), s.metadata.exif, false, |t, v| t.job().webp.metadata.exif = v, window, cx);
	let icc = this.toggle("webp-icc", "Copy ICC profile", Some("-metadata icc · copy the colour profile from the input if present"), s.metadata.icc, false, |t, v| t.job().webp.metadata.icc = v, window, cx);
	let xmp = this.toggle("webp-xmp", "Copy XMP", Some("-metadata xmp · copy XMP from the input if present"), s.metadata.xmp, false, |t, v| t.job().webp.metadata.xmp = v, window, cx);
	sections.push(this.panel("Metadata", None, vec![Skid::grid(3, 16., vec![exif, icc, xmp]).into_any_element()]).into_any_element());

	// Animation: img2webp's and gif2webp's own options.
	let animation = s.animation;
	sections.push(this.disclosure(
		"webp-animation",
		"Animation",
		false,
		move |this, window, cx| {
			let note = help("For an animated WebP or a GIF only: img2webp's and gif2webp's own options. Still images ignore them.").px(px(10.)).into_any_element();
			let min_size = this.toggle("webp-min-size", "Minimise size", Some("-min_size · search harder for the smallest file; slower, and places no keyframes"), animation.minimize_size, false, |t, v| t.job().webp.animation.minimize_size = v, window, cx);
			let mixed = this.toggle("webp-mixed", "Mixed lossy and lossless", Some("-mixed · each frame lossy or lossless, whichever is smaller"), animation.allow_mixed, false, |t, v| t.job().webp.animation.allow_mixed = v, window, cx);
			let loop_compat = this.toggle("webp-loop-compat", "GIF loop compatibility", Some("-loop_compatibility · gif2webp: read the GIF's loop count as Chrome up to M62 did"), animation.loop_compatibility, false, |t, v| t.job().webp.animation.loop_compatibility = v, window, cx);
			let kmin_body = animation.kmin.map(|value| this.number("webp-kmin-value", "At least, in frames", Some("-kmin · min distance between key frames"), f64::from(value), f64::from(i32::MIN), f64::from(i32::MAX), 0, None, false, |t, v| t.job().webp.animation.kmin = Some(to_i32(v)), cx));
			let kmin = this.optional("webp-kmin", "Minimum keyframe distance", None, Some("-kmin · not given: the tool's default (gif2webp 9 lossless, 3 lossy)"), animation.kmin.is_some(), |t, on| t.job().webp.animation.kmin = on.then_some(3), kmin_body, window, cx);
			let kmax_body = animation.kmax.map(|value| this.number("webp-kmax-value", "At most, in frames", Some("-kmax · max distance between key frames; 1 makes every frame a keyframe, 0 none"), f64::from(value), f64::from(i32::MIN), f64::from(i32::MAX), 0, None, false, |t, v| t.job().webp.animation.kmax = Some(to_i32(v)), cx));
			let kmax = this.optional("webp-kmax", "Maximum keyframe distance", None, Some("-kmax · not given: the tool's default (gif2webp 17 lossless, 5 lossy)"), animation.kmax.is_some(), |t, on| t.job().webp.animation.kmax = on.then_some(5), kmax_body, window, cx);
			let loop_body = animation.loop_count.map(|value| this.number("webp-loop-value", "Plays", Some("-loop · how many times it plays; 0 is forever"), f64::from(value), 0., 65535., 0, None, false, |t, v| t.job().webp.animation.loop_count = Some(u16::try_from(to_u32(v).min(65535)).unwrap_or(u16::MAX)), cx));
			let loops = this.optional("webp-loop", "Set the loop count", None, Some("-loop · not given: the source's own loop count is kept"), animation.loop_count.is_some(), |t, on| t.job().webp.animation.loop_count = on.then_some(0), loop_body, window, cx);
			let duration_body = animation.frame_duration.map(|value| this.number("webp-duration-value", "Milliseconds per frame", Some("-d · frame duration, given once for every frame"), f64::from(value), 1., 2_147_483_647., 0, Some("ms"), false, |t, v| t.job().webp.animation.frame_duration = Some(to_u32(v)), cx));
			let duration = this.optional("webp-duration", "Set every frame's duration", None, Some("-d · not given: each frame keeps its own timing"), animation.frame_duration.is_some(), |t, on| t.job().webp.animation.frame_duration = on.then_some(100), duration_body, window, cx);
			let mut rows = vec![note, Skid::grid(3, 16., vec![min_size, mixed, loop_compat]).into_any_element(), Skid::grid(3, 16., vec![kmin, kmax, loops]).into_any_element(), Skid::grid(3, 16., vec![duration]).into_any_element()];
			rows.push(help("img2webp's frame options · a change holds from its frame on, until a later one sets the same option again; -lossy and -lossless do nothing with -mixed. GIF input has no such options in gif2webp, so there they apply on top.").px(px(10.)).into_any_element());
			for (index, change) in animation.frame_changes.iter().enumerate() {
				rows.push(frame_change(this, index, change, window, cx));
			}
			let add = crate::controls::add_button(
				this,
				"webp-frame-change-add",
				"Add a change from a frame",
				|t, _, cx| {
					t.job().webp.animation.frame_changes.push(FrameChange { from_frame: 2, ..FrameChange::default() });
					t.changed(cx);
				},
				window,
				cx,
			);
			rows.push(div().flex().px(px(10.)).child(add).into_any_element());
			rows
		},
		window,
		cx,
	));

	let mt = this.toggle("webp-mt", "Multi-threading", Some("-mt · use multi-threading if available"), s.multi_threading, false, |t, v| t.job().webp.multi_threading = v, window, cx);
	let low_memory = this.toggle("webp-low-memory", "Low memory", Some("-low_memory · reduce memory usage (slower encoding)"), s.low_memory, false, |t, v| t.job().webp.low_memory = v, window, cx);
	sections.push(this.panel("Performance", None, vec![Skid::grid(3, 16., vec![mt, low_memory]).into_any_element()]).into_any_element());

	div().flex().flex_col().gap(px(32.)).children(sections).into_any_element()
}

/// A slider's whole-number value as the `u8` field it edits.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn to_u8(value: f64) -> u8 {
	value.round().clamp(0., 255.) as u8
}

#[allow(clippy::cast_possible_truncation)]
pub fn to_i32(value: f64) -> i32 {
	value.round().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn to_u32(value: f64) -> u32 {
	value.round().clamp(0., f64::from(u32::MAX)) as u32
}

#[allow(clippy::cast_possible_truncation)]
fn to_f32(value: f64) -> f32 {
	value as f32
}

/// A frame change's lossy/lossless or exact switch: left as it was, or set either way.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Switch {
	Keep,
	On,
	Off,
}

impl Switch {
	const fn of(value: Option<bool>) -> Self {
		match value {
			None => Self::Keep,
			Some(true) => Self::On,
			Some(false) => Self::Off,
		}
	}

	const fn value(self) -> Option<bool> {
		match self {
			Self::Keep => None,
			Self::On => Some(true),
			Self::Off => Some(false),
		}
	}
}

/// `WebpSettingsPanel.vue`'s frame change block: from which frame, lossy or lossless, exact,
/// and the optional quality, method and duration, with its remove button.
fn frame_change(this: &mut Skid, index: usize, change: &FrameChange, window: &mut Window, cx: &mut Context<Skid>) -> AnyElement {
	let n = index + 1;
	let id = |what: &str| format!("webp-frame-change-{index}-{what}");
	let from = this.number(&id("from"), &format!("Change {n}: from frame"), Some("the first frame it applies to, counting from 1"), f64::from(change.from_frame), 1., f64::from(u32::MAX), 0, None, false, move |t, v| set_change(t, index, |c| c.from_frame = to_u32(v).max(1)), cx);
	let mode = this.choice(&id("lossless"), &format!("Change {n}: lossy or lossless"), Some("-lossy / -lossless"), &[(Switch::Keep, "Unchanged", None), (Switch::Off, "Lossy", None), (Switch::On, "Lossless", None)], Switch::of(change.lossless), false, false, false, move |t, v: Switch| set_change(t, index, |c| c.lossless = v.value()), window, cx);
	let exact = this.choice(&id("exact"), &format!("Change {n}: exact"), Some("-exact / -noexact · keep the colour under transparent pixels"), &[(Switch::Keep, "Unchanged", None), (Switch::On, "Exact", None), (Switch::Off, "Not exact", None)], Switch::of(change.exact), false, false, false, move |t, v: Switch| set_change(t, index, |c| c.exact = v.value()), window, cx);
	let remove = crate::controls::icon_button(
		this,
		&id("remove"),
		"lucide--x",
		&format!("Remove change {n}"),
		move |t, _, cx| {
			let changes = &mut t.job().webp.animation.frame_changes;
			if index < changes.len() {
				changes.remove(index);
			}
			t.changed(cx);
		},
		window,
		cx,
	);
	let quality_body = change.quality.map(|value| this.number(&id("quality-value"), &format!("Change {n}: quality factor"), Some("-q · quality factor (0:small..100:big)"), f64::from(value), 0., 100., 2, None, false, move |t, v| set_change(t, index, |c| c.quality = Some(to_f32(v))), cx));
	let quality = this.optional(&id("quality"), &format!("Change {n}: quality"), None, Some("-q · not given: unchanged"), change.quality.is_some(), move |t, on| set_change(t, index, |c| c.quality = on.then_some(75.0)), quality_body, window, cx);
	let method_body = change.method.map(|value| this.number(&id("method-value"), &format!("Change {n}: compression method"), Some("-m · compression method (0=fast, 6=slowest)"), f64::from(value), 0., 6., 0, None, false, move |t, v| set_change(t, index, |c| c.method = Some(u8::try_from(to_u32(v).min(6)).unwrap_or(6))), cx));
	let method = this.optional(&id("method"), &format!("Change {n}: method"), None, Some("-m · not given: unchanged"), change.method.is_some(), move |t, on| set_change(t, index, |c| c.method = on.then_some(4)), method_body, window, cx);
	let duration_body = change.duration.map(|value| this.number(&id("duration-value"), &format!("Change {n}: milliseconds per frame"), Some("-d · frame duration"), f64::from(value), 1., 2_147_483_647., 0, Some("ms"), false, move |t, v| set_change(t, index, |c| c.duration = Some(to_u32(v).max(1))), cx));
	let duration = this.optional(&id("duration"), &format!("Change {n}: duration"), None, Some("-d · not given: unchanged"), change.duration.is_some(), move |t, on| set_change(t, index, |c| c.duration = on.then_some(100)), duration_body, window, cx);
	let first = div().flex().items_end().gap(px(8.)).child(div().flex_1().min_w_0().child(Skid::grid(3, 16., vec![from, mode, exact]))).child(div().mb(px(8.)).child(remove));
	let second = div().pr(px(40.)).child(Skid::grid(3, 16., vec![quality, method, duration]));
	div().flex().flex_col().gap(px(12.)).child(first).child(second).into_any_element()
}

fn set_change(this: &mut Skid, index: usize, edit: impl FnOnce(&mut FrameChange)) {
	if let Some(change) = this.job().webp.animation.frame_changes.get_mut(index) {
		edit(change);
	}
}
