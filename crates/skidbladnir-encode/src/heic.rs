//! HEIC output and input, through libheif.
//!
//! The HEVC encoder is **Kvazaar** (BSD-3-Clause) and the decoder **libde265**, both built
//! *into* one shared libheif (LGPL-3.0) by `scripts/build-libheif.sh`, with libheif's
//! plugin loading switched off. The app links that libheif dynamically and ships it beside
//! itself, which is how an ISC app uses an LGPL library, and with no plugin loading it can
//! never pick up a GPL encoder such as x265 from the user's system.
//!
//! The encoder is asked for **by name**. A dev host without the vendored build links the
//! system libheif instead (see build.rs); only then does this fall back to whatever HEVC
//! encoder that libheif offers, so a release can never silently encode with x265.
//!
//! Kvazaar encodes 4:2:0 only, so HEIC has no true lossless mode here and none is offered.
//! Like AVIF, it reports no progress, so progress moves at the start and end of a file.
//!
//! The declarations are written from libheif 1.23's headers. `heif_error` is returned by
//! value, and every enum is `c_int`.

use std::{
	ffi::{CStr, c_char, c_int, c_void}, ptr, sync::Once
};

use crate::{
	encoder::{EncodeError, RgbaImage, rescale_rgba}, settings::{HeicSettings, Resize}
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
const HEIF_CHROMA_INTERLEAVED_RGBA: c_int = 11;
const HEIF_CHANNEL_INTERLEAVED: c_int = 10;

unsafe extern "C" {
	fn heif_get_version() -> *const c_char;
	fn heif_init(params: *const c_void) -> HeifError;
	fn heif_context_alloc() -> *mut Opaque;
	fn heif_context_free(ctx: *mut Opaque);
	fn heif_context_get_encoder_descriptors(ctx: *mut Opaque, format: c_int, name_filter: *const c_char, out: *mut *const Opaque, count: c_int) -> c_int;
	fn heif_context_get_encoder(ctx: *mut Opaque, descriptor: *const Opaque, out: *mut *mut Opaque) -> HeifError;
	fn heif_context_get_encoder_for_format(ctx: *mut Opaque, format: c_int, out: *mut *mut Opaque) -> HeifError;
	fn heif_encoder_set_lossy_quality(encoder: *mut Opaque, quality: c_int) -> HeifError;
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

/// Whether this build uses the system libheif rather than the vendored Kvazaar one.
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

/// The name of the HEVC encoder a release build uses, and asks for by name.
const ENCODER: &CStr = c"kvazaar";

/// Get the HEVC encoder: Kvazaar by name, or — only with the system libheif — whatever
/// HEVC encoder it has.
fn hevc_encoder(ctx: &Context) -> Result<*mut Opaque, String> {
	let mut descriptor: *const Opaque = ptr::null();
	// SAFETY: `ctx` is live, the filter is a NUL-terminated string, and one slot is offered.
	let found = unsafe { heif_context_get_encoder_descriptors(ctx.0, HEIF_COMPRESSION_HEVC, ENCODER.as_ptr(), &raw mut descriptor, 1) };
	let mut encoder: *mut Opaque = ptr::null_mut();
	if found > 0 && !descriptor.is_null() {
		// SAFETY: `descriptor` came from libheif for this context.
		check("heif_context_get_encoder", unsafe { heif_context_get_encoder(ctx.0, descriptor, &raw mut encoder) })?;
	} else if SYSTEM_LIBHEIF {
		// SAFETY: `ctx` is live.
		check("heif_context_get_encoder_for_format", unsafe { heif_context_get_encoder_for_format(ctx.0, HEIF_COMPRESSION_HEVC, &raw mut encoder) })?;
	} else {
		return Err("this libheif has no Kvazaar HEVC encoder".to_owned());
	}
	Ok(encoder)
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
	let ctx = Context::new()?;
	let encoder = hevc_encoder(&ctx)?;
	let result = (|| {
		// SAFETY: `encoder` is live; quality is validated to 0..=100.
		check("heif_encoder_set_lossy_quality", unsafe { heif_encoder_set_lossy_quality(encoder, c_int::from(settings.quality)) })?;

		let mut image = Image(ptr::null_mut());
		// SAFETY: plain creation into a live out-pointer, then a plane of the declared size.
		check("heif_image_create", unsafe { heif_image_create(w, h, HEIF_COLORSPACE_RGB, HEIF_CHROMA_INTERLEAVED_RGBA, &raw mut image.0) })?;
		check("heif_image_add_plane", unsafe { heif_image_add_plane(image.0, HEIF_CHANNEL_INTERLEAVED, w, h, 8) })?;
		let mut stride = 0_usize;
		// SAFETY: the plane was just added; libheif reports its stride.
		let plane = unsafe { heif_image_get_plane2(image.0, HEIF_CHANNEL_INTERLEAVED, &raw mut stride) };
		let row = width as usize * 4;
		if plane.is_null() || stride < row {
			return Err("libheif gave no usable image plane".to_owned());
		}
		for (y, source) in pixels.chunks_exact(row).enumerate() {
			// SAFETY: row `y` of the plane starts at `y * stride` and holds at least `row`
			// bytes, since `stride >= row` and the plane is `height` rows tall.
			unsafe { ptr::copy_nonoverlapping(source.as_ptr(), plane.add(y * stride), row) };
		}

		let mut handle: *mut Opaque = ptr::null_mut();
		// SAFETY: all objects are live; null options select the defaults.
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
	use super::{decode, dimensions, encode, linked_version};
	use crate::{
		encoder::{EncodeError, RgbaImage}, settings::{HeicSettings, Resize}
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
			let bytes = encode(&HeicSettings { quality }, Resize::default(), &RgbaImage { width: 64, height: 64, pixels: &pixels }, &mut |_| true).expect("encode");
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
		let bytes = encode(&HeicSettings::default(), Resize::to(32, 0), &RgbaImage { width: 64, height: 48, pixels: &pixels }, &mut |_| true).expect("encode");
		assert_eq!(dimensions(&bytes), Some((32, 24)));
	}

	#[test]
	fn a_cancel_returns_nothing_and_bad_settings_are_refused() {
		let pixels = fixture(8, 8, false);
		let image = RgbaImage { width: 8, height: 8, pixels: &pixels };
		assert_eq!(encode(&HeicSettings::default(), Resize::default(), &image, &mut |_| false), Err(EncodeError::Cancelled));
		assert!(encode(&HeicSettings { quality: 101 }, Resize::default(), &image, &mut |_| true).is_err());
	}

	#[test]
	fn a_file_that_is_not_heif_is_refused() {
		assert!(decode(b"not a heif file").is_err());
		assert_eq!(dimensions(b"not a heif file"), None);
	}
}
