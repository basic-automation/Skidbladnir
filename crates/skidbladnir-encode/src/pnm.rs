//! PNM input: binary graymaps (`P5`), pixmaps (`P6`) and PAM (`P7`), read the way libwebp
//! 1.6.0's `imageio/pnmdec.c` reads them for `cwebp`.
//!
//! `cwebp` has its own PNM reader rather than an image library, and it is particular: the
//! header is read **a line at a time** (the magic number, then width and height, then the
//! maximum value, each on its own line; a PAM's fields one per line), a line that starts
//! with `#` is a comment, and a sample is rescaled to 8 bits as
//! `(v * 255 + maxval / 2) / maxval`, clamped. This module follows those rules exactly, so
//! that a file `cwebp` reads gives libwebp the same pixels on both sides. The ASCII and
//! bitmap forms (`P1` to `P4`) are refused, as `cwebp` refuses them.
//!
//! `cjxl` reads PNM too, with a reader of its own that keeps the samples at their own
//! depth; [`Header`] and [`raw_samples`] are what `jxl.rs` builds that input from.
//!
//! `cjxl` alone reads **PFM**, the floating-point form (`PF` colour, `Pf` gray), which
//! [`pfm`] reads its way: samples nominally 0 to 1 in sRGB, rows stored bottom to top.

/// A PNM header as `cwebp` reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
	/// 5, 6 or 7.
	pub kind: u8,
	/// Width in pixels.
	pub width: u32,
	/// Height in pixels.
	pub height: u32,
	/// Samples per pixel: 1 gray, 2 gray and alpha, 3 RGB, 4 RGBA.
	pub depth: u8,
	/// The largest sample value, 1 to 65535. Above 255 a sample takes two bytes, big-endian.
	pub max_value: u32,
	/// Where the raster starts.
	pub offset: usize,
}

impl Header {
	/// Bytes per sample.
	#[must_use]
	pub const fn sample_size(&self) -> usize {
		if self.max_value > 255 { 2 } else { 1 }
	}

	/// Whether the image is gray, with or without alpha.
	#[must_use]
	pub const fn gray(&self) -> bool {
		self.depth <= 2
	}

	/// Whether the image has an alpha channel.
	#[must_use]
	pub const fn alpha(&self) -> bool {
		self.depth.is_multiple_of(2)
	}
}

/// `pnmdec.c`'s `MAX_LINE_SIZE`: a longer line is read in pieces.
const MAX_LINE: usize = 1024;

/// `ReadLine`: the next line from `off`, without its `\n`, skipping blank lines and lines
/// starting with `#` — except the last line of the data, which is returned as it is. The
/// line is a C string to `cwebp`, so it ends at a NUL byte.
fn read_line(data: &[u8], mut off: usize) -> (usize, &[u8]) {
	loop {
		let start = off;
		let mut length = 0;
		while length < MAX_LINE && off < data.len() {
			let byte = data[off];
			off += 1;
			if byte == b'\n' {
				break;
			}
			length += 1;
		}
		if off < data.len() && (length == 0 || data[start] == b'#') {
			continue;
		}
		let line = &data[start..start + length];
		let line = line.iter().position(|&b| b == 0).map_or(line, |nul| &line[..nul]);
		return (off, line);
	}
}

/// C's `isspace` in the C locale.
const fn is_c_space(byte: u8) -> bool {
	matches!(byte, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `sscanf`'s `%d`: optional leading white space, an optional sign, at least one digit.
/// Returns the value and the rest of the input. A value that does not fit an `i32` is
/// refused (in C it is undefined).
fn scan_int(input: &[u8]) -> Option<(i64, &[u8])> {
	let start = input.iter().position(|&b| !is_c_space(b)).unwrap_or(input.len());
	let mut rest = &input[start..];
	let negative = match rest.first() {
		Some(b'-') => {
			rest = &rest[1..];
			true
		}
		Some(b'+') => {
			rest = &rest[1..];
			false
		}
		_ => false,
	};
	let digits = rest.iter().take_while(|b| b.is_ascii_digit()).count();
	if digits == 0 {
		return None;
	}
	let mut value: i64 = 0;
	for &digit in &rest[..digits] {
		value = value.checked_mul(10)?.checked_add(i64::from(digit - b'0'))?;
		if value > i64::from(i32::MAX) + 1 {
			return None;
		}
	}
	let value = if negative { -value } else { value };
	i32::try_from(value).ok().map(|_| (value, &rest[digits..]))
}

/// `sscanf(line, "<key> %d")`: the key matched exactly, then an integer.
fn scan_field(line: &[u8], key: &[u8]) -> Option<i64> {
	line.strip_prefix(key).and_then(scan_int).map(|(value, _)| value)
}

/// The numeric PAM fields, in the order `cwebp` tries them.
const PAM_FIELDS: [&[u8]; 4] = [b"WIDTH", b"HEIGHT", b"DEPTH", b"MAXVAL"];

/// `ReadPAMFields`: the lines between `P7` and `ENDHDR`.
fn pam_fields(data: &[u8], mut off: usize) -> Result<(usize, [i64; 4]), String> {
	// Width, height, depth, maximum value, each read once.
	let mut fields: [Option<i64>; 4] = [None; 4];
	let mut expected_depth = None;
	loop {
		let (next, line) = read_line(data, off);
		off = next;
		let field = PAM_FIELDS.iter().enumerate().find_map(|(index, key)| scan_field(line, key).map(|value| (index, key, value)));
		if let Some((index, key, value)) = field {
			if fields[index].replace(value).is_some() {
				return Err(format!("the PAM header has {} twice", String::from_utf8_lossy(key)));
			}
			continue;
		}
		match line {
			b"TUPLTYPE RGB_ALPHA" => expected_depth = Some(4),
			b"TUPLTYPE RGB" => expected_depth = Some(3),
			b"TUPLTYPE GRAYSCALE_ALPHA" => expected_depth = Some(2),
			b"TUPLTYPE GRAYSCALE" => expected_depth = Some(1),
			b"ENDHDR" => break,
			_ => {
				let shown: String = String::from_utf8_lossy(&line[..line.len().min(16)]).chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
				return Err(format!("the PAM header has an entry cwebp does not read: \"{shown}\""));
			}
		}
	}
	let [width, height, depth, max_value] = fields.map(|field| field.unwrap_or(0));
	if let Some(expected) = expected_depth
		&& depth != expected
	{
		return Err(format!("the PAM header's TUPLTYPE has {expected} channels but its DEPTH is {depth}"));
	}
	Ok((off, [width, height, depth, max_value]))
}

/// `ReadHeader`, then `ReadPNM`'s checks on the type and the size of the raster.
///
/// # Errors
///
/// A message saying what is wrong with the file, when `cwebp` would refuse it.
pub fn header(data: &[u8]) -> Result<Header, String> {
	if data.len() < 3 {
		return Err("the file is too short to be a PNM image".to_owned());
	}
	let (off, line) = read_line(data, 0);
	let kind = line.strip_prefix(b"P").and_then(scan_int).map(|(kind, _)| kind).ok_or("the file does not start with a PNM magic number")?;
	let (off, [width, height, depth, max_value]) = if kind == 7 {
		pam_fields(data, off)?
	} else {
		let (off, line) = read_line(data, off);
		let (width, rest) = scan_int(line).ok_or("the PNM header has no width")?;
		let (height, _) = scan_int(rest).ok_or("the PNM header has no height")?;
		let (off, line) = read_line(data, off);
		let (max_value, _) = scan_int(line).ok_or("the PNM header has no maximum value")?;
		(off, [width, height, if kind == 5 { 1 } else { 3 }, max_value])
	};
	if !(5..=7).contains(&kind) {
		return Err(format!("P{kind} PNM files are not supported (cwebp reads only binary P5, P6 and P7)"));
	}
	let (Ok(width @ 1..), Ok(height @ 1..), Ok(depth @ 1..=4), Ok(max_value @ 1..=65_535)) = (u32::try_from(width), u32::try_from(height), u8::try_from(depth), u32::try_from(max_value)) else {
		return Err(format!("the PNM header is out of range: {width}x{height}, depth {depth}, maximum value {max_value}"));
	};
	let header = Header { kind: u8::try_from(kind).unwrap_or_default(), width, height, depth, max_value, offset: off };
	let raster = usize::try_from(width).ok().and_then(|w| w.checked_mul(usize::try_from(height).ok()?)).and_then(|pixels| pixels.checked_mul(usize::from(depth) * header.sample_size()));
	if raster.and_then(|raster| raster.checked_add(off)).is_none_or(|end| end > data.len()) {
		return Err(format!("the P{kind} file is truncated"));
	}
	Ok(header)
}

/// The samples as stored, at their own depth: what `cjxl` reads.
#[must_use]
pub fn raw_samples(data: &[u8], header: &Header) -> Vec<u16> {
	let count = header.width as usize * header.height as usize * usize::from(header.depth);
	let raster = &data[header.offset..];
	if header.sample_size() == 2 { raster.as_chunks::<2>().0.iter().take(count).map(|&pair| u16::from_be_bytes(pair)).collect() } else { raster.iter().take(count).map(|&b| u16::from(b)).collect() }
}

/// A decoded PNM image.
pub struct Decoded {
	/// The header it was read with.
	pub header: Header,
	/// 8-bit RGBA, rescaled as `cwebp` rescales; opaque where the file has no alpha.
	pub pixels: Vec<u8>,
	/// 16-bit RGBA, for a file with more than 8 bits per sample: each sample rescaled to
	/// the full 16-bit range, `(v * 65535 + maxval / 2) / maxval`.
	pub deep: Option<Vec<u16>>,
}

/// Decode a PNM file to RGBA the way `cwebp` does.
///
/// # Errors
///
/// As [`header`].
pub fn decode(data: &[u8]) -> Result<Decoded, String> {
	let header = header(data)?;
	let samples = raw_samples(data, &header);
	let max = header.max_value;
	let round = max / 2;
	// `pnmdec.c`'s rescale. A sample above the maximum is clamped.
	let narrow = |v: u16| -> u8 {
		let v = u32::from(v);
		u8::try_from(if max == 255 { v } else { (v * 255 + round) / max }).unwrap_or(u8::MAX)
	};
	let widen = |v: u16| -> u16 { u16::try_from((u32::from(v) * 65_535 + round) / max).unwrap_or(u16::MAX) };
	let to_rgba = |pixel: &[u16], channel: &dyn Fn(u16) -> u16| -> [u16; 4] {
		match *pixel {
			[gray] => [channel(gray), channel(gray), channel(gray), u16::MAX],
			[gray, alpha] => [channel(gray), channel(gray), channel(gray), channel(alpha)],
			[r, g, b] => [channel(r), channel(g), channel(b), u16::MAX],
			[r, g, b, a] => [channel(r), channel(g), channel(b), channel(a)],
			_ => [0; 4],
		}
	};
	let pixels = samples.chunks_exact(usize::from(header.depth)).flat_map(|pixel| to_rgba(pixel, &|v| u16::from(narrow(v))).map(|v| u8::try_from(v).unwrap_or(u8::MAX))).collect();
	let deep = (max > 255).then(|| samples.chunks_exact(usize::from(header.depth)).flat_map(|pixel| to_rgba(pixel, &widen)).collect());
	Ok(Decoded { header, pixels, deep })
}

/// A PFM image as `cjxl` reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct Pfm {
	/// Width in pixels.
	pub width: u32,
	/// Height in pixels.
	pub height: u32,
	/// One channel (`Pf`) rather than three (`PF`).
	pub gray: bool,
	/// The samples, top row first, one or three per pixel.
	pub samples: Vec<f32>,
}

/// libjxl's `ParseUnsigned`: at least one digit, no sign, no overflow.
fn parse_unsigned(data: &[u8], at: &mut usize) -> Option<u64> {
	let digits = data.get(*at..)?.iter().take_while(|b| b.is_ascii_digit()).count();
	if digits == 0 {
		return None;
	}
	let mut value: u64 = 0;
	for &digit in &data[*at..*at + digits] {
		value = value.checked_mul(10)?.checked_add(u64::from(digit - b'0'))?;
	}
	*at += digits;
	Some(value)
}

/// libjxl's `ParseSigned`: an optional sign, digits, and decimals, any of which may be
/// absent after the first character.
fn parse_signed(data: &[u8], at: &mut usize) -> Option<f64> {
	let negative = data.get(*at) == Some(&b'-');
	match data.get(*at)? {
		b'-' | b'+' => {
			*at += 1;
			if *at >= data.len() {
				return None;
			}
		}
		digit if digit.is_ascii_digit() => {}
		_ => return None,
	}
	let mut value = 0.0_f64;
	while let Some(digit) = data.get(*at).filter(|b| b.is_ascii_digit()) {
		value = value * 10.0 + f64::from(digit - b'0');
		*at += 1;
	}
	if data.get(*at) == Some(&b'.') {
		*at += 1;
		let mut place = 0.1;
		while let Some(digit) = data.get(*at).filter(|b| b.is_ascii_digit()) {
			value += f64::from(digit - b'0') * place;
			place *= 0.1;
			*at += 1;
		}
	}
	Some(if negative { -value } else { value })
}

/// Read a PFM file as libjxl 0.12's `lib/extras/dec/pnm.cc` does: `PF` or `Pf`, one white
/// space, the width, a space or newline, the height, one white space, the scale, one white
/// space, then 32-bit floats. The scale's sign is the byte order (negative: little-endian);
/// its size is ignored, as `cjxl` ignores it. The rows are stored bottom to top.
///
/// # Errors
///
/// A message saying what is wrong with the file.
pub fn pfm(data: &[u8]) -> Result<Pfm, String> {
	let malformed = || "the PFM header is malformed".to_owned();
	let gray = match data {
		[b'P', b'F', ..] => false,
		[b'P', b'f', ..] => true,
		_ => return Err("the file does not start with a PFM magic number".to_owned()),
	};
	let mut at = 2;
	let single_space = |at: &mut usize, allowed: &[u8]| -> Result<(), String> {
		if data.get(*at).is_some_and(|b| allowed.contains(b)) {
			*at += 1;
			Ok(())
		} else {
			Err(malformed())
		}
	};
	let whitespace = b" \t\r\n";
	single_space(&mut at, whitespace)?;
	let width = parse_unsigned(data, &mut at).ok_or_else(malformed)?;
	single_space(&mut at, b" \n")?;
	let height = parse_unsigned(data, &mut at).ok_or_else(malformed)?;
	single_space(&mut at, whitespace)?;
	let scale = parse_signed(data, &mut at).ok_or_else(malformed)?;
	if scale == 0.0 {
		return Err("the PFM scale is zero".to_owned());
	}
	single_space(&mut at, whitespace)?;
	let (Ok(width @ 1..), Ok(height @ 1..)) = (u32::try_from(width), u32::try_from(height)) else {
		return Err(format!("the PFM size is out of range: {width}x{height}"));
	};
	let channels = if gray { 1 } else { 3 };
	let row = usize::try_from(width).ok().and_then(|w| w.checked_mul(channels * 4)).ok_or_else(malformed)?;
	let size = row.checked_mul(usize::try_from(height).map_err(|_| malformed())?).ok_or_else(malformed)?;
	let raster = data.get(at..).filter(|raster| raster.len() >= size).ok_or("the PFM file is truncated")?;
	let read = |bytes: &[u8; 4]| if scale > 0.0 { f32::from_be_bytes(*bytes) } else { f32::from_le_bytes(*bytes) };
	let samples = raster[..size].chunks_exact(row).rev().flat_map(|line| line.as_chunks::<4>().0.iter().map(read)).collect();
	Ok(Pfm { width, height, gray, samples })
}

/// A float sample nominally 0 to 1 as 8 and 16 bits, clamped; NaN reads as 0.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 0..=1 first, so the results are in range")]
fn quantise(sample: f32) -> (u8, u16) {
	let unit = if sample.is_nan() { 0.0 } else { sample.clamp(0.0, 1.0) };
	((unit * 255.0).round() as u8, (unit * 65_535.0).round() as u16)
}

/// A PFM image as RGBA for the encoders that take integers: 8-bit and 16-bit, clamped to
/// the nominal 0 to 1. No reference tool but `cjxl` reads PFM, so there is nothing else to
/// match; `jxl.rs` hands `cjxl`'s floats to libjxl as they are.
#[must_use]
pub fn pfm_rgba(image: &Pfm) -> (Vec<u8>, Vec<u16>) {
	let mut narrow = Vec::with_capacity(image.samples.len() * 4);
	let mut wide = Vec::with_capacity(image.samples.len() * 4);
	let channels = if image.gray { 1 } else { 3 };
	for pixel in image.samples.chunks_exact(channels) {
		let rgb: [f32; 3] = if image.gray { [pixel[0]; 3] } else { [pixel[0], pixel[1], pixel[2]] };
		for sample in rgb {
			let (n, w) = quantise(sample);
			narrow.push(n);
			wide.push(w);
		}
		narrow.push(u8::MAX);
		wide.push(u16::MAX);
	}
	(narrow, wide)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn reads_each_kind() {
		let gray = decode(b"P5\n2 1\n255\n\x00\xff").expect("P5");
		assert_eq!((gray.header.width, gray.header.height, gray.header.depth), (2, 1, 1));
		assert_eq!(gray.pixels, [0, 0, 0, 255, 255, 255, 255, 255]);
		let rgb = decode(b"P6\n1 1\n255\n\x01\x02\x03").expect("P6");
		assert_eq!(rgb.pixels, [1, 2, 3, 255]);
		let pam = decode(b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 2\nMAXVAL 255\nTUPLTYPE GRAYSCALE_ALPHA\nENDHDR\n\x10\x80").expect("P7");
		assert_eq!(pam.pixels, [16, 16, 16, 128]);
		assert!(pam.header.gray() && pam.header.alpha());
		assert!(pam.deep.is_none());
	}

	/// Comments and blank lines between header lines are skipped; a comment is a whole
	/// line, as `cwebp` reads one.
	#[test]
	fn skips_comment_lines() {
		let image = decode(b"P6\n# made by hand\n\n1 1\n# max\n255\n\x07\x08\x09").expect("decodes");
		assert_eq!(image.pixels, [7, 8, 9, 255]);
	}

	/// `cwebp`'s rescale: rounding to nearest, `(v * 255 + maxval / 2) / maxval`, from big-endian
	/// pairs above 255, and clamped where a sample exceeds the maximum.
	#[test]
	fn rescales_as_cwebp_does() {
		let image = decode(b"P5\n4 1\n1023\n\x00\x00\x00\x04\x03\xff\x04\x00").expect("decodes");
		assert_eq!(image.pixels.as_chunks::<4>().0.iter().map(|p| p[0]).collect::<Vec<_>>(), [0, 1, 255, 255]);
		assert_eq!(image.deep.expect("deep").as_chunks::<4>().0.iter().map(|p| p[0]).collect::<Vec<_>>(), [0, 256, 65_535, 65_535]);
		let image = decode(b"P5\n2 1\n15\n\x07\x08").expect("decodes");
		assert_eq!(image.pixels.as_chunks::<4>().0.iter().map(|p| p[0]).collect::<Vec<_>>(), [119, 136]);
	}

	#[test]
	fn refuses_what_cwebp_refuses() {
		for (bytes, why) in [(&b"P3\n1 1\n255\n1 2 3\n"[..], "ASCII pixmap"), (b"P4\n8 1\n\xff", "bitmap"), (b"P6\n2 2\n255\n\x01\x02\x03", "truncated"), (b"P6\n0 2\n255\n", "zero width"), (b"P5\n1 1\n65536\n\x00\x00", "maximum out of range"), (b"P7\nWIDTH 1\nWIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\nENDHDR\n\x00", "a field twice"), (b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 3\nMAXVAL 255\nTUPLTYPE GRAYSCALE\nENDHDR\n\x00\x00\x00", "tuple type against depth"), (b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\nTUPLTYPE BLACKANDWHITE\nENDHDR\n\x00", "a tuple type cwebp does not know"), (b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\n", "no ENDHDR"), (b"P6 1 1 255\n\x01\x02\x03", "the header on one line"), (b"P5\n99999999999 1\n255\n", "a width past i32"), (b"", "empty")] {
			assert!(decode(bytes).is_err(), "{why} must be refused");
		}
	}

	#[test]
	fn reads_pfm_bottom_up_in_either_byte_order() {
		let big = [b"PF\n1 2\n1.0\n".as_slice(), &[0.25_f32, 0.5, 0.75, 1.0, 2.0, -1.0].iter().flat_map(|v| v.to_be_bytes()).collect::<Vec<_>>()].concat();
		let image = pfm(&big).expect("big-endian");
		assert_eq!((image.width, image.height, image.gray), (1, 2, false));
		assert_eq!(image.samples, [1.0, 2.0, -1.0, 0.25, 0.5, 0.75], "the last row stored is the top row");
		let little = [b"Pf 2 1 -1\n".as_slice(), &[0.5_f32, f32::NAN].iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>()].concat();
		let image = pfm(&little).expect("little-endian");
		assert!(image.gray && image.samples[0].to_bits() == 0.5_f32.to_bits() && image.samples[1].is_nan());
		assert_eq!(pfm_rgba(&image).0, [128, 128, 128, 255, 0, 0, 0, 255]);
		for bad in [&b"PF\n1 1\n0\n\0\0\0\0"[..], b"PF\n1 1\n1\n\0\0", b"PF1 1\n1\n", b"PF\n0 1\n1\n", b"PF\n1\t1\n1\n\0\0\0\0\0\0\0\0\0\0\0\0"] {
			assert!(pfm(bad).is_err(), "{:?} must be refused", String::from_utf8_lossy(bad));
		}
	}

	/// `cwebp`'s `ReadLine` gives up on skipping at the end of the data, and reads at most
	/// 1024 bytes as one line; neither may panic or loop.
	#[test]
	fn odd_lines_do_not_panic() {
		let long = [b"P5\n".as_slice(), &[b'#'; 3000], b"\n1 1\n255\n\x00"].concat();
		let _ = decode(&long);
		let _ = decode(b"P7\n#");
		let _ = decode(b"P5\n1\x00 1\n255\n\x00");
		assert!(scan_int(b"  -12x").is_some_and(|(v, rest)| v == -12 && rest == b"x"));
		assert!(scan_int(b"x").is_none());
	}
}
