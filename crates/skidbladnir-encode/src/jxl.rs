//! JPEG XL output, through libjxl, the reference encoder — driven the way `cjxl` drives it.
//!
//! libjxl is bound here directly rather than through `jpegxl-rs`/`jpegxl-sys`, because
//! those wrappers are GPL-3.0 and Skidbladnir is ISC; libjxl itself is BSD-3. The
//! declarations below are written from libjxl 0.12's headers, field for field. Every struct
//! is `repr(C)` with the header's own field order, and `JXL_BOOL` and every C enum are
//! `c_int`.
//!
//! # What "the way `cjxl` drives it" means
//!
//! `cjxl` is more than a flag parser. Between the file and the encoder it builds a
//! `PackedPixelFile` — the samples at the PNG's own depth, its channels, its colour from
//! `cICP`/`iCCP`/`sRGB`/`gAMA`/`cHRM`, its Exif, XMP and orientation — and after the flags
//! it decides the container and the metadata boxes. Each of those changes the bytes, so
//! each is reproduced here from `tools/cjxl_main.cc`, `lib/extras/dec/apng.cc`,
//! `lib/extras/dec/jpg.cc` and `lib/extras/enc/jxl.cc`:
//!
//! - [`Input::from_source`] is the decoders' part: [`Input::png`], [`Input::jpeg`] and [`Input::pnm`].
//! - [`frame_options`] is `ProcessFlags`: the option list, with `cjxl`'s defaults.
//! - [`encode_pixels`] is `EncodeImageJXL`, call for call.
//! - [`recompress_jpeg`] is the lossless-JPEG path (`-j 1`).
//!
//! `tests/jxl_parity.rs` holds it to that: the same file and flags through `cjxl` and
//! through here, byte for byte.
//!
//! Like AVIF, libjxl has no progress callback, so progress moves at the start and the end
//! of a file, and a cancel mid-encode is honoured by discarding the result.

use std::{
	ffi::{c_int, c_void}, ptr
};

use crate::{
	encoder::{EncodeError, transformed}, metadata::{jpeg_icc, jpeg_markers, libpng_iccp, png_chunks, png_text}, settings::{EncodeJob, JxlColorSpace, JxlSettings, JxlTarget, MetadataSource, Primaries, RenderingIntent, TransferFunction, Tristate, WhitePoint}, source::{SourceFormat, SourceImage}
};

#[repr(C)]
#[derive(Clone, Copy)]
struct JxlPreviewHeader {
	xsize: u32,
	ysize: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct JxlAnimationHeader {
	tps_numerator: u32,
	tps_denominator: u32,
	num_loops: u32,
	have_timecodes: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
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
#[derive(Clone, Copy, Debug, PartialEq)]
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

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct JxlBlendInfo {
	blendmode: c_int,
	source: u32,
	alpha: u32,
	clamp: c_int,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct JxlLayerInfo {
	have_crop: c_int,
	crop_x0: i32,
	crop_y0: i32,
	xsize: u32,
	ysize: u32,
	blend_info: JxlBlendInfo,
	save_as_reference: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct JxlFrameHeader {
	duration: u32,
	timecode: u32,
	name_length: u32,
	is_last: c_int,
	layer_info: JxlLayerInfo,
}

#[repr(C)]
struct JxlBitDepth {
	kind: c_int,
	bits_per_sample: u32,
	exponent_bits_per_sample: u32,
}

#[repr(C)]
struct JxlEncoderOutputProcessor {
	opaque: *mut c_void,
	get_buffer: unsafe extern "C" fn(opaque: *mut c_void, size: *mut usize) -> *mut c_void,
	release_buffer: unsafe extern "C" fn(opaque: *mut c_void, written_bytes: usize),
	seek: unsafe extern "C" fn(opaque: *mut c_void, position: u64),
	set_finalized_position: unsafe extern "C" fn(opaque: *mut c_void, finalized_position: u64),
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
const JXL_CHANNEL_ALPHA: c_int = 0;
const JXL_ORIENT_IDENTITY: c_int = 1;
const JXL_TYPE_FLOAT: c_int = 0;
const JXL_TYPE_UINT8: c_int = 2;
const JXL_TYPE_UINT16: c_int = 3;
const JXL_NATIVE_ENDIAN: c_int = 0;
const JXL_TRUE: c_int = 1;
const JXL_FALSE: c_int = 0;
const JXL_BLEND_REPLACE: c_int = 0;
const JXL_BIT_DEPTH_FROM_PIXEL_FORMAT: c_int = 0;
const JXL_BIT_DEPTH_FROM_CODESTREAM: c_int = 1;

const JXL_COLOR_SPACE_RGB: c_int = 0;
const JXL_COLOR_SPACE_GRAY: c_int = 1;
const JXL_WHITE_POINT_D65: c_int = 1;
const JXL_WHITE_POINT_CUSTOM: c_int = 2;
const JXL_WHITE_POINT_E: c_int = 10;
const JXL_WHITE_POINT_DCI: c_int = 11;
const JXL_PRIMARIES_SRGB: c_int = 1;
const JXL_PRIMARIES_CUSTOM: c_int = 2;
const JXL_PRIMARIES_2100: c_int = 9;
const JXL_PRIMARIES_P3: c_int = 11;
const JXL_TRANSFER_FUNCTION_709: c_int = 1;
const JXL_TRANSFER_FUNCTION_LINEAR: c_int = 8;
const JXL_TRANSFER_FUNCTION_SRGB: c_int = 13;
const JXL_TRANSFER_FUNCTION_PQ: c_int = 16;
const JXL_TRANSFER_FUNCTION_DCI: c_int = 17;
const JXL_TRANSFER_FUNCTION_HLG: c_int = 18;
const JXL_TRANSFER_FUNCTION_GAMMA: c_int = 65535;
const JXL_RENDERING_INTENT_PERCEPTUAL: c_int = 0;
const JXL_RENDERING_INTENT_RELATIVE: c_int = 1;

/// `JxlEncoderFrameSettingId`, the options `cjxl` sets.
mod option {
	use std::ffi::c_int;
	pub const EFFORT: c_int = 0;
	pub const DECODING_SPEED: c_int = 1;
	pub const RESAMPLING: c_int = 2;
	pub const EXTRA_CHANNEL_RESAMPLING: c_int = 3;
	pub const ALREADY_DOWNSAMPLED: c_int = 4;
	pub const PHOTON_NOISE: c_int = 5;
	pub const NOISE: c_int = 6;
	pub const DOTS: c_int = 7;
	pub const PATCHES: c_int = 8;
	pub const EPF: c_int = 9;
	pub const GABORISH: c_int = 10;
	pub const MODULAR: c_int = 11;
	pub const KEEP_INVISIBLE: c_int = 12;
	pub const GROUP_ORDER: c_int = 13;
	pub const GROUP_ORDER_CENTER_X: c_int = 14;
	pub const GROUP_ORDER_CENTER_Y: c_int = 15;
	pub const RESPONSIVE: c_int = 16;
	pub const PROGRESSIVE_AC: c_int = 17;
	pub const QPROGRESSIVE_AC: c_int = 18;
	pub const PROGRESSIVE_DC: c_int = 19;
	pub const CHANNEL_COLORS_GLOBAL_PERCENT: c_int = 20;
	pub const CHANNEL_COLORS_GROUP_PERCENT: c_int = 21;
	pub const PALETTE_COLORS: c_int = 22;
	pub const LOSSY_PALETTE: c_int = 23;
	pub const MODULAR_COLOR_SPACE: c_int = 25;
	pub const MODULAR_GROUP_SIZE: c_int = 26;
	pub const MODULAR_PREDICTOR: c_int = 27;
	pub const MODULAR_MA_TREE_LEARNING_PERCENT: c_int = 28;
	pub const MODULAR_NB_PREV_CHANNELS: c_int = 29;
	pub const JPEG_RECON_CFL: c_int = 30;
	pub const FRAME_INDEX_BOX: c_int = 31;
	pub const BROTLI_EFFORT: c_int = 32;
	pub const JPEG_COMPRESS_BOXES: c_int = 33;
	pub const BUFFERING: c_int = 34;
	pub const JPEG_KEEP_EXIF: c_int = 35;
	pub const JPEG_KEEP_XMP: c_int = 36;
	pub const JPEG_KEEP_JUMBF: c_int = 37;
	pub const DISABLE_PERCEPTUAL_HEURISTICS: c_int = 39;
	pub const OUTPUT_MODE: c_int = 40;
}

unsafe extern "C" {
	fn JxlEncoderVersion() -> u32;
	fn JxlEncoderCreate(memory_manager: *const c_void) -> *mut JxlEncoder;
	fn JxlEncoderDestroy(enc: *mut JxlEncoder);
	fn JxlEncoderGetError(enc: *mut JxlEncoder) -> c_int;
	fn JxlEncoderSetParallelRunner(enc: *mut JxlEncoder, parallel_runner: ParallelRunner, parallel_runner_opaque: *mut c_void) -> c_int;
	fn JxlEncoderAllowExpertOptions(enc: *mut JxlEncoder);
	fn JxlEncoderSetBasicInfo(enc: *mut JxlEncoder, info: *const JxlBasicInfo) -> c_int;
	fn JxlEncoderSetColorEncoding(enc: *mut JxlEncoder, color: *const JxlColorEncoding) -> c_int;
	fn JxlEncoderSetICCProfile(enc: *mut JxlEncoder, icc_profile: *const u8, size: usize) -> c_int;
	fn JxlEncoderInitExtraChannelInfo(kind: c_int, info: *mut JxlExtraChannelInfo);
	fn JxlEncoderSetExtraChannelInfo(enc: *mut JxlEncoder, index: usize, info: *const JxlExtraChannelInfo) -> c_int;
	fn JxlEncoderSetExtraChannelBlendInfo(frame_settings: *mut JxlEncoderFrameSettings, index: usize, blend_info: *const JxlBlendInfo) -> c_int;
	fn JxlEncoderSetExtraChannelDistance(frame_settings: *mut JxlEncoderFrameSettings, index: usize, distance: f32) -> c_int;
	fn JxlEncoderFrameSettingsCreate(enc: *mut JxlEncoder, source: *const JxlEncoderFrameSettings) -> *mut JxlEncoderFrameSettings;
	fn JxlEncoderFrameSettingsSetOption(frame_settings: *mut JxlEncoderFrameSettings, option: c_int, value: i64) -> c_int;
	fn JxlEncoderFrameSettingsSetFloatOption(frame_settings: *mut JxlEncoderFrameSettings, option: c_int, value: f32) -> c_int;
	fn JxlEncoderSetFrameHeader(frame_settings: *mut JxlEncoderFrameSettings, frame_header: *const JxlFrameHeader) -> c_int;
	fn JxlEncoderSetFrameLossless(frame_settings: *mut JxlEncoderFrameSettings, lossless: c_int) -> c_int;
	fn JxlEncoderSetFrameDistance(frame_settings: *mut JxlEncoderFrameSettings, distance: f32) -> c_int;
	fn JxlEncoderSetFrameBitDepth(frame_settings: *mut JxlEncoderFrameSettings, bit_depth: *const JxlBitDepth) -> c_int;
	fn JxlEncoderDistanceFromQuality(quality: f32) -> f32;
	fn JxlEncoderSetCodestreamLevel(enc: *mut JxlEncoder, level: c_int) -> c_int;
	fn JxlEncoderSetUpsamplingMode(enc: *mut JxlEncoder, factor: i64, mode: i64) -> c_int;
	fn JxlEncoderUseContainer(enc: *mut JxlEncoder, use_container: c_int) -> c_int;
	fn JxlEncoderUseBoxes(enc: *mut JxlEncoder) -> c_int;
	fn JxlEncoderAddBox(enc: *mut JxlEncoder, kind: *const u8, contents: *const u8, size: usize, compress_box: c_int) -> c_int;
	fn JxlEncoderCloseBoxes(enc: *mut JxlEncoder);
	fn JxlEncoderStoreJPEGMetadata(enc: *mut JxlEncoder, store_jpeg_metadata: c_int) -> c_int;
	fn JxlEncoderAddJPEGFrame(frame_settings: *const JxlEncoderFrameSettings, buffer: *const u8, size: usize) -> c_int;
	fn JxlEncoderAddImageFrame(frame_settings: *const JxlEncoderFrameSettings, pixel_format: *const JxlPixelFormat, buffer: *const c_void, size: usize) -> c_int;
	fn JxlEncoderCloseInput(enc: *mut JxlEncoder);
	fn JxlEncoderProcessOutput(enc: *mut JxlEncoder, next_out: *mut *mut u8, avail_out: *mut usize) -> c_int;
	fn JxlEncoderSetOutputProcessor(enc: *mut JxlEncoder, output_processor: JxlEncoderOutputProcessor) -> c_int;
	fn JxlEncoderFlushInput(enc: *mut JxlEncoder) -> c_int;

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
	/// An encoder with `cjxl`'s thread pool: the machine default, or no workers for
	/// `--num_threads=0`.
	fn new(settings: &JxlSettings) -> Result<Self, EncodeError> {
		// SAFETY: a null memory manager selects libjxl's default allocator.
		let enc = unsafe { JxlEncoderCreate(ptr::null()) };
		if enc.is_null() {
			return Err(EncodeError::Jxl("libjxl could not create an encoder".to_owned()));
		}
		let mut encoder = Self { enc, runner: ptr::null_mut() };
		if settings.allow_expert_options {
			// SAFETY: `enc` is live.
			unsafe { JxlEncoderAllowExpertOptions(enc) };
		}
		// SAFETY: plain calls; the runner is owned by `encoder` from here on.
		let workers = if settings.multi_threading { unsafe { JxlThreadParallelRunnerDefaultNumWorkerThreads() } } else { 0 };
		encoder.runner = unsafe { JxlThreadParallelRunnerCreate(ptr::null(), workers) };
		if encoder.runner.is_null() {
			return Err(EncodeError::Jxl("libjxl could not start its thread pool".to_owned()));
		}
		// SAFETY: `JxlThreadParallelRunner` is the runner function libjxl pairs with the
		// opaque pointer `JxlThreadParallelRunnerCreate` returned.
		encoder.check("JxlEncoderSetParallelRunner", unsafe { JxlEncoderSetParallelRunner(enc, JxlThreadParallelRunner, encoder.runner) })?;
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

	/// A frame settings object with `options` applied, owned by the encoder.
	fn frame_settings(&self, options: &[(c_int, OptionValue)]) -> Result<*mut JxlEncoderFrameSettings, EncodeError> {
		// SAFETY: `self.enc` is live; a null source means libjxl's defaults.
		let settings = unsafe { JxlEncoderFrameSettingsCreate(self.enc, ptr::null()) };
		if settings.is_null() {
			return Err(EncodeError::Jxl("libjxl could not create frame settings".to_owned()));
		}
		for &(id, value) in options {
			// SAFETY: `settings` belongs to the live encoder.
			let status = unsafe {
				match value {
					OptionValue::Int(value) => JxlEncoderFrameSettingsSetOption(settings, id, value),
					OptionValue::Float(value) => JxlEncoderFrameSettingsSetFloatOption(settings, id, value),
				}
			};
			self.check(&format!("JxlEncoderFrameSettingsSetOption({id})"), status)?;
		}
		Ok(settings)
	}

	/// Close the input and drain the whole file, the way `cjxl`'s `ReadCompressedOutput`
	/// does, or through an output processor for `--streaming_output`.
	fn finish(&self, streaming: Option<&mut Streamed>) -> Result<Vec<u8>, EncodeError> {
		// SAFETY: `self.enc` is live and every frame has been added.
		unsafe { JxlEncoderCloseInput(self.enc) };
		if let Some(streamed) = streaming {
			// SAFETY: the processor was installed before any input, and `streamed` is the
			// live target of its callbacks.
			self.check("JxlEncoderFlushInput", unsafe { JxlEncoderFlushInput(self.enc) })?;
			return Ok(std::mem::take(&mut streamed.file));
		}
		let mut output = vec![0_u8; 4096];
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

	/// Send the output through an output processor writing into `streamed`, as
	/// `cjxl --streaming_output` does into its file.
	fn stream_into(&self, streamed: &mut Streamed) -> Result<(), EncodeError> {
		let processor = JxlEncoderOutputProcessor { opaque: ptr::from_mut(streamed).cast(), get_buffer: streamed_get_buffer, release_buffer: streamed_release_buffer, seek: streamed_seek, set_finalized_position: streamed_set_finalized_position };
		// SAFETY: `streamed` outlives every use of the processor: the encoder is flushed in
		// `finish` while the caller still holds it.
		self.check("JxlEncoderSetOutputProcessor", unsafe { JxlEncoderSetOutputProcessor(self.enc, processor) })
	}
}

/// The file `--streaming_output` writes: a seekable byte buffer, handed out 64 KiB at a
/// time as `cjxl`'s `JxlOutputProcessor` does.
#[derive(Default)]
struct Streamed {
	file: Vec<u8>,
	position: usize,
	buffer: Vec<u8>,
}

unsafe extern "C" fn streamed_get_buffer(opaque: *mut c_void, size: *mut usize) -> *mut c_void {
	// SAFETY: `opaque` is the `Streamed` installed by `stream_into`, live for the encode,
	// and `size` is libjxl's valid out-parameter.
	let (streamed, size) = unsafe { (&mut *opaque.cast::<Streamed>(), &mut *size) };
	*size = (*size).min(1 << 16);
	if streamed.buffer.len() < *size {
		streamed.buffer.resize(*size, 0);
	}
	streamed.buffer.as_mut_ptr().cast()
}

unsafe extern "C" fn streamed_release_buffer(opaque: *mut c_void, written_bytes: usize) {
	// SAFETY: as `streamed_get_buffer`.
	let streamed = unsafe { &mut *opaque.cast::<Streamed>() };
	let end = streamed.position + written_bytes;
	if streamed.file.len() < end {
		streamed.file.resize(end, 0);
	}
	streamed.file[streamed.position..end].copy_from_slice(&streamed.buffer[..written_bytes]);
	streamed.position = end;
	streamed.buffer.clear();
}

unsafe extern "C" fn streamed_seek(opaque: *mut c_void, position: u64) {
	// SAFETY: as `streamed_get_buffer`.
	let streamed = unsafe { &mut *opaque.cast::<Streamed>() };
	streamed.position = usize::try_from(position).unwrap_or(usize::MAX);
}

unsafe extern "C" fn streamed_set_finalized_position(_opaque: *mut c_void, _finalized_position: u64) {}

/// A frame option's value, which libjxl takes through one of two setters.
#[derive(Clone, Copy, Debug, PartialEq)]
enum OptionValue {
	Int(i64),
	Float(f32),
}

/// `cjxl`'s `ProcessFlags`: the frame options, with its defaults, after `-p` has rewritten
/// the options it implies. `jpeg` is whether the frame is a recompressed JPEG, which adds
/// the two JPEG-only options.
fn frame_options(s: &JxlSettings, jpeg: bool) -> Vec<(c_int, OptionValue)> {
	use OptionValue::{Float, Int};
	let mut options = Vec::new();
	let flag = |options: &mut Vec<_>, value: Tristate, id| {
		if let Some(value) = value.as_option() {
			options.push((id, Int(value)));
		}
	};
	let patches = if s.progressive && s.patches == Tristate::Default { Tristate::Off } else { s.patches };
	flag(&mut options, s.modular, option::MODULAR);
	flag(&mut options, s.keep_invisible, option::KEEP_INVISIBLE);
	flag(&mut options, s.dots, option::DOTS);
	flag(&mut options, patches, option::PATCHES);
	flag(&mut options, s.gaborish, option::GABORISH);
	flag(&mut options, s.effective_group_order(), option::GROUP_ORDER);
	flag(&mut options, s.noise, option::NOISE);
	if s.disable_perceptual_optimizations {
		options.push((option::DISABLE_PERCEPTUAL_HEURISTICS, Int(1)));
	}
	options.extend([(option::EFFORT, Int(i64::from(s.effort))), (option::BROTLI_EFFORT, Int(i64::from(s.brotli_effort))), (option::BUFFERING, Int(i64::from(s.buffering))), (option::EPF, Int(i64::from(s.epf))), (option::DECODING_SPEED, Int(i64::from(s.faster_decoding))), (option::RESAMPLING, Int(i64::from(s.resampling))), (option::EXTRA_CHANNEL_RESAMPLING, Int(i64::from(s.ec_resampling))), (option::PHOTON_NOISE, Float(s.photon_noise_iso)), (option::ALREADY_DOWNSAMPLED, Int(i64::from(s.already_downsampled))), (option::GROUP_ORDER_CENTER_X, Int(s.center_x)), (option::GROUP_ORDER_CENTER_Y, Int(s.center_y)), (option::PROGRESSIVE_DC, Int(i64::from(if s.progressive && s.progressive_dc == -1 { 1 } else { s.progressive_dc }))), (option::PROGRESSIVE_AC, Int(i64::from(s.progressive_ac || s.progressive)))]);
	match (s.responsive, s.progressive) {
		(Some(responsive), _) => options.push((option::RESPONSIVE, Int(i64::from(responsive)))),
		(None, true) => options.push((option::RESPONSIVE, Int(1))),
		(None, false) => {}
	}
	if s.qprogressive_ac {
		options.push((option::QPROGRESSIVE_AC, Int(1)));
	}
	// `--modular_lossy_palette` is dropped in progressive mode, with a warning.
	let lossy_palette = s.modular_lossy_palette && !(s.progressive || s.qprogressive_ac);
	options.extend([(option::MODULAR_GROUP_SIZE, Int(i64::from(s.modular_group_size))), (option::MODULAR_PREDICTOR, Int(i64::from(s.modular_predictor))), (option::MODULAR_COLOR_SPACE, Int(i64::from(s.modular_colorspace))), (option::MODULAR_MA_TREE_LEARNING_PERCENT, Float(s.iterations)), (option::MODULAR_NB_PREV_CHANNELS, Int(i64::from(s.modular_nb_prev_channels))), (option::LOSSY_PALETTE, Int(i64::from(lossy_palette))), (option::PALETTE_COLORS, Int(s.modular_palette_colors)), (option::CHANNEL_COLORS_GLOBAL_PERCENT, Float(s.pre_compact)), (option::CHANNEL_COLORS_GROUP_PERCENT, Float(s.post_compact))]);
	if jpeg {
		flag(&mut options, s.jpeg_reconstruction_cfl, option::JPEG_RECON_CFL);
		flag(&mut options, s.compress_boxes, option::JPEG_COMPRESS_BOXES);
	}
	if s.frame_index_box {
		options.push((option::FRAME_INDEX_BOX, Int(1)));
	}
	options.push((option::OUTPUT_MODE, Int(i64::from(s.output_mode))));
	options
}

/// The distance a target asks for, where `lossy_source` is whether `cjxl` would have
/// decoded the input from a format it counts as lossy (a JPEG decoded to pixels, or a GIF),
/// which makes its default lossless.
fn distance(target: JxlTarget, lossy_source: bool) -> f32 {
	match target {
		JxlTarget::Default => {
			if lossy_source {
				0.0
			} else {
				1.0
			}
		}
		JxlTarget::Distance(distance) => distance,
		// SAFETY: a pure function of its argument.
		JxlTarget::Quality(quality) => unsafe { JxlEncoderDistanceFromQuality(quality) },
	}
}

/// Pixels in the layout libjxl takes them.
enum Samples {
	U8(Vec<u8>),
	U16(Vec<u16>),
	F32(Vec<f32>),
}

/// How the image's colour is described: an ICC profile, or an enumerated encoding.
#[derive(Clone, Debug, PartialEq)]
enum Colour {
	Icc(Vec<u8>),
	Encoding(JxlColorEncoding),
}

/// `cjxl`'s `PackedPixelFile` for one still image: what its decoders make of the file.
struct Input {
	width: u32,
	height: u32,
	bits_per_sample: u32,
	/// Nonzero for floating-point samples: 8 for 32-bit floats.
	exponent_bits: u32,
	num_color_channels: u32,
	alpha_bits: u32,
	samples: Samples,
	colour: Colour,
	/// Whether the decoder found colour information, which is what decides whether the
	/// `-x color_space` and `-x icc_pathname` hints may apply.
	colour_given: bool,
	/// Whether hints about colour are ignored altogether, as they are for JPEG.
	colour_hints_ignored: bool,
	intensity_target: f32,
	exif: Vec<u8>,
	xmp: Vec<u8>,
	jumbf: Vec<u8>,
	frame: JxlFrameHeader,
	/// Whether this came from a lossy format decoded to pixels, which makes `cjxl`'s
	/// default distance 0.
	lossy_source: bool,
	/// How the samples' range is read: spanning their type (`JXL_BIT_DEPTH_FROM_PIXEL_FORMAT`),
	/// or at the codestream's bit depth (`JXL_BIT_DEPTH_FROM_CODESTREAM`), as `cjxl`'s PNM
	/// reader hands them over.
	bit_depth: c_int,
}

/// The colour `cjxl`'s PNG decoder starts from: sRGB, relative intent.
fn srgb(gray: bool, intent: c_int) -> JxlColorEncoding {
	JxlColorEncoding { color_space: if gray { JXL_COLOR_SPACE_GRAY } else { JXL_COLOR_SPACE_RGB }, white_point: JXL_WHITE_POINT_D65, white_point_xy: [0.0; 2], primaries: JXL_PRIMARIES_SRGB, primaries_red_xy: [0.0; 2], primaries_green_xy: [0.0; 2], primaries_blue_xy: [0.0; 2], transfer_function: JXL_TRANSFER_FUNCTION_SRGB, gamma: 0.0, rendering_intent: intent }
}

/// A big-endian `u32` from four bytes, as a PNG stores its fixed-point numbers, scaled by
/// 1e-5 through a signed conversion as libjxl's `F64FromU32` does.
fn png_fixed(bytes: &[u8]) -> f64 {
	f64::from(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])) * 1e-5
}

impl Input {
	/// Build the image `cjxl` would have built from this source.
	fn from_source(source: &SourceImage) -> Self {
		match source.format {
			// Pixels with no file behind them have no header to read.
			_ if source.bytes.is_empty() => Self::other(source),
			SourceFormat::Png => Self::png(source),
			SourceFormat::Jpeg => Self::jpeg(source),
			SourceFormat::Pnm => Self::pnm(source),
			SourceFormat::Pfm => Self::pfm(source),
			SourceFormat::Gif => Self::gif(source),
			_ => Self::other(source),
		}
	}

	/// The pixels at the depth and channel count given, in libjxl's layout.
	fn samples(source: &SourceImage, gray: bool, alpha: bool, deep: bool) -> Samples {
		let channels: &[usize] = match (gray, alpha) {
			(true, false) => &[0],
			(true, true) => &[0, 3],
			(false, false) => &[0, 1, 2],
			(false, true) => &[0, 1, 2, 3],
		};
		match (&source.deep, deep) {
			(Some(wide), true) => Samples::U16(wide.as_chunks::<4>().0.iter().flat_map(|pixel| channels.iter().map(move |&c| pixel[c])).collect()),
			_ => Samples::U8(source.pixels.as_chunks::<4>().0.iter().flat_map(|pixel| channels.iter().map(move |&c| pixel[c])).collect()),
		}
	}

	/// `lib/extras/dec/apng.cc` for a still PNG.
	fn png(source: &SourceImage) -> Self {
		let chunks = png_chunks(&source.bytes).unwrap_or_default();
		let ihdr = chunks.iter().find(|(kind, _)| kind == b"IHDR").map_or(&[][..], |(_, data)| *data);
		let (bit_depth, colour_type) = (ihdr.get(8).copied().unwrap_or(8), ihdr.get(9).copied().unwrap_or(6));
		let colour = colour_type & 2 != 0;
		let trns = chunks.iter().any(|(kind, _)| kind == b"tRNS");
		let alpha = colour_type & 4 != 0 || trns;

		// SetColorData: the IHDR depth, 8 for a palette, or sBIT, which overrides it
		// without the samples being shifted.
		let mut bits = u32::from(if colour_type & 1 != 0 { 8 } else { bit_depth });
		if let Some((_, sbit)) = chunks.iter().find(|(kind, _)| kind == b"sBIT")
			&& sbit_valid(sbit, colour_type, bit_depth)
		{
			bits = u32::from(if colour { if sbit[0] == sbit[1] && sbit[1] == sbit[2] { sbit[0] } else { sbit[0].max(sbit[1]).max(sbit[2]) } } else { sbit[0] });
		}

		// The colour chunks, in file order, a higher-priority one shutting out the rest.
		let mut given = Given::None;
		let mut encoding = srgb(!colour, JXL_RENDERING_INTENT_RELATIVE);
		let mut icc: Option<Vec<u8>> = None;
		let mut max_cll = 0.0_f32;
		let mut exif = Vec::new();
		for (kind, data) in &chunks {
			match kind {
				b"cICP" if given != Given::Cicp => {
					if let Some(cicp) = cicp_encoding(data, encoding) {
						encoding = cicp;
						icc = None;
						given = Given::Cicp;
					}
				}
				b"iCCP" if given < Given::Iccp => {
					if let Some(profile) = libpng_iccp(data, colour) {
						icc = Some(profile);
						given = Given::Iccp;
					}
				}
				b"sRGB" if given < Given::Srgb && data.len() == 1 && data[0] < 4 => {
					encoding.white_point = JXL_WHITE_POINT_D65;
					encoding.primaries = JXL_PRIMARIES_SRGB;
					encoding.transfer_function = JXL_TRANSFER_FUNCTION_SRGB;
					encoding.rendering_intent = c_int::from(data[0]);
					given = Given::Srgb;
				}
				b"gAMA" if !matches!(given, Given::Gama | Given::GamaAndChrm) && given <= Given::GamaAndChrm && data.len() == 4 => {
					encoding.transfer_function = JXL_TRANSFER_FUNCTION_GAMMA;
					encoding.gamma = png_fixed(data);
					given = if given == Given::Chrm { Given::GamaAndChrm } else { Given::Gama };
				}
				b"cHRM" if !matches!(given, Given::Chrm | Given::GamaAndChrm) && given <= Given::GamaAndChrm && data.len() == 32 => {
					encoding.white_point = JXL_WHITE_POINT_CUSTOM;
					encoding.white_point_xy = [png_fixed(&data[0..]), png_fixed(&data[4..])];
					encoding.primaries = JXL_PRIMARIES_CUSTOM;
					encoding.primaries_red_xy = [png_fixed(&data[8..]), png_fixed(&data[12..])];
					encoding.primaries_green_xy = [png_fixed(&data[16..]), png_fixed(&data[20..])];
					encoding.primaries_blue_xy = [png_fixed(&data[24..]), png_fixed(&data[28..])];
					given = if given == Given::Gama { Given::GamaAndChrm } else { Given::Chrm };
				}
				b"cLLi" if data.len() == 8 => {
					max_cll = clli_nits(u32::from_be_bytes([data[0], data[1], data[2], data[3]]));
				}
				b"eXIf" => exif = data.to_vec(),
				_ => {}
			}
		}

		// The text chunks, in libpng's order, after the image.
		let mut xmp = Vec::new();
		for (keyword, text) in chunks.iter().filter_map(|(kind, data)| png_text(*kind, data)) {
			if keyword.contains("XML:com.adobe.xmp") {
				xmp = text.split(|&b| b == 0).next().unwrap_or_default().to_vec();
			}
			if let Some(kind) = keyword.strip_prefix("Raw profile type ")
				&& kind.len() <= 20
				&& let Some(bytes) = cjxl_raw_profile(&text)
			{
				match kind {
					"exif" => exif = bytes.strip_prefix(b"Exif\0\0").map_or(bytes.clone(), <[u8]>::to_vec),
					"xmp" => xmp = bytes,
					_ => {}
				}
			}
		}

		let colour_given = given != Given::None;
		let intensity_target = if encoding.transfer_function == JXL_TRANSFER_FUNCTION_PQ { max_cll } else { 0.0 };
		let frame = JxlFrameHeader { duration: 100, timecode: 0, name_length: 0, is_last: JXL_TRUE, layer_info: JxlLayerInfo { have_crop: JXL_FALSE, crop_x0: 0, crop_y0: 0, xsize: source.width, ysize: source.height, blend_info: JxlBlendInfo { blendmode: JXL_BLEND_REPLACE, source: 1, alpha: 0, clamp: JXL_FALSE }, save_as_reference: 1 } };
		Self { width: source.width, height: source.height, bits_per_sample: bits, exponent_bits: 0, num_color_channels: if colour { 3 } else { 1 }, alpha_bits: if alpha { bits } else { 0 }, samples: Self::samples(source, !colour, alpha, bit_depth > 8), colour: icc.map_or(Colour::Encoding(encoding), Colour::Icc), colour_given, colour_hints_ignored: false, intensity_target, exif, xmp, jumbf: Vec::new(), frame, lossy_source: false, bit_depth: JXL_BIT_DEPTH_FROM_PIXEL_FORMAT }
	}

	/// `lib/extras/dec/jpg.cc`: a JPEG decoded to 8-bit pixels (`-j 0`).
	fn jpeg(source: &SourceImage) -> Self {
		let markers = jpeg_markers(&source.bytes);
		let exif = markers.iter().find(|(marker, data)| *marker == 0xe1 && data.len() >= 8 && data.starts_with(b"Exif\0\0")).map_or_else(Vec::new, |(_, data)| data[6..].to_vec());
		let colour = jpeg_icc(&markers).map_or_else(|| Colour::Encoding(srgb(source.gray, JXL_RENDERING_INTENT_PERCEPTUAL)), Colour::Icc);
		Self { width: source.width, height: source.height, bits_per_sample: 8, exponent_bits: 0, num_color_channels: if source.gray { 1 } else { 3 }, alpha_bits: 0, samples: Self::samples(source, source.gray, false, false), colour, colour_given: true, colour_hints_ignored: true, intensity_target: 0.0, exif, xmp: Vec::new(), jumbf: Vec::new(), frame: JxlFrameHeader::default(), lossy_source: true, bit_depth: JXL_BIT_DEPTH_FROM_PIXEL_FORMAT }
	}

	/// `lib/extras/dec/pnm.cc`: the samples as stored, at `log2(maxval + 1)` bits and read
	/// at that depth, in sRGB with the perceptual intent.
	/// `cjxl` refuses a maximum that is not one less than a power of two, and a cropped or
	/// resized image is no longer the file's samples; both are encoded as [`Self::other`]
	/// encodes a format `cjxl` does not read.
	fn pnm(source: &SourceImage) -> Self {
		let header = match crate::pnm::header(&source.bytes) {
			Ok(header) if (header.max_value + 1).is_power_of_two() && (header.width, header.height) == (source.width, source.height) => header,
			_ => return Self::other(source),
		};
		let bits = (header.max_value + 1).trailing_zeros();
		let raw = crate::pnm::raw_samples(&source.bytes, &header);
		let samples = if bits > 8 { Samples::U16(raw) } else { Samples::U8(raw.into_iter().map(|v| u8::try_from(v).unwrap_or(u8::MAX)).collect()) };
		let gray = header.gray();
		Self { width: source.width, height: source.height, bits_per_sample: bits, exponent_bits: 0, num_color_channels: if gray { 1 } else { 3 }, alpha_bits: if header.alpha() { bits } else { 0 }, samples, colour: Colour::Encoding(srgb(gray, JXL_RENDERING_INTENT_PERCEPTUAL)), colour_given: false, colour_hints_ignored: false, intensity_target: 0.0, exif: Vec::new(), xmp: Vec::new(), jumbf: Vec::new(), frame: JxlFrameHeader::default(), lossy_source: false, bit_depth: JXL_BIT_DEPTH_FROM_CODESTREAM }
	}

	/// `lib/extras/dec/pnm.cc` for a PFM: 32-bit floats as they are stored, top row first,
	/// in sRGB with the perceptual intent. A cropped or resized image is no longer the
	/// file's samples, and is encoded from its 16-bit reduction as [`Self::other`].
	fn pfm(source: &SourceImage) -> Self {
		let image = match crate::pnm::pfm(&source.bytes) {
			Ok(image) if (image.width, image.height) == (source.width, source.height) => image,
			_ => return Self::other(source),
		};
		Self { width: source.width, height: source.height, bits_per_sample: 32, exponent_bits: 8, num_color_channels: if image.gray { 1 } else { 3 }, alpha_bits: 0, samples: Samples::F32(image.samples), colour: Colour::Encoding(srgb(image.gray, JXL_RENDERING_INTENT_PERCEPTUAL)), colour_given: false, colour_hints_ignored: false, intensity_target: 0.0, exif: Vec::new(), xmp: Vec::new(), jumbf: Vec::new(), frame: JxlFrameHeader::default(), lossy_source: false, bit_depth: JXL_BIT_DEPTH_FROM_PIXEL_FORMAT }
	}

	/// `lib/extras/dec/gif.cc` for a still GIF: always three colour channels, alpha only
	/// when a pixel is transparent (the transparent index reads as 0, 0, 0, 0, as it does
	/// for `gif2webp`, which is how the pixels were decoded), sRGB with the perceptual
	/// intent, and — `cjxl` counting GIF a lossy input, like JPEG — lossless by default.
	fn gif(source: &SourceImage) -> Self {
		Self { colour: Colour::Encoding(srgb(false, JXL_RENDERING_INTENT_PERCEPTUAL)), colour_given: false, lossy_source: true, ..Self::other(source) }
	}

	/// A format `cjxl` does not read (TIFF, WebP, AVIF, HEIC, or JPEG XL decoded to
	/// pixels): the samples at their own depth, the source's ICC profile if it has one,
	/// and otherwise sRGB, which is what every decoder here produces.
	fn other(source: &SourceImage) -> Self {
		let metadata = crate::metadata::Metadata::read_like_cwebp(&source.bytes, source.format);
		let deep = source.deep.is_some();
		let bits = if deep { 16 } else { 8 };
		let colour_given = metadata.icc.is_some();
		let colour = metadata.icc.map_or_else(|| Colour::Encoding(srgb(source.gray, JXL_RENDERING_INTENT_RELATIVE)), Colour::Icc);
		let frame = JxlFrameHeader::default();
		Self { width: source.width, height: source.height, bits_per_sample: bits, exponent_bits: 0, num_color_channels: if source.gray { 1 } else { 3 }, alpha_bits: if source.has_alpha { bits } else { 0 }, samples: Self::samples(source, source.gray, source.has_alpha, deep), colour, colour_given, colour_hints_ignored: false, intensity_target: 0.0, exif: metadata.exif.unwrap_or_default(), xmp: metadata.xmp.unwrap_or_default(), jumbf: Vec::new(), frame, lossy_source: false, bit_depth: JXL_BIT_DEPTH_FROM_PIXEL_FORMAT }
	}

	/// `ApplyColorHints`: the `-x` hints, in the order `cjxl` applies them.
	fn apply_hints(&mut self, settings: &JxlSettings) -> Result<(), EncodeError> {
		if !self.colour_hints_ignored && !self.colour_given {
			if let Some(space) = settings.color_space {
				if space.gray != (self.num_color_channels == 1) {
					return Err(EncodeError::Jxl(format!("the colour space `{}` is {} but the image is not", space.description(), if space.gray { "grayscale" } else { "colour" })));
				}
				// `cjxl` replaces the encoding but leaves an ICC profile primary if one was
				// set, which for an untagged image it never was.
				if let Colour::Encoding(_) = self.colour {
					self.colour = Colour::Encoding(encoding_of(&space));
				}
			}
			if let Some(path) = &settings.icc_file {
				let profile = std::fs::read(path).map_err(|error| EncodeError::Jxl(format!("could not read the ICC profile `{}`: {error}", path.display())))?;
				self.colour = Colour::Icc(profile);
			}
		}
		for (source, slot) in [(&settings.exif, &mut self.exif), (&settings.xmp, &mut self.xmp), (&settings.jumbf, &mut self.jumbf)] {
			match source {
				MetadataSource::Keep => {}
				MetadataSource::Strip => slot.clear(),
				MetadataSource::File(path) => *slot = std::fs::read(path).map_err(|error| EncodeError::Jxl(format!("could not read `{}`: {error}", path.display())))?,
			}
		}
		Ok(())
	}
}

/// Which PNG colour chunks `cjxl`'s decoder has taken, in its priority order: a chunk of
/// lower priority than one already taken is ignored.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Given {
	None,
	Gama,
	Chrm,
	GamaAndChrm,
	Srgb,
	Iccp,
	Cicp,
}

/// libpng's `png_handle_sBIT` checks: one byte per channel, each within `1..=depth`
/// (8 for a palette).
fn sbit_valid(sbit: &[u8], colour_type: u8, bit_depth: u8) -> bool {
	let channels = match colour_type {
		0 => 1,
		2 | 3 => 3,
		4 => 2,
		6 => 4,
		_ => return false,
	};
	let depth = if colour_type == 3 { 8 } else { bit_depth };
	sbit.len() == channels && sbit.iter().all(|&bits| bits >= 1 && bits <= depth)
}

/// `DecodeClliChunk`: `MaxCLL` in nits, from the chunk's units of 0.0001 nit, through the
/// same single-precision arithmetic.
#[expect(clippy::cast_precision_loss, reason = "libjxl converts the clamped u32 to float the same way; matching its rounding is the point")]
fn clli_nits(max_cll: u32) -> f32 {
	max_cll.min(10_000 * 10_000) as f32 / 10_000.0
}

/// `DecodeCicpChunk`: the `cICP` chunk's encoding, or `None` if libjxl does not support it
/// (which fails `cjxl`'s decode; here it is ignored).
fn cicp_encoding(data: &[u8], base: JxlColorEncoding) -> Option<JxlColorEncoding> {
	let &[primaries, transfer, matrix, full_range] = data else { return None };
	let mut c = base;
	let custom = |c: &mut JxlColorEncoding, xy: [f64; 6]| {
		c.primaries = JXL_PRIMARIES_CUSTOM;
		(c.primaries_red_xy, c.primaries_green_xy, c.primaries_blue_xy) = ([xy[0], xy[1]], [xy[2], xy[3]], [xy[4], xy[5]]);
	};
	match primaries {
		1 => (c.primaries, c.white_point) = (JXL_PRIMARIES_SRGB, JXL_WHITE_POINT_D65),
		4 => {
			custom(&mut c, [0.67, 0.33, 0.21, 0.71, 0.14, 0.08]);
			(c.white_point, c.white_point_xy) = (JXL_WHITE_POINT_CUSTOM, [0.310, 0.316]);
		}
		5 => {
			custom(&mut c, [0.64, 0.33, 0.29, 0.60, 0.15, 0.06]);
			c.white_point = JXL_WHITE_POINT_D65;
		}
		6 | 7 => {
			custom(&mut c, [0.630, 0.340, 0.310, 0.595, 0.155, 0.070]);
			c.white_point = JXL_WHITE_POINT_D65;
		}
		8 => {
			custom(&mut c, [0.681, 0.319, 0.243, 0.692, 0.145, 0.049]);
			(c.white_point, c.white_point_xy) = (JXL_WHITE_POINT_CUSTOM, [0.310, 0.316]);
		}
		9 => (c.primaries, c.white_point) = (JXL_PRIMARIES_2100, JXL_WHITE_POINT_D65),
		10 => {
			custom(&mut c, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
			c.white_point = JXL_WHITE_POINT_E;
		}
		11 => (c.primaries, c.white_point) = (JXL_PRIMARIES_P3, JXL_WHITE_POINT_DCI),
		12 => (c.primaries, c.white_point) = (JXL_PRIMARIES_P3, JXL_WHITE_POINT_D65),
		22 => {
			custom(&mut c, [0.630, 0.340, 0.295, 0.605, 0.155, 0.077]);
			c.white_point = JXL_WHITE_POINT_D65;
		}
		_ => return None,
	}
	match transfer {
		1 | 6 | 14 | 15 => c.transfer_function = JXL_TRANSFER_FUNCTION_709,
		4 => (c.transfer_function, c.gamma) = (JXL_TRANSFER_FUNCTION_GAMMA, 1.0 / 2.2),
		5 => (c.transfer_function, c.gamma) = (JXL_TRANSFER_FUNCTION_GAMMA, 1.0 / 2.8),
		8 | 13 | 16 | 17 | 18 => c.transfer_function = c_int::from(transfer),
		_ => return None,
	}
	if matrix != 0 || full_range != 1 {
		return None;
	}
	c.rendering_intent = JXL_RENDERING_INTENT_RELATIVE;
	Some(c)
}

/// `MaybeDecodeBase16`: libjxl's stricter reading of an `ImageMagick` raw profile —
/// lowercase hex only, a newline before every 36 bytes, and exactly one at the end.
fn cjxl_raw_profile(text: &[u8]) -> Option<Vec<u8>> {
	// libpng hands the text over NUL-terminated, so it ends at the first NUL.
	let text = text.split(|&b| b == 0).next().unwrap_or_default();
	let rest = text.strip_prefix(b"\n")?;
	let newline = rest.iter().position(|&b| b == b'\n')?;
	let mut pos = &rest[newline + 1..];
	while let [b' ', tail @ ..] = pos {
		pos = tail;
	}
	let digits = pos.iter().take_while(|b| b.is_ascii_digit()).count();
	if digits == 0 || digits > 8 {
		return None;
	}
	let count: usize = std::str::from_utf8(&pos[..digits]).ok()?.parse().ok()?;
	pos = &pos[digits..];
	if pos.len() / 2 < count || pos.len() - 2 * count != 1 + count.div_ceil(36) {
		return None;
	}
	let nibble = |c: u8| match c {
		b'0'..=b'9' => Some(c - b'0'),
		b'a'..=b'f' => Some(c - b'a' + 10),
		_ => None,
	};
	let mut bytes = Vec::with_capacity(count);
	for i in 0..count {
		if i % 36 == 0 {
			if pos.len() < 2 || pos[0] != b'\n' {
				return None;
			}
			pos = &pos[1..];
		}
		if pos.len() < 3 {
			return None;
		}
		bytes.push(nibble(pos[0])? << 4 | nibble(pos[1])?);
		pos = &pos[2..];
	}
	(pos == b"\n").then_some(bytes)
}

/// `jxl::InterpretExif`: the orientation from IFD0 of a raw TIFF-headed Exif block, or
/// `None`.
fn exif_orientation(exif: &[u8]) -> Option<c_int> {
	if exif.len() < 12 {
		return None;
	}
	let big = match &exif[..4] {
		b"MM\0*" => true,
		b"II*\0" => false,
		_ => return None,
	};
	let u16_at = |at: usize| exif.get(at..at + 2).map(|b| if big { u16::from_be_bytes([b[0], b[1]]) } else { u16::from_le_bytes([b[0], b[1]]) });
	let u32_at = |at: usize| exif.get(at..at + 4).map(|b| if big { u32::from_be_bytes([b[0], b[1], b[2], b[3]]) } else { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) });
	let offset = usize::try_from(u32_at(4)?).ok()?;
	if exif.len() < 12 + offset + 2 || offset < 8 || offset + 2 >= exif.len() {
		return None;
	}
	let count = usize::from(u16_at(offset)?);
	let mut entry = offset + 2;
	for _ in 0..count {
		if entry + 12 >= exif.len() {
			return None;
		}
		if u16_at(entry)? == 274 {
			let (kind, amount, value) = (u16_at(entry + 2)?, u32_at(entry + 4)?, u16_at(entry + 8)?);
			return (kind == 3 && amount == 1 && (1..=8).contains(&value)).then_some(c_int::from(value));
		}
		entry += 12;
	}
	None
}

/// `jxl::IsExif`: a raw TIFF header at the start, of either byte order.
fn is_exif(exif: &[u8]) -> bool {
	exif.len() >= 12 && (exif.starts_with(b"MM\0*") || exif.starts_with(b"II*\0"))
}

/// The `JxlColorEncoding` libjxl's `ParseDescription` makes of a colour space.
fn encoding_of(space: &JxlColorSpace) -> JxlColorEncoding {
	let mut c = JxlColorEncoding { color_space: if space.gray { JXL_COLOR_SPACE_GRAY } else { JXL_COLOR_SPACE_RGB }, white_point: 0, white_point_xy: [0.0; 2], primaries: 0, primaries_red_xy: [0.0; 2], primaries_green_xy: [0.0; 2], primaries_blue_xy: [0.0; 2], transfer_function: 0, gamma: 0.0, rendering_intent: 0 };
	match space.white_point {
		WhitePoint::D65 => c.white_point = JXL_WHITE_POINT_D65,
		WhitePoint::E => c.white_point = JXL_WHITE_POINT_E,
		WhitePoint::Dci => c.white_point = JXL_WHITE_POINT_DCI,
		WhitePoint::D50 => (c.white_point, c.white_point_xy) = (JXL_WHITE_POINT_CUSTOM, [0.345_669, 0.358_496]),
		WhitePoint::Custom(xy) => (c.white_point, c.white_point_xy) = (JXL_WHITE_POINT_CUSTOM, xy),
	}
	if !space.gray {
		let custom = |c: &mut JxlColorEncoding, xy: [f64; 6]| {
			c.primaries = JXL_PRIMARIES_CUSTOM;
			(c.primaries_red_xy, c.primaries_green_xy, c.primaries_blue_xy) = ([xy[0], xy[1]], [xy[2], xy[3]], [xy[4], xy[5]]);
		};
		match space.primaries {
			Primaries::Srgb => c.primaries = JXL_PRIMARIES_SRGB,
			Primaries::Rec2100 => c.primaries = JXL_PRIMARIES_2100,
			Primaries::P3 => c.primaries = JXL_PRIMARIES_P3,
			Primaries::Adobe => custom(&mut c, [0.64, 0.33, 0.21, 0.71, 0.15, 0.06]),
			Primaries::ProPhoto => custom(&mut c, [0.734_699, 0.265_301, 0.159_597, 0.840_403, 0.036_598, 0.000_105]),
			Primaries::Custom(xy) => custom(&mut c, xy),
		}
	}
	c.rendering_intent = match space.rendering_intent {
		RenderingIntent::Perceptual => 0,
		RenderingIntent::Relative => 1,
		RenderingIntent::Saturation => 2,
		RenderingIntent::Absolute => 3,
	};
	(c.transfer_function, c.gamma) = match space.transfer_function {
		TransferFunction::Srgb => (JXL_TRANSFER_FUNCTION_SRGB, 0.0),
		TransferFunction::Bt709 => (JXL_TRANSFER_FUNCTION_709, 0.0),
		TransferFunction::Linear => (JXL_TRANSFER_FUNCTION_LINEAR, 0.0),
		TransferFunction::Pq => (JXL_TRANSFER_FUNCTION_PQ, 0.0),
		TransferFunction::Hlg => (JXL_TRANSFER_FUNCTION_HLG, 0.0),
		TransferFunction::Dci => (JXL_TRANSFER_FUNCTION_DCI, 0.0),
		TransferFunction::Adobe => (JXL_TRANSFER_FUNCTION_GAMMA, 256.0 / 563.0),
		TransferFunction::ProPhoto => (JXL_TRANSFER_FUNCTION_GAMMA, 1.0 / 1.8),
		TransferFunction::Gamma(gamma) => (JXL_TRANSFER_FUNCTION_GAMMA, gamma),
	};
	c
}

/// Encode a source to JPEG XL as `cjxl` would with these settings: a JPEG recompressed
/// losslessly when `-j` is on and nothing needs its pixels, and the pixels otherwise.
///
/// # Errors
///
/// Returns [`EncodeError`] if the settings are out of range, the crop does not fit, a hint
/// file cannot be read, libjxl fails, or `on_progress` asked to stop.
pub fn encode(job: &EncodeJob, source: &SourceImage, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Vec<u8>, EncodeError> {
	let settings = &job.jxl;
	settings.validate()?;
	let untouched = job.crop.is_none() && job.resize.for_source(source.width, source.height).is_noop();
	if settings.lossless_jpeg
		&& untouched
		&& source.format == SourceFormat::Jpeg
		&& source.bytes.starts_with(&[0xff, 0xd8])
		&& let Some(recompressed) = recompress_jpeg(settings, &source.bytes, on_progress)?
	{
		return Ok(recompressed);
	}
	let source = transformed(source, job.crop, job.resize)?;
	if !on_progress(0) {
		return Err(EncodeError::Cancelled);
	}
	let output = encode_pixels(settings, &source)?;
	if !on_progress(100) {
		return Err(EncodeError::Cancelled);
	}
	Ok(output)
}

/// `EncodeImageJXL` for pixels, after `cjxl`'s decoding and container decisions.
fn encode_pixels(settings: &JxlSettings, source: &SourceImage) -> Result<Vec<u8>, EncodeError> {
	let mut input = Input::from_source(source);
	input.apply_hints(settings)?;
	let orientation = exif_orientation(&input.exif).unwrap_or(JXL_ORIENT_IDENTITY);

	// cjxl_main's container decision: metadata asks for a container, and `--container=0`
	// answers by dropping the metadata instead.
	if settings.container == Tristate::Off {
		input.exif.clear();
		input.xmp.clear();
		input.jumbf.clear();
	}
	let use_boxes = !input.exif.is_empty() || !input.xmp.is_empty() || !input.jumbf.is_empty();
	let use_container = settings.container == Tristate::On || use_boxes;

	let encoder = Encoder::new(settings)?;
	let mut streamed = Streamed::default();
	if settings.streaming_output {
		encoder.stream_into(&mut streamed)?;
	}
	let frame = encoder.frame_settings(&frame_options(settings, false))?;
	let distance = distance(settings.target, input.lossy_source);
	// SAFETY: `frame` belongs to the live encoder.
	encoder.check("JxlEncoderSetFrameDistance", unsafe { JxlEncoderSetFrameDistance(frame, distance) })?;
	// SAFETY: `encoder.enc` is live.
	encoder.check("JxlEncoderUseContainer", unsafe { JxlEncoderUseContainer(encoder.enc, c_int::from(use_container)) })?;

	let already_downsampled = if settings.already_downsampled { u32::try_from(settings.resampling).unwrap_or(1) } else { 1 };
	let alpha = input.alpha_bits > 0;
	let lossless = distance == 0.0;
	// cjxl's `PackedPixelFile::info` is value-initialised, not `JxlEncoderInitBasicInfo`'d.
	// SAFETY: every field of `JxlBasicInfo` is plain data for which all-zero is valid.
	let mut info: JxlBasicInfo = unsafe { std::mem::zeroed() };
	info.xsize = input.width * already_downsampled;
	info.ysize = input.height * already_downsampled;
	info.bits_per_sample = input.bits_per_sample;
	info.exponent_bits_per_sample = input.exponent_bits;
	info.intensity_target = if settings.intensity_target > 0.0 { settings.intensity_target } else { input.intensity_target };
	info.uses_original_profile = c_int::from(lossless || settings.disable_perceptual_optimizations);
	info.orientation = orientation;
	info.num_color_channels = input.num_color_channels;
	info.num_extra_channels = u32::from(alpha);
	info.alpha_bits = input.alpha_bits;
	if settings.override_bitdepth != 0 {
		info.bits_per_sample = u32::from(settings.override_bitdepth);
		info.exponent_bits_per_sample = if settings.override_bitdepth == 32 { 8 } else { 0 };
	}
	// SAFETY: `encoder.enc` is live and `info` outlives the call.
	encoder.check("JxlEncoderSetCodestreamLevel", unsafe { JxlEncoderSetCodestreamLevel(encoder.enc, c_int::from(settings.codestream_level)) })?;
	encoder.check("JxlEncoderSetBasicInfo", unsafe { JxlEncoderSetBasicInfo(encoder.enc, &raw const info) })?;
	encoder.check("JxlEncoderSetUpsamplingMode", unsafe { JxlEncoderSetUpsamplingMode(encoder.enc, i64::from(already_downsampled), i64::from(settings.upsampling_mode)) })?;
	let bit_depth = JxlBitDepth { kind: input.bit_depth, bits_per_sample: 0, exponent_bits_per_sample: 0 };
	// SAFETY: `frame` belongs to the live encoder; the struct outlives the call.
	encoder.check("JxlEncoderSetFrameBitDepth", unsafe { JxlEncoderSetFrameBitDepth(frame, &raw const bit_depth) })?;
	if alpha {
		encoder.check("JxlEncoderSetExtraChannelDistance", unsafe { JxlEncoderSetExtraChannelDistance(frame, 0, settings.alpha_distance.unwrap_or(0.0)) })?;
	}
	if lossless {
		encoder.check("JxlEncoderSetFrameLossless", unsafe { JxlEncoderSetFrameLossless(frame, JXL_TRUE) })?;
	}
	match &input.colour {
		// SAFETY: the profile and the encoding outlive their calls.
		Colour::Icc(profile) => encoder.check("JxlEncoderSetICCProfile", unsafe { JxlEncoderSetICCProfile(encoder.enc, profile.as_ptr(), profile.len()) })?,
		Colour::Encoding(encoding) => encoder.check("JxlEncoderSetColorEncoding", unsafe { JxlEncoderSetColorEncoding(encoder.enc, ptr::from_ref(encoding)) })?,
	}

	if use_boxes {
		// SAFETY: `encoder.enc` is live; each box's bytes outlive their call.
		encoder.check("JxlEncoderUseBoxes", unsafe { JxlEncoderUseBoxes(encoder.enc) })?;
		// A raw TIFF header gets four bytes of offset in front of it; anything else is not
		// an Exif block libjxl recognises, and no box is written for it.
		let exif = if is_exif(&input.exif) { [&[0_u8; 4][..], &input.exif].concat() } else { Vec::new() };
		let compress = c_int::from(settings.compress_boxes != Tristate::Off);
		for (kind, bytes) in [(b"Exif", &exif), (b"xml ", &input.xmp), (b"jumb", &input.jumbf)] {
			if !bytes.is_empty() {
				encoder.check("JxlEncoderAddBox", unsafe { JxlEncoderAddBox(encoder.enc, kind.as_ptr(), bytes.as_ptr(), bytes.len(), compress) })?;
			}
		}
		// SAFETY: as above.
		unsafe { JxlEncoderCloseBoxes(encoder.enc) };
	}

	// SetupFrame.
	encoder.check("JxlEncoderSetFrameHeader", unsafe { JxlEncoderSetFrameHeader(frame, &raw const input.frame) })?;
	if alpha {
		let mut extra = std::mem::MaybeUninit::<JxlExtraChannelInfo>::uninit();
		// SAFETY: the init function writes every field.
		let mut extra = unsafe {
			JxlEncoderInitExtraChannelInfo(JXL_CHANNEL_ALPHA, extra.as_mut_ptr());
			extra.assume_init()
		};
		extra.bits_per_sample = input.alpha_bits;
		extra.exponent_bits_per_sample = 0;
		extra.alpha_premultiplied = c_int::from(settings.premultiply == 1);
		encoder.check("JxlEncoderSetExtraChannelInfo", unsafe { JxlEncoderSetExtraChannelInfo(encoder.enc, 0, &raw const extra) })?;
		let blend = JxlBlendInfo { clamp: JXL_FALSE, ..input.frame.layer_info.blend_info };
		encoder.check("JxlEncoderSetExtraChannelBlendInfo", unsafe { JxlEncoderSetExtraChannelBlendInfo(frame, 0, &raw const blend) })?;
	}

	let channels = input.num_color_channels + u32::from(alpha);
	let (format, pointer, size) = match &input.samples {
		Samples::U8(samples) => (JxlPixelFormat { num_channels: channels, data_type: JXL_TYPE_UINT8, endianness: JXL_NATIVE_ENDIAN, align: 0 }, samples.as_ptr().cast::<c_void>(), samples.len()),
		Samples::U16(samples) => (JxlPixelFormat { num_channels: channels, data_type: JXL_TYPE_UINT16, endianness: JXL_NATIVE_ENDIAN, align: 0 }, samples.as_ptr().cast::<c_void>(), samples.len() * 2),
		Samples::F32(samples) => (JxlPixelFormat { num_channels: channels, data_type: JXL_TYPE_FLOAT, endianness: JXL_NATIVE_ENDIAN, align: 0 }, samples.as_ptr().cast::<c_void>(), samples.len() * 4),
	};
	// SAFETY: `pointer` addresses `size` bytes in the declared format, which libjxl copies
	// before returning.
	encoder.check("JxlEncoderAddImageFrame", unsafe { JxlEncoderAddImageFrame(frame, &raw const format, pointer, size) })?;
	encoder.finish(settings.streaming_output.then_some(&mut streamed))
}

/// Recompress a JPEG file's bytes into JPEG XL losslessly: `cjxl -j 1`.
///
/// Metadata stays inside the JPEG data libjxl keeps for reconstruction. Without
/// reconstruction (`--allow_jpeg_reconstruction=0`), `-x strip=` can drop Exif, XMP or
/// JUMBF; with it, `cjxl` refuses to strip anything but JUMBF, and so does this.
///
/// Returns `Ok(None)` when libjxl will not take this JPEG as-is (some unusual JPEGs cannot
/// be transcoded), so the caller can fall back to encoding its pixels instead.
///
/// # Errors
///
/// Returns [`EncodeError`] if the settings are out of range or conflict, libjxl fails for a
/// reason other than refusing the JPEG, or `on_progress` asked to stop.
pub fn recompress_jpeg(settings: &JxlSettings, jpeg: &[u8], on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Option<Vec<u8>>, EncodeError> {
	settings.validate()?;
	let (strip_exif, strip_xmp, strip_jumbf) = (settings.exif == MetadataSource::Strip, settings.xmp == MetadataSource::Strip, settings.jumbf == MetadataSource::Strip);
	let store = settings.allow_jpeg_reconstruction;
	if store && (strip_exif || strip_xmp) {
		return Err(EncodeError::Settings(crate::settings::ValidationError::Conflict("stripping Exif or XMP from a recompressed JPEG needs JPEG reconstruction turned off")));
	}
	if !on_progress(0) {
		return Err(EncodeError::Cancelled);
	}
	let encoder = Encoder::new(settings)?;
	let mut streamed = Streamed::default();
	if settings.streaming_output {
		encoder.stream_into(&mut streamed)?;
	}
	let frame = encoder.frame_settings(&frame_options(settings, true))?;
	// The distance `cjxl` sets on this path is ignored by a JPEG frame, but it is set.
	// SAFETY: `frame` belongs to the live encoder.
	encoder.check("JxlEncoderSetFrameDistance", unsafe { JxlEncoderSetFrameDistance(frame, distance(settings.target, false)) })?;
	encoder.check("JxlEncoderUseContainer", unsafe { JxlEncoderUseContainer(encoder.enc, c_int::from(settings.container == Tristate::On || store)) })?;
	if store {
		encoder.check("JxlEncoderStoreJPEGMetadata", unsafe { JxlEncoderStoreJPEGMetadata(encoder.enc, JXL_TRUE) })?;
	} else {
		for (strip, id) in [(strip_exif, option::JPEG_KEEP_EXIF), (strip_xmp, option::JPEG_KEEP_XMP)] {
			if strip {
				// SAFETY: `frame` belongs to the live encoder.
				unsafe { JxlEncoderFrameSettingsSetOption(frame, id, 0) };
			}
		}
	}
	if strip_jumbf {
		// SAFETY: as above.
		unsafe { JxlEncoderFrameSettingsSetOption(frame, option::JPEG_KEEP_JUMBF, 0) };
	}
	// SAFETY: `jpeg` is a live slice; libjxl copies what it needs.
	if unsafe { JxlEncoderAddJPEGFrame(frame, jpeg.as_ptr(), jpeg.len()) } != JXL_ENC_SUCCESS {
		return Ok(None);
	}
	let output = encoder.finish(settings.streaming_output.then_some(&mut streamed))?;
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
	use super::{OptionValue, cjxl_raw_profile, decode, dimensions, encode, exif_orientation, frame_options, linked_version, option, recompress_jpeg};
	use crate::{
		encoder::RgbaImage, settings::{EncodeJob, JxlSettings, JxlTarget, OutputFormat, Resize, Tristate}, source::SourceImage
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

	fn job(settings: JxlSettings) -> EncodeJob {
		EncodeJob { format: OutputFormat::Jxl, jxl: JxlSettings { effort: 3, ..settings }, ..Default::default() }
	}

	fn run(job: &EncodeJob, width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, crate::encoder::EncodeError> {
		encode(job, &SourceImage::from_rgba(&RgbaImage { width, height, pixels }), &mut |_| true)
	}

	#[test]
	fn links_libjxl_0_11_or_newer() {
		let (major, minor, _) = linked_version();
		assert!(major > 0 || minor >= 11, "linked libjxl is {major}.{minor}");
	}

	#[test]
	fn writes_a_jpeg_xl_file_of_the_right_size() {
		let pixels = fixture(40, 24, true);
		let bytes = run(&job(JxlSettings::default()), 40, 24, &pixels).expect("encode");
		assert_eq!(&bytes[..2], [0xff, 0x0a], "a bare JPEG XL codestream");
		assert_eq!(dimensions(&bytes), Some((40, 24)));
		let (width, height, rgba) = decode(&bytes).expect("decode our own output");
		assert_eq!((width, height, rgba.len()), (40, 24, 40 * 24 * 4));
	}

	#[test]
	fn distance_zero_round_trips_every_pixel_including_under_transparency() {
		let pixels = fixture(33, 17, true);
		let bytes = run(&job(JxlSettings { target: JxlTarget::Distance(0.0), ..Default::default() }), 33, 17, &pixels).expect("encode");
		assert_eq!(decode(&bytes).expect("decode").2, pixels);
	}

	#[test]
	fn an_opaque_image_carries_no_alpha_channel() {
		let pixels = fixture(16, 16, false);
		let bytes = run(&job(JxlSettings::default()), 16, 16, &pixels).expect("encode");
		let image = jxl_oxide::JxlImage::builder().read(bytes.as_slice()).expect("parse");
		assert!(!image.pixel_format().has_alpha());
	}

	#[test]
	fn lower_quality_is_smaller() {
		let pixels = fixture(64, 64, false);
		let size = |quality| run(&job(JxlSettings { target: JxlTarget::Quality(quality), ..Default::default() }), 64, 64, &pixels).expect("encode").len();
		assert!(size(30.0) < size(95.0), "quality 30 should be smaller than quality 95");
	}

	#[test]
	fn applies_the_shared_resize() {
		let pixels = fixture(40, 24, false);
		let bytes = run(&EncodeJob { resize: Resize::to(20, 0), ..job(JxlSettings::default()) }, 40, 24, &pixels).expect("encode");
		assert_eq!(dimensions(&bytes), Some((20, 12)));
	}

	#[test]
	fn a_cancel_returns_nothing() {
		let pixels = fixture(8, 8, false);
		let result = encode(&job(JxlSettings::default()), &SourceImage::from_rgba(&RgbaImage { width: 8, height: 8, pixels: &pixels }), &mut |_| false);
		assert_eq!(result, Err(crate::encoder::EncodeError::Cancelled));
	}

	#[test]
	fn a_recompressed_jpeg_rebuilds_the_original_bit_for_bit() {
		let pixels = fixture(48, 32, false);
		let rgb: Vec<u8> = pixels.as_chunks::<4>().0.iter().flat_map(|&[r, g, b, _]| [r, g, b]).collect();
		let mut jpeg = Vec::new();
		image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 85).encode(&rgb, 48, 32, image::ExtendedColorType::Rgb8).expect("write a JPEG");

		let jxl = recompress_jpeg(&JxlSettings { effort: 3, ..Default::default() }, &jpeg, &mut |_| true).expect("recompress").expect("libjxl takes a baseline JPEG");
		assert!(jxl.len() < jpeg.len(), "recompression should be smaller: {} vs {}", jxl.len(), jpeg.len());

		let image = jxl_oxide::JxlImage::builder().read(jxl.as_slice()).expect("parse");
		let mut rebuilt = Vec::new();
		image.reconstruct_jpeg(&mut rebuilt).expect("reconstruct the JPEG");
		assert_eq!(rebuilt, jpeg, "the reconstructed JPEG must be the original, byte for byte");
	}

	#[test]
	fn out_of_range_settings_are_refused() {
		let pixels = fixture(8, 8, false);
		assert!(run(&EncodeJob { jxl: JxlSettings { effort: 0, ..Default::default() }, format: OutputFormat::Jxl, ..Default::default() }, 8, 8, &pixels).is_err());
		assert!(run(&EncodeJob { jxl: JxlSettings { effort: 11, ..Default::default() }, format: OutputFormat::Jxl, ..Default::default() }, 8, 8, &pixels).is_err());
		assert!(run(&job(JxlSettings { target: JxlTarget::Quality(101.0), ..Default::default() }), 8, 8, &pixels).is_err());
	}

	/// `-p` rewrites the options it implies, unless they were set by hand.
	#[test]
	fn progressive_implies_what_cjxl_says() {
		let find = |options: &[(i32, OptionValue)], id| options.iter().rev().find(|(option, _)| *option == id).map(|(_, value)| *value);
		let options = frame_options(&JxlSettings { progressive: true, ..Default::default() }, false);
		assert_eq!(find(&options, option::PROGRESSIVE_AC), Some(OptionValue::Int(1)));
		assert_eq!(find(&options, option::PROGRESSIVE_DC), Some(OptionValue::Int(1)));
		assert_eq!(find(&options, option::GROUP_ORDER), Some(OptionValue::Int(1)));
		assert_eq!(find(&options, option::PATCHES), Some(OptionValue::Int(0)));
		assert_eq!(find(&options, option::RESPONSIVE), Some(OptionValue::Int(1)));
		let hand = frame_options(&JxlSettings { progressive: true, patches: Tristate::On, progressive_dc: 2, responsive: Some(false), ..Default::default() }, false);
		assert_eq!(find(&hand, option::PATCHES), Some(OptionValue::Int(1)));
		assert_eq!(find(&hand, option::PROGRESSIVE_DC), Some(OptionValue::Int(2)));
		assert_eq!(find(&hand, option::RESPONSIVE), Some(OptionValue::Int(0)));
		let plain = frame_options(&JxlSettings::default(), false);
		assert_eq!(find(&plain, option::RESPONSIVE), None, "-R is only sent when given");
		assert_eq!(find(&plain, option::GROUP_ORDER), None);
	}

	#[test]
	fn raw_profiles_are_read_as_strictly_as_libjxl_reads_them() {
		assert_eq!(cjxl_raw_profile(b"\nexif\n       4\n45786966\n").as_deref(), Some(&b"Exif"[..]));
		assert_eq!(cjxl_raw_profile(b"\nexif\n4\n4578696F\n"), None, "uppercase hex is refused");
		assert_eq!(cjxl_raw_profile(b"\nexif\n4\n45786966"), None, "the closing newline is required");
	}

	#[test]
	fn orientation_comes_from_ifd0() {
		// Big-endian TIFF, IFD0 at 8, one entry: 274 SHORT 1 = 6, then a next-IFD offset.
		let exif = [b"MM\0*".as_slice(), &[0, 0, 0, 8], &[0, 1], &[1, 18, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0], &[0, 0, 0, 0]].concat();
		assert_eq!(exif_orientation(&exif), Some(6));
		assert_eq!(exif_orientation(&[b"Exif\0\0".as_slice(), &exif].concat()), None, "a prefixed block is not read");
	}
}
