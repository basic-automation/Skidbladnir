//! SVG input: a vector image rasterised by `resvg` at its own size, its transparency kept.
//!
//! None of the reference tools reads SVG, so there is no parity to keep; what matters is
//! that the result is the picture the file describes, and that reading it touches nothing
//! but the file itself. An SVG can refer to other files (`<image href="…">`, fonts); here
//! only images embedded as `data:` URLs are drawn, so a file the user dropped can never make
//! the app read another one. Text is set with the system's fonts.

use std::sync::Arc;

/// The most pixels an SVG is rasterised to: its own size can be anything it says, and a
/// `width="100000"` must not allocate tens of gigabytes.
pub const MAX_PIXELS: u64 = 100_000_000;

/// A rasterised SVG: straight (not premultiplied) 8-bit RGBA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raster {
	/// Width in pixels.
	pub width: u32,
	/// Height in pixels.
	pub height: u32,
	/// Tightly packed RGBA.
	pub pixels: Vec<u8>,
}

/// Whether `bytes` (the start of a file is enough) is an SVG document: XML whose first
/// element is `svg`, after any BOM, whitespace, XML declaration, comments or doctype. An
/// HTML page with an inline `<svg>` is not one.
#[must_use]
pub fn sniff(bytes: &[u8]) -> bool {
	let text = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
	let start = text.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(text.len());
	let text = &text[start..];
	if text.first() != Some(&b'<') {
		return false;
	}
	let Some(at) = text.windows(4).position(|window| window == b"<svg") else { return false };
	let after = text.get(at + 4).copied();
	let head = String::from_utf8_lossy(&text[..at]).to_ascii_lowercase();
	matches!(after, Some(b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'/' | b':')) && !head.contains("<html") && !head.contains("<!doctype html")
}

/// Rasterise an SVG document at its own size.
///
/// # Errors
///
/// A description of the failure: not an SVG `resvg` can parse, a size of zero, or more than
/// [`MAX_PIXELS`].
pub fn rasterise(bytes: &[u8]) -> Result<Raster, String> {
	draw(bytes, None)
}

/// Rasterise an SVG document straight at `width` x `height`, either of them `0` to keep the
/// aspect ratio, derived as libwebp's rescaler derives it (rounding up), so the size is the
/// one any other source resized the same way gets. Drawn at that size, edges stay sharp
/// where scaling a raster would blur them.
///
/// # Errors
///
/// As [`rasterise`].
pub fn rasterise_at(bytes: &[u8], width: u32, height: u32) -> Result<Raster, String> {
	draw(bytes, Some((width, height)))
}

fn draw(bytes: &[u8], size: Option<(u32, u32)>) -> Result<Raster, String> {
	let mut fonts = resvg::usvg::fontdb::Database::new();
	fonts.load_system_fonts();
	// Only images embedded in the file itself; no path or URL is ever opened.
	let options = resvg::usvg::Options { image_href_resolver: resvg::usvg::ImageHrefResolver { resolve_data: resvg::usvg::ImageHrefResolver::default_data_resolver(), resolve_string: Box::new(|_, _| None) }, resources_dir: None, fontdb: Arc::new(fonts), ..resvg::usvg::Options::default() };

	let tree = resvg::usvg::Tree::from_data(bytes, &options).map_err(|error| format!("not an SVG resvg can read: {error}"))?;
	let own = tree.size().to_int_size();
	let (own_width, own_height) = (own.width(), own.height());
	let (width, height) = match size {
		None | Some((0, 0)) => (own_width, own_height),
		// WebPRescalerGetScaledDimensions.
		Some((0, height)) => (u32::try_from((u64::from(own_width) * u64::from(height)).div_ceil(u64::from(own_height))).unwrap_or(u32::MAX), height),
		Some((width, 0)) => (width, u32::try_from((u64::from(own_height) * u64::from(width)).div_ceil(u64::from(own_width))).unwrap_or(u32::MAX)),
		Some(size) => size,
	};
	if u64::from(width) * u64::from(height) > MAX_PIXELS {
		return Err(format!("the SVG is {width}x{height}, more than {MAX_PIXELS} pixels; resize it in an editor first"));
	}
	let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).ok_or_else(|| format!("cannot draw an SVG of {width}x{height}"))?;
	#[expect(clippy::cast_precision_loss, reason = "a scale factor; sizes are far below f32's exact-integer range")]
	let scale = resvg::tiny_skia::Transform::from_scale(width as f32 / own_width as f32, height as f32 / own_height as f32);
	resvg::render(&tree, scale, &mut pixmap.as_mut());
	// tiny-skia keeps premultiplied colour; every encoder here takes straight.
	let pixels = pixmap.pixels().iter().flat_map(|pixel| {
		let straight = pixel.demultiply();
		[straight.red(), straight.green(), straight.blue(), straight.alpha()]
	});
	Ok(Raster { width, height, pixels: pixels.collect() })
}

#[cfg(test)]
mod tests {
	use super::{MAX_PIXELS, rasterise, sniff};

	#[test]
	fn knows_an_svg_by_its_content() {
		assert!(sniff(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"));
		assert!(sniff(b"\xef\xbb\xbf\n  <?xml version=\"1.0\"?>\n<!-- made by hand -->\n<!DOCTYPE svg>\n<svg width=\"2\"></svg>"));
		assert!(sniff(b"<svg:svg xmlns:svg=\"http://www.w3.org/2000/svg\"/>"));
		assert!(!sniff(b"<!DOCTYPE html><html><body><svg></svg></body></html>"), "an HTML page is not an SVG");
		assert!(!sniff(b"<svgfoo/>"));
		assert!(!sniff(b"not xml <svg>"));
		assert!(!sniff(b"\x89PNG\r\n\x1a\n"));
	}

	/// The picture the file describes, at its size, transparency kept and colours
	/// straight.
	#[test]
	fn rasterises_at_its_own_size() {
		let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="4"><rect x="0" y="0" width="4" height="4" fill="#ff0000"/><rect x="4" y="0" width="4" height="4" fill="#0000ff" fill-opacity="0.5"/></svg>"##;
		let raster = rasterise(svg).expect("rasterise");
		assert_eq!((raster.width, raster.height), (8, 4));
		assert_eq!(&raster.pixels[0..4], &[255, 0, 0, 255]);
		let right = &raster.pixels[(4 * 4)..(4 * 4 + 4)];
		assert_eq!((right[0], right[1], right[2]), (0, 0, 255), "straight colour under half transparency");
		assert!((127..=128).contains(&right[3]));
	}

	/// A file it refers to is never read: only images embedded in the SVG are drawn.
	#[test]
	fn never_reads_another_file() {
		let dir = std::env::temp_dir().join(format!("skidbladnir-svg-{}", std::process::id()));
		std::fs::create_dir_all(&dir).expect("create");
		let png = dir.join("red.png");
		image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255])).save(&png).expect("write");
		let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="4" height="4"><image href="{0}" xlink:href="{0}" width="4" height="4"/></svg>"#, png.display());
		let raster = rasterise(svg.as_bytes()).expect("rasterise");
		assert!(raster.pixels.chunks(4).all(|p| p[3] == 0), "the referenced file must not be drawn");
		let _ = std::fs::remove_dir_all(&dir);
	}

	/// Drawn at a resize's size, an edge stays exactly where it falls, with no blur, and a
	/// missing dimension is derived as libwebp derives it.
	#[test]
	fn draws_at_the_size_asked_for() {
		let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="4"><rect width="5" height="4" fill="#ff0000"/><rect x="5" width="5" height="4" fill="#0000ff"/></svg>"##;
		let big = super::rasterise_at(svg, 100, 40).expect("rasterise");
		assert_eq!((big.width, big.height), (100, 40));
		let at = |x: usize| &big.pixels[x * 4..x * 4 + 4];
		assert_eq!((at(49), at(50)), (&[255, 0, 0, 255][..], &[0, 0, 255, 255][..]), "a sharp edge, not a blurred one");
		for (asked, expected) in [((25, 0), (25, 10)), ((0, 3), (8, 3)), ((7, 0), (7, 3))] {
			let drawn = super::rasterise_at(svg, asked.0, asked.1).expect("rasterise");
			let pixels = vec![0_u8; 10 * 4 * 4];
			let (w, h, _) = crate::encoder::rescale_rgba(&crate::encoder::RgbaImage { width: 10, height: 4, pixels: &pixels }, crate::settings::Resize::to(asked.0, asked.1)).expect("libwebp rescales");
			assert_eq!(((drawn.width, drawn.height), (w, h)), (expected, expected), "{asked:?}");
		}
	}

	#[test]
	fn refuses_what_it_cannot_or_should_not_draw() {
		assert!(rasterise(b"<svg").is_err());
		let huge = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{MAX_PIXELS}" height="{MAX_PIXELS}"/>"#);
		assert!(rasterise(huge.as_bytes()).expect_err("too big").contains("more than"));
	}
}
