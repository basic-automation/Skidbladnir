//! HEIC output and input, through libheif.
//!
//! The HEVC encoder depends on the edition. The standard edition's is **Kvazaar**
//! (BSD-3-Clause); the GPL edition's, with the `x265` feature, is **x265** (GPL-2.0-or-later).
//! Either is built *into* one shared libheif (LGPL-3.0) by `scripts/build-libheif.sh`,
//! with the **libde265** decoder and with libheif's plugin loading switched off. The app
//! links that libheif dynamically and ships it beside itself, which is how an ISC app uses
//! an LGPL library, and with no plugin loading the standard edition can never pick up
//! x265 from the user's system. `build.rs` refuses a libheif built for the other edition.
//!
//! The encoder is asked for **by name**. A dev host without the vendored build links the
//! system libheif instead (see build.rs); only then does this fall back to whatever HEVC
//! encoder that libheif offers, so a release can never silently encode with the wrong one.
//!
//! Kvazaar encodes 8-bit 4:2:0 only, so the standard edition offers quality alone: no
//! lossless mode, no 4:4:4, no 10-bit. x265 has all three, and the tuning controls in
//! [`HeicSettings`], set through libheif's own parameters and its `x265:` pass-through.
//! Neither reports progress, so progress moves at the start and end of a file.
//!
//! The declarations are written from libheif 1.23's headers. `heif_error` is returned by
//! value, and every enum is `c_int`.

use std::{
	ffi::{CStr, CString, c_char, c_int, c_void}, ptr, sync::Once
};

use crate::{
	encoder::{EncodeError, RgbaImage, rescale_rgba}, settings::{HEIC_X265, HeicBitDepth, HeicPreset, HeicSettings, Resize}
};

#[repr(C)]
#[derive(Clone, Copy)]
struct HeifError {
	code: c_int,
	subcode: c_int,
	message: *const c_char,
}

#[repr(C)]
struct Opaque {
	_private: [u8; 0],
}

#[repr(C)]
struct HeifWriter {
	writer_api_version: c_int,
	write: unsafe extern "C" fn(ctx: *mut Opaque, data: *const c_void, size: usize, userdata: *mut c_void) -> HeifError,
}

const HEIF_ERROR_OK: c_int = 0;
const HEIF_COMPRESSION_HEVC: c_int = 1;
const HEIF_COLORSPACE_RGB: c_int = 1;
const HEIF_CHROMA_INTERLEAVED_RGB: c_int = 10;
const HEIF_CHROMA_INTERLEAVED_RGBA: c_int = 11;
const HEIF_CHROMA_INTERLEAVED_RRGGBB_BE: c_int = 12;
const HEIF_CHROMA_INTERLEAVED_RRGGBBAA_BE: c_int = 13;
const HEIF_CHANNEL_INTERLEAVED: c_int = 10;
/// The identity matrix: RGB stored as RGB (as GBR), which is what lossless needs.
const HEIF_MATRIX_COEFFICIENTS_RGB_GBR: u16 = 0;

unsafe extern "C" {
	fn heif_get_version() -> *const c_char;
	fn heif_init(params: *const c_void) -> HeifError;
	fn heif_context_alloc() -> *mut Opaque;
	fn heif_context_free(ctx: *mut Opaque);
	fn heif_context_get_encoder_descriptors(ctx: *mut Opaque, format: c_int, name_filter: *const c_char, out: *mut *const Opaque, count: c_int) -> c_int;
	fn heif_context_get_encoder(ctx: *mut Opaque, descriptor: *const Opaque, out: *mut *mut Opaque) -> HeifError;
	fn heif_context_get_encoder_for_format(ctx: *mut Opaque, format: c_int, out: *mut *mut Opaque) -> HeifError;
	fn heif_encoder_descriptor_get_name(descriptor: *const Opaque) -> *const c_char;
	fn heif_encoder_set_lossy_quality(encoder: *mut Opaque, quality: c_int) -> HeifError;
	fn heif_encoder_set_lossless(encoder: *mut Opaque, enable: c_int) -> HeifError;
	fn heif_encoder_set_parameter_integer(encoder: *mut Opaque, name: *const c_char, value: c_int) -> HeifError;
	fn heif_encoder_set_parameter_string(encoder: *mut Opaque, name: *const c_char, value: *const c_char) -> HeifError;
	fn heif_nclx_color_profile_alloc() -> *mut Opaque;
	fn heif_nclx_color_profile_set_matrix_coefficients(nclx: *mut Opaque, matrix_coefficients: u16) -> HeifError;
	fn heif_nclx_color_profile_free(nclx: *mut Opaque);
	fn heif_image_set_nclx_color_profile(image: *mut Opaque, nclx: *const Opaque) -> HeifError;
	fn heif_encoder_release(encoder: *mut Opaque);
	fn heif_image_create(width: c_int, height: c_int, colorspace: c_int, chroma: c_int, out: *mut *mut Opaque) -> HeifError;
	fn heif_image_add_plane(image: *mut Opaque, channel: c_int, width: c_int, height: c_int, bit_depth: c_int) -> HeifError;
	fn heif_image_get_plane2(image: *mut Opaque, channel: c_int, out_stride: *mut usize) -> *mut u8;
	fn heif_image_get_plane_readonly2(image: *const Opaque, channel: c_int, out_stride: *mut usize) -> *const u8;
	fn heif_image_get_width(image: *const Opaque, channel: c_int) -> c_int;
	fn heif_image_get_height(image: *const Opaque, channel: c_int) -> c_int;
	fn heif_image_release(image: *const Opaque);
	fn heif_context_encode_image(ctx: *mut Opaque, image: *const Opaque, encoder: *mut Opaque, options: *const c_void, out_handle: *mut *mut Opaque) -> HeifError;
	fn heif_context_write(ctx: *mut Opaque, writer: *mut HeifWriter, userdata: *mut c_void) -> HeifError;
	fn heif_context_read_from_memory_without_copy(ctx: *mut Opaque, data: *const c_void, size: usize, options: *const c_void) -> HeifError;
	fn heif_context_get_primary_image_handle(ctx: *mut Opaque, out: *mut *mut Opaque) -> HeifError;
	fn heif_image_handle_get_width(handle: *const Opaque) -> c_int;
	fn heif_image_handle_get_height(handle: *const Opaque) -> c_int;
	fn heif_image_handle_release(handle: *const Opaque);
	fn heif_decode_image(handle: *const Opaque, out: *mut *mut Opaque, colorspace: c_int, chroma: c_int, options: *const c_void) -> HeifError;
}

/// Whether this build uses the system libheif rather than the vendored one.
const SYSTEM_LIBHEIF: bool = cfg!(skidbladnir_system_libheif);

fn init() {
	static INIT: Once = Once::new();
	// SAFETY: a null parameter block selects the defaults. libheif keeps its own reference
	// count; the library stays initialised for the life of the process.
	INIT.call_once(|| {
		let _ = unsafe { heif_init(ptr::null()) };
	});
}

fn check(call: &str, error: HeifError) -> Result<(), String> {
	if error.code == HEIF_ERROR_OK {
		return Ok(());
	}
	// SAFETY: libheif's messages are static, NUL-terminated strings.
	let message = if error.message.is_null() { String::new() } else { unsafe { CStr::from_ptr(error.message) }.to_string_lossy().into_owned() };
	Err(format!("libheif `{call}` failed: {message} ({}.{})", error.code, error.subcode))
}

/// The linked libheif's version string.
#[must_use]
pub fn linked_version() -> String {
	// SAFETY: returns a static, NUL-terminated string.
	unsafe { CStr::from_ptr(heif_get_version()) }.to_string_lossy().into_owned()
}

/// A context, freed on drop, with the objects hanging off it.
struct Context(*mut Opaque);

impl Drop for Context {
	fn drop(&mut self) {
		// SAFETY: allocated by `heif_context_alloc` and freed once.
		unsafe { heif_context_free(self.0) };
	}
}

impl Context {
	fn new() -> Result<Self, String> {
		init();
		// SAFETY: a plain allocation.
		let ctx = unsafe { heif_context_alloc() };
		if ctx.is_null() { Err("libheif could not allocate a context".to_owned()) } else { Ok(Self(ctx)) }
	}
}

/// An image, released on drop.
struct Image(*mut Opaque);

impl Drop for Image {
	fn drop(&mut self) {
		if !self.0.is_null() {
			// SAFETY: created by libheif and released once.
			unsafe { heif_image_release(self.0) };
		}
	}
}

/// The name of the HEVC encoder this edition uses, and asks libheif for by name.
const ENCODER: &CStr = if HEIC_X265 { c"x265" } else { c"kvazaar" };

/// This edition's HEVC encoder's descriptor, or `None` if the linked libheif lacks it.
fn descriptor(ctx: &Context, name: &CStr) -> Option<*const Opaque> {
	let mut descriptor: *const Opaque = ptr::null();
	// SAFETY: `ctx` is live, the filter is a NUL-terminated string, and one slot is offered.
	let found = unsafe { heif_context_get_encoder_descriptors(ctx.0, HEIF_COMPRESSION_HEVC, name.as_ptr(), &raw mut descriptor, 1) };
	(found > 0 && !descriptor.is_null()).then_some(descriptor)
}

/// Get the HEVC encoder: this edition's by name, or — only with the system libheif —
/// whatever HEVC encoder it has.
fn hevc_encoder(ctx: &Context) -> Result<*mut Opaque, String> {
	let mut encoder: *mut Opaque = ptr::null_mut();
	if let Some(descriptor) = descriptor(ctx, ENCODER) {
		// SAFETY: `descriptor` came from libheif for this context.
		check("heif_context_get_encoder", unsafe { heif_context_get_encoder(ctx.0, descriptor, &raw mut encoder) })?;
	} else if SYSTEM_LIBHEIF {
		// SAFETY: `ctx` is live.
		check("heif_context_get_encoder_for_format", unsafe { heif_context_get_encoder_for_format(ctx.0, HEIF_COMPRESSION_HEVC, &raw mut encoder) })?;
	} else {
		return Err(format!("this libheif has no {} HEVC encoder", ENCODER.to_string_lossy()));
	}
	Ok(encoder)
}

/// The HEVC encoder this build writes HEIC with, as libheif names it, with its version
/// where it gives one ("x265 HEVC encoder (4.2+…)").
#[must_use]
pub fn encoder_name() -> String {
	let Ok(ctx) = Context::new() else { return "no HEVC encoder".to_owned() };
	match descriptor(&ctx, ENCODER) {
		// SAFETY: the descriptor's name is a static, NUL-terminated string.
		Some(descriptor) => unsafe { CStr::from_ptr(heif_encoder_descriptor_get_name(descriptor)) }.to_string_lossy().into_owned(),
		None => format!("no {} HEVC encoder", ENCODER.to_string_lossy()),
	}
}

/// Whether the linked libheif has an HEVC encoder called `name` ("x265", "kvazaar"): how
/// the tests prove each edition ships its own encoder and not the other's.
#[must_use]
pub fn has_encoder(name: &str) -> bool {
	let (Ok(ctx), Ok(name)) = (Context::new(), CString::new(name)) else { return false };
	descriptor(&ctx, &name).is_some()
}

/// Apply `settings` to `encoder`, for an image this size: quality alone for Kvazaar,
/// everything for x265.
fn configure(encoder: *mut Opaque, settings: &HeicSettings, (width, height): (u32, u32)) -> Result<(), String> {
	let string = |name: &CStr, value: &str| {
		let value = CString::new(value).map_err(|_| format!("`{value}` is not a valid libheif parameter"))?;
		// SAFETY: `encoder` is live, and both strings are NUL-terminated and outlive the
		// call; libheif copies what it keeps.
		check(&format!("heif_encoder_set_parameter {}", name.to_string_lossy()), unsafe { heif_encoder_set_parameter_string(encoder, name.as_ptr(), value.as_ptr()) })
	};
	let tenths = |value: u16| format!("{}.{}", value / 10, value % 10);

	// SAFETY: `encoder` is live; quality is validated to 0..=100.
	check("heif_encoder_set_lossy_quality", unsafe { heif_encoder_set_lossy_quality(encoder, c_int::from(settings.quality)) })?;
	if !HEIC_X265 {
		return Ok(());
	}
	if settings.lossless {
		// SAFETY: `encoder` is live.
		check("heif_encoder_set_lossless", unsafe { heif_encoder_set_lossless(encoder, 1) })?;
		// Lossless keeps every colour sample, so the chroma must be full resolution.
		string(c"chroma", "444")?;
	} else {
		string(c"chroma", settings.chroma.parameter())?;
	}
	string(c"preset", settings.preset.parameter())?;
	string(c"tune", settings.tune.parameter())?;
	// libheif codes an image under 32 pixels on a side in 16-pixel CTUs, where x265 allows
	// transform units at most 3 levels deep, for inter prediction too. The preset's inter
	// depth (4 for placebo) goes unused in a still image, but x265 still checks it.
	let small = width.min(height) < 32;
	let intra_depth = if small { settings.tu_intra_depth.min(3) } else { settings.tu_intra_depth };
	// SAFETY: `encoder` is live and the name is NUL-terminated; the depth is validated to 1..=4.
	check("heif_encoder_set_parameter tu-intra-depth", unsafe { heif_encoder_set_parameter_integer(encoder, c"tu-intra-depth".as_ptr(), c_int::from(intra_depth)) })?;
	if small {
		string(c"x265:tu-inter-depth", "3")?;
	}
	// Straight to x265. libheif applies these after its own settings and the preset and
	// tune, so they override all of them.
	string(c"x265:aq-mode", settings.aq_mode.parameter())?;
	string(c"x265:aq-strength", &tenths(u16::from(settings.aq_strength)))?;
	string(c"x265:psy-rd", &tenths(u16::from(settings.psy_rd)))?;
	string(c"x265:psy-rdoq", &tenths(settings.psy_rdoq))?;
	string(c"x265:deblock", &if settings.deblock { format!("{}:{}", settings.deblock_strength, settings.deblock_threshold) } else { "false".to_owned() })?;
	string(c"x265:sao", if settings.sao { "true" } else { "false" })?;
	Ok(())
}

unsafe extern "C" fn write_to_vec(_ctx: *mut Opaque, data: *const c_void, size: usize, userdata: *mut c_void) -> HeifError {
	// SAFETY: `userdata` is the `Vec<u8>` passed to `heif_context_write` below, and `data`
	// points at `size` readable bytes for the duration of the call.
	unsafe {
		let output = &mut *userdata.cast::<Vec<u8>>();
		output.extend_from_slice(std::slice::from_raw_parts(data.cast::<u8>(), size));
	}
	HeifError { code: HEIF_ERROR_OK, subcode: 0, message: c"".as_ptr() }
}

/// Encode RGBA pixels to a HEIC file. Transparency is kept as HEIC's alpha auxiliary image.
///
/// # Errors
///
/// Returns [`EncodeError`] if the settings are out of range, the buffer does not match its
/// dimensions, the resize fails, libheif fails, or `on_progress` asked to stop.
pub fn encode(settings: &HeicSettings, resize: Resize, image: &RgbaImage<'_>, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Vec<u8>, EncodeError> {
	settings.validate()?;
	let (width, height, pixels) = rescale_rgba(image, resize)?;
	if !on_progress(0) {
		return Err(EncodeError::Cancelled);
	}
	let output = encode_pixels(settings, width, height, &pixels).map_err(EncodeError::Heic)?;
	if !on_progress(100) {
		return Err(EncodeError::Cancelled);
	}
	Ok(output)
}

fn encode_pixels(settings: &HeicSettings, width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, String> {
	let (Ok(w), Ok(h)) = (c_int::try_from(width), c_int::try_from(height)) else { return Err(format!("{width}x{height} is too large for HEIC")) };
	let opaque = pixels.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 255);
	// libheif encodes transparency with a second x265 that does not get the preset's TU
	// inter depth fixed for a small image (see `configure`), and placebo's is one x265
	// refuses there.
	if HEIC_X265 && !opaque && settings.preset == HeicPreset::Placebo && width.min(height) < 32 {
		return Err("x265's placebo preset cannot encode the transparency of an image under 32 pixels on a side; use veryslow".to_owned());
	}
	let ctx = Context::new()?;
	let encoder = hevc_encoder(&ctx)?;
	let result = (|| {
		configure(encoder, settings, (width, height))?;
		let image = rgba_image(settings, (width, height), (w, h), pixels, opaque)?;

		let mut handle: *mut Opaque = ptr::null_mut();
		// SAFETY: all objects are live; null options select the defaults, which take the
		// output colour profile from the image.
		check("heif_context_encode_image", unsafe { heif_context_encode_image(ctx.0, image.0, encoder, ptr::null(), &raw mut handle) })?;
		// SAFETY: the handle was returned by the encode above.
		unsafe { heif_image_handle_release(handle) };

		let mut output = Vec::new();
		let mut writer = HeifWriter { writer_api_version: 1, write: write_to_vec };
		// SAFETY: `writer` and `output` outlive the call, and `write_to_vec` treats the
		// userdata as exactly this `Vec<u8>`.
		check("heif_context_write", unsafe { heif_context_write(ctx.0, &raw mut writer, (&raw mut output).cast()) })?;
		Ok(output)
	})();
	// SAFETY: the encoder came from this context and is released once.
	unsafe { heif_encoder_release(encoder) };
	result
}

/// The RGBA pixels as a libheif image, at the precision the settings ask for.
///
/// An `opaque` image goes in as RGB, so the file carries no alpha image that would say
/// only "opaque" at the cost of a second encode. 10-bit output is fed 10-bit RGB, each
/// 8-bit sample widened by repeating its top bits, so libheif's colour conversion runs at
/// 10 bits and not at 8. Lossless marks the image as RGB stored as RGB, so libheif does no
/// colour conversion at all.
fn rgba_image(settings: &HeicSettings, (width, height): (u32, u32), (w, h): (c_int, c_int), pixels: &[u8], opaque: bool) -> Result<Image, String> {
	let ten = HEIC_X265 && settings.bit_depth == HeicBitDepth::Ten && !settings.lossless;
	let channels = if opaque { 3 } else { 4 };
	let (chroma, depth, sample) = match (ten, opaque) {
		(false, true) => (HEIF_CHROMA_INTERLEAVED_RGB, 8, 1),
		(false, false) => (HEIF_CHROMA_INTERLEAVED_RGBA, 8, 1),
		(true, true) => (HEIF_CHROMA_INTERLEAVED_RRGGBB_BE, 10, 2),
		(true, false) => (HEIF_CHROMA_INTERLEAVED_RRGGBBAA_BE, 10, 2),
	};
	let mut image = Image(ptr::null_mut());
	// SAFETY: plain creation into a live out-pointer, then a plane of the declared size.
	check("heif_image_create", unsafe { heif_image_create(w, h, HEIF_COLORSPACE_RGB, chroma, &raw mut image.0) })?;
	check("heif_image_add_plane", unsafe { heif_image_add_plane(image.0, HEIF_CHANNEL_INTERLEAVED, w, h, depth) })?;
	let mut stride = 0_usize;
	// SAFETY: the plane was just added; libheif reports its stride.
	let plane = unsafe { heif_image_get_plane2(image.0, HEIF_CHANNEL_INTERLEAVED, &raw mut stride) };
	let row = width as usize * channels * sample;
	if plane.is_null() || stride < row {
		return Err("libheif gave no usable image plane".to_owned());
	}
	for (y, source) in pixels.chunks_exact(width as usize * 4).take(height as usize).enumerate() {
		// SAFETY: row `y` of the plane starts at `y * stride` and holds at least `row`
		// bytes, since `stride >= row` and the plane is `height` rows tall.
		let target = unsafe { std::slice::from_raw_parts_mut(plane.add(y * stride), row) };
		let samples = source.as_chunks::<4>().0.iter().flat_map(|pixel| &pixel[..channels]);
		if ten {
			for (widened, &value) in target.as_chunks_mut::<2>().0.iter_mut().zip(samples) {
				*widened = ((u16::from(value) << 2) | (u16::from(value) >> 6)).to_be_bytes();
			}
		} else {
			for (sample, &value) in target.iter_mut().zip(samples) {
				*sample = value;
			}
		}
	}
	if HEIC_X265 && settings.lossless {
		// SAFETY: a plain allocation with sRGB defaults, freed below after libheif has
		// copied it into the image.
		let nclx = unsafe { heif_nclx_color_profile_alloc() };
		if nclx.is_null() {
			return Err("libheif could not allocate a colour profile".to_owned());
		}
		// SAFETY: `nclx` is live, and `image` holds it by copy once set.
		let set = check("heif_nclx_color_profile_set_matrix_coefficients", unsafe { heif_nclx_color_profile_set_matrix_coefficients(nclx, HEIF_MATRIX_COEFFICIENTS_RGB_GBR) }).and_then(|()| check("heif_image_set_nclx_color_profile", unsafe { heif_image_set_nclx_color_profile(image.0, nclx) }));
		// SAFETY: allocated above and freed once.
		unsafe { heif_nclx_color_profile_free(nclx) };
		set?;
	}
	Ok(image)
}

/// Decode the primary image of a HEIC (or any HEIF this libheif reads) into
/// `(width, height, rgba)`, with its rotation and mirroring applied.
///
/// # Errors
///
/// Returns libheif's message if the file cannot be read or decoded.
pub fn decode(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
	let ctx = Context::new()?;
	// SAFETY: `bytes` outlives `ctx`, which is dropped at the end of this function.
	check("heif_context_read_from_memory", unsafe { heif_context_read_from_memory_without_copy(ctx.0, bytes.as_ptr().cast(), bytes.len(), ptr::null()) })?;
	let mut handle: *mut Opaque = ptr::null_mut();
	// SAFETY: `ctx` holds a parsed file.
	check("heif_context_get_primary_image_handle", unsafe { heif_context_get_primary_image_handle(ctx.0, &raw mut handle) })?;
	let mut image = Image(ptr::null_mut());
	// SAFETY: `handle` is live until released below.
	let decoded = unsafe { heif_decode_image(handle, &raw mut image.0, HEIF_COLORSPACE_RGB, HEIF_CHROMA_INTERLEAVED_RGBA, ptr::null()) };
	// SAFETY: released once, after its last use.
	unsafe { heif_image_handle_release(handle) };
	check("heif_decode_image", decoded)?;

	// SAFETY: `image` is a decoded interleaved RGBA image; its own plane size is used.
	let (width, height) = unsafe { (heif_image_get_width(image.0, HEIF_CHANNEL_INTERLEAVED), heif_image_get_height(image.0, HEIF_CHANNEL_INTERLEAVED)) };
	let (Ok(width), Ok(height)) = (u32::try_from(width), u32::try_from(height)) else { return Err("libheif decoded an image with no size".to_owned()) };
	let mut stride = 0_usize;
	// SAFETY: as above.
	let plane = unsafe { heif_image_get_plane_readonly2(image.0, HEIF_CHANNEL_INTERLEAVED, &raw mut stride) };
	let row = width as usize * 4;
	if plane.is_null() || stride < row {
		return Err("libheif gave no usable image plane".to_owned());
	}
	let mut rgba = Vec::with_capacity(row * height as usize);
	for y in 0..height as usize {
		// SAFETY: row `y` starts at `y * stride` with at least `row` readable bytes.
		rgba.extend_from_slice(unsafe { std::slice::from_raw_parts(plane.add(y * stride), row) });
	}
	Ok((width, height, rgba))
}

/// A HEIF file's primary image dimensions, read without decoding it.
#[must_use]
pub fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
	let ctx = Context::new().ok()?;
	// SAFETY: `bytes` outlives `ctx`.
	check("heif_context_read_from_memory", unsafe { heif_context_read_from_memory_without_copy(ctx.0, bytes.as_ptr().cast(), bytes.len(), ptr::null()) }).ok()?;
	let mut handle: *mut Opaque = ptr::null_mut();
	// SAFETY: `ctx` holds a parsed file.
	check("heif_context_get_primary_image_handle", unsafe { heif_context_get_primary_image_handle(ctx.0, &raw mut handle) }).ok()?;
	// SAFETY: `handle` is live until released.
	let size = unsafe { (heif_image_handle_get_width(handle), heif_image_handle_get_height(handle)) };
	// SAFETY: released once.
	unsafe { heif_image_handle_release(handle) };
	Some((u32::try_from(size.0).ok()?, u32::try_from(size.1).ok()?))
}

#[cfg(test)]
mod tests {
	use super::{SYSTEM_LIBHEIF, decode, dimensions, encode, encoder_name, has_encoder, linked_version};
	use crate::{
		encoder::{EncodeError, RgbaImage}, settings::{HEIC_X265, HeicSettings, Resize}
	};

	/// Smooth ramps (HEVC's natural material) with a transparent left quarter.
	fn fixture(width: u32, height: u32, alpha: bool) -> Vec<u8> {
		(0..height)
			.flat_map(|y| {
				(0..width).flat_map(move |x| {
					let a = if alpha && x < width / 4 { 0 } else { 255 };
					[u8::try_from(x * 255 / width).unwrap_or(0), u8::try_from(y * 255 / height).unwrap_or(0), 128, a]
				})
			})
			.collect()
	}

	fn psnr(a: &[u8], b: &[u8]) -> f64 {
		let (mut squared, mut samples) = (0.0_f64, 0.0_f64);
		for (p, q) in a.as_chunks::<4>().0.iter().zip(b.as_chunks::<4>().0) {
			if p[3] == 255 {
				for channel in 0..3 {
					let difference = f64::from(p[channel]) - f64::from(q[channel]);
					squared += difference * difference;
					samples += 1.0;
				}
			}
		}
		if squared == 0.0 { f64::INFINITY } else { 10.0 * (255.0 * 255.0 * samples / squared).log10() }
	}

	#[test]
	fn links_libheif_1_20_or_newer() {
		let version = linked_version();
		let mut parts = version.split('.').map(|part| part.parse::<u32>().unwrap_or(0));
		let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
		assert!(major > 1 || minor >= 20, "linked libheif is {version}");
	}

	#[test]
	fn writes_a_heic_file_that_reads_back_at_the_right_size_with_alpha() {
		let pixels = fixture(64, 48, true);
		let bytes = encode(&HeicSettings::default(), Resize::default(), &RgbaImage { width: 64, height: 48, pixels: &pixels }, &mut |_| true).expect("encode");
		assert_eq!(&bytes[4..12], b"ftypheic", "a HEIC file");
		assert_eq!(dimensions(&bytes), Some((64, 48)));
		let (width, height, rgba) = decode(&bytes).expect("decode our own output");
		assert_eq!((width, height), (64, 48));
		assert!(psnr(&pixels, &rgba) > 30.0, "decodes at only {:.1} dB", psnr(&pixels, &rgba));
		for (source, decoded) in pixels.as_chunks::<4>().0.iter().zip(rgba.as_chunks::<4>().0) {
			if source[3] == 0 {
				assert!(decoded[3] < 16, "a transparent pixel decoded with alpha {}", decoded[3]);
			} else {
				assert!(decoded[3] > 239, "an opaque pixel decoded with alpha {}", decoded[3]);
			}
		}
	}

	#[test]
	fn higher_quality_is_closer_and_larger() {
		let pixels = fixture(64, 64, false);
		let run = |quality| {
			let bytes = encode(&HeicSettings { quality, ..HeicSettings::default() }, Resize::default(), &RgbaImage { width: 64, height: 64, pixels: &pixels }, &mut |_| true).expect("encode");
			let (_, _, rgba) = decode(&bytes).expect("decode");
			(bytes.len(), psnr(&pixels, &rgba))
		};
		let (low_size, low_psnr) = run(20);
		let (high_size, high_psnr) = run(90);
		assert!(high_psnr > low_psnr, "quality 90 at {high_psnr:.1} dB vs quality 20 at {low_psnr:.1} dB");
		assert!(high_size > low_size, "quality 90 is {high_size} bytes vs quality 20 at {low_size}");
	}

	#[test]
	fn applies_the_shared_resize() {
		let pixels = fixture(64, 48, false);
		let bytes = encode(&HeicSettings::default(), Resize { width: 32, height: 0, no_enlarge: false }, &RgbaImage { width: 64, height: 48, pixels: &pixels }, &mut |_| true).expect("encode");
		assert_eq!(dimensions(&bytes), Some((32, 24)));
	}

	#[test]
	fn a_cancel_returns_nothing_and_bad_settings_are_refused() {
		let pixels = fixture(8, 8, false);
		let image = RgbaImage { width: 8, height: 8, pixels: &pixels };
		assert_eq!(encode(&HeicSettings::default(), Resize::default(), &image, &mut |_| false), Err(EncodeError::Cancelled));
		assert!(encode(&HeicSettings { quality: 101, ..HeicSettings::default() }, Resize::default(), &image, &mut |_| true).is_err());
	}

	#[test]
	fn an_opaque_image_carries_no_alpha_image_and_a_transparent_one_does() {
		use std::{ffi::c_int, ptr};

		use super::{Context, HeifError, Opaque, check};
		unsafe extern "C" {
			fn heif_context_read_from_memory_without_copy(ctx: *mut Opaque, data: *const std::ffi::c_void, size: usize, options: *const std::ffi::c_void) -> HeifError;
			fn heif_context_get_primary_image_handle(ctx: *mut Opaque, out: *mut *mut Opaque) -> HeifError;
			fn heif_image_handle_has_alpha_channel(handle: *const Opaque) -> c_int;
			fn heif_image_handle_release(handle: *const Opaque);
		}
		let has_alpha = |bytes: &[u8]| {
			let ctx = Context::new().expect("context");
			// SAFETY: `bytes` outlives `ctx`; the handle is released once.
			unsafe {
				check("read", heif_context_read_from_memory_without_copy(ctx.0, bytes.as_ptr().cast(), bytes.len(), ptr::null())).expect("read");
				let mut handle: *mut Opaque = ptr::null_mut();
				check("handle", heif_context_get_primary_image_handle(ctx.0, &raw mut handle)).expect("handle");
				let alpha = heif_image_handle_has_alpha_channel(handle) != 0;
				heif_image_handle_release(handle);
				alpha
			}
		};
		let heic = |alpha| {
			let pixels = fixture(64, 48, alpha);
			encode(&HeicSettings::default(), Resize::default(), &RgbaImage { width: 64, height: 48, pixels: &pixels }, &mut |_| true).expect("encode")
		};
		let (opaque, transparent) = (heic(false), heic(true));
		assert!(!has_alpha(&opaque), "an opaque image was written with an alpha image");
		assert!(has_alpha(&transparent), "a transparent image lost its alpha image");
		assert!(decode(&opaque).expect("decode").2.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 255), "an opaque image decoded with transparency");
	}

	/// The licence rests on this: the standard edition ships Kvazaar and not x265, and the
	/// GPL edition x265 and not Kvazaar.
	#[test]
	fn the_linked_libheif_has_this_editions_encoder_and_not_the_other() {
		if SYSTEM_LIBHEIF {
			eprintln!("EDITION CHECK NOT RUN: this build links the system libheif, which has whatever encoders it has");
			return;
		}
		assert_eq!(has_encoder("x265"), HEIC_X265, "x265 in the linked libheif: {}", has_encoder("x265"));
		assert_eq!(has_encoder("kvazaar"), !HEIC_X265, "Kvazaar in the linked libheif: {}", has_encoder("kvazaar"));
		let name = encoder_name().to_lowercase();
		assert!(name.contains(if HEIC_X265 { "x265" } else { "kvazaar" }), "encodes with {name}");
	}

	#[test]
	fn a_file_that_is_not_heif_is_refused() {
		assert!(decode(b"not a heif file").is_err());
		assert_eq!(dimensions(b"not a heif file"), None);
	}

	/// x265's controls, only in the GPL edition.
	#[cfg(feature = "x265")]
	mod x265 {
		use std::{
			ffi::{c_int, c_void}, ptr
		};

		use super::{fixture, psnr};
		use crate::{
			encoder::RgbaImage, heic::{Context, HeifError, Opaque, check, decode, encode}, settings::{HeicAqMode, HeicBitDepth, HeicChroma, HeicPreset, HeicSettings, HeicTune, Resize}
		};

		unsafe extern "C" {
			fn heif_context_read_from_memory_without_copy(ctx: *mut Opaque, data: *const c_void, size: usize, options: *const c_void) -> HeifError;
			fn heif_context_get_primary_image_handle(ctx: *mut Opaque, out: *mut *mut Opaque) -> HeifError;
			fn heif_image_handle_get_luma_bits_per_pixel(handle: *const Opaque) -> c_int;
			fn heif_image_handle_get_preferred_decoding_colorspace(handle: *const Opaque, colorspace: *mut c_int, chroma: *mut c_int) -> HeifError;
			fn heif_image_handle_release(handle: *const Opaque);
		}

		/// The coded bit depth and libheif's chroma enum (1 = 4:2:0, 2 = 4:2:2, 3 = 4:4:4)
		/// of a HEIC's primary image.
		fn coded(bytes: &[u8]) -> (c_int, c_int) {
			let ctx = Context::new().expect("context");
			// SAFETY: `bytes` outlives `ctx`; the handle is released once.
			unsafe {
				check("read", heif_context_read_from_memory_without_copy(ctx.0, bytes.as_ptr().cast(), bytes.len(), ptr::null())).expect("read");
				let mut handle: *mut Opaque = ptr::null_mut();
				check("handle", heif_context_get_primary_image_handle(ctx.0, &raw mut handle)).expect("handle");
				let (mut colorspace, mut chroma) = (0, 0);
				check("colorspace", heif_image_handle_get_preferred_decoding_colorspace(handle, &raw mut colorspace, &raw mut chroma)).expect("colorspace");
				let bits = heif_image_handle_get_luma_bits_per_pixel(handle);
				heif_image_handle_release(handle);
				(bits, chroma)
			}
		}

		fn heic(settings: &HeicSettings, width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
			encode(settings, Resize::default(), &RgbaImage { width, height, pixels }, &mut |_| true).expect("encode")
		}

		#[test]
		fn lossless_keeps_every_pixel_and_its_alpha() {
			let pixels = fixture(64, 48, true);
			let bytes = heic(&HeicSettings { lossless: true, ..HeicSettings::default() }, 64, 48, &pixels);
			let (_, _, rgba) = decode(&bytes).expect("decode");
			let opaque = |image: &[u8]| image.as_chunks::<4>().0.iter().filter(|pixel| pixel[3] == 255).copied().collect::<Vec<_>>();
			assert_eq!(opaque(&rgba), opaque(&pixels), "an opaque pixel changed");
			assert_eq!(rgba.as_chunks::<4>().0.iter().map(|pixel| pixel[3]).collect::<Vec<_>>(), pixels.as_chunks::<4>().0.iter().map(|pixel| pixel[3]).collect::<Vec<_>>(), "alpha changed");
		}

		#[test]
		fn each_chroma_is_written_as_asked_and_444_keeps_colour_detail() {
			// One-pixel stripes of red and blue: all of the detail is in the colour.
			let pixels: Vec<u8> = (0..32 * 32).flat_map(|i| if i % 2 == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }).collect();
			let at = |chroma| {
				let bytes = heic(&HeicSettings { quality: 90, chroma, ..HeicSettings::default() }, 32, 32, &pixels);
				(coded(&bytes).1, psnr(&pixels, &decode(&bytes).expect("decode").2))
			};
			let (c420, p420) = at(HeicChroma::Yuv420);
			let (c422, _) = at(HeicChroma::Yuv422);
			let (c444, p444) = at(HeicChroma::Yuv444);
			assert_eq!((c420, c422, c444), (1, 2, 3), "coded chroma formats");
			assert!(p444 > p420 + 6.0, "4:4:4 decodes at {p444:.1} dB, 4:2:0 at {p420:.1} dB");
		}

		#[test]
		fn ten_bit_is_written_as_main_10_and_reads_back() {
			let pixels = fixture(64, 48, true);
			let bytes = heic(&HeicSettings { bit_depth: HeicBitDepth::Ten, ..HeicSettings::default() }, 64, 48, &pixels);
			assert_eq!(coded(&bytes).0, 10);
			let (width, height, rgba) = decode(&bytes).expect("decode");
			assert_eq!((width, height), (64, 48));
			assert!(psnr(&pixels, &rgba) > 30.0, "decodes at only {:.1} dB", psnr(&pixels, &rgba));
			assert_eq!(coded(&heic(&HeicSettings::default(), 64, 48, &pixels)).0, 8);
		}

		/// At every CTU size libheif picks: 16 pixels under 32 on a side, where x265 is
		/// strictest, 32 under 64, and 64.
		#[test]
		fn every_preset_tune_and_aq_mode_is_one_x265_accepts_at_any_size() {
			let settings: Vec<HeicSettings> = HeicPreset::ALL.map(|preset| HeicSettings { preset, tu_intra_depth: 4, ..HeicSettings::default() }).into_iter().chain([HeicTune::Psnr, HeicTune::Ssim, HeicTune::Grain, HeicTune::Fastdecode].map(|tune| HeicSettings { tune, ..HeicSettings::default() })).chain([HeicAqMode::Off, HeicAqMode::Variance, HeicAqMode::AutoVariance, HeicAqMode::AutoVarianceDark, HeicAqMode::AutoVarianceEdge].map(|aq_mode| HeicSettings { aq_mode, ..HeicSettings::default() })).collect();
			let mut refused = Vec::new();
			for (width, height) in [(16, 16), (48, 40), (64, 64)] {
				for alpha in [false, true] {
					let pixels = fixture(width, height, alpha);
					for settings in &settings {
						// The one combination x265 cannot do, refused up front with a reason.
						let impossible = alpha && width < 32 && settings.preset == HeicPreset::Placebo;
						match encode(settings, Resize::default(), &RgbaImage { width, height, pixels: &pixels }, &mut |_| true) {
							Err(error) if !impossible => refused.push(format!("{width}x{height} alpha {alpha} {:?}/{:?}/{:?}: {error}", settings.preset, settings.tune, settings.aq_mode)),
							Ok(_) if impossible => refused.push(format!("{width}x{height} transparent placebo was not refused")),
							_ => {}
						}
					}
				}
			}
			assert!(refused.is_empty(), "x265 refused {refused:#?}");
		}

		#[test]
		fn the_tuning_controls_reach_x265() {
			let pixels = fixture(64, 64, false);
			let default = heic(&HeicSettings::default(), 64, 64, &pixels);
			let tuned = HeicSettings { preset: HeicPreset::Ultrafast, tu_intra_depth: 4, aq_strength: 25, psy_rd: 40, psy_rdoq: 200, deblock_strength: -3, deblock_threshold: 4, ..HeicSettings::default() };
			assert_ne!(heic(&tuned, 64, 64, &pixels), default, "the tuning changed nothing");
			let off = HeicSettings { deblock: false, sao: false, aq_mode: HeicAqMode::Off, psy_rd: 0, psy_rdoq: 0, ..HeicSettings::default() };
			let bytes = heic(&off, 64, 64, &pixels);
			assert!(psnr(&pixels, &decode(&bytes).expect("decode").2) > 30.0);
		}
	}
}
