//! `HeicSettingsPanel.vue`: every heif-enc option that changes the file it writes for a
//! still image, with the edition's encoder (Kvazaar, or x265 in the GPL edition) and
//! heif-enc's own help text.

use gpui::{AnyElement, AppContext, ClickEvent, Context, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div, px};
use gpuikit::{elements::input::input, input::InputStateEvent};
use skidbladnir_encode::settings::{
	ChromaDownsampling, ColorProfile, HEIC_X265, HeicAqMode, HeicBitDepth, HeicChroma, HeicPreset, HeicTune, OmafProjection, Orientation
};

use crate::{
	app::Skid, controls::{field, field_label, help, icon}, panels::webp::{to_u8, to_u32}, theme::{DIM, FG, FIELD, RULE, Type, c}
};

const X265_CHROMA: [(HeicChroma, &str, Option<&str>); 3] = [(HeicChroma::Yuv420, "4:2:0", None), (HeicChroma::Yuv422, "4:2:2", None), (HeicChroma::Yuv444, "4:4:4", None)];
const X265_DEPTH: [(HeicBitDepth, &str, Option<&str>); 2] = [(HeicBitDepth::Eight, "8-bit", None), (HeicBitDepth::Ten, "10-bit", None)];
const X265_PRESETS: [(HeicPreset, &str); 10] = [
	(HeicPreset::Ultrafast, "Ultrafast"),
	(HeicPreset::Superfast, "Superfast"),
	(HeicPreset::Veryfast, "Very fast"),
	(HeicPreset::Faster, "Faster"),
	(HeicPreset::Fast, "Fast"),
	(HeicPreset::Medium, "Medium"),
	(HeicPreset::Slow, "Slow (libheif's default)"),
	(HeicPreset::Slower, "Slower"),
	(HeicPreset::Veryslow, "Very slow"),
	(HeicPreset::Placebo, "Placebo"),
];
const X265_TUNES: [(HeicTune, &str, Option<&str>); 4] = [(HeicTune::Ssim, "SSIM", None), (HeicTune::Psnr, "PSNR", None), (HeicTune::Grain, "Grain", None), (HeicTune::Fastdecode, "Fast decode", None)];
const X265_AQ: [(HeicAqMode, &str); 5] = [
	(HeicAqMode::Off, "Off"),
	(HeicAqMode::Variance, "Variance"),
	(HeicAqMode::AutoVariance, "Auto-variance"),
	(HeicAqMode::AutoVarianceDark, "Auto-variance, dark bias"),
	(HeicAqMode::AutoVarianceEdge, "Auto-variance, edges"),
];
const CHROMA: [(Option<ChromaDownsampling>, &str, Option<&str>); 4] = [(None, "Encoder decides", None), (Some(ChromaDownsampling::NearestNeighbor), "Nearest neighbour", None), (Some(ChromaDownsampling::Average), "Average", None), (Some(ChromaDownsampling::SharpYuv), "Sharp YUV", None)];
const ORIENTATIONS: [(Orientation, &str); 8] = [
	(Orientation::Normal, "As stored"),
	(Orientation::FlipHorizontally, "Flip horizontally (--flip-h)"),
	(Orientation::Rotate180, "Rotate 180° (--rotate-cw 180)"),
	(Orientation::FlipVertically, "Flip vertically (--flip-v)"),
	(Orientation::Rotate90CwThenFlipHorizontally, "Rotate 90° clockwise, flip horizontally"),
	(Orientation::Rotate90Cw, "Rotate 90° clockwise (--rotate-cw 90)"),
	(Orientation::Rotate90CwThenFlipVertically, "Rotate 90° clockwise, flip vertically"),
	(Orientation::Rotate270Cw, "Rotate 270° clockwise (--rotate-cw 270)"),
];
const PROJECTIONS: [(OmafProjection, &str, Option<&str>); 2] = [(OmafProjection::Equirectangular, "Equirectangular", None), (OmafProjection::CubeMap, "Cube map", None)];

/// `--color-profile`'s preset, without the custom code points.
#[derive(Clone, Copy, PartialEq)]
enum Profile {
	Custom,
	Auto,
	Compatible,
	Bt601,
	Bt709,
	Bt2020,
}

const PROFILES: [(Profile, &str, Option<&str>); 6] = [(Profile::Custom, "Custom", None), (Profile::Auto, "Auto", None), (Profile::Compatible, "Compatible", None), (Profile::Bt601, "Rec. 601", None), (Profile::Bt709, "Rec. 709", None), (Profile::Bt2020, "Rec. 2020", None)];

fn owned<T: Copy>(items: &[(T, &str)]) -> Vec<(T, String)> {
	items.iter().map(|(value, label)| (*value, (*label).to_owned())).collect()
}

#[allow(clippy::too_many_lines)]
pub fn render(this: &mut Skid, window: &mut Window, cx: &mut Context<Skid>) -> AnyElement {
	let s = this.job().heic.clone();
	let x265 = HEIC_X265;

	// Quality.
	let quality = this.slider(
		"heic-quality",
		"Quality",
		f64::from(s.quality),
		0.,
		100.,
		1.,
		None,
		Some(if x265 { "-q · set output quality (0-100) for lossy compression; x265's CRF is (100 − quality) ÷ 2" } else { "-q · set output quality (0-100) for lossy compression" }),
		x265 && s.lossless,
		None,
		|t, v| t.job().heic.quality = to_u8(v),
		window,
		cx,
	);
	let lossless = if x265 {
		this.toggle("heic-lossless", "Lossless", Some("-L · generate lossless output: RGB at 4:4:4, every pixel kept (-q and the chroma, depth and rate controls do not apply)"), s.lossless, false, |t, v| t.job().heic.lossless = v, window, cx)
	} else {
		this.toggle("heic-lossless", "Lossless coding", Some("-p lossless=true · Kvazaar codes the 4:2:0 image without loss"), s.lossless, false, |t, v| t.job().heic.lossless = v, window, cx)
	};
	let downsampling = this.choice("heic-chroma-downsampling", "Chroma downsampling", Some("-C · force chroma downsampling algorithm; left out, libheif picks, which is not the same as average"), &CHROMA, s.chroma_downsampling, false, false, false, |t, v| t.job().heic.chroma_downsampling = v, window, cx);
	let mut quality_rows = vec![Skid::grid(3, 16., vec![quality, lossless, downsampling]).into_any_element()];
	if x265 && !s.lossless {
		let chroma = this.choice("heic-chroma", "Chroma", Some("-p chroma · 4:2:0 is what phones write and every reader decodes; 4:4:4 keeps colour at full resolution"), &X265_CHROMA, s.chroma, false, false, false, |t, v| t.job().heic.chroma = v, window, cx);
		let depth = this.choice("heic-bit-depth", "Bit depth", Some("-b · 10-bit is HEVC Main 10, as phones write HDR photos; smooth gradients band less, even from an 8-bit image"), &X265_DEPTH, s.bit_depth, false, false, false, |t, v| t.job().heic.bit_depth = v, window, cx);
		quality_rows.push(Skid::grid(3, 16., vec![chroma, depth]).into_any_element());
	}
	let encoder_note = if x265 {
		"HEVC in HEIF, the format iPhones use, encoded by x265.".to_owned()
	} else {
		"HEVC in HEIF, the format iPhones use, encoded by Kvazaar, which writes 8-bit 4:2:0 colour. The GPL edition encodes with x265 instead, which adds -L lossless, 4:4:4, 10-bit and x265's own controls.".to_owned()
	};
	quality_rows.push(help(encoder_note).px(px(10.)).into_any_element());
	let mut sections = vec![div().pt(px(32.)).child(this.panel("Quality", None, quality_rows)).into_any_element()];

	// x265.
	if x265 {
		let preset = this.select("heic-preset", "Preset", Some("-p preset · slower presets search harder for a smaller file"), &owned(&X265_PRESETS), s.preset, false, |t, v| t.job().heic.preset = v, window, cx);
		let tune = this.choice("heic-tune", "Tune", Some("-p tune · sets x265's remaining decisions for a goal; the controls here apply over it"), &X265_TUNES, s.tune, false, false, false, |t, v| t.job().heic.tune = v, window, cx);
		let tu = this.slider("heic-tu-intra-depth", "TU intra depth", f64::from(s.tu_intra_depth), 1., 4., 1., None, Some("-p tu-intra-depth · how far transform units split (at most 3 under 32 px)"), false, None, |t, v| t.job().heic.tu_intra_depth = to_u8(v), window, cx);
		let mut x265_rows = vec![Skid::grid(3, 16., vec![preset, tune, tu]).into_any_element()];
		if !s.lossless {
			let aq = this.select("heic-aq-mode", "Adaptive quantisation", Some("-p x265:aq-mode · how bits move between flat and detailed areas"), &owned(&X265_AQ), s.aq_mode, false, |t, v| t.job().heic.aq_mode = v, window, cx);
			let aq_strength = this.slider("heic-aq-strength", "AQ strength", f64::from(s.aq_strength), 0., 30., 1., None, Some("-p x265:aq-strength · 0.0 .. 3.0"), s.aq_mode == HeicAqMode::Off, Some(10.), |t, v| t.job().heic.aq_strength = to_u8(v), window, cx);
			let psy_rd = this.slider("heic-psy-rd", "Psy-RD", f64::from(s.psy_rd), 0., 50., 1., None, Some("-p x265:psy-rd · 0.0 .. 5.0; keeps texture rather than the smoothest match"), false, Some(10.), |t, v| t.job().heic.psy_rd = to_u8(v), window, cx);
			let psy_rdoq = this.slider("heic-psy-rdoq", "Psy-RDOQ", f64::from(s.psy_rdoq), 0., 500., 1., None, Some("-p x265:psy-rdoq · 0.0 .. 50.0; keeps detail when quantising"), false, Some(10.), |t, v| t.job().heic.psy_rdoq = to_u16(v), window, cx);
			let deblock = this.toggle("heic-deblock", "Deblocking", Some("-p x265:deblock · the in-loop filter that smooths block edges"), s.deblock, false, |t, v| t.job().heic.deblock = v, window, cx);
			let strength = this.slider("heic-deblock-strength", "Deblocking strength", f64::from(s.deblock_strength), -6., 6., 1., None, Some("tC offset: higher smooths more"), !s.deblock, None, |t, v| t.job().heic.deblock_strength = to_i8(v), window, cx);
			let threshold = this.slider("heic-deblock-threshold", "Deblocking threshold", f64::from(s.deblock_threshold), -6., 6., 1., None, Some("beta offset: higher treats more edges as blocking"), !s.deblock, None, |t, v| t.job().heic.deblock_threshold = to_i8(v), window, cx);
			let sao = this.toggle("heic-sao", "SAO", Some("-p x265:sao · sample adaptive offset: smooths ringing around edges"), s.sao, false, |t, v| t.job().heic.sao = v, window, cx);
			x265_rows.push(Skid::grid(4, 12., vec![aq, aq_strength, psy_rd, psy_rdoq]).into_any_element());
			x265_rows.push(Skid::grid(4, 12., vec![deblock, strength, threshold, sao]).into_any_element());
			x265_rows.push(help("These shape the colour image. libheif encodes transparency separately, with its own values for them.").px(px(10.)).into_any_element());
		}
		sections.push(this.panel("x265", None, x265_rows).into_any_element());
	}

	// Transparency and thumbnail.
	let drop_alpha = this.toggle("heic-no-alpha", "Drop alpha", Some("--no-alpha · do not save alpha channel"), !s.alpha, false, |t, drop| t.job().heic.alpha = !drop, window, cx);
	let premultiplied = this.toggle("heic-premultiplied-alpha", "Premultiplied alpha", Some("--premultiplied-alpha · input image has premultiplied alpha"), s.premultiplied_alpha, false, |t, v| t.job().heic.premultiplied_alpha = v, window, cx);
	let thumbnail_body = s.thumbnail.map(|size| this.number("heic-thumbnail-size", "Thumbnail size", Some("-t · generate thumbnail with maximum size #; none for an image already inside it"), f64::from(size), 1., f64::from(u32::MAX), 0, Some("px"), false, |t, v| t.job().heic.thumbnail = Some(to_u32(v)), cx));
	let thumbnail = this.optional("heic-thumbnail", "Thumbnail", None, Some("-t · not given: no thumbnail"), s.thumbnail.is_some(), |t, on| t.job().heic.thumbnail = on.then_some(320), thumbnail_body, window, cx);
	let mut thumbnail_row = vec![thumbnail];
	if s.thumbnail.is_some() {
		thumbnail_row.push(this.toggle("heic-no-thumb-alpha", "Drop thumbnail alpha", Some("--no-thumb-alpha · do not save alpha channel in thumbnail image"), !s.thumbnail_alpha, false, |t, drop| t.job().heic.thumbnail_alpha = !drop, window, cx));
	}
	sections.push(this.panel("Transparency and thumbnail", None, vec![Skid::grid(3, 16., vec![drop_alpha, premultiplied]).into_any_element(), Skid::grid(3, 16., thumbnail_row).into_any_element()]).into_any_element());

	// Colour.
	let profile_kind = match s.color_profile {
		ColorProfile::Custom { .. } => Profile::Custom,
		ColorProfile::Auto => Profile::Auto,
		ColorProfile::Compatible => Profile::Compatible,
		ColorProfile::Bt601 => Profile::Bt601,
		ColorProfile::Bt709 => Profile::Bt709,
		ColorProfile::Bt2020 => Profile::Bt2020,
	};
	let profile = this.choice(
		"heic-color-profile",
		"Colour profile",
		Some("--color-profile · the NCLX written with the image; custom uses the code points"),
		&PROFILES,
		profile_kind,
		false,
		false,
		false,
		|t, kind| {
			t.job().heic.color_profile = match kind {
				Profile::Custom => ColorProfile::Custom { matrix_coefficients: 6, colour_primaries: 1, transfer_characteristics: 13, full_range: true },
				Profile::Auto => ColorProfile::Auto,
				Profile::Compatible => ColorProfile::Compatible,
				Profile::Bt601 => ColorProfile::Bt601,
				Profile::Bt709 => ColorProfile::Bt709,
				Profile::Bt2020 => ColorProfile::Bt2020,
			};
		},
		window,
		cx,
	);
	let profile_row = if let ColorProfile::Custom { matrix_coefficients, colour_primaries, transfer_characteristics, full_range } = s.color_profile {
		let matrix = this.number("heic-matrix-coefficients", "Matrix coefficients", Some("--matrix_coefficients · 0, 1, 2 or 4 to 14 (see H.273); default 6"), f64::from(matrix_coefficients), 0., 14., 0, None, false, |t, v| edit_custom(t, |m, _, _, _| *m = to_u16(v)), cx);
		let primaries = this.number("heic-colour-primaries", "Colour primaries", Some("--colour_primaries · 1, 2, 4 to 12 or 22 (see H.273); default 1"), f64::from(colour_primaries), 1., 22., 0, None, false, |t, v| edit_custom(t, |_, p, _, _| *p = to_u16(v)), cx);
		let transfer = this.number("heic-transfer-characteristics", "Transfer characteristics", Some("--transfer_characteristic · 1, 2 or 4 to 18 (see H.273); default 13"), f64::from(transfer_characteristics), 1., 18., 0, None, false, |t, v| edit_custom(t, |_, _, tc, _| *tc = to_u16(v)), cx);
		let range = this.toggle("heic-full-range", "Full range", Some("--full_range_flag · default: 1"), full_range, false, |t, v| edit_custom(t, |_, _, _, f| *f = v), window, cx);
		// `grid-cols-3 gap-4` with the code points in a `col-span-2 grid grid-cols-2 gap-2`:
		// the spanning cell is two columns and the gap between them, which a 16px basis and
		// twice the growth of the first cell gives exactly.
		let mut span = div().flex_basis(px(16.)).min_w_0().flex().flex_col().child(Skid::grid(2, 8., vec![matrix, primaries, transfer, range]));
		span.style().flex_grow = Some(2.);
		div().flex().gap(px(16.)).child(div().flex_1().min_w_0().flex().flex_col().child(profile)).child(span).into_any_element()
	} else {
		Skid::grid(3, 16., vec![profile]).into_any_element()
	};
	let two_colr = this.toggle("heic-two-colr-boxes", "Two colour boxes", Some("--enable-two-colr-boxes · write both an ICC and an nclx color profile if both are present"), s.two_colr_boxes, false, |t, v| t.job().heic.two_colr_boxes = v, window, cx);
	let clli_body = s.clli.map(|[max_cll, max_pall]| {
		let cll = this.number("heic-clli-max-cll", "MaxCLL", Some("--clli MaxCLL,MaxPALL"), f64::from(max_cll), 0., 65535., 0, Some("cd/m²"), false, |t, v| if let Some(clli) = t.job().heic.clli.as_mut() { clli[0] = to_u16(v) }, cx);
		let pall = this.number("heic-clli-max-pall", "MaxPALL", None, f64::from(max_pall), 0., 65535., 0, Some("cd/m²"), false, |t, v| if let Some(clli) = t.job().heic.clli.as_mut() { clli[1] = to_u16(v) }, cx);
		div().flex().flex_col().child(cll).child(pall).into_any_element()
	});
	let clli = this.optional("heic-clli", "Content light level", None, Some("--clli · not given"), s.clli.is_some(), |t, on| t.job().heic.clli = on.then_some([1000, 400]), clli_body, window, cx);
	sections.push(this.panel("Colour", None, vec![profile_row, Skid::grid(3, 16., vec![two_colr, clli]).into_any_element()]).into_any_element());

	// Metadata.
	let icc = this.toggle("heic-icc", "Keep ICC profile", Some("heif-enc copies the input's colour profile; off is the same as removing it from the input first"), s.metadata.icc, false, |t, v| t.job().heic.metadata.icc = v, window, cx);
	let exif = this.toggle("heic-exif", "Keep Exif", Some("heif-enc copies the input's Exif; off is the same as removing it from the input first"), s.metadata.exif, false, |t, v| t.job().heic.metadata.exif = v, window, cx);
	let xmp = this.toggle("heic-xmp", "Keep XMP", Some("heif-enc copies the input's XMP; off is the same as removing it from the input first"), s.metadata.xmp, false, |t, v| t.job().heic.metadata.xmp = v, window, cx);
	sections.push(this.panel("Metadata", None, vec![Skid::grid(3, 16., vec![icc, exif, xmp]).into_any_element()]).into_any_element());

	// Geometry and container, folded away.
	sections.push(this.disclosure(
		"heic-geometry",
		"Geometry and container",
		false,
		move |this, window, cx| {
			let orientation = this.select("heic-orientation", "Rotation and mirroring", Some("--rotate-cw / --flip-h / --flip-v · signalled in the file, after a JPEG's own Exif orientation"), &owned(&ORIENTATIONS), s.orientation, false, |t, v| t.job().heic.orientation = v, window, cx);
			let pasp_body = s.pasp.map(|[h, v]| {
				let horizontal = this.number("heic-pasp-h", "Horizontal spacing", Some("--pasp h,v"), f64::from(h), 0., f64::from(u32::MAX), 0, None, false, |t, v| if let Some(pasp) = t.job().heic.pasp.as_mut() { pasp[0] = to_u32(v) }, cx);
				let vertical = this.number("heic-pasp-v", "Vertical spacing", None, f64::from(v), 0., f64::from(u32::MAX), 0, None, false, |t, v| if let Some(pasp) = t.job().heic.pasp.as_mut() { pasp[1] = to_u32(v) }, cx);
				div().flex().flex_col().child(horizontal).child(vertical).into_any_element()
			});
			let pasp = this.optional("heic-pasp", "Pixel aspect ratio", None, Some("--pasp · not given"), s.pasp.is_some(), |t, on| t.job().heic.pasp = on.then_some([1, 1]), pasp_body, window, cx);
			let tiles_body = s.cut_tiles.map(|size| this.number("heic-cut-tiles-size", "Tile size", Some("--cut-tiles · cuts the input image into square tiles of the given width"), f64::from(size), 1., f64::from(u32::MAX), 0, Some("px"), false, |t, v| t.job().heic.cut_tiles = Some(to_u32(v)), cx));
			let tiles = this.optional("heic-cut-tiles", "Cut into tiles", None, Some("--cut-tiles · not given: one image"), s.cut_tiles.is_some(), |t, on| t.job().heic.cut_tiles = on.then_some(512), tiles_body, window, cx);

			let projection_body = s.omaf_projection.map(|projection| this.choice("heic-omaf-projection-kind", "Projection", Some("--omaf-image-projection"), &PROJECTIONS, projection, false, false, false, |t, v| t.job().heic.omaf_projection = Some(v), window, cx));
			let projection = this.optional("heic-omaf-projection", "360° projection", None, Some("--omaf-image-projection · not given"), s.omaf_projection.is_some(), |t, on| t.job().heic.omaf_projection = on.then_some(OmafProjection::Equirectangular), projection_body, window, cx);
			let description = this.text_field("heic-description", "Description", Some("--pitm-description · set user description for primary image"), "None", &s.description, |t, v| t.job().heic.description = v, cx);
			let brands = brands_field(this, &s.compatible_brands, cx);

			let unif = this.toggle("heic-unif", "Unified IDs", Some("--unif · use unified ID namespace (adds 'unif' compatible brand)"), s.unif, false, |t, v| t.job().heic.unif = v, window, cx);
			let mini = this.toggle("heic-mini", "Compact format", Some("--mini · use compact 'mini' box format, when the image allows it"), s.mini, false, |t, v| t.job().heic.mini = v, window, cx);
			vec![Skid::grid(3, 16., vec![orientation, pasp, tiles]).into_any_element(), Skid::grid(3, 16., vec![projection, description, brands]).into_any_element(), Skid::grid(3, 16., vec![unif, mini]).into_any_element()]
		},
		window,
		cx,
	));

	div().flex().flex_col().gap(px(32.)).children(sections).into_any_element()
}

/// Edit the custom colour profile's code points, if the profile is custom.
fn edit_custom(this: &mut Skid, edit: impl FnOnce(&mut u16, &mut u16, &mut u16, &mut bool)) {
	if let ColorProfile::Custom { matrix_coefficients, colour_primaries, transfer_characteristics, full_range } = &mut this.job().heic.color_profile {
		edit(matrix_coefficients, colour_primaries, transfer_characteristics, full_range);
	}
}

/// The compatible brands: a `UInputTags` (`max-length 4`, `variant soft`, `size sm`, on
/// `bg-paleday-rule`) in the window's field frame. Enter adds the typed brand; each tag's
/// cross removes it.
fn brands_field(this: &mut Skid, brands: &[String], cx: &mut Context<Skid>) -> AnyElement {
	let key: SharedString = "text-heic-compatible-brands".into();
	let state = if let Some(state) = this.inputs.get(&key) {
		state.clone()
	} else {
		let state = cx.new(|cx| gpuikit::input::InputState::new_singleline(cx).submit_on(gpuikit::input::SubmitOn::Enter));
		let editing_key = key.clone();
		this.subscriptions.push(cx.subscribe(&state, move |this, state, event: &InputStateEvent, cx| match event {
			InputStateEvent::Focus => this.editing = Some(editing_key.clone()),
			InputStateEvent::Blur => this.editing = None,
			InputStateEvent::TextChanged => {
				// `max-length="4"`: the field takes no more than four characters.
				let content = state.read(cx).content().to_owned();
				if content.chars().count() > 4 {
					let kept: String = content.chars().take(4).collect();
					state.update(cx, |state, cx| state.set_content_silent(kept, cx));
				}
				cx.notify();
			}
			InputStateEvent::Submit => {
				// As Reka's TagsInput: a blank or repeated tag is not added.
				let brand = state.read(cx).content().trim().to_owned();
				if !brand.is_empty() && !this.job().heic.compatible_brands.contains(&brand) {
					this.job().heic.compatible_brands.push(brand);
					this.changed(cx);
				}
				state.update(cx, |state, cx| state.set_content_silent(String::new(), cx));
			}
			_ => {}
		}));
		this.inputs.insert(key, state.clone());
		state
	};

	let mut tags = div().flex().flex_wrap().items_center().gap(px(6.)).min_h(px(28.)).px(px(10.)).py(px(6.)).rounded(px(9.)).bg(c(RULE));
	for (index, brand) in brands.iter().enumerate() {
		tags = tags.child(
			div()
				.flex()
				.items_center()
				.gap(px(2.))
				.px(px(6.))
				.py(px(2.))
				.rounded(px(4.))
				.bg(c(FIELD))
				.xs()
				.medium()
				.text_color(c(FG))
				.child(brand.clone())
				.child(
					div()
						.id(ElementId::Name(format!("heic-brand-remove-{index}").into()))
						.cursor_pointer()
						.on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
							let brands = &mut t.job().heic.compatible_brands;
							if index < brands.len() {
								brands.remove(index);
							}
							t.changed(cx);
						}))
						.child(icon("lucide--x", 12., c(DIM))),
				),
		);
	}
	let tags = tags.child(input(&state, cx).placeholder("e.g. mif2").flex_1().min_w(px(48.)).h(px(16.)).bg(gpui::transparent_black()).xs().text_color(c(FG)));

	field()
		.child(field_label("Compatible brands"))
		.child(tags)
		.child(help("--add-compatible-brand · add a compatible brand to the output file (4 characters each)"))
		.into_any_element()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_u16(value: f64) -> u16 {
	value.round().clamp(0., f64::from(u16::MAX)) as u16
}

#[allow(clippy::cast_possible_truncation)]
fn to_i8(value: f64) -> i8 {
	value.round().clamp(f64::from(i8::MIN), f64::from(i8::MAX)) as i8
}
