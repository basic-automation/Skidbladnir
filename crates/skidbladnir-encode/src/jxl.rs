//! JPEG XL output, through libjxl, the reference encoder.
//!
//! libjxl is bound here directly rather than through `jpegxl-rs`/`jpegxl-sys`, because
//! those wrappers are GPL-3.0 and Skidbladnir is ISC; libjxl itself is BSD-3. The surface
//! used is small — one encoder, one frame, one output loop — so the declarations below
//! are written from libjxl 0.12's `encode.h`, `codestream_header.h`, `color_encoding.h`,
//! `types.h` and `thread_parallel_runner.h`, field for field. Every struct is `repr(C)`
//! with the header's own field order, and `JXL_BOOL` and every C enum are `c_int`.
//!
//! Two ways in:
//! - [`encode`] encodes pixels, after the shared resize, like the other formats.
//! - [`recompress_jpeg`] repacks a JPEG's own DCT data without decoding it, which is
//!   lossless and reversible: the original JPEG can be rebuilt from the file bit for bit.
//!
//! Like AVIF, libjxl has no progress callback, so progress moves at the start and the end
//! of a file, and a cancel mid-encode is honoured by discarding the result.

use std::{
	ffi::{c_int, c_void}, ptr
};

use crate::{
	encoder::{EncodeError, RgbaImage, rescale_rgba}, settings::{JxlSettings, Resize}
};

#[repr(C)]
struct JxlPreviewHeader {
	xsize: u32,
	ysize: u32,
}

#[repr(C)]
struct JxlAnimationHeader {
	tps_numerator: u32,
	tps_denominator: u32,
	num_loops: u32,
	have_timecodes: c_int,
}

#[repr(C)]
struct JxlBasicInfo {
	have_container: c_int,
	xsize: u32,
	ysize: u32,
	bits_per_sample: u32,
	exponent_bits_per_sample: u32,
	intensity_target: f32,
	min_nits: f32,
	relative_to_max_display: c_int,
	linear_below: f32,
	uses_original_profile: c_int,
	have_preview: c_int,
	have_animation: c_int,
	orientation: c_int,
	num_color_channels: u32,
	num_extra_channels: u32,
	alpha_bits: u32,
	alpha_exponent_bits: u32,
	alpha_premultiplied: c_int,
	preview: JxlPreviewHeader,
	animation: JxlAnimationHeader,
	intrinsic_xsize: u32,
	intrinsic_ysize: u32,
	padding: [u8; 100],
}

#[repr(C)]
struct JxlExtraChannelInfo {
	kind: c_int,
	bits_per_sample: u32,
	exponent_bits_per_sample: u32,
	dim_shift: u32,
	name_length: u32,
	alpha_premultiplied: c_int,
	spot_color: [f32; 4],
	cfa_channel: u32,
}

#[repr(C)]
struct JxlColorEncoding {
	color_space: c_int,
	white_point: c_int,
	white_point_xy: [f64; 2],
	primaries: c_int,
	primaries_red_xy: [f64; 2],
	primaries_green_xy: [f64; 2],
	primaries_blue_xy: [f64; 2],
	transfer_function: c_int,
	gamma: f64,
	rendering_intent: c_int,
}

#[repr(C)]
struct JxlPixelFormat {
	num_channels: u32,
	data_type: c_int,
	endianness: c_int,
	align: usize,
}

/// `JxlParallelRunner`. The init and run callbacks are function pointers, passed through
/// untouched, so they are declared as opaque pointers of the same size.
type ParallelRunner = unsafe extern "C" fn(runner_opaque: *mut c_void, jpegxl_opaque: *mut c_void, init: *const c_void, func: *const c_void, start_range: u32, end_range: u32) -> i32;

/// Opaque `JxlEncoder`.
#[repr(C)]
struct JxlEncoder {
	_private: [u8; 0],
}

/// Opaque `JxlEncoderFrameSettings`.
#[repr(C)]
struct JxlEncoderFrameSettings {
	_private: [u8; 0],
}

const JXL_ENC_SUCCESS: c_int = 0;
const JXL_ENC_NEED_MORE_OUTPUT: c_int = 2;
const JXL_ENC_FRAME_SETTING_EFFORT: c_int = 0;
const JXL_CHANNEL_ALPHA: c_int = 0;
const JXL_ORIENT_IDENTITY: c_int = 1;
const JXL_TYPE_UINT8: c_int = 2;
const JXL_NATIVE_ENDIAN: c_int = 0;
const JXL_TRUE: c_int = 1;
const JXL_FALSE: c_int = 0;

unsafe extern "C" {
	fn JxlEncoderVersion() -> u32;
	fn JxlEncoderCreate(memory_manager: *const c_void) -> *mut JxlEncoder;
	fn JxlEncoderDestroy(enc: *mut JxlEncoder);
	fn JxlEncoderGetError(enc: *mut JxlEncoder) -> c_int;
	fn JxlEncoderSetParallelRunner(enc: *mut JxlEncoder, parallel_runner: ParallelRunner, parallel_runner_opaque: *mut c_void) -> c_int;
	fn JxlEncoderInitBasicInfo(info: *mut JxlBasicInfo);
	fn JxlEncoderSetBasicInfo(enc: *mut JxlEncoder, info: *const JxlBasicInfo) -> c_int;
	fn JxlColorEncodingSetToSRGB(color_encoding: *mut JxlColorEncoding, is_gray: c_int);
	fn JxlEncoderSetColorEncoding(enc: *mut JxlEncoder, color: *const JxlColorEncoding) -> c_int;
	fn JxlEncoderInitExtraChannelInfo(kind: c_int, info: *mut JxlExtraChannelInfo);
	fn JxlEncoderSetExtraChannelInfo(enc: *mut JxlEncoder, index: usize, info: *const JxlExtraChannelInfo) -> c_int;
	fn JxlEncoderFrameSettingsCreate(enc: *mut JxlEncoder, source: *const JxlEncoderFrameSettings) -> *mut JxlEncoderFrameSettings;
	fn JxlEncoderFrameSettingsSetOption(frame_settings: *mut JxlEncoderFrameSettings, option: c_int, value: i64) -> c_int;
	fn JxlEncoderSetFrameLossless(frame_settings: *mut JxlEncoderFrameSettings, lossless: c_int) -> c_int;
	fn JxlEncoderSetFrameDistance(frame_settings: *mut JxlEncoderFrameSettings, distance: f32) -> c_int;
	fn JxlEncoderDistanceFromQuality(quality: f32) -> f32;
	fn JxlEncoderStoreJPEGMetadata(enc: *mut JxlEncoder, store_jpeg_metadata: c_int) -> c_int;
	fn JxlEncoderAddJPEGFrame(frame_settings: *const JxlEncoderFrameSettings, buffer: *const u8, size: usize) -> c_int;
	fn JxlEncoderAddImageFrame(frame_settings: *const JxlEncoderFrameSettings, pixel_format: *const JxlPixelFormat, buffer: *const c_void, size: usize) -> c_int;
	fn JxlEncoderCloseInput(enc: *mut JxlEncoder);
	fn JxlEncoderProcessOutput(enc: *mut JxlEncoder, next_out: *mut *mut u8, avail_out: *mut usize) -> c_int;

	fn JxlThreadParallelRunner(runner_opaque: *mut c_void, jpegxl_opaque: *mut c_void, init: *const c_void, func: *const c_void, start_range: u32, end_range: u32) -> i32;
	fn JxlThreadParallelRunnerCreate(memory_manager: *const c_void, num_worker_threads: usize) -> *mut c_void;
	fn JxlThreadParallelRunnerDestroy(runner_opaque: *mut c_void);
	fn JxlThreadParallelRunnerDefaultNumWorkerThreads() -> usize;
}

/// The linked libjxl's version as `(major, minor, patch)`.
#[must_use]
pub fn linked_version() -> (u32, u32, u32) {
	// SAFETY: a pure query with no arguments.
	let packed = unsafe { JxlEncoderVersion() };
	(packed / 1_000_000, packed / 1_000 % 1_000, packed % 1_000)
}

/// An encoder, and the thread pool it runs on, destroyed together.
struct Encoder {
	enc: *mut JxlEncoder,
	runner: *mut c_void,
}

impl Drop for Encoder {
	fn drop(&mut self) {
		// SAFETY: both pointers came from their `Create` functions and are destroyed once.
		// The encoder goes first, since it holds the runner.
		unsafe {
			JxlEncoderDestroy(self.enc);
			if !self.runner.is_null() {
				JxlThreadParallelRunnerDestroy(self.runner);
			}
		}
	}
}

impl Encoder {
	fn new(multi_threading: bool) -> Result<Self, EncodeError> {
		// SAFETY: a null memory manager selects libjxl's default allocator.
		let enc = unsafe { JxlEncoderCreate(ptr::null()) };
		if enc.is_null() {
			return Err(EncodeError::Jxl("libjxl could not create an encoder".to_owned()));
		}
		let mut encoder = Self { enc, runner: ptr::null_mut() };
		if multi_threading {
			// SAFETY: plain calls; the runner is owned by `encoder` from here on.
			encoder.runner = unsafe { JxlThreadParallelRunnerCreate(ptr::null(), JxlThreadParallelRunnerDefaultNumWorkerThreads()) };
			if encoder.runner.is_null() {
				return Err(EncodeError::Jxl("libjxl could not start its thread pool".to_owned()));
			}
			// SAFETY: `JxlThreadParallelRunner` is the runner function libjxl pairs with the
			// opaque pointer `JxlThreadParallelRunnerCreate` returned.
			encoder.check("JxlEncoderSetParallelRunner", unsafe { JxlEncoderSetParallelRunner(enc, JxlThreadParallelRunner, encoder.runner) })?;
		}
		Ok(encoder)
	}

	fn check(&self, call: &str, status: c_int) -> Result<(), EncodeError> {
		if status == JXL_ENC_SUCCESS {
			return Ok(());
		}
		// SAFETY: `self.enc` is live.
		let code = unsafe { JxlEncoderGetError(self.enc) };
		Err(EncodeError::Jxl(format!("libjxl `{call}` failed (error {code})")))
	}

	/// A frame settings object with the effort applied, owned by the encoder.
	fn frame_settings(&self, effort: u8) -> Result<*mut JxlEncoderFrameSettings, EncodeError> {
		// SAFETY: `self.enc` is live; a null source means libjxl's defaults.
		let settings = unsafe { JxlEncoderFrameSettingsCreate(self.enc, ptr::null()) };
		if settings.is_null() {
			return Err(EncodeError::Jxl("libjxl could not create frame settings".to_owned()));
		}
		// SAFETY: `settings` belongs to the live encoder.
		self.check("JxlEncoderFrameSettingsSetOption(effort)", unsafe { JxlEncoderFrameSettingsSetOption(settings, JXL_ENC_FRAME_SETTING_EFFORT, i64::from(effort)) })?;
		Ok(settings)
	}

	/// Close the input and drain the whole codestream.
	fn finish(&self) -> Result<Vec<u8>, EncodeError> {
		// SAFETY: `self.enc` is live and every frame has been added.
		unsafe { JxlEncoderCloseInput(self.enc) };
		let mut output = vec![0_u8; 64 * 1024];
		let mut written = 0_usize;
		loop {
			let mut next = output[written..].as_mut_ptr();
			let mut available = output.len() - written;
			// SAFETY: `next` points at `available` writable bytes inside `output`.
			let status = unsafe { JxlEncoderProcessOutput(self.enc, &raw mut next, &raw mut available) };
			written = output.len() - available;
			match status {
				JXL_ENC_SUCCESS => break,
				JXL_ENC_NEED_MORE_OUTPUT => output.resize(output.len() * 2, 0),
				_ => self.check("JxlEncoderProcessOutput", status)?,
			}
		}
		output.truncate(written);
		Ok(output)
	}
}

/// Encode RGBA pixels to a JPEG XL file.
///
/// An opaque image is encoded as RGB with no alpha channel at all, so a JPEG or an opaque
/// PNG does not carry a constant alpha plane.
///
/// # Errors
///
/// Returns [`EncodeError`] if the settings are out of range, the buffer does not match its
/// dimensions, the resize fails, libjxl fails, or `on_progress` asked to stop.
pub fn encode(settings: &JxlSettings, resize: Resize, image: &RgbaImage<'_>, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Vec<u8>, EncodeError> {
	settings.validate()?;
	let (width, height, pixels) = rescale_rgba(image, resize)?;
	if !on_progress(0) {
		return Err(EncodeError::Cancelled);
	}

	let opaque = pixels.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == u8::MAX);
	let (channels, buffer) = if opaque { (3, pixels.as_chunks::<4>().0.iter().flat_map(|&[r, g, b, _]| [r, g, b]).collect()) } else { (4, pixels) };

	let encoder = Encoder::new(settings.multi_threading)?;
	let mut info = std::mem::MaybeUninit::<JxlBasicInfo>::uninit();
	// SAFETY: `JxlEncoderInitBasicInfo` writes every field of the struct.
	let mut info = unsafe {
		JxlEncoderInitBasicInfo(info.as_mut_ptr());
		info.assume_init()
	};
	info.xsize = width;
	info.ysize = height;
	info.bits_per_sample = 8;
	info.exponent_bits_per_sample = 0;
	info.num_color_channels = 3;
	info.orientation = JXL_ORIENT_IDENTITY;
	// Lossless has to keep the original colour space rather than convert to XYB.
	info.uses_original_profile = if settings.lossless { JXL_TRUE } else { JXL_FALSE };
	if !opaque {
		info.num_extra_channels = 1;
		info.alpha_bits = 8;
		info.alpha_exponent_bits = 0;
	}
	// SAFETY: `info` is fully initialised and outlives the call.
	encoder.check("JxlEncoderSetBasicInfo", unsafe { JxlEncoderSetBasicInfo(encoder.enc, &raw const info) })?;

	if !opaque {
		let mut alpha = std::mem::MaybeUninit::<JxlExtraChannelInfo>::uninit();
		// SAFETY: the init function writes every field.
		let mut alpha = unsafe {
			JxlEncoderInitExtraChannelInfo(JXL_CHANNEL_ALPHA, alpha.as_mut_ptr());
			alpha.assume_init()
		};
		alpha.bits_per_sample = 8;
		// SAFETY: `alpha` is initialised; index 0 is the one extra channel declared above.
		encoder.check("JxlEncoderSetExtraChannelInfo", unsafe { JxlEncoderSetExtraChannelInfo(encoder.enc, 0, &raw const alpha) })?;
	}

	let mut color = std::mem::MaybeUninit::<JxlColorEncoding>::uninit();
	// SAFETY: `JxlColorEncodingSetToSRGB` writes every field. The source pixels are
	// sRGB, which is what every decoder in this crate produces.
	let color = unsafe {
		JxlColorEncodingSetToSRGB(color.as_mut_ptr(), JXL_FALSE);
		color.assume_init()
	};
	// SAFETY: `color` is initialised and outlives the call.
	encoder.check("JxlEncoderSetColorEncoding", unsafe { JxlEncoderSetColorEncoding(encoder.enc, &raw const color) })?;

	let frame = encoder.frame_settings(settings.effort)?;
	if settings.lossless {
		// SAFETY: `frame` belongs to the live encoder.
		encoder.check("JxlEncoderSetFrameLossless", unsafe { JxlEncoderSetFrameLossless(frame, JXL_TRUE) })?;
	} else {
		// SAFETY: a pure function of its argument, then a setter on a live frame.
		let distance = unsafe { JxlEncoderDistanceFromQuality(f32::from(settings.quality)) };
		encoder.check("JxlEncoderSetFrameDistance", unsafe { JxlEncoderSetFrameDistance(frame, distance) })?;
	}

	let format = JxlPixelFormat { num_channels: channels, data_type: JXL_TYPE_UINT8, endianness: JXL_NATIVE_ENDIAN, align: 0 };
	// SAFETY: `buffer` holds `width * height * channels` bytes in the declared format, and
	// libjxl copies it before returning.
	encoder.check("JxlEncoderAddImageFrame", unsafe { JxlEncoderAddImageFrame(frame, &raw const format, buffer.as_ptr().cast(), buffer.len()) })?;
	let output = encoder.finish()?;

	if !on_progress(100) {
		return Err(EncodeError::Cancelled);
	}
	Ok(output)
}

/// Recompress a JPEG file's bytes into JPEG XL losslessly, keeping what libjxl needs to
/// rebuild the original JPEG bit for bit.
///
/// Returns `Ok(None)` when libjxl will not take this JPEG as-is (some unusual JPEGs cannot
/// be transcoded), so the caller can fall back to encoding its pixels instead.
///
/// # Errors
///
/// Returns [`EncodeError`] if the settings are out of range, libjxl fails for a reason
/// other than refusing the JPEG, or `on_progress` asked to stop.
pub fn recompress_jpeg(settings: &JxlSettings, jpeg: &[u8], on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Option<Vec<u8>>, EncodeError> {
	settings.validate()?;
	if !on_progress(0) {
		return Err(EncodeError::Cancelled);
	}
	let encoder = Encoder::new(settings.multi_threading)?;
	// SAFETY: `encoder.enc` is live; this must precede adding the frame.
	encoder.check("JxlEncoderStoreJPEGMetadata", unsafe { JxlEncoderStoreJPEGMetadata(encoder.enc, JXL_TRUE) })?;
	let frame = encoder.frame_settings(settings.effort)?;
	// SAFETY: `jpeg` is a live slice; libjxl copies what it needs.
	if unsafe { JxlEncoderAddJPEGFrame(frame, jpeg.as_ptr(), jpeg.len()) } != JXL_ENC_SUCCESS {
		return Ok(None);
	}
	let output = encoder.finish()?;
	if !on_progress(100) {
		return Err(EncodeError::Cancelled);
	}
	Ok(Some(output))
}

/// Decode a JPEG XL file into `(width, height, rgba)`.
///
/// Used for JPEG XL input, and to show our own output in the preview on web engines that
/// cannot decode JPEG XL, which today is most of them.
///
/// # Errors
///
/// Returns the decoder's message if the file is not a JPEG XL image it can render.
pub fn decode(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
	let image = jxl_oxide::JxlImage::builder().read(bytes).map_err(|error| error.to_string())?;
	let render = image.render_frame(0).map_err(|error| error.to_string())?;
	let mut stream = render.stream();
	let (width, height, channels) = (stream.width(), stream.height(), stream.channels() as usize);
	let mut samples = vec![0_u8; width as usize * height as usize * channels];
	stream.write_to_buffer(&mut samples);
	let rgba = match channels {
		1 => samples.iter().flat_map(|&v| [v, v, v, u8::MAX]).collect(),
		2 => samples.as_chunks::<2>().0.iter().flat_map(|&[v, a]| [v, v, v, a]).collect(),
		3 => samples.as_chunks::<3>().0.iter().flat_map(|&[r, g, b]| [r, g, b, u8::MAX]).collect(),
		4 => samples,
		other => return Err(format!("unexpected {other}-channel JPEG XL image")),
	};
	Ok((width, height, rgba))
}

/// Read a JPEG XL file's dimensions from its header, without decoding the image.
#[must_use]
pub fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
	let image = jxl_oxide::JxlImage::builder().read(bytes).ok()?;
	Some((image.width(), image.height()))
}

#[cfg(test)]
mod tests {
	use super::{decode, dimensions, encode, linked_version, recompress_jpeg};
	use crate::{
		encoder::RgbaImage, settings::{JxlSettings, Resize}
	};

	/// Ramps with a transparent left quarter, so colour and alpha both have something to
	/// get wrong, and colour under the transparent pixels that lossless must keep.
	fn fixture(width: u32, height: u32, alpha: bool) -> Vec<u8> {
		(0..height)
			.flat_map(|y| {
				(0..width).flat_map(move |x| {
					let a = if alpha && x < width / 4 { 0 } else { 255 };
					[u8::try_from(x * 255 / width).unwrap_or(0), u8::try_from(y * 255 / height).unwrap_or(0), u8::try_from((x + y) % 256).unwrap_or(0), a]
				})
			})
			.collect()
	}

	fn quick(settings: &JxlSettings) -> JxlSettings {
		JxlSettings { effort: 3, ..settings.clone() }
	}

	#[test]
	fn links_libjxl_0_11_or_newer() {
		let (major, minor, _) = linked_version();
		assert!(major > 0 || minor >= 11, "linked libjxl is {major}.{minor}");
	}

	#[test]
	fn writes_a_jpeg_xl_file_of_the_right_size() {
		let pixels = fixture(40, 24, true);
		let bytes = encode(&quick(&JxlSettings::default()), Resize::default(), &RgbaImage { width: 40, height: 24, pixels: &pixels }, &mut |_| true).expect("encode");
		assert_eq!(&bytes[..2], [0xff, 0x0a], "a bare JPEG XL codestream");
		assert_eq!(dimensions(&bytes), Some((40, 24)));
		let (width, height, rgba) = decode(&bytes).expect("decode our own output");
		assert_eq!((width, height, rgba.len()), (40, 24, 40 * 24 * 4));
	}

	#[test]
	fn lossless_round_trips_every_pixel_including_under_transparency() {
		let pixels = fixture(33, 17, true);
		let bytes = encode(&quick(&JxlSettings { lossless: true, ..Default::default() }), Resize::default(), &RgbaImage { width: 33, height: 17, pixels: &pixels }, &mut |_| true).expect("encode");
		let (_, _, rgba) = decode(&bytes).expect("decode");
		assert_eq!(rgba, pixels);
	}

	#[test]
	fn an_opaque_image_carries_no_alpha_channel() {
		let pixels = fixture(16, 16, false);
		let bytes = encode(&quick(&JxlSettings::default()), Resize::default(), &RgbaImage { width: 16, height: 16, pixels: &pixels }, &mut |_| true).expect("encode");
		let image = jxl_oxide::JxlImage::builder().read(bytes.as_slice()).expect("parse");
		assert!(!image.pixel_format().has_alpha());
	}

	#[test]
	fn lower_quality_is_smaller() {
		let pixels = fixture(64, 64, false);
		let size = |quality| encode(&quick(&JxlSettings { quality, ..Default::default() }), Resize::default(), &RgbaImage { width: 64, height: 64, pixels: &pixels }, &mut |_| true).expect("encode").len();
		assert!(size(30) < size(95), "quality 30 should be smaller than quality 95");
	}

	#[test]
	fn applies_the_shared_resize() {
		let pixels = fixture(40, 24, false);
		let bytes = encode(&quick(&JxlSettings::default()), Resize { width: 20, height: 0, no_enlarge: false }, &RgbaImage { width: 40, height: 24, pixels: &pixels }, &mut |_| true).expect("encode");
		assert_eq!(dimensions(&bytes), Some((20, 12)));
	}

	#[test]
	fn a_cancel_returns_nothing() {
		let pixels = fixture(8, 8, false);
		let result = encode(&JxlSettings::default(), Resize::default(), &RgbaImage { width: 8, height: 8, pixels: &pixels }, &mut |_| false);
		assert_eq!(result, Err(crate::encoder::EncodeError::Cancelled));
	}

	#[test]
	fn a_recompressed_jpeg_rebuilds_the_original_bit_for_bit() {
		let pixels = fixture(48, 32, false);
		let rgb: Vec<u8> = pixels.as_chunks::<4>().0.iter().flat_map(|&[r, g, b, _]| [r, g, b]).collect();
		let mut jpeg = Vec::new();
		image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 85).encode(&rgb, 48, 32, image::ExtendedColorType::Rgb8).expect("write a JPEG");

		let jxl = recompress_jpeg(&quick(&JxlSettings::default()), &jpeg, &mut |_| true).expect("recompress").expect("libjxl takes a baseline JPEG");
		assert!(jxl.len() < jpeg.len(), "recompression should be smaller: {} vs {}", jxl.len(), jpeg.len());

		let image = jxl_oxide::JxlImage::builder().read(jxl.as_slice()).expect("parse");
		let mut rebuilt = Vec::new();
		image.reconstruct_jpeg(&mut rebuilt).expect("reconstruct the JPEG");
		assert_eq!(rebuilt, jpeg, "the reconstructed JPEG must be the original, byte for byte");
	}

	#[test]
	fn out_of_range_settings_are_refused() {
		let pixels = fixture(8, 8, false);
		let image = RgbaImage { width: 8, height: 8, pixels: &pixels };
		assert!(encode(&JxlSettings { effort: 0, ..Default::default() }, Resize::default(), &image, &mut |_| true).is_err());
		assert!(encode(&JxlSettings { effort: 11, ..Default::default() }, Resize::default(), &image, &mut |_| true).is_err());
		assert!(encode(&JxlSettings { quality: 101, ..Default::default() }, Resize::default(), &image, &mut |_| true).is_err());
	}
}
