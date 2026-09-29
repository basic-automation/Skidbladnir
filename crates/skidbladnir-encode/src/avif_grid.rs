//! Grid (tiled) AVIFs: an image stored as a grid of separately encoded tiles.
//!
//! AV1 caps the size of a single frame, and some encoders tile large images anyway, so a
//! big AVIF can be a `grid` item: a small descriptor (rows, columns, output size) plus a
//! `dimg` reference to each tile, in row-major order, each an ordinary AV1 image item.
//! `avif-parse` refuses these outright, and `avif-decode` only decodes whole files.
//!
//! So this reads the container itself and, for each tile, **rewraps it as a minimal
//! single-image AVIF** — the tile's own AV1 data and property boxes (and those of the
//! matching alpha tile, if the alpha plane is a grid too), under a fresh `meta` — hands that
//! to the ordinary decoder, and stitches the decoded tiles onto the canvas, cropping to the
//! grid's output size as the format requires (edge tiles may overhang it).
//!
//! Everything the grid item itself carries (crop, rotation, mirror) is left to
//! [`crate::avif_transform`], which reads them from the primary item — the grid.
//!
//! The gate is `tests/avif_input.rs` (`decodes_grid_avifs`): lossless `avifenc --grid`
//! files must come back exactly as their source, and lossy ones as `avifdec` shows them.

use std::collections::HashMap;

/// A box as found in a file: its type, its payload, and where the payload starts in the
/// file (needed because `iloc` offsets are absolute).
#[derive(Clone, Copy)]
struct Found<'a> {
	kind: [u8; 4],
	body: &'a [u8],
	start: usize,
}

/// Iterate the boxes laid end to end in `data`, which begins at `base` in the file.
fn boxes(data: &[u8], base: usize) -> impl Iterator<Item = Found<'_>> {
	let mut at = 0_usize;
	std::iter::from_fn(move || {
		let rest = data.get(at..)?;
		let size = u64::from(u32::from_be_bytes(rest.get(..4)?.try_into().ok()?));
		let kind: [u8; 4] = rest.get(4..8)?.try_into().ok()?;
		let (header, size) = match size {
			0 => (8, rest.len() as u64),
			1 => (16, u64::from_be_bytes(rest.get(8..16)?.try_into().ok()?)),
			size => (8, size),
		};
		let size = usize::try_from(size).ok()?;
		if size < header || size > rest.len() {
			return None;
		}
		let found = Found { kind, body: &rest[header..size], start: base + at + header };
		at += size;
		Some(found)
	})
}

/// A cursor over big-endian fields, returning `None` past the end.
struct Reader<'a> {
	data: &'a [u8],
	at: usize,
}

impl<'a> Reader<'a> {
	const fn new(data: &'a [u8]) -> Self {
		Self { data, at: 0 }
	}

	fn bytes(&mut self, count: usize) -> Option<&'a [u8]> {
		let out = self.data.get(self.at..self.at.checked_add(count)?)?;
		self.at += count;
		Some(out)
	}

	fn u8(&mut self) -> Option<u8> {
		self.bytes(1).map(|b| b[0])
	}

	fn u16(&mut self) -> Option<u16> {
		self.bytes(2)?.try_into().ok().map(u16::from_be_bytes)
	}

	fn u32(&mut self) -> Option<u32> {
		self.bytes(4)?.try_into().ok().map(u32::from_be_bytes)
	}

	/// An unsigned field of `size` bytes (0, 4 or 8, as `iloc` allows).
	fn sized(&mut self, size: u8) -> Option<u64> {
		match size {
			0 => Some(0),
			4 => self.u32().map(u64::from),
			8 => self.bytes(8)?.try_into().ok().map(u64::from_be_bytes),
			_ => None,
		}
	}

	/// An item ID: 16 bits in version-0 boxes, 32 bits otherwise.
	fn id(&mut self, wide: bool) -> Option<u32> {
		if wide { self.u32() } else { self.u16().map(u32::from) }
	}
}

/// What the `meta` box says, as far as a grid needs it.
struct Meta<'a> {
	primary: u32,
	types: HashMap<u32, [u8; 4]>,
	/// Each item's data, gathered from its `iloc` extents.
	data: HashMap<u32, Vec<u8>>,
	/// `(reference type, from, to)`, in file order.
	references: Vec<([u8; 4], u32, Vec<u32>)>,
	/// The `ipco` property boxes, whole (header included), in order.
	properties: Vec<&'a [u8]>,
	/// Each item's associated property indices (1-based into `properties`), with the
	/// essential flag.
	associations: HashMap<u32, Vec<(bool, u16)>>,
}

/// Parse the parts of `meta` a grid needs. `None` for anything malformed.
fn parse_meta(file: &[u8]) -> Option<Meta<'_>> {
	let meta = boxes(file, 0).find(|b| &b.kind == b"meta")?;
	let children_start = meta.start + 4;
	let children: Vec<Found<'_>> = boxes(meta.body.get(4..)?, children_start).collect();
	let child = |kind: &[u8; 4]| children.iter().find(|b| &b.kind == kind).copied();

	let pitm = child(b"pitm")?;
	let primary = Reader::new(pitm.body.get(4..)?).id(pitm.body.first()? != &0)?;

	let mut types = HashMap::new();
	let iinf = child(b"iinf")?;
	let skip = if iinf.body.first()? == &0 { 6 } else { 8 };
	for infe in boxes(iinf.body.get(skip..)?, 0).filter(|b| &b.kind == b"infe") {
		let version = *infe.body.first()?;
		let mut r = Reader::new(infe.body.get(4..)?);
		if version < 2 {
			continue;
		}
		let id = r.id(version >= 3)?;
		r.u16()?; // protection index
		types.insert(id, r.bytes(4)?.try_into().ok()?);
	}

	let idat = child(b"idat").map(|b| b.body);
	let mut data = HashMap::new();
	let iloc = child(b"iloc")?;
	let version = *iloc.body.first()?;
	let mut r = Reader::new(iloc.body.get(4..)?);
	let sizes = r.u8()?;
	let (offset_size, length_size) = (sizes >> 4, sizes & 0x0f);
	let sizes = r.u8()?;
	let (base_size, index_size) = (sizes >> 4, if version >= 1 { sizes & 0x0f } else { 0 });
	let items = if version < 2 { u32::from(r.u16()?) } else { r.u32()? };
	for _ in 0..items {
		let id = r.id(version >= 2)?;
		let construction = if version >= 1 { r.u16()? & 0x0f } else { 0 };
		r.u16()?; // data reference index
		let base = r.sized(base_size)?;
		let extents = r.u16()?;
		let mut bytes = Vec::new();
		for _ in 0..extents {
			r.sized(index_size)?;
			let offset = usize::try_from(base.checked_add(r.sized(offset_size)?)?).ok()?;
			let length = usize::try_from(r.sized(length_size)?).ok()?;
			let source = match construction {
				0 => file,
				1 => idat?,
				_ => return None,
			};
			// A zero length means "to the end of the source".
			let end = if length == 0 { source.len() } else { offset.checked_add(length)? };
			bytes.extend_from_slice(source.get(offset..end)?);
		}
		data.insert(id, bytes);
	}

	let mut references = Vec::new();
	if let Some(iref) = child(b"iref") {
		let wide = iref.body.first()? != &0;
		for reference in boxes(iref.body.get(4..)?, 0) {
			let mut r = Reader::new(reference.body);
			let from = r.id(wide)?;
			let count = r.u16()?;
			let to = (0..count).map(|_| r.id(wide)).collect::<Option<Vec<_>>>()?;
			references.push((reference.kind, from, to));
		}
	}

	let iprp = child(b"iprp")?;
	let ipco = boxes(iprp.body, 0).find(|b| &b.kind == b"ipco")?;
	let mut properties = Vec::new();
	let mut at = 0;
	for property in boxes(ipco.body, 0) {
		let whole = 8 + property.body.len();
		properties.push(ipco.body.get(at..at + whole)?);
		at += whole;
	}
	let mut associations: HashMap<u32, Vec<(bool, u16)>> = HashMap::new();
	for ipma in boxes(iprp.body, 0).filter(|b| &b.kind == b"ipma") {
		let (version, flags) = (*ipma.body.first()?, *ipma.body.get(3)?);
		let mut r = Reader::new(ipma.body.get(4..)?);
		for _ in 0..r.u32()? {
			let id = r.id(version >= 1)?;
			for _ in 0..r.u8()? {
				let (essential, index) = if flags & 1 == 1 {
					let v = r.u16()?;
					(v & 0x8000 != 0, v & 0x7fff)
				} else {
					let v = r.u8()?;
					(v & 0x80 != 0, u16::from(v & 0x7f))
				};
				associations.entry(id).or_default().push((essential, index));
			}
		}
	}

	Some(Meta { primary, types, data, references, properties, associations })
}

/// A grid descriptor: tile layout and the size of the finished image.
struct Grid {
	rows: u32,
	columns: u32,
	width: u32,
	height: u32,
}

fn parse_grid(payload: &[u8]) -> Option<Grid> {
	let mut r = Reader::new(payload);
	r.u8()?; // version
	let flags = r.u8()?;
	let (rows, columns) = (u32::from(r.u8()?) + 1, u32::from(r.u8()?) + 1);
	let (width, height) = if flags & 1 == 1 { (r.u32()?, r.u32()?) } else { (u32::from(r.u16()?), u32::from(r.u16()?)) };
	Some(Grid { rows, columns, width, height })
}

/// Whether the primary item of the AVIF in `bytes` is a grid.
#[must_use]
pub fn is_grid(bytes: &[u8]) -> bool {
	parse_meta(bytes).is_some_and(|meta| meta.types.get(&meta.primary) == Some(b"grid"))
}

/// A decoded image: `(width, height, rgba)`.
pub type Decoded = (u32, u32, Vec<u8>);

/// Decode a grid AVIF to `(width, height, rgba)`, decoding each tile with `decode_tile`
/// (the ordinary single-image decoder).
///
/// # Errors
///
/// A description of what was wrong with the grid, or the tile decoder's own error.
pub fn decode(bytes: &[u8], decode_tile: &dyn Fn(&[u8]) -> Result<Decoded, String>) -> Result<Decoded, String> {
	let meta = parse_meta(bytes).ok_or("could not read the AVIF grid's boxes")?;
	let grid = parse_grid(meta.data.get(&meta.primary).ok_or("the AVIF grid has no descriptor")?).ok_or("the AVIF grid descriptor is malformed")?;
	let tiles = tiles_of(&meta, meta.primary)?;
	if tiles.len() != (grid.rows * grid.columns) as usize {
		return Err(format!("the AVIF grid is {}x{} but has {} tiles", grid.columns, grid.rows, tiles.len()));
	}

	// An alpha plane is an auxiliary item pointing at the grid; for a grid image it is
	// itself a grid of the same layout, tile for tile.
	let alpha_tiles = alpha_of(&meta, meta.primary).map(|alpha| if meta.types.get(&alpha) == Some(b"grid") { tiles_of(&meta, alpha) } else { Err("the AVIF grid's alpha is not a grid".to_owned()) }).transpose()?;
	if alpha_tiles.as_ref().is_some_and(|alpha| alpha.len() != tiles.len()) {
		return Err("the AVIF grid's alpha has a different number of tiles".to_owned());
	}

	let (width, height) = (grid.width as usize, grid.height as usize);
	// The canvas is allocated only once the first tile has shown the tile size, and only
	// if the tiles fit the grid's declared output the way MIAF requires: covering it, and
	// overhanging it by less than one tile. A damaged or hostile descriptor claiming a huge
	// output over a few small tiles is refused rather than allocated.
	let mut canvas: Vec<u8> = Vec::new();
	let mut tile_size = None;
	for (index, &tile) in tiles.iter().enumerate() {
		let alpha = alpha_tiles.as_ref().map(|alpha| alpha[index]);
		let file = single_image(&meta, tile, alpha).ok_or("could not rewrap an AVIF grid tile")?;
		let (tile_w, tile_h, pixels) = decode_tile(&file)?;
		match tile_size {
			None => {
				if !fits(tile_w, grid.columns, grid.width) || !fits(tile_h, grid.rows, grid.height) {
					return Err(format!("the AVIF grid's {tile_w}x{tile_h} tiles do not fit its {}x{} output", grid.width, grid.height));
				}
				canvas = vec![0_u8; width.checked_mul(height).and_then(|p| p.checked_mul(4)).ok_or("the AVIF grid is too large")?];
				tile_size = Some((tile_w, tile_h));
			}
			// Every tile is the same size.
			Some(size) if size != (tile_w, tile_h) => return Err("the AVIF grid's tiles differ in size".to_owned()),
			Some(_) => {}
		}
		let (column, row) = (index % grid.columns as usize, index / grid.columns as usize);
		let (left, top) = (column * tile_w as usize, row * tile_h as usize);
		// `fits` guarantees every tile starts inside the output; edge tiles are cropped.
		let visible = (tile_w as usize).min(width - left);
		for y in 0..(tile_h as usize).min(height - top) {
			let from = &pixels[y * tile_w as usize * 4..][..visible * 4];
			canvas[((top + y) * width + left) * 4..][..visible * 4].copy_from_slice(from);
		}
	}
	Ok((grid.width, grid.height, canvas))
}

/// Whether `count` tiles of `tile` pixels lay out an `output`-pixel dimension as MIAF
/// requires: covering it, with the last tile starting inside it.
fn fits(tile: u32, count: u32, output: u32) -> bool {
	let (Some(all), Some(but_last)) = (u64::from(tile).checked_mul(u64::from(count)), u64::from(tile).checked_mul(u64::from(count - 1))) else { return false };
	tile > 0 && output > 0 && all >= u64::from(output) && but_last < u64::from(output)
}

/// The tiles a grid item references (`dimg`), in order.
fn tiles_of(meta: &Meta<'_>, grid: u32) -> Result<Vec<u32>, String> {
	meta.references.iter().find(|(kind, from, _)| kind == b"dimg" && *from == grid).map(|(_, _, to)| to.clone()).ok_or_else(|| "the AVIF grid references no tiles".to_owned())
}

/// The auxiliary (`auxl`) item that is `item`'s alpha, if any. (Other auxiliary images,
/// such as depth maps, are not alpha; the tile rewrap copies their `auxC`, and the decoder
/// then ignores anything that is not alpha.)
fn alpha_of(meta: &Meta<'_>, item: u32) -> Option<u32> {
	meta.references.iter().find(|(kind, _, to)| kind == b"auxl" && to.contains(&item)).map(|(_, from, _)| *from)
}

/// Build a minimal AVIF file holding `tile` as its primary item, with `alpha` as its alpha
/// auxiliary item if given — each with its own data and property boxes, copied verbatim.
fn single_image(meta: &Meta<'_>, tile: u32, alpha: Option<u32>) -> Option<Vec<u8>> {
	let items: Vec<u32> = std::iter::once(tile).chain(alpha).collect();
	let mut ipco = Vec::new();
	let mut ipma_entries = Vec::new();
	let mut count = 0_u8;
	for (new_id, &item) in (1_u16..).zip(&items) {
		let mut associations = Vec::new();
		for &(essential, index) in meta.associations.get(&item).map_or(&[][..], Vec::as_slice) {
			let property = meta.properties.get(usize::from(index).checked_sub(1)?)?;
			ipco.extend_from_slice(property);
			count = count.checked_add(1)?;
			associations.push(if essential { 0x80 | count } else { count });
		}
		ipma_entries.push((new_id, associations));
	}
	if count > 0x7f {
		return None;
	}

	let full_box = |kind: &[u8; 4], version: u8, body: &[u8]| -> Vec<u8> {
		let mut out = u32::try_from(12 + body.len()).unwrap_or(u32::MAX).to_be_bytes().to_vec();
		out.extend_from_slice(kind);
		out.extend_from_slice(&[version, 0, 0, 0]);
		out.extend_from_slice(body);
		out
	};
	let plain_box = |kind: &[u8; 4], body: &[u8]| -> Vec<u8> {
		let mut out = u32::try_from(8 + body.len()).unwrap_or(u32::MAX).to_be_bytes().to_vec();
		out.extend_from_slice(kind);
		out.extend_from_slice(body);
		out
	};

	let payloads: Vec<&[u8]> = items.iter().map(|item| meta.data.get(item).map(Vec::as_slice)).collect::<Option<_>>()?;
	let ftyp = plain_box(b"ftyp", b"avif\0\0\0\0avifmif1miaf");
	let build_meta = |offsets: &[u32]| -> Vec<u8> {
		let mut body = full_box(b"hdlr", 0, b"\0\0\0\0pict\0\0\0\0\0\0\0\0\0\0\0\0\0");
		body.extend(full_box(b"pitm", 0, &1_u16.to_be_bytes()));
		let mut iloc = vec![0x44, 0x00];
		iloc.extend_from_slice(&u16::try_from(items.len()).unwrap_or(0).to_be_bytes());
		for ((new_id, payload), offset) in (1_u16..).zip(&payloads).zip(offsets) {
			iloc.extend_from_slice(&new_id.to_be_bytes());
			iloc.extend_from_slice(&0_u16.to_be_bytes()); // data reference index
			iloc.extend_from_slice(&1_u16.to_be_bytes()); // one extent
			iloc.extend_from_slice(&offset.to_be_bytes());
			iloc.extend_from_slice(&u32::try_from(payload.len()).unwrap_or(0).to_be_bytes());
		}
		body.extend(full_box(b"iloc", 0, &iloc));
		let mut iinf = u16::try_from(items.len()).unwrap_or(0).to_be_bytes().to_vec();
		for new_id in (1_u16..).take(items.len()) {
			let mut infe = new_id.to_be_bytes().to_vec();
			infe.extend_from_slice(&[0, 0]);
			infe.extend_from_slice(b"av01\0");
			iinf.extend(full_box(b"infe", 2, &infe));
		}
		body.extend(full_box(b"iinf", 0, &iinf));
		if items.len() == 2 {
			body.extend(full_box(b"iref", 0, &plain_box(b"auxl", &[0, 2, 0, 1, 0, 1])));
		}
		let mut ipma = u32::try_from(ipma_entries.len()).unwrap_or(0).to_be_bytes().to_vec();
		for (new_id, associations) in &ipma_entries {
			ipma.extend_from_slice(&new_id.to_be_bytes());
			ipma.push(u8::try_from(associations.len()).unwrap_or(0));
			ipma.extend_from_slice(associations);
		}
		let mut iprp = plain_box(b"ipco", &ipco);
		iprp.extend(full_box(b"ipma", 0, &ipma));
		body.extend(plain_box(b"iprp", &iprp));
		full_box(b"meta", 0, &body)
	};

	// The meta box's size does not depend on the offsets' values, so build it once to learn
	// where the data will start, then again with the real offsets.
	let meta_len = build_meta(&vec![0; items.len()]).len();
	let mut offset = ftyp.len() + meta_len + 8;
	let mut offsets = Vec::new();
	for payload in &payloads {
		offsets.push(u32::try_from(offset).ok()?);
		offset += payload.len();
	}
	let mut file = ftyp;
	file.extend(build_meta(&offsets));
	file.extend(plain_box(b"mdat", &payloads.concat()));
	Some(file)
}

#[cfg(test)]
mod tests {
	use super::fits;

	/// MIAF's layout rule: the tiles cover the output, and none starts past its edge.
	#[test]
	fn tiles_must_cover_the_output_by_less_than_a_tile() {
		assert!(fits(128, 2, 256), "exact");
		assert!(fits(128, 2, 200), "the last tile overhangs and is cropped");
		assert!(!fits(128, 2, 257), "a gap at the edge");
		assert!(!fits(128, 3, 256), "a whole tile past the edge");
		assert!(!fits(64, 1, 60_000), "a huge output over one small tile is refused before any allocation");
		assert!(!fits(0, 2, 10));
		assert!(fits(u32::MAX, 1, u32::MAX));
	}

	/// Anything that is not a readable grid is simply not a grid — never a panic.
	#[test]
	fn junk_is_not_a_grid() {
		assert!(!super::is_grid(b""));
		assert!(!super::is_grid(b"\0\0\0\x1cftypavif\0\0\0\0avifmif1miaf"));
		assert!(!super::is_grid(&[0xff; 64]));
		assert!(super::decode(b"\0\0\0\x08meta", &|_| Err(String::new())).is_err());
	}
}
