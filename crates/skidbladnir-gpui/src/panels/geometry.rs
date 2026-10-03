//! `GeometrySettingsPanel.vue`: the crop and resize every output format shares, and how a
//! TIFF's alpha and a raw YUV file are read.

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use skidbladnir_encode::settings::{Crop, RawSize, ResizeMode};

use crate::{app::Skid, panels::webp::to_u32};

pub fn render(this: &mut Skid, window: &mut Window, cx: &mut Context<Skid>) -> AnyElement {
	let crop = this.job().crop;
	let resize = this.job().resize;

	let crop_body = crop.map(|value| {
		let left = this.number("crop-x", "Left", Some("-crop x"), f64::from(value.x), 0., f64::from(u32::MAX), 0, Some("px"), false, |t, v| set_crop(t, |crop| crop.x = to_u32(v)), cx);
		let top = this.number("crop-y", "Top", Some("-crop y"), f64::from(value.y), 0., f64::from(u32::MAX), 0, Some("px"), false, |t, v| set_crop(t, |crop| crop.y = to_u32(v)), cx);
		let width = this.number("crop-w", "Width", Some("-crop w"), f64::from(value.width), 1., f64::from(u32::MAX), 0, Some("px"), false, |t, v| set_crop(t, |crop| crop.width = to_u32(v)), cx);
		let height = this.number("crop-h", "Height", Some("-crop h"), f64::from(value.height), 1., f64::from(u32::MAX), 0, Some("px"), false, |t, v| set_crop(t, |crop| crop.height = to_u32(v)), cx);
		Skid::grid(4, 8., vec![left, top, width, height]).into_any_element()
	});
	let crop_control = this.optional("crop", "Crop the source", None, Some("-crop · not given: the whole image"), crop.is_some(), |t, on| t.job().crop = on.then_some(Crop { x: 0, y: 0, width: 1024, height: 1024 }), crop_body, window, cx);

	let width = this.number("resize-w", "Width", Some("-resize w · 0 derives it from the height, keeping the aspect ratio"), f64::from(resize.width), 0., f64::from(u32::MAX), 0, Some("px"), false, |t, v| t.job().resize.width = to_u32(v), cx);
	let height = this.number("resize-h", "Height", Some("-resize h · 0 derives it from the width; both 0 is no resize"), f64::from(resize.height), 0., f64::from(u32::MAX), 0, Some("px"), false, |t, v| t.job().resize.height = to_u32(v), cx);
	let mode = this.choice("resize-mode", "When", Some("-resize_mode · always, down_only or up_only"), &[(ResizeMode::Always, "Always", None), (ResizeMode::DownOnly, "Only shrink", None), (ResizeMode::UpOnly, "Only enlarge", None)], resize.mode, false, false, false, |t, v| t.job().resize.mode = v, window, cx);

	let yuv_size = this.job().yuv_size;
	let yuv_body = yuv_size.map(|value| {
		let width = this.number("yuv-w", "Width", Some("-s w"), f64::from(value.width), 1., 16383., 0, Some("px"), false, |t, v| set_yuv_size(t, |size| size.width = to_u32(v)), cx);
		let height = this.number("yuv-h", "Height", Some("-s h"), f64::from(value.height), 1., 16383., 0, Some("px"), false, |t, v| set_yuv_size(t, |size| size.height = to_u32(v)), cx);
		Skid::grid(4, 8., vec![width, height]).into_any_element()
	});
	let yuv = this.optional("yuv-size", "Read .yuv files as raw I420", None, Some("-s · not given: a .yuv file cannot be read"), yuv_size.is_some(), |t, on| t.job().yuv_size = on.then_some(RawSize { width: 1920, height: 1080 }), yuv_body, window, cx);

	let tiff_alpha = this.job().tiff_alpha_like_reference;
	let tiff = this.toggle("tiff-alpha", "Read transparency as the official tool does", Some("Off: a TIFF's semi-transparent colours are read correctly. On: as cwebp reads them for WebP (straight alpha comes out darker) and heif-enc for HEIC (premultiplied alpha comes out darker), byte for byte. AVIF and JPEG XL are unaffected."), tiff_alpha, false, |t, v| t.job().tiff_alpha_like_reference = v, window, cx);

	div().flex().flex_col().gap(px(32.)).child(this.panel("Crop", None, vec![crop_control])).child(this.panel("Resize", None, vec![Skid::grid(3, 12., vec![width, height, mode]).into_any_element()])).child(this.panel("Raw YUV input", None, vec![yuv])).child(this.panel("TIFF input", None, vec![tiff])).into_any_element()
}

fn set_crop(this: &mut Skid, edit: impl FnOnce(&mut Crop)) {
	if let Some(crop) = this.job().crop.as_mut() {
		edit(crop);
	}
}

fn set_yuv_size(this: &mut Skid, edit: impl FnOnce(&mut RawSize)) {
	if let Some(size) = this.job().yuv_size.as_mut() {
		edit(size);
	}
}
