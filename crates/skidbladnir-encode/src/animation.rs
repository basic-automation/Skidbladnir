//! Animated WebP: decoding every frame of one, and encoding frames back into one with the
//! same control surface a still image gets.
//!
//! # The shape of the problem
//!
//! An animated WebP is not a list of full pictures. Each frame is a sub-rectangle with a
//! blend and a dispose rule, drawn over what the previous frames left on the canvas. Two
//! libwebp components deal with that so this module does not have to:
//!
//! - **`WebPAnimDecoder`** composites every frame onto the canvas and hands back the
//!   whole canvas, so a decoded [`Animation`] is a list of full-canvas RGBA frames with
//!   their durations. Nothing here re-implements blending or disposal.
//! - **`WebPAnimEncoder`** takes full-canvas frames and works out the sub-rectangles,
//!   blend and dispose methods and keyframes itself.
//!
//! # Which settings apply, and to what
//!
//! **Every WebP control applies to every frame.** `WebPAnimEncoderAdd` takes a whole
//! `WebPConfig` per frame, and it is built by the same function the still-image path uses,
//! so the preset, filter, segments, SNS, alpha and every other control mean for each frame
//! exactly what they mean for a still image. Two of them change meaning in the process and
//! the UI has to say so: **target size** and **target PSNR** are per frame, not for the
//! whole file.
//!
//! The **resize** runs through the same `cwebp`-matching rescale as a still image,
//! including its `-exact` handling of transparent pixels, on each full-canvas frame.
//!
//! The source's **loop count** and **background colour** are carried over, so a
//! re-encoded animation loops the way the original did.
//!
//! # The gate
//!
//! `tests/animation.rs` holds this to libwebp's own `img2webp`, byte for byte, for every
//! control `img2webp` exposes; see that file for what it covers and what it cannot.

use std::ffi::c_int;

use libwebp_sys::{WEBP_CSP_MODE, WEBP_DEMUX_ABI_VERSION, WEBP_MUX_ABI_VERSION, WebPAnimDecoder, WebPAnimDecoderDelete, WebPAnimDecoderGetInfo, WebPAnimDecoderGetNext, WebPAnimDecoderHasMoreFrames, WebPAnimDecoderNewInternal, WebPAnimDecoderOptions, WebPAnimDecoderOptionsInitInternal, WebPAnimEncoder, WebPAnimEncoderAdd, WebPAnimEncoderAssemble, WebPAnimEncoderDelete, WebPAnimEncoderNewInternal, WebPAnimEncoderOptions, WebPAnimEncoderOptionsInitInternal, WebPAnimInfo, WebPData, WebPDataClear, WebPMux, WebPMuxAssemble, WebPMuxCreateInternal, WebPMuxDelete, WebPMuxError, WebPMuxSetChunk};

use crate::{
	encoder::{EncodeError, RgbaImage, argb_picture, build_config, resize_picture}, settings::{Resize, WebpSettings}
};

/// One frame of an animation, composited onto the full canvas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
	/// Tightly packed, non-premultiplied RGBA for the whole canvas.
	pub pixels: Vec<u8>,
	/// How long this frame is shown, in milliseconds.
	pub duration_ms: u32,
}

/// A decoded animation: full-canvas frames plus the parameters that apply to all of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Animation {
	/// Canvas width in pixels.
	pub width: u32,
	/// Canvas height in pixels.
	pub height: u32,
	/// How many times to play it; `0` means forever.
	pub loop_count: u32,
	/// The background colour hint as `0xAARRGGBB` — the value `gif2webp` builds and
	/// libwebp's mux stores, written to the file little-endian as blue, green, red, alpha.
	/// Carried over untouched.
	pub background: u32,
	/// Every frame, in display order.
	pub frames: Vec<Frame>,
}

/// Owns a `WebPAnimDecoder`.
struct Decoder(*mut WebPAnimDecoder);

impl Drop for Decoder {
	fn drop(&mut self) {
		// SAFETY: the pointer came from WebPAnimDecoderNewInternal and is deleted once.
		unsafe { WebPAnimDecoderDelete(self.0) }
	}
}

/// Owns a `WebPAnimEncoder`.
struct Encoder(*mut WebPAnimEncoder);

impl Drop for Encoder {
	fn drop(&mut self) {
		// SAFETY: the pointer came from WebPAnimEncoderNewInternal and is deleted once.
		unsafe { WebPAnimEncoderDelete(self.0) }
	}
}

/// Owns the `WebPData` an assembled animation is written into.
struct Assembled(WebPData);

impl Drop for Assembled {
	fn drop(&mut self) {
		// SAFETY: a zeroed WebPData is valid to clear, and so is one libwebp filled.
		unsafe { WebPDataClear(&mut self.0) }
	}
}

/// Owns a `WebPMux`.
struct Mux(*mut WebPMux);

impl Drop for Mux {
	fn drop(&mut self) {
		// SAFETY: the pointer came from WebPMuxCreateInternal and is deleted once.
		unsafe { WebPMuxDelete(self.0) }
	}
}

/// Add an ICC profile, Exif and an XMP packet to an encoded animation as `gif2webp
/// -metadata` and `webpmux -set` do: the file re-muxed by libwebp's mux with `ICCP`, `EXIF`
/// and `XMP ` chunks, which also sets the `VP8X` flags. With none (or only empty ones, which
/// libwebp's mux refuses), the file is returned as it is.
///
/// # Errors
///
/// [`EncodeError::Libwebp`] if libwebp's mux refuses the file or a chunk.
pub fn with_metadata(encoded: Vec<u8>, icc: Option<&[u8]>, exif: Option<&[u8]>, xmp: Option<&[u8]>) -> Result<Vec<u8>, EncodeError> {
	let chunks: Vec<(&std::ffi::CStr, &[u8])> = [(c"ICCP", icc), (c"EXIF", exif), (c"XMP ", xmp)].into_iter().filter_map(|(fourcc, payload)| payload.filter(|payload| !payload.is_empty()).map(|payload| (fourcc, payload))).collect();
	if chunks.is_empty() {
		return Ok(encoded);
	}
	// SAFETY: every WebPData points into a live slice for the call that reads it, and the
	// mux copies what it keeps (`copy_data` 1); the mux and its output are owned by guards.
	unsafe {
		let data = WebPData { bytes: encoded.as_ptr(), size: encoded.len() };
		let mux = Mux(WebPMuxCreateInternal(&raw const data, 1, WEBP_MUX_ABI_VERSION.cast_signed()));
		if mux.0.is_null() {
			return Err(EncodeError::Libwebp("WebPMuxCreate"));
		}
		for (fourcc, payload) in chunks {
			let chunk = WebPData { bytes: payload.as_ptr(), size: payload.len() };
			if WebPMuxSetChunk(mux.0, fourcc.as_ptr(), &raw const chunk, 1) != WebPMuxError::WEBP_MUX_OK {
				return Err(EncodeError::Libwebp("WebPMuxSetChunk"));
			}
		}
		let mut assembled = Assembled(std::mem::zeroed::<WebPData>());
		if WebPMuxAssemble(mux.0, &raw mut assembled.0) != WebPMuxError::WEBP_MUX_OK || assembled.0.bytes.is_null() {
			return Err(EncodeError::Libwebp("WebPMuxAssemble"));
		}
		Ok(std::slice::from_raw_parts(assembled.0.bytes, assembled.0.size).to_vec())
	}
}

/// Decode every frame of an animated WebP.
///
/// Also accepts a still WebP, which decodes as a one-frame animation — libwebp's animation
/// decoder handles both. Returns `None` if libwebp cannot parse the file.
#[must_use]
pub fn decode(bytes: &[u8]) -> Option<Animation> {
	// SAFETY: every out-parameter is a live local for its call; `data` borrows `bytes`,
	// which outlives the decoder, and the decoder is owned by a guard that deletes it.
	unsafe {
		let mut options = std::mem::zeroed::<WebPAnimDecoderOptions>();
		if WebPAnimDecoderOptionsInitInternal(&raw mut options, WEBP_DEMUX_ABI_VERSION.cast_signed()) == 0 {
			return None;
		}
		// Straight (not premultiplied) RGBA: the encoder's input shape.
		options.color_mode = WEBP_CSP_MODE::MODE_RGBA;
		options.use_threads = 0;

		let data = WebPData { bytes: bytes.as_ptr(), size: bytes.len() };
		let decoder = Decoder(WebPAnimDecoderNewInternal(&raw const data, &raw const options, WEBP_DEMUX_ABI_VERSION.cast_signed()));
		if decoder.0.is_null() {
			return None;
		}

		let mut info = std::mem::zeroed::<WebPAnimInfo>();
		if WebPAnimDecoderGetInfo(decoder.0, &raw mut info) == 0 {
			return None;
		}
		let len = (info.canvas_width as usize).checked_mul(info.canvas_height as usize)?.checked_mul(4)?;

		let mut frames = Vec::with_capacity(info.frame_count as usize);
		let mut previous_end = 0_i32;
		while WebPAnimDecoderHasMoreFrames(decoder.0) != 0 {
			let mut buffer = std::ptr::null_mut::<u8>();
			let mut end = 0_i32;
			if WebPAnimDecoderGetNext(decoder.0, &raw mut buffer, &raw mut end) == 0 || buffer.is_null() {
				return None;
			}
			// The decoder reports when each frame *ends*; the duration is the difference.
			let duration_ms = u32::try_from(end.saturating_sub(previous_end)).unwrap_or(0);
			previous_end = end;
			// SAFETY: in MODE_RGBA the decoder's buffer is the whole canvas, width * height *
			// 4 bytes, valid until the next GetNext; it is copied out before that.
			frames.push(Frame { pixels: std::slice::from_raw_parts(buffer, len).to_vec(), duration_ms });
		}

		Some(Animation { width: info.canvas_width, height: info.canvas_height, loop_count: info.loop_count, background: info.bgcolor, frames })
	}
}

/// Encode an animation to an animated WebP with the given WebP controls and resize.
///
/// `on_progress` is called with 0..=100 after each frame is added, and returning `false`
/// stops the encode between frames with [`EncodeError::Cancelled`]. libwebp's per-picture
/// progress hook is not used: the animation encoder encodes several candidates per frame,
/// so its percentages would run backwards.
///
/// # Errors
///
/// Returns [`EncodeError`] if the settings are invalid, the animation has no frames or a
/// frame's buffer does not match the canvas, or libwebp fails.
pub fn encode(settings: &WebpSettings, resize: Resize, animation: &Animation, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Vec<u8>, EncodeError> {
	encode_with(settings, resize, animation, Keyframes::Libwebp, on_progress)
}

/// How far apart the animation encoder may place keyframes.
///
/// A keyframe is a frame encoded whole rather than as a change to the one before; spacing
/// them trades size against how far back a viewer must decode to show any given frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keyframes {
	/// libwebp's defaults, which `img2webp` uses. For re-encoding an animated WebP.
	Libwebp,
	/// What `gif2webp` uses: at least 9 and at most 17 frames apart for lossless, 3 and 5
	/// for lossy. For GIF input, so the conversion matches `gif2webp`.
	Gif,
}

/// [`encode`], choosing the keyframe spacing.
///
/// # Errors
///
/// As [`encode`].
pub fn encode_with(settings: &WebpSettings, resize: Resize, animation: &Animation, keyframes: Keyframes, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Vec<u8>, EncodeError> {
	let config = build_config(settings)?;
	// "Never enlarge" is decided once, for the canvas, exactly as a still image decides it.
	let resize = resize.for_source(animation.width, animation.height);
	// gif2webp decides from the finished config, so near-lossless (lossless underneath)
	// gets the lossless spacing too.
	let spacing = match keyframes {
		Keyframes::Libwebp => None,
		Keyframes::Gif if config.lossless != 0 => Some((9, 17)),
		Keyframes::Gif => Some((3, 5)),
	};
	let frame_count = animation.frames.len();
	if frame_count == 0 {
		return Err(EncodeError::MalformedImage { width: animation.width, height: animation.height, actual: 0, expected: (animation.width as usize).saturating_mul(animation.height as usize).saturating_mul(4) });
	}

	let mut encoder: Option<(Encoder, c_int, c_int)> = None;
	let mut timestamp: c_int = 0;
	for (index, frame) in animation.frames.iter().enumerate() {
		let mut picture = argb_picture(&RgbaImage { width: animation.width, height: animation.height, pixels: &frame.pixels })?;
		if !resize.is_noop() {
			resize_picture(&mut picture, resize, &config)?;
		}

		let (width, height) = (picture.0.width, picture.0.height);
		let (encoder, canvas_w, canvas_h) = match &encoder {
			Some(existing) => existing,
			None => encoder.insert((new_encoder(width, height, animation, spacing)?, width, height)),
		};
		// Every frame goes through the same rescale, so a mismatch means a bug, not bad input.
		if (width, height) != (*canvas_w, *canvas_h) {
			return Err(EncodeError::Libwebp("WebPAnimEncoderAdd: frame size changed after resize"));
		}

		// SAFETY: the picture and config are live locals; the encoder copies what it keeps.
		if unsafe { WebPAnimEncoderAdd(encoder.0, &raw mut picture.0, timestamp, &raw const config) } == 0 {
			return Err(EncodeError::Libwebp("WebPAnimEncoderAdd"));
		}
		timestamp = timestamp.saturating_add(c_int::try_from(frame.duration_ms).unwrap_or(c_int::MAX));

		let done = u32::try_from((index + 1) * 100 / frame_count).unwrap_or(100);
		if !on_progress(done) {
			return Err(EncodeError::Cancelled);
		}
	}

	let Some((encoder, _, _)) = encoder else { return Err(EncodeError::Libwebp("WebPAnimEncoderNew")) };
	// SAFETY: a null frame is how libwebp is told the last frame's end time; the output
	// WebPData is owned by a guard that frees it.
	unsafe {
		if WebPAnimEncoderAdd(encoder.0, std::ptr::null_mut(), timestamp, std::ptr::null()) == 0 {
			return Err(EncodeError::Libwebp("WebPAnimEncoderAdd"));
		}
		let mut assembled = Assembled(std::mem::zeroed::<WebPData>());
		if WebPAnimEncoderAssemble(encoder.0, &raw mut assembled.0) == 0 || assembled.0.bytes.is_null() {
			return Err(EncodeError::Libwebp("WebPAnimEncoderAssemble"));
		}
		Ok(std::slice::from_raw_parts(assembled.0.bytes, assembled.0.size).to_vec())
	}
}

/// Create an animation encoder for a `width` x `height` canvas, carrying over the source's
/// loop count and background colour, with the keyframe spacing given as `(kmin, kmax)`.
/// Every other option is libwebp's default, which is also what `img2webp` and `gif2webp`
/// use.
fn new_encoder(width: c_int, height: c_int, animation: &Animation, spacing: Option<(c_int, c_int)>) -> Result<Encoder, EncodeError> {
	// SAFETY: `options` is a live local for both calls; the encoder is owned by a guard.
	unsafe {
		let mut options = std::mem::zeroed::<WebPAnimEncoderOptions>();
		if WebPAnimEncoderOptionsInitInternal(&raw mut options, WEBP_MUX_ABI_VERSION.cast_signed()) == 0 {
			return Err(EncodeError::Libwebp("WebPAnimEncoderOptionsInit"));
		}
		options.anim_params.loop_count = c_int::try_from(animation.loop_count).unwrap_or(0);
		options.anim_params.bgcolor = animation.background;
		if let Some((kmin, kmax)) = spacing {
			options.kmin = kmin;
			options.kmax = kmax;
		}
		let encoder = Encoder(WebPAnimEncoderNewInternal(width, height, &raw const options, WEBP_MUX_ABI_VERSION.cast_signed()));
		if encoder.0.is_null() {
			return Err(EncodeError::Libwebp("WebPAnimEncoderNew"));
		}
		Ok(encoder)
	}
}
