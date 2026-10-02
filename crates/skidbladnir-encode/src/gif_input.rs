//! Reading GIF files — still or animated — into an [`Animation`].
//!
//! # Why it follows `gif2webp` rather than a general-purpose decoder
//!
//! GIF-to-WebP already has a reference implementation: libwebp's own `gif2webp`. Matching
//! it exactly is what makes the conversion checkable, so this reproduces its reading of a
//! GIF rule for rule, from its `gif2webp.c` and `gifdec.c` (libwebp 1.6.0):
//!
//! - The canvas starts **fully transparent** (`0x00000000`), and each frame is drawn onto
//!   it: pixels of the frame's transparent index leave the canvas alone, every other pixel
//!   replaces it, opaque.
//! - After a frame is emitted, *dispose to background* clears its rectangle to
//!   transparent — not to the GIF's background colour — and *dispose to previous*
//!   restores the rectangle from before the frame. Anything else keeps it.
//! - A frame shown for **10 ms or less is shown for 100 ms**, as browsers do.
//! - **Loop count** changes meaning between the formats. A GIF's NETSCAPE extension counts
//!   *repeats*, WebP counts *plays*: no extension means play once (WebP 1), `0` means
//!   forever (WebP 0), and `n` means `n + 1` plays. A single-frame GIF has no loop count.
//! - The **background colour** hint is the global palette's background entry, opaque;
//!   transparent if that entry is the first frame's transparent index; white if the GIF
//!   has no such entry.
//!
//! The raw frames come from the `gif` crate with indexed output, and the compositing is
//! done here, so none of `image`'s own compositing choices can leak in.
//!
//! The gate is `tests/gif.rs`, which holds whole conversions to `gif2webp`'s bytes.

use gif::{ColorOutput, DecodeOptions, DisposalMethod, Repeat};

use crate::{
	animation::{Animation, Frame}, metadata::Metadata
};

/// Decode a GIF into full-canvas RGBA frames with `gif2webp`'s timing, loop count and
/// background.
///
/// # Errors
///
/// Returns a description of the failure: a malformed GIF, a frame outside the canvas, a
/// colour index with no palette entry, or no frames at all.
pub fn decode(bytes: &[u8]) -> Result<Animation, String> {
	decode_with_compatible_loop(bytes).map(|(animation, _)| animation)
}

/// [`decode`], and also the loop count `gif2webp -loop_compatibility` gives the file: a
/// GIF's repeat count taken as the number of plays, no loop extension as forever, and still
/// none for a single frame.
///
/// # Errors
///
/// As [`decode`].
pub fn decode_with_compatible_loop(bytes: &[u8]) -> Result<(Animation, u32), String> {
	let mut options = DecodeOptions::new();
	options.set_color_output(ColorOutput::Indexed);
	let mut decoder = options.read_info(bytes).map_err(|error| format!("could not read the GIF: {error}"))?;
	let global = decoder.global_palette().map(<[u8]>::to_vec);
	let background_index = decoder.bg_color();
	let (mut width, mut height) = (usize::from(decoder.width()), usize::from(decoder.height()));

	// ARGB words, as gif2webp's canvases are.
	let mut canvas: Vec<u32> = Vec::new();
	let mut previous: Vec<u32> = Vec::new();
	let mut background = 0xffff_ffff_u32;
	let mut frames = Vec::new();

	while let Some(frame) = decoder.read_next_frame().map_err(|error| format!("could not read the GIF: {error}"))? {
		let (mut left, mut top) = (usize::from(frame.left), usize::from(frame.top));
		if frames.is_empty() {
			// Some broken GIFs report a 0x0 screen; gif2webp takes the first frame's size.
			if width == 0 || height == 0 {
				(left, top, width, height) = (0, 0, usize::from(frame.width), usize::from(frame.height));
				if width == 0 || height == 0 {
					return Err("the GIF has no size".to_owned());
				}
			}
			canvas = vec![0; width * height];
			previous.clone_from(&canvas);
			background = background_colour(global.as_deref(), background_index, frame.transparent);
		}

		// And some have a zero-sized frame, which gif2webp stretches to the screen.
		let (frame_width, frame_height) = if frame.width == 0 || frame.height == 0 { (width, height) } else { (usize::from(frame.width), usize::from(frame.height)) };
		if left + frame_width > width || top + frame_height > height {
			return Err(format!("a GIF frame ({frame_width}x{frame_height} at {left},{top}) lies outside the {width}x{height} canvas"));
		}
		if frame.buffer.len() < frame_width * frame_height {
			return Err("a GIF frame is shorter than its size".to_owned());
		}
		let palette = frame.palette.as_deref().or(global.as_deref()).ok_or("a GIF frame has no palette")?;

		for y in 0..frame_height {
			let row = &frame.buffer[y * frame_width..(y + 1) * frame_width];
			let target = &mut canvas[(top + y) * width + left..][..frame_width];
			for (pixel, &index) in target.iter_mut().zip(row) {
				if frame.transparent == Some(index) {
					continue;
				}
				let entry = palette.get(usize::from(index) * 3..usize::from(index) * 3 + 3).ok_or("a GIF colour index has no palette entry")?;
				*pixel = 0xff00_0000 | u32::from(entry[0]) << 16 | u32::from(entry[1]) << 8 | u32::from(entry[2]);
			}
		}

		let duration_ms = match u32::from(frame.delay) * 10 {
			0..=10 => 100,
			longer => longer,
		};
		frames.push(Frame { pixels: canvas.iter().flat_map(|argb| rgba(*argb)).collect(), duration_ms });

		match frame.dispose {
			DisposalMethod::Background => (0..frame_height).for_each(|y| canvas[(top + y) * width + left..][..frame_width].fill(0)),
			DisposalMethod::Previous => (0..frame_height).for_each(|y| {
				let at = (top + y) * width + left;
				canvas[at..at + frame_width].copy_from_slice(&previous[at..at + frame_width]);
			}),
			DisposalMethod::Any | DisposalMethod::Keep => {}
		}
		previous.copy_from_slice(&canvas);
	}

	if frames.is_empty() {
		return Err("the GIF has no frames".to_owned());
	}
	let loop_count = match (frames.len(), decoder.repeat()) {
		(1, _) | (_, Repeat::Infinite) => 0,
		(_, Repeat::Finite(repeats)) if repeats < u16::MAX => u32::from(repeats) + 1,
		(_, Repeat::Finite(repeats)) => u32::from(repeats),
	};
	let compatible_loop_count = match (frames.len(), decoder.repeat()) {
		(1, _) | (_, Repeat::Infinite) => 0,
		(_, Repeat::Finite(repeats)) => u32::from(repeats),
	};
	let (width, height) = (u32::try_from(width).map_err(|_| "GIF too wide")?, u32::try_from(height).map_err(|_| "GIF too tall")?);
	Ok((Animation { width, height, loop_count, background, frames }, compatible_loop_count))
}

/// Whether a GIF has more than one frame, found without decoding any pixels. A GIF that
/// cannot be read is not reported as an animation.
#[must_use]
pub fn is_animated(bytes: &[u8]) -> bool {
	let mut options = DecodeOptions::new();
	options.set_color_output(ColorOutput::Indexed);
	let Ok(mut decoder) = options.read_info(bytes) else { return false };
	let mut frames = 0;
	// Asking for the next frame's header skips the pixel data of the one before.
	while let Ok(Some(_)) = decoder.next_frame_info() {
		frames += 1;
		if frames > 1 {
			return true;
		}
	}
	false
}

/// The ICC profile and XMP packet a GIF carries, read as `gif2webp` reads them for
/// `-metadata` (its `gif2webp.c` and `gifdec.c`'s `GIFReadMetadata`): from application
/// extensions identified as `ICCRGBG1012` and `XMP DataXMP`, the **first** of each kind in
/// the file. A profile is its sub-blocks' data. An XMP packet keeps each sub-block's length
/// byte, because GIF's XMP encoding writes the packet raw and lets its bytes double as
/// lengths, and loses its last 257 bytes, the "magic trailer" that makes that work.
///
/// The walk stops at the trailer or at anything malformed, keeping what it found; the
/// frames are the decoder's business, not this one's. A GIF has no Exif.
#[must_use]
pub fn metadata(bytes: &[u8]) -> Metadata {
	/// The sub-blocks starting at `at`, each with its length byte, and where they end.
	fn sub_blocks(bytes: &[u8], mut at: usize) -> Option<(Vec<&[u8]>, usize)> {
		let mut blocks = Vec::new();
		loop {
			let length = usize::from(*bytes.get(at)?);
			if length == 0 {
				return Some((blocks, at + 1));
			}
			blocks.push(bytes.get(at..at + 1 + length)?);
			at += 1 + length;
		}
	}
	/// The size of a colour table a packed-fields byte announces.
	fn table(flags: u8) -> usize {
		if flags & 0x80 == 0 { 0 } else { 3 << ((flags & 7) + 1) }
	}

	let mut metadata = Metadata::default();
	let Some(&screen) = bytes.get(10) else { return metadata };
	let mut at = 13 + table(screen);
	loop {
		match bytes.get(at) {
			Some(0x21) => {
				let Some(&label) = bytes.get(at + 1) else { break };
				let Some((blocks, next)) = sub_blocks(bytes, at + 2) else { break };
				at = next;
				// An application extension whose identifier block is the usual 11 bytes.
				let Some((identifier, data)) = blocks.split_first().filter(|(first, _)| label == 0xff && first.len() == 12) else { continue };
				match &identifier[1..] {
					b"ICCRGBG1012" if metadata.icc.is_none() => metadata.icc = Some(data.iter().flat_map(|block| block[1..].iter().copied()).collect()),
					b"XMP DataXMP" if metadata.xmp.is_none() => {
						let mut xmp = data.concat();
						if xmp.len() > 257 {
							xmp.truncate(xmp.len() - 257);
						}
						metadata.xmp = Some(xmp);
					}
					_ => {}
				}
			}
			// An image: its descriptor, local colour table and LZW code size, then its data.
			Some(0x2c) => {
				let Some(&flags) = bytes.get(at + 9) else { break };
				let Some((_, next)) = sub_blocks(bytes, at + 10 + table(flags) + 1) else { break };
				at = next;
			}
			_ => break,
		}
	}
	metadata
}

/// `gif2webp`'s `GIFGetBackgroundColor`, as `0xAARRGGBB`.
fn background_colour(global: Option<&[u8]>, index: Option<usize>, first_transparent: Option<u8>) -> u32 {
	let Some(index) = index else { return 0xffff_ffff };
	if first_transparent.map(usize::from) == Some(index) {
		return 0;
	}
	match global.and_then(|palette| palette.get(index * 3..index * 3 + 3)) {
		Some(entry) => 0xff00_0000 | u32::from(entry[0]) << 16 | u32::from(entry[1]) << 8 | u32::from(entry[2]),
		None => 0xffff_ffff,
	}
}

/// An ARGB word as RGBA bytes.
const fn rgba(argb: u32) -> [u8; 4] {
	let [a, r, g, b] = argb.to_be_bytes();
	[r, g, b, a]
}
