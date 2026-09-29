//! Fixtures the parity tests share: PNGs built chunk by chunk, so every chunk a reference
//! tool reads can be put exactly where it matters, and the ICC profile and Exif block they
//! carry.

use std::io::Write as _;

/// A PNG chunk, CRC included.
pub fn chunk(kind: [u8; 4], data: &[u8]) -> Vec<u8> {
	let mut out = u32::try_from(data.len()).expect("small").to_be_bytes().to_vec();
	out.extend_from_slice(&kind);
	out.extend_from_slice(data);
	let mut crc = flate2::Crc::new();
	crc.update(&kind);
	crc.update(data);
	out.extend_from_slice(&crc.sum().to_be_bytes());
	out
}

pub fn zlib(data: &[u8]) -> Vec<u8> {
	let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
	encoder.write_all(data).expect("compress");
	encoder.finish().expect("compress")
}

/// A PNG from raw rows (without filter bytes), with `extra` chunks before the image data
/// and `trailing` ones after it.
pub fn png(width: u32, height: u32, bit_depth: u8, colour_type: u8, rows: &[u8], extra: &[Vec<u8>], trailing: &[Vec<u8>]) -> Vec<u8> {
	let mut ihdr = width.to_be_bytes().to_vec();
	ihdr.extend_from_slice(&height.to_be_bytes());
	ihdr.extend_from_slice(&[bit_depth, colour_type, 0, 0, 0]);
	let row = rows.len() / height as usize;
	let filtered: Vec<u8> = rows.chunks_exact(row).flat_map(|line| std::iter::once(0).chain(line.iter().copied())).collect();
	let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
	out.extend(chunk(*b"IHDR", &ihdr));
	for extra in extra {
		out.extend_from_slice(extra);
	}
	out.extend(chunk(*b"IDAT", &zlib(&filtered)));
	for trailing in trailing {
		out.extend_from_slice(trailing);
	}
	out.extend(chunk(*b"IEND", &[]));
	out
}

/// Sample `c` of pixel `(x, y)`: ramps and a ripple, so every encoder tool has detail.
pub fn sample(x: u32, y: u32, c: u32, max: u32) -> u32 {
	match c {
		0 => (x * 13 + y * 7) % 97 * max / 97,
		1 => x * max / 63,
		2 => y * max / 47,
		_ => {
			if (x / 8 + y / 8).is_multiple_of(3) {
				0
			} else {
				max - x * max / 127
			}
		}
	}
}

pub const W: u32 = 64;
pub const H: u32 = 48;

/// Rows for a colour type and depth: `channels` samples of `bit_depth` bits each.
pub fn rows(channels: u32, bit_depth: u8) -> Vec<u8> {
	let max = (1_u32 << bit_depth) - 1;
	let mut out = Vec::new();
	for y in 0..H {
		let mut bits: Vec<u8> = Vec::new();
		for x in 0..W {
			for c in 0..channels {
				// Gray is channel 0; gray+alpha takes alpha from channel 3.
				let source = if channels == 2 && c == 1 { 3 } else { c };
				let value = sample(x, y, source, max);
				match bit_depth {
					16 => out.extend_from_slice(&u16::try_from(value).expect("16-bit").to_be_bytes()),
					8 => out.push(u8::try_from(value).expect("8-bit")),
					_ => bits.push(u8::try_from(value).expect("low-bit")),
				}
			}
		}
		if bit_depth < 8 {
			let per_byte = 8 / bit_depth;
			for group in bits.chunks(usize::from(per_byte)) {
				let mut byte = 0_u8;
				for (i, value) in group.iter().enumerate() {
					byte |= value << (8 - bit_depth * (u8::try_from(i).expect("small") + 1));
				}
				out.push(byte);
			}
		}
	}
	out
}

/// A matrix/TRC RGB (or gray TRC) ICC v2 profile that libpng and libjxl both accept.
pub fn icc(gray: bool) -> Vec<u8> {
	fn s15(v: f64) -> [u8; 4] {
		#[expect(clippy::cast_possible_truncation, reason = "test fixture: the values are small and exact enough")]
		let fixed = (v * 65536.0).round() as i32;
		fixed.to_be_bytes()
	}
	let xyz = |x: f64, y: f64, z: f64| [b"XYZ ".as_slice(), &[0; 4], &s15(x), &s15(y), &s15(z)].concat();
	let mut desc = b"desc\0\0\0\0".to_vec();
	desc.extend_from_slice(&5_u32.to_be_bytes());
	desc.extend_from_slice(b"test\0");
	desc.extend_from_slice(&[0; 4 + 4 + 2 + 1 + 67]);
	let curv = [b"curv".as_slice(), &[0; 4], &1_u32.to_be_bytes(), &[0x02, 0x33, 0, 0]].concat();
	let mut tags: Vec<(&[u8; 4], Vec<u8>)> = vec![(b"desc", desc), (b"wtpt", xyz(0.9642, 1.0, 0.8249))];
	if gray {
		tags.push((b"kTRC", curv));
	} else {
		tags.extend([(b"rXYZ", xyz(0.4361, 0.2225, 0.0139)), (b"gXYZ", xyz(0.3851, 0.7169, 0.0971)), (b"bXYZ", xyz(0.1431, 0.0606, 0.7141)), (b"rTRC", curv.clone()), (b"gTRC", curv.clone()), (b"bTRC", curv)]);
	}
	let table = 132 + 12 * tags.len();
	let mut data = Vec::new();
	let mut entries = Vec::new();
	for (sig, bytes) in &tags {
		let offset = table + data.len();
		entries.extend_from_slice(*sig);
		entries.extend_from_slice(&u32::try_from(offset).expect("small").to_be_bytes());
		entries.extend_from_slice(&u32::try_from(bytes.len()).expect("small").to_be_bytes());
		data.extend_from_slice(bytes);
		while data.len() % 4 != 0 {
			data.push(0);
		}
	}
	let total = table + data.len();
	let mut header = vec![0_u8; 128];
	header[0..4].copy_from_slice(&u32::try_from(total).expect("small").to_be_bytes());
	header[8..12].copy_from_slice(&0x0210_0000_u32.to_be_bytes());
	header[12..16].copy_from_slice(b"mntr");
	header[16..20].copy_from_slice(if gray { b"GRAY" } else { b"RGB " });
	header[20..24].copy_from_slice(b"XYZ ");
	header[36..40].copy_from_slice(b"acsp");
	header[68..80].copy_from_slice(&[0, 0, 0xf6, 0xd6, 0, 1, 0, 0, 0, 0, 0xd3, 0x2d]);
	[header, u32::try_from(tags.len()).expect("small").to_be_bytes().to_vec(), entries, data].concat()
}

/// Big-endian TIFF with one IFD0 entry: orientation 6.
pub fn exif() -> Vec<u8> {
	[b"MM\0*".as_slice(), &[0, 0, 0, 8], &[0, 1], &[1, 18, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0], &[0, 0, 0, 0]].concat()
}

/// `ImageMagick`'s raw profile text, lowercase hex, 36 bytes a line, as libjxl requires.
pub fn raw_profile(name: &str, bytes: &[u8]) -> Vec<u8> {
	let mut text = format!("\n{name}\n{:8}", bytes.len()).into_bytes();
	for (i, byte) in bytes.iter().enumerate() {
		if i % 36 == 0 {
			text.push(b'\n');
		}
		text.extend_from_slice(format!("{byte:02x}").as_bytes());
	}
	text.push(b'\n');
	text
}
