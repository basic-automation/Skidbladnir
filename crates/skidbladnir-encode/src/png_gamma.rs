//! The gamma correction `cwebp` asks libpng for.
//!
//! `cwebp`'s PNG reader (`imageio/pngdec.c`) calls `png_set_gamma(png, 2.2, file_gamma)`
//! when the PNG has an `sRGB` chunk (with `file_gamma` 1/2.2) or a `gAMA` chunk (with its
//! value), so a PNG whose gamma is not close to 1/2.2 reaches the encoder with its colour
//! samples corrected to a 2.2 display. libpng 1.6.58's arithmetic is reproduced here: the
//! fixed-point conversions, the 5% threshold, the 8-bit table, and for 16-bit samples the
//! table that corrects and reduces to 8 bits at once, sized by the `sBIT` chunk. Alpha is
//! never corrected.
//!
//! Only `cwebp` does this. `avifenc` and `cjxl` signal the gamma instead, and `heif-enc`
//! ignores it.

use crate::{
	metadata::png_chunks, source::{SourceFormat, SourceImage}
};

/// libpng's fixed-point one.
const FP_1: f64 = 100_000.0;
/// `PNG_GAMMA_THRESHOLD_FIXED`.
const THRESHOLD: i64 = 5_000;
/// `PNG_MAX_GAMMA_8`: the significant input bits kept when the output is 8-bit.
const MAX_GAMMA_8: u32 = 11;

/// `png_gamma_significant`.
fn significant(gamma: i64) -> bool {
	!(100_000 - THRESHOLD..=100_000 + THRESHOLD).contains(&gamma)
}

/// `floor(x + .5)`, the rounding every libpng fixed-point helper uses.
#[expect(clippy::cast_possible_truncation, reason = "libpng checks the range and so does the caller: gamma values are small")]
fn round(x: f64) -> i64 {
	(x + 0.5).floor() as i64
}

/// `png_gamma_8bit_correct`.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "the result is in 0..=255 by construction")]
fn correct_8(value: u32, gamma: i64) -> u8 {
	if value > 0 && value < 255 {
		#[expect(clippy::cast_precision_loss, reason = "gamma is a small fixed-point value")]
		let r = (255.0 * (f64::from(value) / 255.0).powf(gamma as f64 * 0.00001) + 0.5).floor();
		r as u8
	} else {
		u8::try_from(value & 0xff).unwrap_or(u8::MAX)
	}
}

/// `png_gamma_16bit_correct`.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "the result is in 0..=65535 by construction")]
fn correct_16(value: u32, gamma: i64) -> u32 {
	if value > 0 && value < 65_535 {
		#[expect(clippy::cast_precision_loss, reason = "gamma is a small fixed-point value")]
		let r = (65_535.0 * (f64::from(value) / 65_535.0).powf(gamma as f64 * 0.00001) + 0.5).floor();
		r as u32
	} else {
		value
	}
}

/// `png_build_16to8_table`: `table[low bits >> shift][high byte]`, whose high byte is the
/// corrected 8-bit value.
fn table_16_to_8(shift: u32, gamma: i64) -> Vec<[u16; 256]> {
	let num = 1_u32 << (8 - shift);
	let max = (1_u32 << (16 - shift)) - 1;
	let mut table = vec![[0_u16; 256]; num as usize];
	let mut set = |at: u32, out: u16| table[(at & (0xff >> shift)) as usize][(at >> (8 - shift)) as usize] = out;
	let mut last = 0_u32;
	for i in 0..255_u32 {
		let out = u16::try_from(i * 257).unwrap_or(u16::MAX);
		let bound = (correct_16(u32::from(out) + 128, gamma) * max + 32_768) / 65_535 + 1;
		while last < bound {
			set(last, out);
			last += 1;
		}
	}
	while last < (num << 8) {
		set(last, u16::MAX);
		last += 1;
	}
	table
}

/// The first chunk of a kind before `PLTE` and `IDAT` that libpng would accept.
fn chunk_before_palette<'a>(chunks: &[([u8; 4], &'a [u8])], kind: [u8; 4], valid: impl Fn(&[u8]) -> bool) -> Option<&'a [u8]> {
	chunks.iter().take_while(|(k, _)| k != b"PLTE" && k != b"IDAT").find(|(k, data)| *k == kind && valid(data)).map(|(_, data)| *data)
}

/// The RGBA pixels `cwebp` would encode for this PNG, when they differ from the decoded
/// ones: `None` when there is no gamma to correct.
pub(crate) fn like_cwebp(source: &SourceImage) -> Option<Vec<u8>> {
	if source.format != SourceFormat::Png || source.bytes.is_empty() {
		return None;
	}
	let chunks = png_chunks(&source.bytes)?;
	let ihdr = chunks.first().filter(|(kind, _)| kind == b"IHDR").map(|(_, data)| *data)?;
	let (bit_depth, colour_type) = (*ihdr.get(8)?, *ihdr.get(9)?);
	// An `sRGB` chunk means 1/2.2, which is no correction at all.
	if chunk_before_palette(&chunks, *b"sRGB", |data| data.len() == 1 && data[0] <= 3).is_some() {
		return None;
	}
	let chunk = chunk_before_palette(&chunks, *b"gAMA", |data| data.len() == 4 && u32::from_be_bytes([data[0], data[1], data[2], data[3]]) <= 0x7fff_ffff)?;
	let chunk_gamma = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);

	// png_get_gAMA hands back a double; png_set_gamma turns both values back into fixed point.
	let convert = |gamma: f64| round(if gamma > 0.0 && gamma < 128.0 { gamma * FP_1 } else { gamma });
	let file = convert(f64::from(chunk_gamma) * 0.00001);
	let screen = convert(2.2);
	// `unsupported_gamma`: png_set_gamma warns and sets nothing, which leaves no correction.
	if !(1_000..=10_000_000).contains(&file) {
		return None;
	}
	// `png_gamma_threshold`.
	#[expect(clippy::cast_precision_loss, reason = "gamma values are far below 2^52")]
	let product = round(file as f64 * screen as f64 / FP_1);
	if !significant(product) {
		return None;
	}
	// `png_reciprocal2(screen, file)`.
	#[expect(clippy::cast_precision_loss, reason = "gamma values are far below 2^52")]
	let correction = round(1e15 / screen as f64 / file as f64);

	let mut pixels = source.pixels.clone();
	if let (Some(deep), 16) = (&source.deep, bit_depth) {
		// The significant bits: the widest colour channel's, or gray's.
		let colour = colour_type & 2 != 0;
		let channels = [1_usize, 0, 3, 1, 2, 0, 4][usize::from(colour_type.min(6))];
		let sig_bit = chunk_before_palette(&chunks, *b"sBIT", |data| data.len() == channels && data.iter().all(|&v| v > 0 && v <= 16)).map_or(0, |data| if colour { data[..3].iter().copied().max().unwrap_or(0) } else { data[0] });
		let mut shift = if (1..16).contains(&sig_bit) { 16 - u32::from(sig_bit) } else { 0 };
		shift = shift.clamp(16 - MAX_GAMMA_8, 8);
		// `png_reciprocal(correction)`.
		#[expect(clippy::cast_precision_loss, reason = "gamma values are far below 2^52")]
		let table = table_16_to_8(shift, round(1e10 / correction as f64));
		for (pixel, wide) in pixels.as_chunks_mut::<4>().0.iter_mut().zip(deep.as_chunks::<4>().0) {
			for c in 0..3 {
				let v = wide[c];
				pixel[c] = u8::try_from(table[usize::from((v & 0xff) >> shift)][usize::from(v >> 8)] >> 8).unwrap_or(u8::MAX);
			}
		}
	} else {
		if !significant(correction) {
			return None;
		}
		let table: Vec<u8> = (0..256).map(|i| correct_8(i, correction)).collect();
		for pixel in pixels.as_chunks_mut::<4>().0 {
			for c in 0..3 {
				pixel[c] = table[usize::from(pixel[c])];
			}
		}
	}
	Some(pixels)
}

#[cfg(test)]
mod tests {
	use super::{correct_8, table_16_to_8};

	#[test]
	fn the_8_bit_table_keeps_its_ends_and_brightens_for_a_1_8_file() {
		// A 1/1.8 file on a 2.2 display: correction 1/(0.55556 * 2.2), about 0.818.
		let correction = 81_818;
		assert_eq!((correct_8(0, correction), correct_8(255, correction)), (0, 255));
		assert!(correct_8(128, correction) > 128);
	}

	#[test]
	fn the_16_to_8_table_is_monotonic_and_spans_the_range() {
		let table = table_16_to_8(5, 122_222);
		let lookup = |v: u16| table[usize::from((v & 0xff) >> 5)][usize::from(v >> 8)] >> 8;
		assert_eq!((lookup(0), lookup(u16::MAX)), (0, 255));
		let mut previous = 0;
		for v in (0..=u16::MAX).step_by(97) {
			let out = lookup(v);
			assert!(out >= previous, "{v} maps to {out}, below {previous}");
			previous = out;
		}
	}
}
