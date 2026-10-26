//! Camera raw input (`.cr3`), developed by `LibRaw` (`native/raw_shim.cc`).
//!
//! No reference tool reads a raw file, so there is nothing to match byte for byte. The
//! development is `dcraw`'s defaults with the camera's white balance, into sRGB with the
//! sRGB curve at 16 bits, which every encoder then takes as it takes a 16-bit PNG.

use std::ffi::{CStr, c_char, c_int};

#[repr(C)]
struct SkidRawImage {
	width: u32,
	height: u32,
	pixels: *mut u16,
}

unsafe extern "C" {
	fn skid_raw_decode(data: *const u8, size: usize, out: *mut SkidRawImage, error: *mut c_char, error_size: usize) -> c_int;
	fn skid_raw_free(pixels: *mut u16);
}

/// A developed raw file: 16-bit RGB, rows tight.
pub(crate) struct Developed {
	pub width: u32,
	pub height: u32,
	pub samples: Vec<u16>,
}

/// Develop a camera raw file.
///
/// # Errors
///
/// `LibRaw`'s message for a file it cannot read.
pub(crate) fn develop(bytes: &[u8]) -> Result<Developed, String> {
	let mut out = SkidRawImage { width: 0, height: 0, pixels: std::ptr::null_mut() };
	let mut error = [0 as c_char; 256];
	// SAFETY: `bytes` is readable for its length for the whole call, and the shim writes at
	// most `error.len()` bytes of message, NUL-terminated.
	let result = unsafe { skid_raw_decode(bytes.as_ptr(), bytes.len(), &raw mut out, error.as_mut_ptr(), error.len()) };
	if result != 1 {
		// SAFETY: the shim NUL-terminates the message within the buffer.
		return Err(unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy().into_owned());
	}
	let length = out.width as usize * out.height as usize * 3;
	// SAFETY: on success `pixels` is a `malloc`ed buffer of exactly this many samples,
	// copied before it is freed, once.
	let samples = unsafe { std::slice::from_raw_parts(out.pixels, length) }.to_vec();
	unsafe { skid_raw_free(out.pixels) };
	Ok(Developed { width: out.width, height: out.height, samples })
}
