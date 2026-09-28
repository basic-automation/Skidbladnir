//! The crop, rotation and mirror an AVIF asks to be shown with.
//!
//! An AVIF's pixels are stored as encoded; how to *display* them is a set of item
//! properties beside them — `clap` (clean aperture: a crop), `irot` (rotate by quarter
//! turns anticlockwise) and `imir` (mirror). A phone that stores a portrait photo sideways
//! and marks it `irot` looks right in a browser and wrong in anything that ignores the
//! property, so a converter that ignores them writes a sideways WebP. `avif-parse` does not
//! expose them, so this reads them itself: the primary item (`pitm`), its property
//! associations (`ipma`) and the properties (`ipco`), all inside the `meta` box.
//!
//! The properties are applied in the order the file associates them, which MIAF requires
//! to be crop, then rotation, then mirror. Semantics, as `avifenc` documents and `avifdec`
//! and `magick` (libheif) both display them:
//!
//! - `irot` angle *n*: rotate 90 × *n* degrees anticlockwise.
//! - `imir` axis 0: mirror top-to-bottom; axis 1: mirror left-to-right.
//! - `clap`: a width and height, and a centre offset, as fractions. The crop is used only
//!   when it lands on whole pixels inside the image, as libavif requires; otherwise it is
//!   ignored and the whole image is shown.
//!
//! Parsing never panics: a malformed or truncated box means no transforms, never a crash —
//! the image itself still decodes.

/// One display transform, in the order it applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transform {
	/// A clean aperture: keep this rectangle.
	Crop {
		/// Left edge.
		x: u32,
		/// Top edge.
		y: u32,
		/// Width.
		width: u32,
		/// Height.
		height: u32,
	},
	/// Rotate by this many quarter turns anticlockwise (1 to 3).
	Rotate(u8),
	/// Mirror top-to-bottom (`false`) or left-to-right (`true`).
	Mirror {
		/// Whether the mirror is left-to-right.
		left_to_right: bool,
	},
}

/// A property as parsed, before a `clap` is resolved against the image size.
#[derive(Clone, Copy, Debug)]
enum Property {
	/// `clap`: width, height, horizontal offset, vertical offset, each as (numerator,
	/// denominator).
	CleanAperture([(i64, i64); 4]),
	Rotate(u8),
	Mirror(bool),
}

/// The display transforms of the primary item of the AVIF in `bytes`, in order, for an
/// image `width` x `height` as decoded. Empty when there are none or the boxes cannot be
/// read.
#[must_use]
pub fn transforms(bytes: &[u8], width: u32, height: u32) -> Vec<Transform> {
	let Some(properties) = primary_properties(bytes) else { return Vec::new() };
	let (mut width, mut height) = (i64::from(width), i64::from(height));
	let mut out = Vec::new();
	for property in properties {
		match property {
			Property::CleanAperture(fractions) => {
				if let Some(crop) = crop_rect(fractions, width, height) {
					(width, height) = (i64::from(crop.2), i64::from(crop.3));
					out.push(Transform::Crop { x: crop.0, y: crop.1, width: crop.2, height: crop.3 });
				}
			}
			Property::Rotate(quarters) if quarters % 4 != 0 => {
				if quarters % 2 == 1 {
					(width, height) = (height, width);
				}
				out.push(Transform::Rotate(quarters % 4));
			}
			Property::Rotate(_) => {}
			Property::Mirror(left_to_right) => out.push(Transform::Mirror { left_to_right }),
		}
	}
	out
}

/// Resolve a `clap` against the image size, libavif's way: the crop must be a whole number
/// of pixels, centred on the image centre plus the offset, and lie inside the image.
fn crop_rect([(wn, wd), (hn, hd), (xn, xd), (yn, yd)]: [(i64, i64); 4], width: i64, height: i64) -> Option<(u32, u32, u32, u32)> {
	if wd <= 0 || hd <= 0 || xd <= 0 || yd <= 0 || wn % wd != 0 || hn % hd != 0 {
		return None;
	}
	let (crop_w, crop_h) = (wn / wd, hn / hd);
	// x = offset + (width - crop_w) / 2, kept exact as a fraction over 2 * xd.
	let x_num = 2 * xn + (width - crop_w) * xd;
	let y_num = 2 * yn + (height - crop_h) * yd;
	if x_num % (2 * xd) != 0 || y_num % (2 * yd) != 0 {
		return None;
	}
	let (x, y) = (x_num / (2 * xd), y_num / (2 * yd));
	if crop_w <= 0 || crop_h <= 0 || x < 0 || y < 0 || x + crop_w > width || y + crop_h > height {
		return None;
	}
	Some((u32::try_from(x).ok()?, u32::try_from(y).ok()?, u32::try_from(crop_w).ok()?, u32::try_from(crop_h).ok()?))
}

/// Apply `transforms` to straight RGBA pixels, returning the new size and pixels.
#[must_use]
pub fn apply(mut width: u32, mut height: u32, mut pixels: Vec<u8>, transforms: &[Transform]) -> (u32, u32, Vec<u8>) {
	for transform in transforms {
		let (w, h) = (width as usize, height as usize);
		let at = |x: usize, y: usize| (y * w + x) * 4;
		let mut out = Vec::with_capacity(pixels.len());
		match *transform {
			Transform::Crop { x, y, width: crop_w, height: crop_h } => {
				for row in y as usize..(y + crop_h) as usize {
					out.extend_from_slice(&pixels[at(x as usize, row)..at((x + crop_w) as usize, row)]);
				}
				(width, height) = (crop_w, crop_h);
			}
			Transform::Rotate(quarters) => {
				let (new_w, new_h) = if quarters % 2 == 1 { (h, w) } else { (w, h) };
				for ny in 0..new_h {
					for nx in 0..new_w {
						// Where the output pixel comes from, for an anticlockwise rotation.
						let (sx, sy) = match quarters {
							1 => (w - 1 - ny, nx),
							2 => (w - 1 - nx, h - 1 - ny),
							_ => (ny, h - 1 - nx),
						};
						out.extend_from_slice(&pixels[at(sx, sy)..at(sx, sy) + 4]);
					}
				}
				(width, height) = (u32::try_from(new_w).unwrap_or(width), u32::try_from(new_h).unwrap_or(height));
			}
			Transform::Mirror { left_to_right } => {
				for ny in 0..h {
					for nx in 0..w {
						let (sx, sy) = if left_to_right { (w - 1 - nx, ny) } else { (nx, h - 1 - ny) };
						out.extend_from_slice(&pixels[at(sx, sy)..at(sx, sy) + 4]);
					}
				}
			}
		}
		pixels = out;
	}
	(width, height, pixels)
}

/// A box: its four-character type and its payload (after the header).
struct Box<'a> {
	kind: [u8; 4],
	body: &'a [u8],
}

/// Iterate the boxes laid end to end in `data`, stopping at the first malformed one.
fn boxes(mut data: &[u8]) -> impl Iterator<Item = Box<'_>> {
	std::iter::from_fn(move || {
		let size = u64::from(u32::from_be_bytes(data.get(..4)?.try_into().ok()?));
		let kind: [u8; 4] = data.get(4..8)?.try_into().ok()?;
		let (header, size) = match size {
			0 => (8, data.len() as u64),
			1 => (16, u64::from_be_bytes(data.get(8..16)?.try_into().ok()?)),
			size => (8, size),
		};
		let size = usize::try_from(size).ok()?;
		if size < header || size > data.len() {
			return None;
		}
		let body = &data[header..size];
		data = &data[size..];
		Some(Box { kind, body })
	})
}

/// The `clap`/`irot`/`imir` properties associated with the primary item, in order.
fn primary_properties(bytes: &[u8]) -> Option<Vec<Property>> {
	let meta = boxes(bytes).find(|b| &b.kind == b"meta")?;
	// `meta` is a full box: skip its version and flags.
	let children = meta.body.get(4..)?;
	let pitm = boxes(children).find(|b| &b.kind == b"pitm")?;
	let primary = match pitm.body.first()? {
		0 => u32::from(u16::from_be_bytes(pitm.body.get(4..6)?.try_into().ok()?)),
		_ => u32::from_be_bytes(pitm.body.get(4..8)?.try_into().ok()?),
	};
	let iprp = boxes(children).find(|b| &b.kind == b"iprp")?;
	let ipco = boxes(iprp.body).find(|b| &b.kind == b"ipco")?;
	let properties: Vec<Box<'_>> = boxes(ipco.body).collect();

	let mut found = Vec::new();
	for ipma in boxes(iprp.body).filter(|b| &b.kind == b"ipma") {
		let (version, flags) = (*ipma.body.first()?, *ipma.body.get(3)?);
		let mut at = 4;
		let count = u32::from_be_bytes(ipma.body.get(at..at + 4)?.try_into().ok()?);
		at += 4;
		for _ in 0..count {
			let item = if version < 1 {
				let id = u32::from(u16::from_be_bytes(ipma.body.get(at..at + 2)?.try_into().ok()?));
				at += 2;
				id
			} else {
				let id = u32::from_be_bytes(ipma.body.get(at..at + 4)?.try_into().ok()?);
				at += 4;
				id
			};
			let associations = *ipma.body.get(at)?;
			at += 1;
			for _ in 0..associations {
				// The top bit is "essential"; the rest is a 1-based index into `ipco`.
				let index = if flags & 1 == 1 {
					let value = u16::from_be_bytes(ipma.body.get(at..at + 2)?.try_into().ok()?);
					at += 2;
					usize::from(value & 0x7fff)
				} else {
					let value = *ipma.body.get(at)?;
					at += 1;
					usize::from(value & 0x7f)
				};
				if item == primary
					&& index > 0
					&& let Some(property) = properties.get(index - 1).and_then(parse_property)
				{
					found.push(property);
				}
			}
		}
	}
	Some(found)
}

/// Parse a `clap`, `irot` or `imir` property box; `None` for any other property.
fn parse_property(property: &Box<'_>) -> Option<Property> {
	let body = property.body;
	match &property.kind {
		b"clap" => {
			let word = |i: usize| body.get(i * 4..i * 4 + 4).and_then(|b| b.try_into().ok()).map(u32::from_be_bytes);
			// Widths and heights are unsigned; offsets are signed.
			let (wn, wd, hn, hd) = (i64::from(word(0)?), i64::from(word(1)?), i64::from(word(2)?), i64::from(word(3)?));
			let (xn, xd, yn, yd) = (i64::from(word(4)?.cast_signed()), i64::from(word(5)?), i64::from(word(6)?.cast_signed()), i64::from(word(7)?));
			Some(Property::CleanAperture([(wn, wd), (hn, hd), (xn, xd), (yn, yd)]))
		}
		b"irot" => Some(Property::Rotate(body.first()? & 0b11)),
		b"imir" => Some(Property::Mirror(body.first()? & 1 == 1)),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::{Transform, apply, crop_rect};

	/// A 3x2 image whose pixels are numbered 0..6 in reading order, in the red channel.
	fn numbered() -> Vec<u8> {
		(0..6_u8).flat_map(|n| [n, 0, 0, 255]).collect()
	}

	fn reds(pixels: &[u8]) -> Vec<u8> {
		pixels.as_chunks::<4>().0.iter().map(|p| p[0]).collect()
	}

	#[test]
	fn rotations_turn_anticlockwise() {
		// 0 1 2      a quarter turn anticlockwise:  2 5
		// 3 4 5                                     1 4
		//                                           0 3
		let (w, h, px) = apply(3, 2, numbered(), &[Transform::Rotate(1)]);
		assert_eq!((w, h, reds(&px)), (2, 3, vec![2, 5, 1, 4, 0, 3]));
		let (_, _, px) = apply(3, 2, numbered(), &[Transform::Rotate(2)]);
		assert_eq!(reds(&px), [5, 4, 3, 2, 1, 0]);
		let (w, h, px) = apply(3, 2, numbered(), &[Transform::Rotate(3)]);
		assert_eq!((w, h, reds(&px)), (2, 3, vec![3, 0, 4, 1, 5, 2]));
	}

	#[test]
	fn mirrors_flip_the_named_way() {
		let (_, _, px) = apply(3, 2, numbered(), &[Transform::Mirror { left_to_right: false }]);
		assert_eq!(reds(&px), [3, 4, 5, 0, 1, 2], "axis 0 is top-to-bottom");
		let (_, _, px) = apply(3, 2, numbered(), &[Transform::Mirror { left_to_right: true }]);
		assert_eq!(reds(&px), [2, 1, 0, 5, 4, 3], "axis 1 is left-to-right");
	}

	#[test]
	fn crops_keep_the_rectangle() {
		let (w, h, px) = apply(3, 2, numbered(), &[Transform::Crop { x: 1, y: 0, width: 2, height: 2 }]);
		assert_eq!((w, h, reds(&px)), (2, 2, vec![1, 2, 4, 5]));
	}

	#[test]
	fn a_clean_aperture_resolves_like_libavif() {
		// 20x20 centred in 40x20, offset 0: x = (40 - 20) / 2 = 10.
		assert_eq!(crop_rect([(20, 1), (20, 1), (0, 1), (0, 1)], 40, 20), Some((10, 0, 20, 20)));
		// A half-pixel offset lands between pixels, so the crop is ignored.
		assert_eq!(crop_rect([(20, 1), (20, 1), (1, 2), (0, 1)], 40, 20), None);
		// Offsets are relative to the centre and may be negative: -10 puts it at the left edge.
		assert_eq!(crop_rect([(20, 1), (20, 1), (-10, 1), (0, 1)], 40, 20), Some((0, 0, 20, 20)));
		// Outside the image, or a zero denominator, is ignored rather than trusted.
		assert_eq!(crop_rect([(20, 1), (20, 1), (-11, 1), (0, 1)], 40, 20), None);
		assert_eq!(crop_rect([(20, 0), (20, 1), (0, 1), (0, 1)], 40, 20), None);
	}

	#[test]
	fn malformed_boxes_mean_no_transforms_not_a_panic() {
		assert_eq!(super::transforms(b"", 10, 10), []);
		assert_eq!(super::transforms(b"\x00\x00\x00\x08meta", 10, 10), []);
		assert_eq!(super::transforms(b"\xff\xff\xff\xffmeta\x00\x00\x00\x00", 10, 10), []);
		assert_eq!(super::transforms(&[0_u8; 3], 10, 10), []);
	}
}
