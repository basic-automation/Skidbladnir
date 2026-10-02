//! `AvifSettingsPanel.vue`: every avifenc option that changes the file it writes for a still
//! image, with libaom, and avifenc's own help text.

use std::path::PathBuf;

use gpui::{AnyElement, ClickEvent, Context, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div, px};
use skidbladnir_encode::settings::{Cicp, CleanAperture, CodecOption, Fraction, Grid, MetadataSource, QuantizerRange, Tiling, YuvFormat};

use crate::fade::FadeBg;
use crate::{
	app::Skid, controls::{help, icon}, panels::webp::{to_u8, to_u32}, theme::{FG, FIELD, RULE, Type, c, ca}
};

const DEPTHS: [(Option<u8>, &str, Option<&str>); 4] = [(None, "Auto", None), (Some(8), "8", None), (Some(10), "10", None), (Some(12), "12", None)];
const EXTENSIONS: [(Option<u8>, &str, Option<&str>); 3] = [(None, "None", None), (Some(4), "4", None), (Some(8), "8", None)];
const YUV: [(YuvFormat, &str, Option<&str>); 5] = [(YuvFormat::Auto, "Auto", None), (YuvFormat::Yuv444, "4:4:4", None), (YuvFormat::Yuv422, "4:2:2", None), (YuvFormat::Yuv420, "4:2:0", None), (YuvFormat::Yuv400, "4:0:0", None)];
const RANGES: [(bool, &str, Option<&str>); 2] = [(false, "Full", None), (true, "Limited", None)];
const APERTURES: [(Aperture, &str, Option<&str>); 3] = [(Aperture::None, "None", None), (Aperture::Crop, "Crop rectangle", None), (Aperture::Raw, "Fractions", None)];
const TILINGS: [(bool, &str, Option<&str>); 2] = [(false, "Automatic", None), (true, "Manual", None)];
const ROTATIONS: [(u8, &str, Option<&str>); 4] = [(0, "0°", None), (1, "90°", None), (2, "180°", None), (3, "270°", None)];
const MIRRORS: [(u8, &str, Option<&str>); 2] = [(0, "Top-to-bottom", None), (1, "Left-to-right", None)];
const METADATA: [(MetaKind, &str, Option<&str>); 3] = [(MetaKind::Keep, "From the input", None), (MetaKind::Strip, "Leave out", None), (MetaKind::File, "From a file", None)];
const CROP_LABELS: [&str; 4] = ["X", "Y", "Width", "Height"];
const CLAP_LABELS: [&str; 8] = ["Width numerator", "Width denominator", "Height numerator", "Height denominator", "Horizontal offset numerator", "Horizontal offset denominator", "Vertical offset numerator", "Vertical offset denominator"];

#[derive(Clone, Copy, PartialEq)]
enum Aperture {
	None,
	Crop,
	Raw,
}

#[derive(Clone, Copy, PartialEq)]
enum MetaKind {
	Keep,
	Strip,
	File,
}

#[allow(clippy::too_many_lines)]
pub fn render(this: &mut Skid, window: &mut Window, cx: &mut Context<Skid>) -> AnyElement {
	let s = this.job().avif.clone();
	let mut sections = Vec::new();

	// Quality.
	let quality_body = s.quality.map(|q| this.slider("avif-quality-value", "Quality", f64::from(q), 0., 100., 1., None, Some("-q · quality for color in 0..100 where 100 is lossless"), false, None, |t, v| t.job().avif.quality = Some(to_u8(v)), window, cx));
	let quality = this.optional("avif-quality", "Colour quality", None, Some("-q · not given: avifenc's 60, or searched by --target-size"), s.quality.is_some(), |t, on| t.job().avif.quality = on.then_some(60), quality_body, window, cx);
	let alpha_body = s.quality_alpha.map(|q| this.slider("avif-qalpha-value", "Alpha quality", f64::from(q), 0., 100., 1., None, Some("--qalpha · quality for alpha in 0..100 where 100 is lossless"), false, None, |t, v| t.job().avif.quality_alpha = Some(to_u8(v)), window, cx));
	let alpha = this.optional("avif-qalpha", "Alpha quality", None, Some("--qalpha · not given: follows the colour quality"), s.quality_alpha.is_some(), |t, on| t.job().avif.quality_alpha = on.then_some(60), alpha_body, window, cx);
	let speed_body = s.speed.map(|q| this.slider("avif-speed-value", "Speed", f64::from(q), 0., 10., 1., None, Some("-s · encoder speed in 0..10 where 0 is the slowest, 10 is the fastest"), false, None, |t, v| t.job().avif.speed = Some(to_u8(v)), window, cx));
	let speed = this.optional("avif-speed", "Speed", None, Some("-s default · libaom's own default"), s.speed.is_some(), |t, on| t.job().avif.speed = on.then_some(6), speed_body, window, cx);
	let lossless = this.toggle("avif-lossless", "Lossless", Some("-l · set all defaults to encode losslessly"), s.lossless, false, |t, v| t.job().avif.lossless = v, window, cx);
	let target_body = s.target_size.map(|bytes| this.number("avif-target-size-value", "Target size", Some("--target-size · set target file size in bytes (up to 7 times slower)"), f64::from(bytes), 1., f64::from(u32::MAX), 0, Some("bytes"), false, |t, v| t.job().avif.target_size = Some(to_u32(v)), cx));
	let target = this.optional("avif-target-size", "Target file size", None, Some("--target-size · not given"), s.target_size.is_some(), |t, on| t.job().avif.target_size = on.then_some(51200), target_body, window, cx);
	sections.push(div().pt(px(32.)).child(this.panel("Quality", None, vec![Skid::grid(3, 16., vec![quality, alpha, speed]).into_any_element(), Skid::grid(3, 16., vec![lossless, target]).into_any_element()])).into_any_element());

	// Pixel format.
	let depth = this.choice(
		"avif-depth",
		"Bit depth",
		Some("-d · output bit depth per channel; auto is 8 for 8-bit input, 12 for deeper"),
		&DEPTHS,
		s.depth,
		false,
		false,
		false,
		|t, v| {
			let avif = &mut t.job().avif;
			avif.depth = v;
			if v.is_none() {
				avif.depth_extension = None;
			}
		},
		window,
		cx,
	);
	let mut depth_row = vec![depth];
	if matches!(s.depth, Some(8 | 12)) {
		depth_row.push(this.choice("avif-depth-extension", "Depth extension", Some("-d D,E · a hidden extension image reaching 16 bits (8,8 and 12,4 and 12,8)"), &EXTENSIONS, s.depth_extension, false, false, false, |t, v| t.job().avif.depth_extension = v, window, cx));
	}
	depth_row.push(this.choice("avif-yuv", "YUV format", Some("-y · auto honours a JPEG's own format; 4:0:0 for grayscale PNG; otherwise 4:4:4"), &YUV, s.yuv, false, false, false, |t, v| t.job().avif.yuv = v, window, cx));
	let range = this.choice("avif-range", "YUV range", Some("-r · YUV range"), &RANGES, s.limited_range, false, false, false, |t, v| t.job().avif.limited_range = v, window, cx);
	let premultiply = this.toggle("avif-premultiply", "Premultiply alpha", Some("-p · premultiply color by the alpha channel and signal this in the AVIF"), s.premultiply, false, |t, v| t.job().avif.premultiply = v, window, cx);
	let sharp_yuv = this.toggle("avif-sharpyuv", "Sharp YUV", Some("--sharpyuv · sharp RGB to YUV420 conversion; only for 4:2:0 output"), s.sharp_yuv, false, |t, v| t.job().avif.sharp_yuv = v, window, cx);
	sections.push(this.panel("Pixel format", None, vec![Skid::grid(3, 16., depth_row).into_any_element(), Skid::grid(3, 16., vec![range, premultiply, sharp_yuv]).into_any_element()]).into_any_element());

	// Colour and metadata.
	let cicp_body = s.cicp.map(|value| {
		let primaries = this.number("avif-cicp-p", "Colour primaries", Some("P, e.g. 1 BT.709, 9 BT.2020, 12 Display P3"), f64::from(value.primaries), 0., 255., 0, None, false, |t, v| set_cicp(t, |cicp| cicp.primaries = to_u16(v)), cx);
		let transfer = this.number("avif-cicp-t", "Transfer characteristics", Some("T, e.g. 13 sRGB, 16 PQ, 18 HLG"), f64::from(value.transfer), 0., 255., 0, None, false, |t, v| set_cicp(t, |cicp| cicp.transfer = to_u16(v)), cx);
		let matrix = this.number("avif-cicp-m", "Matrix coefficients", Some("M, e.g. 6 BT.601, 1 BT.709, 0 identity"), f64::from(value.matrix), 0., 255., 0, None, false, |t, v| set_cicp(t, |cicp| cicp.matrix = to_u16(v)), cx);
		column(vec![primaries, transfer, matrix])
	});
	let cicp = this.optional(
		"avif-cicp",
		"Colour signalling (CICP)",
		Some("--cicp P/T/M · set CICP values (nclx colr box); 2 leaves one unspecified"),
		Some("--cicp · not given: from the input"),
		s.cicp.is_some(),
		|t, on| t.job().avif.cicp = on.then_some(Cicp { primaries: 1, transfer: 13, matrix: 6 }),
		cicp_body,
		window,
		cx,
	);
	let clli_body = s.clli.map(|value| {
		let max_cll = this.number("avif-clli-cll", "MaxCLL", Some("--clli · maximum content light level"), f64::from(value[0]), 0., 65535., 0, Some("cd/m²"), false, |t, v| set_clli(t, 0, v), cx);
		let max_pall = this.number("avif-clli-pall", "MaxPALL", Some("--clli · maximum picture-average light level"), f64::from(value[1]), 0., 65535., 0, Some("cd/m²"), false, |t, v| set_clli(t, 1, v), cx);
		column(vec![max_cll, max_pall])
	});
	let clli = this.optional("avif-clli", "Content light level", None, Some("--clli · not given"), s.clli.is_some(), |t, on| t.job().avif.clli = on.then_some([1000, 400]), clli_body, window, cx);
	let icc = metadata(this, "avif-icc", "ICC profile", "--ignore-icc / --icc FILE", &s.icc, |t| &mut t.job().avif.icc, window, cx);
	let exif = metadata(this, "avif-exif", "Exif", "--ignore-exif / --exif FILE", &s.exif, |t| &mut t.job().avif.exif, window, cx);
	let xmp = metadata(this, "avif-xmp", "XMP", "--ignore-xmp / --xmp FILE", &s.xmp, |t| &mut t.job().avif.xmp, window, cx);
	sections.push(this.panel("Colour and metadata", None, vec![Skid::grid(3, 16., vec![cicp, clli]).into_any_element(), Skid::grid(3, 16., vec![icc, exif, xmp]).into_any_element()]).into_any_element());

	// Layout.
	let layout = s.clone();
	sections.push(this.disclosure(
		"avif-layout",
		"Layout",
		false,
		move |this, window, cx| {
			let s = layout;
			let progressive = this.toggle("avif-progressive", "Progressive", Some("--progressive · a simple layered image supporting progressive rendering"), s.progressive, false, |t, v| t.job().avif.progressive = v, window, cx);
			let grid_body = s.grid.map(|value| {
				let columns = this.number("avif-grid-columns", "Columns", Some("-g MxN · M columns"), f64::from(value.columns), 1., 256., 0, None, false, |t, v| set_grid(t, |grid| grid.columns = to_u32(v)), cx);
				let rows = this.number("avif-grid-rows", "Rows", Some("-g MxN · N rows"), f64::from(value.rows), 1., 256., 0, None, false, |t, v| set_grid(t, |grid| grid.rows = to_u32(v)), cx);
				column(vec![columns, rows])
			});
			let grid = this.optional("avif-grid", "Grid", None, Some("-g · not given: one image"), s.grid.is_some(), |t, on| t.job().avif.grid = on.then_some(Grid { columns: 2, rows: 2 }), grid_body, window, cx);
			let scaling_body = s.scaling_mode.map(|value| {
				let numerator = this.number("avif-scaling-num", "Numerator", Some("--scaling-mode N/D · frame scaling as a fraction"), f64::from(value.numerator), 0., f64::from(u32::MAX), 0, None, false, |t, v| set_scaling(t, |fraction| fraction.numerator = to_u32(v)), cx);
				let denominator = this.number("avif-scaling-den", "Denominator", None, f64::from(value.denominator), 1., f64::from(u32::MAX), 0, None, false, |t, v| set_scaling(t, |fraction| fraction.denominator = to_u32(v)), cx);
				column(vec![numerator, denominator])
			});
			let scaling = this.optional("avif-scaling", "Scaling mode", None, Some("--scaling-mode · not given: 1/1"), s.scaling_mode.is_some(), |t, on| t.job().avif.scaling_mode = on.then_some(Fraction { numerator: 1, denominator: 2 }), scaling_body, window, cx);
			let manual = matches!(s.tiling, Tiling::Manual { .. });
			let tiling = this.choice(
				"avif-tiling",
				"Tiling",
				Some("--autotiling / --tilerowslog2, --tilecolslog2"),
				&TILINGS,
				manual,
				false,
				false,
				false,
				|t, manual| {
					let avif = &mut t.job().avif;
					if manual != matches!(avif.tiling, Tiling::Manual { .. }) {
						avif.tiling = if manual { Tiling::Manual { rows_log2: 0, cols_log2: 0 } } else { Tiling::Automatic };
					}
				},
				window,
				cx,
			);
			let mut tiling_row = vec![tiling];
			if let Tiling::Manual { rows_log2, cols_log2 } = s.tiling {
				tiling_row.push(this.slider("avif-tile-rows", "Tile rows (log2)", f64::from(rows_log2), 0., 6., 1., None, Some("--tilerowslog2 · log2 of number of tile rows in 0..6"), false, None, |t, v| set_tiles(t, Some(to_u8(v)), None), window, cx));
				tiling_row.push(this.slider("avif-tile-cols", "Tile columns (log2)", f64::from(cols_log2), 0., 6., 1., None, Some("--tilecolslog2 · log2 of number of tile columns in 0..6"), false, None, |t, v| set_tiles(t, None, Some(to_u8(v))), window, cx));
			}
			vec![Skid::grid(3, 16., vec![progressive, grid, scaling]).into_any_element(), Skid::grid(3, 16., tiling_row).into_any_element()]
		},
		window,
		cx,
	));

	// Quantizers.
	let quantizers = s.clone();
	sections.push(this.disclosure(
		"avif-quantizers",
		"Quantizers (deprecated in avifenc)",
		false,
		move |this, window, cx| {
			let s = quantizers;
			let colour_body = s.quantizer.map(|q| {
				let min = this.slider("avif-qmin", "Minimum", f64::from(q.min), 0., 63., 1., None, Some("--min QP"), false, None, |t, v| set_quantizer(t, false, |q| q.min = to_u8(v)), window, cx);
				let max = this.slider("avif-qmax", "Maximum", f64::from(q.max), 0., 63., 1., None, Some("--max QP"), false, None, |t, v| set_quantizer(t, false, |q| q.max = to_u8(v)), window, cx);
				column(vec![min, max])
			});
			let colour = this.optional("avif-quantizer", "Colour quantizers", None, Some("--min / --max · not given"), s.quantizer.is_some(), |t, on| t.job().avif.quantizer = on.then_some(QuantizerRange { min: 0, max: 63 }), colour_body, window, cx);
			let alpha_body = s.alpha_quantizer.map(|q| {
				let min = this.slider("avif-qmin-alpha", "Minimum", f64::from(q.min), 0., 63., 1., None, Some("--minalpha QP"), false, None, |t, v| set_quantizer(t, true, |q| q.min = to_u8(v)), window, cx);
				let max = this.slider("avif-qmax-alpha", "Maximum", f64::from(q.max), 0., 63., 1., None, Some("--maxalpha QP"), false, None, |t, v| set_quantizer(t, true, |q| q.max = to_u8(v)), window, cx);
				column(vec![min, max])
			});
			let alpha = this.optional("avif-alpha-quantizer", "Alpha quantizers", None, Some("--minalpha / --maxalpha · not given"), s.alpha_quantizer.is_some(), |t, on| t.job().avif.alpha_quantizer = on.then_some(QuantizerRange { min: 0, max: 63 }), alpha_body, window, cx);
			vec![Skid::grid(3, 16., vec![colour, alpha]).into_any_element()]
		},
		window,
		cx,
	));

	// Transform properties.
	let transform = s.clone();
	sections.push(this.disclosure(
		"avif-transform",
		"Transform properties",
		false,
		move |this, window, cx| {
			let s = transform;
			let note = help("Written into the file for the viewer to apply; the pixels are not changed.").px(px(10.)).into_any_element();
			let irot_body = s.irot.map(|irot| this.choice("avif-irot-value", "Anticlockwise", Some("--irot · (90 * ANGLE) degree rotation anti-clockwise"), &ROTATIONS, irot, false, false, false, |t, v| t.job().avif.irot = Some(v), window, cx));
			let irot = this.optional("avif-irot", "Rotation", None, Some("--irot · not given"), s.irot.is_some(), |t, on| t.job().avif.irot = on.then_some(1), irot_body, window, cx);
			let imir_body = s.imir.map(|imir| this.choice("avif-imir-value", "Axis", Some("--imir · 0=top-to-bottom, 1=left-to-right"), &MIRRORS, imir, false, false, false, |t, v| t.job().avif.imir = Some(v), window, cx));
			let imir = this.optional("avif-imir", "Mirroring", None, Some("--imir · not given"), s.imir.is_some(), |t, on| t.job().avif.imir = on.then_some(1), imir_body, window, cx);
			let pasp_body = s.pasp.map(|value| {
				let horizontal = this.number("avif-pasp-h", "Horizontal spacing", Some("--pasp H,V"), f64::from(value[0]), 0., f64::from(u32::MAX), 0, None, false, |t, v| set_pasp(t, 0, v), cx);
				let vertical = this.number("avif-pasp-v", "Vertical spacing", None, f64::from(value[1]), 0., f64::from(u32::MAX), 0, None, false, |t, v| set_pasp(t, 1, v), cx);
				column(vec![horizontal, vertical])
			});
			let pasp = this.optional("avif-pasp", "Pixel aspect ratio", None, Some("--pasp · not given"), s.pasp.is_some(), |t, on| t.job().avif.pasp = on.then_some([1, 1]), pasp_body, window, cx);

			let kind = match s.clean_aperture {
				None => Aperture::None,
				Some(CleanAperture::Crop(_)) => Aperture::Crop,
				Some(CleanAperture::Raw(_)) => Aperture::Raw,
			};
			let aperture = this.choice(
				"avif-aperture",
				"Clean aperture",
				Some("--crop CROPX,CROPY,CROPW,CROPH or --clap WN,WD,HN,HD,HON,HOD,VON,VOD"),
				&APERTURES,
				kind,
				false,
				false,
				false,
				move |t, next| {
					if next != kind {
						t.job().avif.clean_aperture = match next {
							Aperture::None => None,
							Aperture::Crop => Some(CleanAperture::Crop([0, 0, 1, 1])),
							Aperture::Raw => Some(CleanAperture::Raw([1, 1, 1, 1, 0, 1, 0, 1])),
						};
					}
				},
				window,
				cx,
			);
			let aperture_row = match s.clean_aperture {
				None => Skid::grid(3, 16., vec![aperture]).into_any_element(),
				Some(value) => {
					let (prefix, values, labels): (&str, &[u32], &[&str]) = match &value {
						CleanAperture::Crop(values) => ("avif-crop", values, &CROP_LABELS),
						CleanAperture::Raw(values) => ("avif-clap", values, &CLAP_LABELS),
					};
					let fields: Vec<AnyElement> = values.iter().zip(labels).enumerate().map(|(index, (value, label))| this.number(&format!("{prefix}-{index}"), label, None, f64::from(*value), 0., f64::from(u32::MAX), 0, None, false, move |t, v| set_aperture(t, index, v), cx)).collect();
					// `grid-cols-3 gap-4` with the values in a `col-span-2` cell: the first cell
					// takes one third of the space the gaps leave, the second two thirds plus the
					// gap it spans.
					div()
						.flex()
						.gap(px(16.))
						.child(div().flex_1().min_w_0().flex().flex_col().child(aperture))
						.child(div().flex_grow(2.).flex_shrink(1.).flex_basis(px(16.)).min_w_0().flex().flex_col().child(Skid::grid(4, 8., fields)))
						.into_any_element()
				}
			};
			vec![note, Skid::grid(3, 16., vec![irot, imir, pasp]).into_any_element(), aperture_row]
		},
		window,
		cx,
	));

	// libaom options.
	let options = s.codec_options.clone();
	sections.push(this.disclosure(
		"avif-libaom",
		"libaom options",
		false,
		move |this, _, cx| {
			let mut children = vec![help("-a KEY[=VALUE] · passed straight to libaom, in order. Prefix a key with color: or alpha: to apply it to one plane only, for example tune=ssim, color:aq-mode=1, alpha:end-usage=q.").px(px(10.)).into_any_element()];
			for (index, option) in options.iter().enumerate() {
				let key = this.text_field(&format!("avif-option-{index}-key"), &format!("Option {} key", index + 1), None, "tune", &option.key, move |t, v| set_option(t, index, |option| option.key = v), cx);
				let value = this.text_field(&format!("avif-option-{index}-value"), &format!("Option {} value", index + 1), None, "ssim", &option.value, move |t, v| set_option(t, index, |option| option.value = v), cx);
				// UButton color="neutral" variant="ghost" icon="i-lucide-x", size md.
				let remove = div()
					.id(ElementId::Name(SharedString::from(format!("avif-option-{index}-remove"))))
					.flex_none()
					.mb(px(8.))
					.p(px(6.))
					.rounded(px(6.))
					.fade_bg_clear(format!("avif-option-{index}-remove"), c(FIELD))
					.cursor_pointer()
					.on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
						let options = &mut t.job().avif.codec_options;
						if index < options.len() {
							options.remove(index);
						}
						t.changed(cx);
					}))
					.child(icon("lucide--x", 20., c(FG)));
				children.push(div().flex().items_end().gap(px(8.)).child(div().flex_1().min_w_0().flex().flex_col().child(key)).child(div().flex_1().min_w_0().flex().flex_col().child(value)).child(remove).into_any_element());
			}
			// UButton size="sm" color="neutral" variant="soft" icon="i-material-symbols-add".
			let add = div()
				.id("avif-option-add")
				.flex()
				.flex_none()
				.items_center()
				.gap(px(6.))
				.px(px(10.))
				.py(px(6.))
				.rounded(px(9.))
				.bg(c(FIELD))
				.fade_bg("avif-option-add", c(FIELD), ca(RULE, 0.75))
				.xs()
				.medium()
				.text_color(c(FG))
				.cursor_pointer()
				.on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
					t.job().avif.codec_options.push(CodecOption { key: String::new(), value: String::new() });
					t.changed(cx);
				}))
				.child(icon("material-symbols--add", 16., c(FG)))
				.child("Add an option");
			children.push(div().flex().px(px(10.)).child(add).into_any_element());
			children
		},
		window,
		cx,
	));

	// Performance.
	let all_jobs = this.toggle("avif-jobs-all", "Every core", Some("-j all · number of jobs (worker threads)"), s.jobs.is_none(), false, |t, all| t.job().avif.jobs = if all { None } else { Some(1) }, window, cx);
	let mut performance = vec![all_jobs];
	if let Some(jobs) = s.jobs {
		performance.push(this.number("avif-jobs", "Worker threads", Some("-j · number of jobs"), f64::from(jobs), 1., f64::from(u32::MAX), 0, None, false, |t, v| t.job().avif.jobs = Some(to_u32(v)), cx));
	}
	sections.push(this.panel("Performance", None, vec![Skid::grid(3, 16., performance).into_any_element()]).into_any_element());

	div().flex().flex_col().gap(px(32.)).children(sections).into_any_element()
}

/// `ControlMetadata`: where a piece of metadata comes from, and the file when it is one.
#[allow(clippy::too_many_arguments)]
fn metadata(this: &mut Skid, id: &str, label: &str, help_text: &str, value: &MetadataSource, field: fn(&mut Skid) -> &mut MetadataSource, window: &Window, cx: &mut Context<Skid>) -> AnyElement {
	let kind = match value {
		MetadataSource::Keep => MetaKind::Keep,
		MetadataSource::Strip => MetaKind::Strip,
		MetadataSource::File(_) => MetaKind::File,
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
		move |t, next| {
			let source = field(t);
			*source = match next {
				MetaKind::Keep => MetadataSource::Keep,
				MetaKind::Strip => MetadataSource::Strip,
				MetaKind::File => MetadataSource::File(if let MetadataSource::File(path) = source { std::mem::take(path) } else { PathBuf::new() }),
			};
		},
		window,
		cx,
	);
	let file = if let MetadataSource::File(path) = value {
		// `class="pl-5"` on ControlFile overrides its own `px-2.5` on the left: 20px in all.
		let file = this.file(&format!("{id}-file"), &format!("{label} file"), None, &path.display().to_string(), move |t, v| *field(t) = MetadataSource::File(PathBuf::from(v)), cx);
		Some(div().flex().flex_col().pl(px(10.)).child(file))
	} else {
		None
	};
	div().flex().flex_col().min_w_0().child(choice).children(file).into_any_element()
}

/// The value controls of a ControlOptional's slot, one under another.
fn column(children: Vec<AnyElement>) -> AnyElement {
	div().flex().flex_col().children(children).into_any_element()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_u16(value: f64) -> u16 {
	value.round().clamp(0., f64::from(u16::MAX)) as u16
}

fn set_cicp(this: &mut Skid, edit: impl FnOnce(&mut Cicp)) {
	if let Some(cicp) = this.job().avif.cicp.as_mut() {
		edit(cicp);
	}
}

fn set_clli(this: &mut Skid, index: usize, value: f64) {
	if let Some(clli) = this.job().avif.clli.as_mut() {
		clli[index] = to_u16(value);
	}
}

fn set_grid(this: &mut Skid, edit: impl FnOnce(&mut Grid)) {
	if let Some(grid) = this.job().avif.grid.as_mut() {
		edit(grid);
	}
}

fn set_scaling(this: &mut Skid, edit: impl FnOnce(&mut Fraction)) {
	if let Some(fraction) = this.job().avif.scaling_mode.as_mut() {
		edit(fraction);
	}
}

fn set_tiles(this: &mut Skid, rows: Option<u8>, cols: Option<u8>) {
	if let Tiling::Manual { rows_log2, cols_log2 } = &mut this.job().avif.tiling {
		if let Some(rows) = rows {
			*rows_log2 = rows;
		}
		if let Some(cols) = cols {
			*cols_log2 = cols;
		}
	}
}

fn set_quantizer(this: &mut Skid, alpha: bool, edit: impl FnOnce(&mut QuantizerRange)) {
	let avif = &mut this.job().avif;
	let range = if alpha { avif.alpha_quantizer.as_mut() } else { avif.quantizer.as_mut() };
	if let Some(range) = range {
		edit(range);
	}
}

fn set_pasp(this: &mut Skid, index: usize, value: f64) {
	if let Some(pasp) = this.job().avif.pasp.as_mut() {
		pasp[index] = to_u32(value);
	}
}

fn set_aperture(this: &mut Skid, index: usize, value: f64) {
	let values: &mut [u32] = match this.job().avif.clean_aperture.as_mut() {
		Some(CleanAperture::Crop(values)) => values,
		Some(CleanAperture::Raw(values)) => values,
		None => return,
	};
	if let Some(slot) = values.get_mut(index) {
		*slot = to_u32(value);
	}
}

fn set_option(this: &mut Skid, index: usize, edit: impl FnOnce(&mut CodecOption)) {
	if let Some(option) = this.job().avif.codec_options.get_mut(index) {
		edit(option);
	}
}
