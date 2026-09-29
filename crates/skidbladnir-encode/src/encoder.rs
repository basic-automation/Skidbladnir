//! Encoding with libwebp directly, instead of shelling out to `cwebp`.
//!
//! # Why this is the backend
//!
//! The Phase 2 decision (see ROADMAP.md) is to bind libwebp rather than ship the `cwebp`
//! binary or use a pure-Rust encoder, and the reason is parity. libwebp's own
//! `WebPConfig` struct carries a 1:1 field for every single control the Electron UI
//! exposes — `segments`, `sns_strength`, `filter_strength`, `filter_sharpness`,
//! `filter_type`, `autofilter`, `partition_limit`, `pass`, `target_size`, `target_PSNR`,
//! `alpha_quality`, `alpha_filtering`, `emulate_jpeg_size`, `low_memory`,
//! `use_sharp_yuv`, `near_lossless`, `exact`, `thread_level` — so nothing has to be
//! dropped or approximated. A pure-Rust WebP encoder exposes a small fraction of those,
//! which is why it was rejected: the advanced controls *are* the product.
//!
//! # What "parity" means here, precisely
//!
//! `cwebp` is a thin CLI over the same library, and the mapping below is taken from
//! `cwebp.c` itself rather than inferred from its `--help` output. Two of those mappings
//! are not guessable from the Electron app alone and are the kind of thing that makes a
//! port silently wrong:
//!
//! - **`-near_lossless N` also sets `lossless = 1`.** That happens in `cwebp.c`, not in
//!   libwebp, so an implementation that only sets `near_lossless` encodes a lossy file
//!   and produces completely different output.
//! - **`picture.use_argb` is derived, not defaulted.** `cwebp` sets it when the encode is
//!   lossless, uses sharp YUV, or resizes, which changes the colour path the samples take
//!   into the encoder.
//!
//! The gate for all of it is `tests/parity.rs`, which encodes the same pixels through
//! this module and through a real `cwebp` and compares the output byte for byte.
//!
//! # Safety invariants
//!
//! This module is mostly FFI, so rather than repeating the same note on thirty calls, the
//! invariants that hold throughout are stated once here. Anything that does **not** follow
//! from these carries its own `SAFETY:` comment at the call site.
//!
//! - **Every pointer handed to libwebp points at a live local** of this module, and the
//!   local outlives the call. Nothing is stored by libwebp beyond a call except
//!   `WebPPicture::writer`/`custom_ptr` and `progress_hook`/`user_data`, which are cleared
//!   before their targets go out of scope.
//! - **`WebPConfig` and `WebPPicture` are `#[repr(C)]` plain data** and are zeroed before
//!   their `*Init*` function runs, which is what libwebp's own examples do. A zeroed
//!   struct is a valid starting state for both.
//! - **Allocations are owned by guards, not by control flow.** [`Picture`] and [`Writer`]
//!   free their buffers in `Drop`, so an early return from any fallible step in between
//!   cannot leak.
//! - **The ABI version is passed to every `*Internal` entry point** via [`abi_version`],
//!   which is how libwebp detects a caller built against a different struct layout.
//! - **Nothing here is `Send` or `Sync` across a call.** An encode owns its picture and
//!   config for the duration; concurrency happens inside libwebp via `thread_level`.

use std::ffi::c_int;

use libwebp_sys::{VP8StatusCode, WEBP_CSP_MODE, WEBP_DECODER_ABI_VERSION, WEBP_ENCODER_ABI_VERSION, WebPBlendAlpha, WebPConfig, WebPConfigInitInternal, WebPDecode, WebPDecoderConfig, WebPEncCSP, WebPEncode, WebPFreeDecBuffer, WebPGetFeaturesInternal, WebPInitDecoderConfigInternal, WebPMemoryWrite, WebPMemoryWriter, WebPMemoryWriterClear, WebPMemoryWriterInit, WebPPicture, WebPPictureAlloc, WebPPictureCopy, WebPPictureFree, WebPPictureImportRGBA, WebPPictureImportRGBX, WebPPictureInitInternal, WebPPictureRescale, WebPPictureView, WebPPreset, WebPValidateConfig, WebPYUVABuffer};
use thiserror::Error;

use crate::{
	metadata::Metadata, settings::{Crop, EncodeJob, FilterType, OutputFormat, Resize, TargetMetric, WebpMetadata, WebpSettings}, source::{SourceFormat, SourceImage}
};

/// An 8-bit RGBA source image, borrowed.
///
/// Rows are tightly packed: `pixels` must be exactly `width * height * 4` bytes, in
/// R, G, B, A order. This is the one input shape the encoder takes, so whatever decodes
/// the user's PNG or JPEG converts to it once and the encoder stays free of image-format
/// concerns.
#[derive(Clone, Copy, Debug)]
pub struct RgbaImage<'a> {
	/// Width in pixels. Must be non-zero.
	pub width: u32,
	/// Height in pixels. Must be non-zero.
	pub height: u32,
	/// Tightly packed RGBA bytes, `width * height * 4` of them.
	pub pixels: &'a [u8],
}

/// Why an encode did not produce a file.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EncodeError {
	/// The settings themselves are not encodable.
	#[error("invalid settings: {0}")]
	Settings(#[from] crate::settings::ValidationError),
	/// The image has a zero dimension, or `pixels` is not `width * height * 4` bytes.
	#[error("image is {width}x{height} but its buffer is {actual} bytes, expected {expected}")]
	MalformedImage {
		/// The claimed width.
		width: u32,
		/// The claimed height.
		height: u32,
		/// The buffer length actually supplied.
		actual: usize,
		/// The buffer length the dimensions imply.
		expected: usize,
	},
	/// The image is larger than libwebp's `WebPPicture` can address.
	#[error("image is {width}x{height}, larger than libwebp can encode")]
	ImageTooLarge {
		/// The claimed width.
		width: u32,
		/// The claimed height.
		height: u32,
	},
	/// libwebp rejected the translated `WebPConfig`.
	///
	/// The range checks in [`EncodeJob::validate`] should make this unreachable, so
	/// it means the two disagree and the settings model needs fixing.
	#[error("libwebp rejected the encoder configuration")]
	InvalidConfig,
	/// A libwebp call that allocates or converts failed, almost always out of memory.
	#[error("libwebp call `{0}` failed")]
	Libwebp(&'static str),
	/// `WebPEncode` failed, reporting this `error_code` from `WebPEncodingError`.
	#[error("libwebp encoding failed with error code {0}")]
	EncodeFailed(c_int),
	/// The caller's progress callback asked to stop.
	///
	/// Distinguished from a failure because it is not one: the user asked for it, and the
	/// UI should not show it as an error.
	#[error("the encode was cancelled")]
	Cancelled,
	/// The AVIF encoder failed.
	#[error("AVIF encoding failed: {0}")]
	Avif(String),
	/// libjxl failed.
	#[error("JPEG XL encoding failed: {0}")]
	Jxl(String),
	/// libheif failed.
	#[error("HEIC encoding failed: {0}")]
	Heic(String),
	/// The crop rectangle does not lie inside the image.
	#[error("the crop {crop_width}x{crop_height} at ({x}, {y}) does not fit inside the {width}x{height} image")]
	CropOutside {
		/// Left edge of the crop.
		x: u32,
		/// Top edge of the crop.
		y: u32,
		/// Width of the crop.
		crop_width: u32,
		/// Height of the crop.
		crop_height: u32,
		/// Width of the image.
		width: u32,
		/// Height of the image.
		height: u32,
	},
	/// The metadata was asked to be kept but is malformed, in a way `cwebp -metadata` also
	/// refuses (see [`Metadata::cwebp_refusal`]).
	#[error("could not keep the metadata: {0}")]
	Metadata(&'static str),
	/// Adding the metadata would take the file past the 4 GiB RIFF limit.
	#[error("adding the metadata would exceed the WebP container's size limit")]
	MetadataTooLarge,
}

/// Translate settings into a libwebp `WebPConfig`, as `cwebp.c` would for the equivalent
/// command line.
///
/// Every field is written, not only the ones that differ from the default, so the config
/// is fully determined by the settings. The one piece of `cwebp.c` logic that is not a
/// plain assignment is kept: a size or PSNR target with a single pass forces six passes.
/// Animations (`crate::animation`) build each frame's config here too.
pub(crate) fn build_config(settings: &WebpSettings) -> Result<WebPConfig, EncodeError> {
	settings.validate()?;

	// WebPConfigInit: the inline helper libwebp declares in its header is not exported,
	// so call the internal entry point it expands to.
	let mut config = unsafe { std::mem::zeroed::<WebPConfig>() };
	if unsafe { WebPConfigInitInternal(&raw mut config, WebPPreset::WEBP_PRESET_DEFAULT, 75.0, abi_version()) } == 0 {
		return Err(EncodeError::Libwebp("WebPConfigInit"));
	}

	config.lossless = c_int::from(settings.lossless);
	config.near_lossless = c_int::from(settings.near_lossless);
	config.exact = c_int::from(settings.exact);
	config.quality = settings.quality;
	config.alpha_quality = c_int::from(settings.alpha_quality);
	config.alpha_compression = c_int::from(settings.alpha_compression);
	config.alpha_filtering = settings.alpha_filtering.as_config();
	config.method = c_int::from(settings.method);
	config.image_hint = match settings.image_hint.as_config() {
		1 => libwebp_sys::WebPImageHint::WEBP_HINT_PICTURE,
		2 => libwebp_sys::WebPImageHint::WEBP_HINT_PHOTO,
		3 => libwebp_sys::WebPImageHint::WEBP_HINT_GRAPH,
		_ => libwebp_sys::WebPImageHint::WEBP_HINT_DEFAULT,
	};
	match settings.target {
		Some(TargetMetric::Size(bytes)) => config.target_size = c_int::try_from(bytes).unwrap_or(c_int::MAX),
		Some(TargetMetric::Psnr(psnr)) => config.target_PSNR = psnr,
		None => {}
	}
	config.segments = c_int::from(settings.segments);
	config.sns_strength = c_int::from(settings.sns);
	config.filter_strength = c_int::from(settings.filter_strength);
	config.filter_sharpness = c_int::from(settings.filter_sharpness);
	config.filter_type = match settings.filter_type {
		FilterType::Simple => 0,
		FilterType::Strong => 1,
	};
	config.autofilter = c_int::from(settings.autofilter);
	config.pass = c_int::from(settings.passes);
	config.qmin = c_int::from(settings.qmin);
	config.qmax = c_int::from(settings.qmax);
	config.preprocessing = c_int::from(settings.preprocessing);
	config.partition_limit = c_int::from(settings.partition_limit);
	config.emulate_jpeg_size = c_int::from(settings.jpeg_like);
	config.use_sharp_yuv = c_int::from(settings.sharp_yuv);
	config.low_memory = c_int::from(settings.low_memory);
	config.thread_level = c_int::from(settings.multi_threading);

	// cwebp.c: "If a target size or PSNR was given, but somehow the -pass option was
	// omitted, force a reasonable value."
	if (config.target_size > 0 || config.target_PSNR > 0.0) && config.pass == 1 {
		config.pass = 6;
	}

	if unsafe { WebPValidateConfig(&raw const config) } == 0 {
		return Err(EncodeError::InvalidConfig);
	}

	Ok(config)
}

/// The libwebp **encoder** version this binary is linked against, as
/// `(major, minor, revision)`.
///
/// Worth surfacing rather than hardcoding: encoder output depends on it, so a parity
/// claim or a bug report is only meaningful alongside the version that produced it.
#[must_use]
pub fn linked_encoder_version() -> (i32, i32, i32) {
	// SAFETY: takes no arguments and reads a compile-time constant.
	unpack_version(unsafe { libwebp_sys::WebPGetEncoderVersion() })
}

/// The libwebp **decoder** version this binary is linked against, as
/// `(major, minor, revision)`.
#[must_use]
pub fn linked_decoder_version() -> (i32, i32, i32) {
	// SAFETY: takes no arguments and reads a compile-time constant.
	unpack_version(unsafe { libwebp_sys::WebPGetDecoderVersion() })
}

/// Split libwebp's packed version integer into its three components.
const fn unpack_version(packed: c_int) -> (i32, i32, i32) {
	((packed >> 16) & 0xff, (packed >> 8) & 0xff, packed & 0xff)
}

/// libwebp's encoder ABI version as the `c_int` its `*Internal` entry points expect.
///
/// The `*Internal` functions take this so a binary built against one libwebp cannot
/// silently pass a differently-shaped struct to another.
pub(crate) const fn abi_version() -> c_int {
	WEBP_ENCODER_ABI_VERSION.cast_signed()
}

/// Owns a `WebPPicture` so its buffers are released even if an encode step fails.
pub(crate) struct Picture(pub(crate) WebPPicture);

impl Drop for Picture {
	fn drop(&mut self) {
		unsafe { WebPPictureFree(&raw mut self.0) }
	}
}

/// Owns a `WebPMemoryWriter` so its buffer is released on every path.
struct Writer(WebPMemoryWriter);

impl Drop for Writer {
	fn drop(&mut self) {
		unsafe { WebPMemoryWriterClear(&raw mut self.0) }
	}
}

/// What the caller's progress callback holds while an encode runs.
///
/// libwebp only carries a single `void*`, so this is what that pointer points at.
struct ProgressState<'a> {
	/// Called with 0..=100. Returning `false` aborts the encode.
	on_progress: &'a mut dyn FnMut(u32) -> bool,
	/// Set when the callback asked to stop, so the caller can tell a cancellation from a
	/// genuine encode failure — libwebp reports both as `WebPEncode` returning 0.
	cancelled: bool,
}

/// The `extern "C"` shim libwebp calls, which forwards to the Rust closure.
///
/// # Safety
///
/// `picture` must be non-null and its `user_data` must point at a live [`ProgressState`],
/// which holds for the duration of [`encode_rgba_with_progress`] and nowhere else.
unsafe extern "C" fn progress_trampoline(percent: c_int, picture: *const WebPPicture) -> c_int {
	let Some(picture) = (unsafe { picture.as_ref() }) else { return 1 };
	let state = picture.user_data.cast::<ProgressState<'_>>();
	let Some(state) = (unsafe { state.as_mut() }) else { return 1 };
	let percent = u32::try_from(percent).unwrap_or(0).min(100);
	if (state.on_progress)(percent) {
		1
	} else {
		state.cancelled = true;
		0
	}
}

/// Encode an RGBA image to a WebP bitstream.
///
/// # Errors
///
/// Returns [`EncodeError`] if the settings are invalid, the image buffer does not match
/// its stated dimensions, or libwebp fails to allocate, rescale or encode.
pub fn encode_rgba(job: &EncodeJob, image: &RgbaImage<'_>) -> Result<Vec<u8>, EncodeError> {
	encode_rgba_with_progress(job, image, &mut |_| true)
}

/// Encode an RGBA image, reporting progress and allowing the caller to stop.
///
/// The pixels stand alone: there is no file behind them, so no metadata, and an alpha
/// channel only if some pixel is not opaque. See [`encode_source_with_progress`].
///
/// # Errors
///
/// As [`encode_rgba`], plus [`EncodeError::Cancelled`] if `on_progress` returned `false`.
pub fn encode_rgba_with_progress(job: &EncodeJob, image: &RgbaImage<'_>, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Vec<u8>, EncodeError> {
	checked_dimensions(image)?;
	encode_source_with_progress(job, &SourceImage::from_rgba(image), on_progress)
}

/// Encode a decoded source image, reporting progress and allowing the caller to stop.
///
/// `on_progress` is called with a percentage from 0 to 100. Returning `false` aborts the
/// encode, which is how cancellation works: there is no way to interrupt `WebPEncode` from
/// another thread, so stopping has to come from inside its own progress callback. A
/// cancelled encode returns [`EncodeError::Cancelled`] rather than a failure, because the
/// user asked for it.
///
/// Two things about libwebp's reporting that a progress bar has to allow for, both
/// observed rather than assumed: it does not call the hook at a fixed cadence, and **it
/// does not guarantee a final call at 100** — a default-quality encode of a 96x64 image
/// stops reporting at 68. Completion is signalled by this function returning, so a UI
/// that waits for 100% will sit at two-thirds forever.
///
/// # Errors
///
/// Returns [`EncodeError`] if the settings are invalid, the crop does not fit, the encoder
/// fails, or the encode was cancelled.
pub fn encode_source_with_progress(job: &EncodeJob, source: &SourceImage, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Vec<u8>, EncodeError> {
	job.validate()?;
	if let Some(crop) = job.crop
		&& !crop.fits(source.width, source.height)
	{
		return Err(EncodeError::CropOutside { x: crop.x, y: crop.y, crop_width: crop.width, crop_height: crop.height, width: source.width, height: source.height });
	}
	match job.format {
		OutputFormat::Webp => encode_webp(job, source, on_progress),
		OutputFormat::Jxl => crate::jxl::encode(job, source, on_progress),
		OutputFormat::Avif => crate::avif::encode(job, source, on_progress),
		OutputFormat::Heic => crate::heic::encode(job, source, on_progress),
	}
}

/// The source after the job's crop and resize, with every sample depth it carries.
///
/// The 8-bit pixels go through libwebp's rescaler, as they always have, so a resize gives
/// the same dimensions and filtering whichever format is chosen. 16-bit samples cannot go
/// through it without losing their depth, so they are resampled by the `image` crate's
/// triangle filter instead, to the dimensions libwebp chose. Neither reference tool other
/// than `cwebp` resizes at all, so there is nothing else to match.
///
/// # Errors
///
/// Returns [`EncodeError`] if the crop does not fit or the rescale fails.
pub(crate) fn transformed(source: &SourceImage, crop: Option<Crop>, resize: Resize) -> Result<std::borrow::Cow<'_, SourceImage>, EncodeError> {
	let resize_after_crop = resize.for_source(crop.map_or(source.width, |c| c.width), crop.map_or(source.height, |c| c.height));
	if crop.is_none() && resize_after_crop.is_noop() {
		return Ok(std::borrow::Cow::Borrowed(source));
	}
	let (width, height, pixels) = crop_and_rescale_rgba(&source.as_rgba(), crop, resize)?;
	let deep = match &source.deep {
		None => None,
		Some(samples) => {
			let full = image::ImageBuffer::<image::Rgba<u16>, Vec<u16>>::from_raw(source.width, source.height, samples.clone()).ok_or(EncodeError::MalformedImage { width: source.width, height: source.height, actual: samples.len(), expected: source.width as usize * source.height as usize * 4 })?;
			let cropped = match crop {
				Some(c) => image::imageops::crop_imm(&full, c.x, c.y, c.width, c.height).to_image(),
				None => full,
			};
			let resized = if (cropped.width(), cropped.height()) == (width, height) { cropped } else { image::imageops::resize(&cropped, width, height, image::imageops::FilterType::Triangle) };
			Some(resized.into_raw())
		}
	};
	Ok(std::borrow::Cow::Owned(SourceImage { width, height, pixels, deep, gray: source.gray, has_alpha: source.has_alpha, format: source.format, bytes: source.bytes.clone() }))
}

/// Crop and then resize RGBA pixels, the order `cwebp` applies them in. The resize's mode
/// is judged against the cropped size.
///
/// # Errors
///
/// Returns [`EncodeError`] if the buffer does not match its dimensions, the crop does not
/// fit, or libwebp fails.
pub fn crop_and_rescale_rgba(image: &RgbaImage<'_>, crop: Option<Crop>, resize: Resize) -> Result<(u32, u32, Vec<u8>), EncodeError> {
	checked_dimensions(image)?;
	let Some(crop) = crop else { return rescale_rgba(image, resize.for_source(image.width, image.height)) };
	if !crop.fits(image.width, image.height) {
		return Err(EncodeError::CropOutside { x: crop.x, y: crop.y, crop_width: crop.width, crop_height: crop.height, width: image.width, height: image.height });
	}
	let row = image.width as usize * 4;
	let pixels: Vec<u8> = image.pixels.chunks_exact(row).skip(crop.y as usize).take(crop.height as usize).flat_map(|line| &line[crop.x as usize * 4..(crop.x + crop.width) as usize * 4]).copied().collect();
	let cropped = RgbaImage { width: crop.width, height: crop.height, pixels: &pixels };
	rescale_rgba(&cropped, resize.for_source(crop.width, crop.height))
}

/// Check that an image's buffer matches its dimensions and that libwebp can address it,
/// returning the dimensions as the `c_int`s libwebp takes.
fn checked_dimensions(image: &RgbaImage<'_>) -> Result<(c_int, c_int), EncodeError> {
	let expected = (image.width as usize).checked_mul(image.height as usize).and_then(|pixels| pixels.checked_mul(4)).ok_or(EncodeError::ImageTooLarge { width: image.width, height: image.height })?;
	if image.width == 0 || image.height == 0 || image.pixels.len() != expected {
		return Err(EncodeError::MalformedImage { width: image.width, height: image.height, actual: image.pixels.len(), expected });
	}
	match (c_int::try_from(image.width), c_int::try_from(image.height)) {
		(Ok(width), Ok(height)) => Ok((width, height)),
		_ => Err(EncodeError::ImageTooLarge { width: image.width, height: image.height }),
	}
}

/// Import RGBA pixels into a fresh ARGB `WebPPicture`.
pub(crate) fn argb_picture(image: &RgbaImage<'_>) -> Result<Picture, EncodeError> {
	let (width, height) = checked_dimensions(image)?;
	let mut picture = Picture(unsafe {
		let mut picture = std::mem::zeroed::<WebPPicture>();
		if WebPPictureInitInternal(&raw mut picture, abi_version()) == 0 {
			return Err(EncodeError::Libwebp("WebPPictureInit"));
		}
		picture
	});
	picture.0.use_argb = 1;
	picture.0.width = width;
	picture.0.height = height;
	let stride = width.checked_mul(4).ok_or(EncodeError::ImageTooLarge { width: image.width, height: image.height })?;
	// SAFETY: checked_dimensions proved `pixels.len()` is exactly `width * height * 4`.
	if unsafe { WebPPictureImportRGBA(&raw mut picture.0, image.pixels.as_ptr(), stride) } == 0 {
		return Err(EncodeError::Libwebp("WebPPictureImportRGBA"));
	}
	Ok(picture)
}

/// Apply a [`Resize`] to RGBA pixels with libwebp's rescaler, returning the new width,
/// height and pixels.
///
/// This is the resize for formats other than WebP. Using libwebp's rescaler rather than a
/// second implementation means a given resize produces the same dimensions for every
/// format — including how a `0` dimension is derived from the aspect ratio — and the same
/// filtering. A no-op resize returns the pixels unchanged.
///
/// # Errors
///
/// Returns [`EncodeError`] if the buffer does not match its dimensions or libwebp fails.
pub fn rescale_rgba(image: &RgbaImage<'_>, resize: Resize) -> Result<(u32, u32, Vec<u8>), EncodeError> {
	if resize.is_noop() {
		checked_dimensions(image)?;
		return Ok((image.width, image.height, image.pixels.to_vec()));
	}
	let mut picture = argb_picture(image)?;
	let too_large = EncodeError::ImageTooLarge { width: resize.width, height: resize.height };
	let (Ok(target_w), Ok(target_h)) = (c_int::try_from(resize.width), c_int::try_from(resize.height)) else {
		return Err(too_large);
	};
	if unsafe { WebPPictureRescale(&raw mut picture.0, target_w, target_h) } == 0 {
		return Err(EncodeError::Libwebp("WebPPictureRescale"));
	}
	let (Ok(width), Ok(height), Ok(stride)) = (u32::try_from(picture.0.width), u32::try_from(picture.0.height), usize::try_from(picture.0.argb_stride)) else {
		return Err(EncodeError::Libwebp("WebPPictureRescale"));
	};
	let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
	for row in 0..height as usize {
		// SAFETY: after a successful rescale `argb` holds `height` rows of `argb_stride`
		// u32s each, of which the first `width` are the row's pixels.
		let line = unsafe { std::slice::from_raw_parts(picture.0.argb.add(row * stride), width as usize) };
		for &argb in line {
			let [a, r, g, b] = argb.to_be_bytes();
			pixels.extend_from_slice(&[r, g, b, a]);
		}
	}
	Ok((width, height, pixels))
}

/// The WebP encode, which is the parity-gated path: `cwebp.c`'s `main` from reading the
/// picture to writing the file, step for step.
fn encode_webp(job: &EncodeJob, source: &SourceImage, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Vec<u8>, EncodeError> {
	let settings = &job.webp;
	let config = build_config(settings)?;
	if settings.metadata.any()
		&& let Some(reason) = Metadata::cwebp_refusal(&source.bytes, source.format)
	{
		return Err(EncodeError::Metadata(reason));
	}
	// cwebp's PNG reader has libpng correct a PNG's gamma to a 2.2 display.
	let corrected = crate::png_gamma::like_cwebp(source);
	let image = corrected.as_deref().map_or_else(|| source.as_rgba(), |pixels| RgbaImage { width: source.width, height: source.height, pixels });
	let (width, height) = checked_dimensions(&image)?;

	// cwebp.c decides the colour path *before* reading the input, because it changes which
	// conversion the samples go through. It looks at the resize as given, before
	// `-resize_mode` has had a chance to decline it. Matching it is a parity requirement,
	// not an optimisation.
	let use_argb = config.lossless == 1 || config.use_sharp_yuv == 1 || config.preprocessing > 0 || job.crop.is_some() || !job.resize.is_noop();
	let mut picture = if !use_argb && source.format == SourceFormat::Webp && !source.bytes.is_empty() {
		// A WebP source off the ARGB path is decoded straight into YUV 4:2:0, never RGB.
		yuv_picture(&source.bytes, settings.keep_alpha)?
	} else {
		let mut picture = Picture(unsafe {
			let mut picture = std::mem::zeroed::<WebPPicture>();
			if WebPPictureInitInternal(&raw mut picture, abi_version()) == 0 {
				return Err(EncodeError::Libwebp("WebPPictureInit"));
			}
			picture
		});
		picture.0.use_argb = c_int::from(use_argb);
		picture.0.width = width;
		picture.0.height = height;

		// 4 bytes per pixel, tightly packed, so the stride is the row length.
		let stride = width.checked_mul(4).ok_or(EncodeError::ImageTooLarge { width: image.width, height: image.height })?;
		// `-noalpha` makes cwebp's readers drop the alpha channel, keeping the colour under
		// it as it is. Importing RGBX is exactly that. An opaque RGBA import is identical to
		// an RGB one, which is what cwebp's readers do for a source with no alpha.
		// SAFETY: beyond the module-wide invariants, this one depends on the buffer actually
		// being as large as the dimensions claim — libwebp reads `height * stride` bytes
		// from it. That was checked above: `pixels.len()` is exactly `width * height * 4`.
		let imported = unsafe { if settings.keep_alpha { WebPPictureImportRGBA(&raw mut picture.0, image.pixels.as_ptr(), stride) } else { WebPPictureImportRGBX(&raw mut picture.0, image.pixels.as_ptr(), stride) } };
		if imported == 0 {
			return Err(EncodeError::Libwebp("WebPPictureImportRGBA"));
		}
		picture
	};

	if let Some(colour) = settings.blend_alpha {
		// SAFETY: the picture was imported above; the call reads and writes its own planes.
		unsafe { WebPBlendAlpha(&raw mut picture.0, colour & 0x00ff_ffff) };
	}

	if let Some(crop) = job.crop {
		let (Ok(x), Ok(y), Ok(w), Ok(h)) = (c_int::try_from(crop.x), c_int::try_from(crop.y), c_int::try_from(crop.width), c_int::try_from(crop.height)) else {
			return Err(EncodeError::CropOutside { x: crop.x, y: crop.y, crop_width: crop.width, crop_height: crop.height, width: image.width, height: image.height });
		};
		// Self-cropping with a view, as cwebp does. The view keeps pointing into the
		// picture's own allocation, which the picture still owns.
		let view: *mut WebPPicture = &raw mut picture.0;
		// SAFETY: libwebp documents `src == dst` as valid for WebPPictureView.
		if unsafe { WebPPictureView(view, x, y, w, h, view) } == 0 {
			return Err(EncodeError::CropOutside { x: crop.x, y: crop.y, crop_width: crop.width, crop_height: crop.height, width: image.width, height: image.height });
		}
	}

	let resize = job.resize.for_source(u32::try_from(picture.0.width).unwrap_or(0), u32::try_from(picture.0.height).unwrap_or(0));
	if !resize.is_noop() {
		resize_picture(&mut picture, resize, &config)?;
	}

	let dimensions = (picture.0.width, picture.0.height);
	let bytes = encode_picture(&config, picture, on_progress)?;
	if !settings.metadata.any() {
		return Ok(bytes);
	}
	let metadata = Metadata::read_like_cwebp(&source.bytes, source.format);
	with_metadata(bytes, &metadata, settings.metadata, dimensions)
}

/// Decode a WebP file into a fresh YUV 4:2:0 `WebPPicture` (with an alpha plane if the
/// file has alpha and it is kept), exactly as `cwebp`'s `ReadWebP` (`imageio/webpdec.c`)
/// does when `use_argb` is off. Decoding to RGBA and converting back lands a few bytes away.
fn yuv_picture(webp: &[u8], keep_alpha: bool) -> Result<Picture, EncodeError> {
	let decode_failed = EncodeError::Libwebp("WebPDecode");
	let mut decoder = unsafe {
		let mut config = std::mem::zeroed::<WebPDecoderConfig>();
		if WebPInitDecoderConfigInternal(&raw mut config, WEBP_DECODER_ABI_VERSION.cast_signed()) == 0 {
			return Err(EncodeError::Libwebp("WebPInitDecoderConfig"));
		}
		config
	};
	// SAFETY: `webp` is a live slice of `webp.len()` bytes for the whole call.
	if unsafe { WebPGetFeaturesInternal(webp.as_ptr(), webp.len(), &raw mut decoder.input, WEBP_DECODER_ABI_VERSION.cast_signed()) } != VP8StatusCode::VP8_STATUS_OK {
		return Err(decode_failed);
	}
	let has_alpha = keep_alpha && decoder.input.has_alpha != 0;

	let mut picture = Picture(unsafe {
		let mut picture = std::mem::zeroed::<WebPPicture>();
		if WebPPictureInitInternal(&raw mut picture, abi_version()) == 0 {
			return Err(EncodeError::Libwebp("WebPPictureInit"));
		}
		picture
	});
	picture.0.use_argb = 0;
	picture.0.width = decoder.input.width;
	picture.0.height = decoder.input.height;
	picture.0.colorspace = if has_alpha { WebPEncCSP::WEBP_YUV420A } else { WebPEncCSP::WEBP_YUV420 };
	if unsafe { WebPPictureAlloc(&raw mut picture.0) } == 0 {
		return Err(EncodeError::ImageTooLarge { width: decoder.input.width.cast_unsigned(), height: decoder.input.height.cast_unsigned() });
	}

	// The decoder writes into the picture's own planes, laid out as WebPPictureAlloc made
	// them, and sized as cwebp sizes them.
	let (height, uv_rows) = (picture.0.height, (picture.0.height + 1) / 2);
	let plane = |stride: c_int, rows: c_int| usize::try_from(stride).unwrap_or(0) * usize::try_from(rows).unwrap_or(0);
	decoder.output.colorspace = if has_alpha { WEBP_CSP_MODE::MODE_YUVA } else { WEBP_CSP_MODE::MODE_YUV };
	decoder.output.u.YUVA = WebPYUVABuffer { y: picture.0.y, u: picture.0.u, v: picture.0.v, a: if has_alpha { picture.0.a } else { std::ptr::null_mut() }, y_stride: picture.0.y_stride, u_stride: picture.0.uv_stride, v_stride: picture.0.uv_stride, a_stride: if has_alpha { picture.0.a_stride } else { 0 }, y_size: plane(picture.0.y_stride, height), u_size: plane(picture.0.uv_stride, uv_rows), v_size: plane(picture.0.uv_stride, uv_rows), a_size: plane(picture.0.a_stride, height) };
	decoder.output.is_external_memory = 1;
	// SAFETY: the output buffer points into the picture's allocation with the sizes just
	// computed from its own strides, and the picture outlives the call.
	let status = unsafe { WebPDecode(webp.as_ptr(), webp.len(), &raw mut decoder) };
	unsafe { WebPFreeDecBuffer(&raw mut decoder.output) };
	if status != VP8StatusCode::VP8_STATUS_OK {
		return Err(decode_failed);
	}
	Ok(picture)
}

/// Run `WebPEncode` on a prepared picture, with progress and cancellation, and return the
/// file it wrote.
fn encode_picture(config: &WebPConfig, mut picture: Picture, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Vec<u8>, EncodeError> {
	let mut writer = Writer(unsafe {
		let mut writer = std::mem::zeroed::<WebPMemoryWriter>();
		WebPMemoryWriterInit(&raw mut writer);
		writer
	});
	picture.0.writer = Some(WebPMemoryWrite);
	picture.0.custom_ptr = (&raw mut writer.0).cast();

	// `state` lives on this stack frame for the whole of WebPEncode and is unreachable
	// afterwards, which is exactly the lifetime the trampoline's safety requires.
	let mut state = ProgressState { on_progress, cancelled: false };
	picture.0.user_data = (&raw mut state).cast();
	picture.0.progress_hook = Some(progress_trampoline);

	let encoded = unsafe { WebPEncode(config, &raw mut picture.0) };
	// Clear the borrow of `state` from the picture before it goes out of scope.
	picture.0.progress_hook = None;
	picture.0.user_data = std::ptr::null_mut();

	if encoded == 0 {
		// libwebp reports a cancelled encode the same way it reports a failed one, so the
		// flag the trampoline set is the only way to tell them apart.
		if state.cancelled {
			return Err(EncodeError::Cancelled);
		}
		return Err(EncodeError::EncodeFailed(picture.0.error_code as c_int));
	}

	// SAFETY: on success WebPMemoryWrite has filled `mem` with exactly `size` bytes; the
	// null check covers the case where the encode produced nothing. The slice is copied
	// before `writer` is dropped and frees it.
	Ok(if writer.0.mem.is_null() { Vec::new() } else { unsafe { std::slice::from_raw_parts(writer.0.mem, writer.0.size) }.to_vec() })
}

/// Add the source's metadata to an encoded WebP: `cwebp.c`'s `WriteWebPWithMetadata`.
///
/// A simple WebP has no room for metadata, so a `VP8X` header is added (or an existing
/// one's flags updated), the `ICCP` chunk goes before the image data and `EXIF` and `XMP `
/// after it, each padded to an even size, and the RIFF size is rewritten to cover them.
/// Metadata the source does not have is simply not written.
fn with_metadata(webp: Vec<u8>, metadata: &Metadata, keep: WebpMetadata, (width, height): (c_int, c_int)) -> Result<Vec<u8>, EncodeError> {
	const CHUNK_HEADER: usize = 8;
	const RIFF_HEADER: usize = 12;
	const VP8X_CHUNK: usize = 18;
	fn chosen(wanted: bool, payload: Option<&[u8]>) -> Option<&[u8]> {
		payload.filter(|bytes| wanted && !bytes.is_empty())
	}
	let (exif, icc, xmp) = (chosen(keep.exif, metadata.exif.as_deref()), chosen(keep.icc, metadata.icc.as_deref()), chosen(keep.xmp, metadata.xmp.as_deref()));
	let chunk_size = |payload: &[u8]| CHUNK_HEADER + payload.len() + (payload.len() & 1);
	let metadata_size: usize = [exif, icc, xmp].into_iter().flatten().map(chunk_size).sum();
	if metadata_size == 0 || webp.len() < RIFF_HEADER + CHUNK_HEADER {
		return Ok(webp);
	}
	let flags = (u32::from(exif.is_some()) * 0x08) | (u32::from(icc.is_some()) * 0x20) | (u32::from(xmp.is_some()) * 0x04);

	let has_vp8x = &webp[RIFF_HEADER..RIFF_HEADER + 4] == b"VP8X";
	let riff_size = webp.len() - CHUNK_HEADER + if has_vp8x { 0 } else { VP8X_CHUNK } + metadata_size;
	let riff_size = u32::try_from(riff_size).map_err(|_| EncodeError::MetadataTooLarge)?;
	let mut out = Vec::with_capacity(webp.len() + VP8X_CHUNK + metadata_size);
	out.extend_from_slice(b"RIFF");
	out.extend_from_slice(&riff_size.to_le_bytes());
	out.extend_from_slice(b"WEBP");
	let mut image = &webp[RIFF_HEADER..];
	if has_vp8x {
		let mut vp8x = image[..VP8X_CHUNK].to_vec();
		vp8x[CHUNK_HEADER] |= u8::try_from(flags & 0xff).unwrap_or(0);
		out.extend_from_slice(&vp8x);
		image = &image[VP8X_CHUNK..];
	} else {
		// A lossless bitstream records whether it has alpha in the 29th bit after its
		// signature; a lossy one without VP8X has none.
		let alpha = &image[..4] == b"VP8L" && image.get(CHUNK_HEADER + 4).is_some_and(|byte| byte & (1 << 4) != 0);
		let flags = flags | if alpha { 0x10 } else { 0 };
		let le24 = |value: c_int| u32::try_from(value - 1).unwrap_or(0).to_le_bytes()[..3].to_vec();
		out.extend_from_slice(b"VP8X\x0a\x00\x00\x00");
		out.extend_from_slice(&flags.to_le_bytes());
		out.extend_from_slice(&le24(width));
		out.extend_from_slice(&le24(height));
	}
	let mut chunk = |fourcc: &[u8; 4], payload: &[u8]| {
		out.extend_from_slice(fourcc);
		out.extend_from_slice(&u32::try_from(payload.len()).unwrap_or(u32::MAX).to_le_bytes());
		out.extend_from_slice(payload);
		if payload.len() & 1 == 1 {
			out.push(0);
		}
	};
	if let Some(icc) = icc {
		chunk(b"ICCP", icc);
	}
	out.extend_from_slice(image);
	let mut chunk = |fourcc: &[u8; 4], payload: &[u8]| {
		out.extend_from_slice(fourcc);
		out.extend_from_slice(&u32::try_from(payload.len()).unwrap_or(u32::MAX).to_le_bytes());
		out.extend_from_slice(payload);
		if payload.len() & 1 == 1 {
			out.push(0);
		}
	};
	if let Some(exif) = exif {
		chunk(b"EXIF", exif);
	}
	if let Some(xmp) = xmp {
		chunk(b"XMP ", xmp);
	}
	Ok(out)
}

/// Rescale the picture the way `cwebp` does, including its `-exact` special case.
///
/// A plain `WebPPictureRescale` premultiplies RGB by alpha, which destroys the colour of
/// fully transparent pixels — exactly what `-exact` promises to preserve, and the
/// Electron app pairs `-exact` with every lossless encode. `cwebp` works around it by
/// rescaling an opaque copy for the colour channels and the real picture for alpha, then
/// reassembling. Without this, a lossless resize diverges from `cwebp` in every
/// transparent pixel.
pub(crate) fn resize_picture(picture: &mut Picture, resize: Resize, config: &WebPConfig) -> Result<(), EncodeError> {
	let target_w = i32::try_from(resize.width).map_err(|_| EncodeError::ImageTooLarge { width: resize.width, height: resize.height })?;
	let target_h = i32::try_from(resize.height).map_err(|_| EncodeError::ImageTooLarge { width: resize.width, height: resize.height })?;

	if config.exact == 0 {
		if unsafe { WebPPictureRescale(&raw mut picture.0, target_w, target_h) } == 0 {
			return Err(EncodeError::Libwebp("WebPPictureRescale"));
		}
		return Ok(());
	}

	let mut opaque = Picture(unsafe {
		let mut copy = std::mem::zeroed::<WebPPicture>();
		if WebPPictureInitInternal(&raw mut copy, abi_version()) == 0 {
			return Err(EncodeError::Libwebp("WebPPictureInit"));
		}
		copy
	});
	if unsafe { WebPPictureCopy(&raw const picture.0, &raw mut opaque.0) } == 0 {
		return Err(EncodeError::Libwebp("WebPPictureCopy"));
	}

	// use_argb was forced on above for any resize, so argb is the live plane.
	for_each_argb_row(&mut opaque, |row| {
		for pixel in row {
			*pixel |= 0xff00_0000;
		}
	});

	if unsafe { WebPPictureRescale(&raw mut opaque.0, target_w, target_h) } == 0 {
		return Err(EncodeError::Libwebp("WebPPictureRescale"));
	}
	if unsafe { WebPPictureRescale(&raw mut picture.0, target_w, target_h) } == 0 {
		return Err(EncodeError::Libwebp("WebPPictureRescale"));
	}

	// Keep the rescaled alpha, take the non-premultiplied RGB from the opaque copy.
	// Both pictures must be on the ARGB path for this to be meaningful; use_argb is forced
	// on for any resize, but that invariant is set far enough away from here that walking
	// the planes on the strength of it would be a latent null dereference.
	if picture.0.argb.is_null() || opaque.0.argb.is_null() {
		return Err(EncodeError::Libwebp("WebPPictureRescale: expected ARGB pictures for the -exact path"));
	}
	let rows = usize::try_from(picture.0.height).unwrap_or(0);
	let width = usize::try_from(picture.0.width).unwrap_or(0);
	// SAFETY: both pictures were confirmed non-null just above and are on the ARGB path, so
	// each holds `height` rows of `argb_stride` u32s and `width <= argb_stride`. The two
	// slices come from different allocations, so they cannot alias.
	for y in 0..rows {
		let dst = unsafe { std::slice::from_raw_parts_mut(picture.0.argb.add(y * usize::try_from(picture.0.argb_stride).unwrap_or(0)), width) };
		let src = unsafe { std::slice::from_raw_parts(opaque.0.argb.add(y * usize::try_from(opaque.0.argb_stride).unwrap_or(0)), width) };
		for (dst, src) in dst.iter_mut().zip(src) {
			*dst = (*dst & 0xff00_0000) | (*src & 0x00ff_ffff);
		}
	}

	Ok(())
}

/// Run `f` over each row of a picture's ARGB plane, honouring `argb_stride`.
fn for_each_argb_row(picture: &mut Picture, mut f: impl FnMut(&mut [u32])) {
	let rows = usize::try_from(picture.0.height).unwrap_or(0);
	let width = usize::try_from(picture.0.width).unwrap_or(0);
	let stride = usize::try_from(picture.0.argb_stride).unwrap_or(0);
	if picture.0.argb.is_null() {
		return;
	}
	for y in 0..rows {
		f(unsafe { std::slice::from_raw_parts_mut(picture.0.argb.add(y * stride), width) });
	}
}

#[cfg(test)]
mod tests {
	use super::{EncodeError, RgbaImage, build_config, encode_rgba};
	use crate::{
		metadata::Metadata, settings::{AlphaFiltering, Crop, EncodeJob, FilterType, ImageHint, Resize, ResizeMode, TargetMetric, WebpMetadata, WebpSettings}, source::{SourceFormat, SourceImage}
	};

	/// A small RGBA test image with real colour variation and a real alpha ramp, so the
	/// alpha controls have something to act on.
	fn fixture(width: u32, height: u32) -> Vec<u8> {
		let mut pixels = Vec::with_capacity((width * height * 4) as usize);
		for y in 0..height {
			for x in 0..width {
				pixels.push(u8::try_from((x * 7 + y * 3) % 256).unwrap_or(0));
				pixels.push(u8::try_from((x * 255) / width.max(1)).unwrap_or(0));
				pixels.push(u8::try_from((y * 255) / height.max(1)).unwrap_or(0));
				pixels.push(if (x / 4 + y / 4) % 3 == 0 { 0 } else { 255 });
			}
		}
		pixels
	}

	fn encode(settings: &EncodeJob) -> Result<Vec<u8>, EncodeError> {
		let pixels = fixture(48, 32);
		encode_rgba(settings, &RgbaImage { width: 48, height: 32, pixels: &pixels })
	}

	fn decode(bytes: &[u8]) -> (i32, i32, Vec<u8>) {
		let mut width = 0;
		let mut height = 0;
		let decoded = unsafe { libwebp_sys::WebPDecodeRGBA(bytes.as_ptr(), bytes.len(), &raw mut width, &raw mut height) };
		assert!(!decoded.is_null(), "the output must decode");
		let pixels = unsafe { std::slice::from_raw_parts(decoded, usize::try_from(width * height * 4).expect("positive")) }.to_vec();
		unsafe { libwebp_sys::WebPFree(decoded.cast()) };
		(width, height, pixels)
	}

	fn dimensions(bytes: &[u8]) -> (i32, i32) {
		let mut width = 0;
		let mut height = 0;
		assert_ne!(unsafe { libwebp_sys::WebPGetInfo(bytes.as_ptr(), bytes.len(), &raw mut width, &raw mut height) }, 0);
		(width, height)
	}

	/// Every setting must reach its `WebPConfig` field. This is the parity claim at the
	/// config level; `tests/parity.rs` is the claim at the byte level.
	#[test]
	fn every_setting_reaches_webpconfig() {
		let settings = WebpSettings { lossless: true, near_lossless: 40, exact: true, quality: 42.5, alpha_quality: 33, alpha_compression: false, alpha_filtering: AlphaFiltering::Off, method: 5, image_hint: ImageHint::Graph, target: Some(TargetMetric::Size(12_345)), segments: 2, sns: 81, filter_strength: 64, filter_sharpness: 6, filter_type: FilterType::Simple, autofilter: false, passes: 9, qmin: 11, qmax: 88, preprocessing: 5, partition_limit: 17, jpeg_like: true, sharp_yuv: true, low_memory: true, multi_threading: true, ..WebpSettings::default() };
		let c = build_config(&settings).expect("config builds");
		assert_eq!((c.lossless, c.near_lossless, c.exact), (1, 40, 1));
		assert!((c.quality - 42.5).abs() < f32::EPSILON);
		assert_eq!((c.alpha_quality, c.alpha_compression, c.alpha_filtering, c.method), (33, 0, 0, 5));
		assert_eq!(c.image_hint, libwebp_sys::WebPImageHint::WEBP_HINT_GRAPH);
		assert_eq!(c.target_size, 12_345);
		assert_eq!((c.segments, c.sns_strength, c.filter_strength, c.filter_sharpness, c.filter_type, c.autofilter), (2, 81, 64, 6, 0, 0));
		assert_eq!((c.pass, c.qmin, c.qmax, c.preprocessing, c.partition_limit), (9, 11, 88, 5, 17));
		assert_eq!((c.emulate_jpeg_size, c.use_sharp_yuv, c.low_memory, c.thread_level), (1, 1, 1, 1));
	}

	/// cwebp.c forces six passes for a size or PSNR target left at one pass.
	#[test]
	fn a_target_with_one_pass_gets_six() {
		let c = build_config(&WebpSettings { passes: 1, target: Some(TargetMetric::Psnr(40.0)), ..WebpSettings::default() }).expect("config builds");
		assert_eq!(c.pass, 6);
		assert_eq!(build_config(&WebpSettings { passes: 1, ..WebpSettings::default() }).expect("config builds").pass, 1);
	}

	#[test]
	fn invalid_settings_are_rejected_before_libwebp_sees_them() {
		let err = build_config(&WebpSettings { method: 9, ..Default::default() }).expect_err("method 9 is out of range");
		assert!(matches!(err, EncodeError::Settings(_)), "got {err:?}");
	}

	/// The version helpers are shown to users and used to gate the parity test, so a
	/// zeroed or nonsense value must not pass unnoticed.
	#[test]
	fn linked_versions_are_plausible() {
		for (label, (major, minor, revision)) in [("encoder", super::linked_encoder_version()), ("decoder", super::linked_decoder_version())] {
			assert!(major >= 1, "{label} major version was {major}");
			assert!((0..=255).contains(&minor) && (0..=255).contains(&revision), "{label} version {major}.{minor}.{revision}");
		}
	}

	/// libavif and libheif are compiled against the sharpyuv headers in `third_party/libwebp`
	/// but linked with the sharpyuv `libwebp-sys` compiles, so the submodule has to be the
	/// libwebp `libwebp-sys` vendors. Bumping one without the other fails here.
	#[test]
	fn the_libwebp_submodule_matches_the_linked_libwebp() {
		unsafe extern "C" {
			fn SharpYuvGetVersion() -> std::ffi::c_int;
		}
		let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
		let gitmodules = std::fs::read_to_string(root.join(".gitmodules")).expect("read .gitmodules");
		let pinned = gitmodules.split("[submodule ").find(|section| section.starts_with("\"third_party/libwebp\"")).and_then(|section| section.lines().find_map(|line| line.trim().strip_prefix("version = v"))).expect("third_party/libwebp has a version in .gitmodules");
		let (major, minor, revision) = super::linked_encoder_version();
		assert_eq!(pinned, format!("{major}.{minor}.{revision}"), "third_party/libwebp is pinned to {pinned}, libwebp-sys links {major}.{minor}.{revision}");

		let Ok(header) = std::fs::read_to_string(root.join("third_party/libwebp/sharpyuv/sharpyuv.h")) else { return };
		let part = |name: &str| header.lines().find_map(|line| line.strip_prefix(&format!("#define SHARPYUV_VERSION_{name} "))).and_then(|value| value.trim().parse::<i32>().ok()).unwrap_or_else(|| panic!("SHARPYUV_VERSION_{name} in sharpyuv.h"));
		let headers = (part("MAJOR") << 24) | (part("MINOR") << 16) | part("PATCH");
		// SAFETY: takes no arguments and returns a constant.
		let linked = unsafe { SharpYuvGetVersion() };
		assert_eq!(headers, linked, "the sharpyuv headers are {headers:#x}, the linked sharpyuv is {linked:#x}");
	}

	#[test]
	fn a_default_encode_produces_a_webp() {
		let bytes = encode(&EncodeJob::default()).expect("default settings encode");
		assert!(bytes.len() > 20, "suspiciously small output: {} bytes", bytes.len());
		assert_eq!(&bytes[0..4], b"RIFF");
		assert_eq!(&bytes[8..12], b"WEBP");
	}

	/// Lossless with `-exact` must decode back to the exact input, which is the only
	/// end-to-end check that the pixels actually survived the import path.
	#[test]
	fn lossless_exact_round_trips_the_pixels() {
		let pixels = fixture(48, 32);
		let bytes = encode_rgba(&EncodeJob::from(WebpSettings { lossless: true, exact: true, ..Default::default() }), &RgbaImage { width: 48, height: 32, pixels: &pixels }).expect("lossless encodes");
		assert_eq!(decode(&bytes), (48, 32, pixels), "lossless + exact must preserve every byte, including RGB under transparent pixels");
	}

	/// Without `-exact`, libwebp may recolour fully transparent pixels.
	#[test]
	fn lossless_without_exact_may_recolour_the_invisible() {
		let pixels = fixture(48, 32);
		let (.., decoded) = decode(&encode_rgba(&EncodeJob::from(WebpSettings { lossless: true, exact: false, ..Default::default() }), &RgbaImage { width: 48, height: 32, pixels: &pixels }).expect("encodes"));
		for (got, want) in decoded.as_chunks::<4>().0.iter().zip(pixels.as_chunks::<4>().0) {
			if want[3] == 255 {
				assert_eq!(got, want, "visible pixels are still exact");
			}
		}
		assert_ne!(decoded, pixels, "some invisible colour was cleaned up");
	}

	/// `-noalpha` drops the alpha channel and keeps the colour that was under it.
	#[test]
	fn no_alpha_drops_the_channel() {
		let pixels = fixture(48, 32);
		let (.., decoded) = decode(&encode_rgba(&EncodeJob::from(WebpSettings { lossless: true, exact: true, keep_alpha: false, ..Default::default() }), &RgbaImage { width: 48, height: 32, pixels: &pixels }).expect("encodes"));
		for (got, want) in decoded.as_chunks::<4>().0.iter().zip(pixels.as_chunks::<4>().0) {
			assert_eq!(got, &[want[0], want[1], want[2], 255]);
		}
	}

	/// `-blend_alpha` flattens onto the background colour, leaving the image opaque.
	#[test]
	fn blend_alpha_flattens_onto_the_background() {
		let pixels = fixture(48, 32);
		let (.., decoded) = decode(&encode_rgba(&EncodeJob::from(WebpSettings { lossless: true, exact: true, blend_alpha: Some(0x00ff_0000), ..Default::default() }), &RgbaImage { width: 48, height: 32, pixels: &pixels }).expect("encodes"));
		for (got, want) in decoded.as_chunks::<4>().0.iter().zip(pixels.as_chunks::<4>().0) {
			assert_eq!(got[3], 255, "the result is opaque");
			if want[3] == 0 {
				assert_eq!(&got[..3], &[255, 0, 0], "fully transparent pixels become the background colour");
			}
		}
	}

	#[test]
	fn crop_then_resize() {
		let pixels = fixture(48, 32);
		let image = RgbaImage { width: 48, height: 32, pixels: &pixels };
		let crop = Some(Crop { x: 8, y: 4, width: 20, height: 10 });
		let (width, height, decoded) = decode(&encode_rgba(&EncodeJob { crop, webp: WebpSettings { lossless: true, exact: true, ..Default::default() }, ..Default::default() }, &image).expect("encodes"));
		assert_eq!((width, height), (20, 10));
		assert_eq!(&decoded[..4], &pixels[(4 * 48 + 8) * 4..(4 * 48 + 8) * 4 + 4], "the crop starts at (8, 4)");
		assert_eq!(dimensions(&encode_rgba(&EncodeJob { crop, resize: Resize::to(10, 0), ..Default::default() }, &image).expect("encodes")), (10, 5), "the resize applies to the cropped size");
		let err = encode_rgba(&EncodeJob { crop: Some(Crop { x: 40, y: 0, width: 20, height: 10 }), ..Default::default() }, &image).expect_err("does not fit");
		assert!(matches!(err, EncodeError::CropOutside { .. }), "got {err:?}");
	}

	#[test]
	fn resize_changes_the_output_dimensions() {
		let pixels = fixture(48, 32);
		for (resize, expected) in [(Resize::to(24, 16), (24, 16)), (Resize::to(24, 0), (24, 16)), (Resize::to(0, 16), (24, 16))] {
			let bytes = encode_rgba(&EncodeJob { resize, ..Default::default() }, &RgbaImage { width: 48, height: 32, pixels: &pixels }).expect("resized encode");
			assert_eq!(dimensions(&bytes), expected, "resize {resize:?}");
		}
	}

	#[test]
	fn resize_modes_decide_whether_to_resize() {
		let pixels = fixture(48, 32);
		let image = RgbaImage { width: 48, height: 32, pixels: &pixels };
		for (resize, expected) in [(Resize { width: 96, height: 0, mode: ResizeMode::DownOnly }, (48, 32)), (Resize { width: 24, height: 0, mode: ResizeMode::DownOnly }, (24, 16)), (Resize { width: 24, height: 0, mode: ResizeMode::UpOnly }, (48, 32)), (Resize::to(96, 0), (96, 64))] {
			assert_eq!(dimensions(&encode_rgba(&EncodeJob { resize, ..Default::default() }, &image).expect("encode")), expected, "resize {resize:?}");
		}
	}

	/// The `-exact` resize path is a different code path; make sure it still produces a
	/// decodable file at the right size rather than failing or corrupting.
	#[test]
	fn exact_resize_uses_the_exact_path_and_still_decodes() {
		let pixels = fixture(48, 32);
		let bytes = encode_rgba(&EncodeJob { resize: Resize::to(24, 16), webp: WebpSettings { lossless: true, exact: true, ..Default::default() }, ..Default::default() }, &RgbaImage { width: 48, height: 32, pixels: &pixels }).expect("lossless resize encodes");
		assert_eq!(dimensions(&bytes), (24, 16));
	}

	/// The metadata writer is `WriteWebPWithMetadata`: VP8X added with the right flags,
	/// ICCP before the image, EXIF and XMP after, odd sizes padded, the RIFF size right.
	#[test]
	fn metadata_is_written_like_cwebp() {
		let mut png = Vec::new();
		image::RgbaImage::from_raw(48, 32, fixture(48, 32)).expect("fixture").write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).expect("png");
		let source = SourceImage { bytes: png, ..SourceImage::from_rgba(&RgbaImage { width: 48, height: 32, pixels: &fixture(48, 32) }) };
		let source_with = |bytes: Vec<u8>| SourceImage { bytes, format: SourceFormat::Webp, ..source.clone() };
		// A WebP source carrying all three, as the metadata to copy.
		let mut carrier = encode_rgba(&EncodeJob::default(), &source.as_rgba()).expect("encode");
		let carrier = {
			let metadata = Metadata { icc: Some(b"icc".to_vec()), exif: Some(b"exif".to_vec()), xmp: Some(b"xmp!".to_vec()) };
			carrier = super::with_metadata(carrier, &metadata, WebpMetadata { exif: true, icc: true, xmp: true }, (48, 32)).expect("with metadata");
			carrier
		};
		for lossless in [false, true] {
			let job = EncodeJob::from(WebpSettings { lossless, metadata: WebpMetadata { exif: true, icc: true, xmp: false }, ..Default::default() });
			let out = super::encode_source_with_progress(&job, &source_with(carrier.clone()), &mut |_| true).expect("encode");
			assert_eq!(&out[12..16], b"VP8X");
			let flags = out[20];
			assert_eq!(flags & 0x2c, 0x28, "ICC and Exif, no XMP: {flags:#x}");
			assert_eq!(flags & 0x10, 0x10, "the fixture has alpha");
			assert_eq!(u32::from_le_bytes(out[4..8].try_into().expect("four bytes")) as usize, out.len() - 8, "RIFF size");
			assert_eq!(&out[30..34], b"ICCP", "ICCP directly after VP8X");
			let copied = Metadata::read_like_cwebp(&out, SourceFormat::Webp);
			assert_eq!((copied.icc.as_deref(), copied.exif.as_deref(), copied.xmp.as_deref()), (Some(&b"icc"[..]), Some(&b"exif"[..]), None));
			let (width, height, _) = decode(&out);
			assert_eq!((width, height), (48, 32));
		}
	}

	#[test]
	fn a_malformed_buffer_is_rejected() {
		let err = encode_rgba(&EncodeJob::default(), &RgbaImage { width: 4, height: 4, pixels: &[0; 10] }).expect_err("a short buffer must be rejected");
		assert!(matches!(err, EncodeError::MalformedImage { expected: 64, actual: 10, .. }), "got {err:?}");
	}

	#[test]
	fn zero_dimensions_are_rejected() {
		assert!(encode_rgba(&EncodeJob::default(), &RgbaImage { width: 0, height: 4, pixels: &[] }).is_err());
		assert!(encode_rgba(&EncodeJob::default(), &RgbaImage { width: 4, height: 0, pixels: &[] }).is_err());
	}

	/// The hook must actually fire, with sane percentages, and must not change the output.
	#[test]
	fn progress_is_reported_without_changing_the_output() {
		let pixels = fixture(96, 64);
		let image = RgbaImage { width: 96, height: 64, pixels: &pixels };
		let settings = EncodeJob::default();

		let mut seen: Vec<u32> = Vec::new();
		let with_hook = super::encode_rgba_with_progress(&settings, &image, &mut |percent| {
			seen.push(percent);
			true
		})
		.expect("encodes");

		assert!(!seen.is_empty(), "libwebp reported no progress at all");
		assert!(seen.iter().all(|percent| *percent <= 100), "out-of-range percentages: {seen:?}");
		assert!(seen.windows(2).all(|pair| pair[0] <= pair[1]), "progress went backwards: {seen:?}");
		// Deliberately NOT asserting that the last report is 100. libwebp does not guarantee
		// it: a default-quality encode of this fixture stops reporting at 68. Completion is
		// signalled by the encode returning, not by the hook reaching 100, and a UI that
		// waits for 100% would sit at two-thirds forever.
		assert!(seen.len() > 1, "expected more than one progress report: {seen:?}");

		// Attaching a hook must not perturb the encoder.
		assert_eq!(with_hook, encode_rgba(&settings, &image).expect("encodes"), "the progress hook changed the output");
	}

	/// Returning false from the hook is how cancellation works, and it must be reported as
	/// a cancellation rather than as an encode failure.
	#[test]
	fn returning_false_cancels_the_encode() {
		let pixels = fixture(96, 64);
		let image = RgbaImage { width: 96, height: 64, pixels: &pixels };
		let mut calls = 0_u32;
		let error = super::encode_rgba_with_progress(&EncodeJob::default(), &image, &mut |_| {
			calls += 1;
			false
		})
		.expect_err("a cancelled encode must not succeed");
		assert!(matches!(error, EncodeError::Cancelled), "got {error:?}");
		assert_eq!(calls, 1, "the encode should stop at the first refusal");
	}

	/// Cancelling partway through must still be a cancellation, not a corrupt success.
	#[test]
	fn cancelling_partway_through_is_still_a_cancellation() {
		let pixels = fixture(128, 128);
		let image = RgbaImage { width: 128, height: 128, pixels: &pixels };
		let mut calls = 0_u32;
		let result = super::encode_rgba_with_progress(&EncodeJob::from(WebpSettings { method: 6, ..Default::default() }), &image, &mut |_| {
			calls += 1;
			calls < 2
		});
		// A small image may complete inside a single callback, in which case there was
		// nothing left to cancel — both outcomes are correct, a corrupt one is not.
		match result {
			Err(EncodeError::Cancelled) => {}
			Ok(bytes) => assert_eq!(&bytes[0..4], b"RIFF", "if it completed, it must be a valid file"),
			Err(other) => panic!("unexpected failure: {other}"),
		}
	}

	/// Quality must actually move the output size, or the control is decorative.
	#[test]
	fn quality_changes_the_output_size() {
		let low = encode(&EncodeJob::from(WebpSettings { quality: 5.0, ..Default::default() })).expect("q5 encodes");
		let high = encode(&EncodeJob::from(WebpSettings { quality: 95.0, ..Default::default() })).expect("q95 encodes");
		assert!(low.len() < high.len(), "q5 produced {} bytes, q95 produced {} bytes", low.len(), high.len());
	}
}
