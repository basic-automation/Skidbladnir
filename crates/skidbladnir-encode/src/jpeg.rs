//! JPEG decoding through libjpeg-turbo, with libjpeg's defaults.
//!
//! Every reference tool reads JPEG through libjpeg-turbo, and a different decoder rounds its
//! IDCT, upsampling and colour conversion differently: up to three levels per sample, on a
//! quarter of the samples of an ordinary 4:2:0 photo. So the app decodes with the same
//! library, the way `cwebp` and `cjxl` do (`native/jpeg_shim.c`). `avifenc` and `heif-enc`
//! use their own readers, compiled into their shims.

use std::ffi::{CStr, c_char, c_int};

#[repr(C)]
struct SkidJpegImage {
	width: u32,
	height: u32,
	components: u32,
	pixels: *mut u8,
}

unsafe extern "C" {
	fn skid_jpeg_decode(data: *const u8, size: usize, out: *mut SkidJpegImage, error: *mut c_char, error_size: usize) -> c_int;
	fn skid_jpeg_free(pixels: *mut u8);
}

/// A decoded JPEG: 8-bit samples, one (gray) or three (RGB) per pixel, rows tight.
pub(crate) struct Decoded {
	pub width: u32,
	pub height: u32,
	pub gray: bool,
	pub samples: Vec<u8>,
}

/// Decode a JPEG as `cwebp` and `cjxl` do. `Ok(None)` for a CMYK or YCCK JPEG, which both
/// refuse, so the caller decodes it some other way.
///
/// # Errors
///
/// libjpeg's message for a file it cannot decode.
pub(crate) fn decode(bytes: &[u8]) -> Result<Option<Decoded>, String> {
	let mut out = SkidJpegImage { width: 0, height: 0, components: 0, pixels: std::ptr::null_mut() };
	let mut error = [0 as c_char; 256];
	// SAFETY: `bytes` is readable for its length for the whole call, and the shim writes at
	// most `error.len()` bytes of message, NUL-terminated.
	let result = unsafe { skid_jpeg_decode(bytes.as_ptr(), bytes.len(), &raw mut out, error.as_mut_ptr(), error.len()) };
	match result {
		1 => {
			let length = out.width as usize * out.height as usize * out.components as usize;
			// SAFETY: on success `pixels` is a `malloc`ed buffer of exactly this many
			// bytes, copied before it is freed, once.
			let samples = unsafe { std::slice::from_raw_parts(out.pixels, length) }.to_vec();
			unsafe { skid_jpeg_free(out.pixels) };
			Ok(Some(Decoded { width: out.width, height: out.height, gray: out.components == 1, samples }))
		}
		2 => Ok(None),
		// SAFETY: the shim NUL-terminates the message within the buffer.
		_ => Err(unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy().into_owned()),
	}
}
