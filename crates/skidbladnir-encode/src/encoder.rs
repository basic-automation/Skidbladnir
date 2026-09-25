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

use libwebp_sys::{WEBP_ENCODER_ABI_VERSION, WebPConfig, WebPConfigInitInternal, WebPEncode, WebPMemoryWrite, WebPMemoryWriter, WebPMemoryWriterClear, WebPMemoryWriterInit, WebPPicture, WebPPictureCopy, WebPPictureFree, WebPPictureImportRGBA, WebPPictureInitInternal, WebPPictureRescale, WebPPreset, WebPValidateConfig};
use thiserror::Error;

use crate::settings::{AlphaFiltering, EncodeSettings, FilterType, Mode, Preset, TargetMetric};

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
#[derive(Clone, Debug, Error, PartialEq, Eq)]
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
	/// The range checks in [`EncodeSettings::validate`] should make this unreachable, so
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
}

/// Translate settings into a libwebp `WebPConfig`.
///
/// The order of assignment matters and is deliberately the order the flags appear on the
/// `cwebp` command line, because `-preset` is not a field but a *re-initialisation*:
/// `WebPConfigPreset` resets the whole config to the preset's values, keeping only the
/// quality. Applying it first, as the Electron app's command line does, means the
/// explicit controls that follow win — which is the behaviour a user of the old app saw.
fn build_config(settings: &EncodeSettings) -> Result<WebPConfig, EncodeError> {
	settings.validate()?;

	// WebPConfigInit: the inline helper libwebp declares in its header is not exported,
	// so call the internal entry point it expands to.
	let mut config = unsafe { std::mem::zeroed::<WebPConfig>() };
	if unsafe { WebPConfigInitInternal(&raw mut config, WebPPreset::WEBP_PRESET_DEFAULT, 75.0, abi_version()) } == 0 {
		return Err(EncodeError::Libwebp("WebPConfigInit"));
	}

	match settings.mode {
		Mode::Preset => {
			if let Some(preset) = settings.preset {
				// WebPConfigPreset(&config, preset, config.quality).
				let quality = config.quality;
				if unsafe { WebPConfigInitInternal(&raw mut config, webp_preset(preset), quality, abi_version()) } == 0 {
					return Err(EncodeError::Libwebp("WebPConfigPreset"));
				}
			}
		}
		Mode::NearLossless => {
			config.near_lossless = i32::from(settings.quality);
			// cwebp.c: "use near-lossless only with lossless". Without this the encode
			// silently becomes an ordinary lossy one.
			config.lossless = 1;
		}
		Mode::JpegLike => config.emulate_jpeg_size = 1,
		Mode::Lossless => {
			config.lossless = 1;
			config.exact = 1;
		}
		Mode::Lossy => {}
	}

	if settings.mode.uses_lossy_options() {
		match settings.filter {
			FilterType::Auto => config.autofilter = 1,
			FilterType::Simple => config.filter_type = 0,
			FilterType::Strong => config.filter_type = 1,
		}
	}

	if settings.multi_threading {
		config.thread_level = 1;
	}

	if settings.mode.uses_lossy_options() && settings.filter.uses_manual_strength() {
		config.filter_strength = i32::from(settings.filter_strength);
		config.filter_sharpness = i32::from(settings.filter_sharpness);
	}

	if settings.mode.uses_lossy_options() && settings.sharp_yuv {
		config.use_sharp_yuv = 1;
	}

	if settings.mode != Mode::Preset
		&& let Some(alpha_filtering) = settings.alpha_filtering
	{
		config.alpha_filtering = match alpha_filtering {
			AlphaFiltering::Off => 0,
			AlphaFiltering::Fast => 1,
			AlphaFiltering::Best => 2,
		};
	}

	if settings.mode != Mode::NearLossless {
		config.quality = f32::from(settings.quality);
	}

	if matches!(settings.mode, Mode::Lossy | Mode::Lossless | Mode::JpegLike) {
		config.alpha_quality = i32::from(settings.alpha_quality);
		config.method = i32::from(settings.method);
	}

	if settings.mode.uses_lossy_options() {
		if settings.low_memory {
			config.low_memory = 1;
		}
		config.segments = i32::from(settings.segments);
		config.partition_limit = i32::from(settings.partition_limit);

		match settings.target {
			Some(TargetMetric::Size(bytes)) => config.target_size = bytes.cast_signed(),
			Some(TargetMetric::Psnr(psnr)) => {
				// The Electron app asks for -print_psnr alongside -psnr, and cwebp sets
				// show_compressed for it. Matched so the config is identical.
				config.show_compressed = 1;
				config.target_PSNR = f32::from(u16::try_from(psnr).unwrap_or(u16::MAX));
			}
			None => {}
		}

		config.pass = i32::from(settings.passes);
		config.sns_strength = i32::from(settings.sns);
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
const fn abi_version() -> c_int {
	WEBP_ENCODER_ABI_VERSION.cast_signed()
}

/// Map our preset enum onto libwebp's.
const fn webp_preset(preset: Preset) -> WebPPreset {
	match preset {
		Preset::Default => WebPPreset::WEBP_PRESET_DEFAULT,
		Preset::Photo => WebPPreset::WEBP_PRESET_PHOTO,
		Preset::Picture => WebPPreset::WEBP_PRESET_PICTURE,
		Preset::Drawing => WebPPreset::WEBP_PRESET_DRAWING,
		Preset::Icon => WebPPreset::WEBP_PRESET_ICON,
		Preset::Text => WebPPreset::WEBP_PRESET_TEXT,
	}
}

/// Owns a `WebPPicture` so its buffers are released even if an encode step fails.
struct Picture(WebPPicture);

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
pub fn encode_rgba(settings: &EncodeSettings, image: &RgbaImage<'_>) -> Result<Vec<u8>, EncodeError> {
	encode_rgba_with_progress(settings, image, &mut |_| true)
}

/// Encode an RGBA image, reporting progress and allowing the caller to stop.
///
/// `on_progress` is called by libwebp with a percentage from 0 to 100. Returning `false`
/// aborts the encode, which is how cancellation works: there is no way to interrupt
/// `WebPEncode` from another thread, so stopping has to come from inside its own progress
/// callback. A cancelled encode returns [`EncodeError::Cancelled`] rather than a failure,
/// because the user asked for it.
///
/// Two things about libwebp's reporting that a progress bar has to allow for, both
/// observed rather than assumed: it does not call the hook at a fixed cadence, and **it
/// does not guarantee a final call at 100** — a default-quality encode of a 96x64 image
/// stops reporting at 68. Completion is signalled by this function returning, so a UI
/// that waits for 100% will sit at two-thirds forever.
///
/// # Errors
///
/// As [`encode_rgba`], plus [`EncodeError::Cancelled`] if `on_progress` returned `false`.
pub fn encode_rgba_with_progress(settings: &EncodeSettings, image: &RgbaImage<'_>, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Vec<u8>, EncodeError> {
	let config = build_config(settings)?;

	let expected = (image.width as usize).checked_mul(image.height as usize).and_then(|pixels| pixels.checked_mul(4)).ok_or(EncodeError::ImageTooLarge { width: image.width, height: image.height })?;
	if image.width == 0 || image.height == 0 || image.pixels.len() != expected {
		return Err(EncodeError::MalformedImage { width: image.width, height: image.height, actual: image.pixels.len(), expected });
	}
	let (width, height) = (i32::try_from(image.width), i32::try_from(image.height));
	let (Ok(width), Ok(height)) = (width, height) else {
		return Err(EncodeError::ImageTooLarge { width: image.width, height: image.height });
	};

	let mut picture = Picture(unsafe {
		let mut picture = std::mem::zeroed::<WebPPicture>();
		if WebPPictureInitInternal(&raw mut picture, abi_version()) == 0 {
			return Err(EncodeError::Libwebp("WebPPictureInit"));
		}
		picture
	});

	// cwebp.c decides the colour path *before* reading the input, because it changes
	// which conversion the samples go through. Matching it is a parity requirement, not
	// an optimisation.
	let resizing = !settings.resize.is_noop();
	picture.0.use_argb = c_int::from(config.lossless == 1 || config.use_sharp_yuv == 1 || config.preprocessing > 0 || resizing);
	picture.0.width = width;
	picture.0.height = height;

	// 4 bytes per pixel, tightly packed, so the stride is the row length.
	let stride = width.checked_mul(4).ok_or(EncodeError::ImageTooLarge { width: image.width, height: image.height })?;
	// SAFETY: beyond the module-wide invariants, this one depends on the buffer actually
	// being as large as the dimensions claim — libwebp reads `height * stride` bytes from
	// it. That was checked above: `pixels.len()` is exactly `width * height * 4`.
	if unsafe { WebPPictureImportRGBA(&raw mut picture.0, image.pixels.as_ptr(), stride) } == 0 {
		return Err(EncodeError::Libwebp("WebPPictureImportRGBA"));
	}

	if resizing {
		resize_picture(&mut picture, settings, &config)?;
	}

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

	let encoded = unsafe { WebPEncode(&raw const config, &raw mut picture.0) };
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
	let bytes = if writer.0.mem.is_null() { Vec::new() } else { unsafe { std::slice::from_raw_parts(writer.0.mem, writer.0.size) }.to_vec() };
	Ok(bytes)
}

/// Rescale the picture the way `cwebp` does, including its `-exact` special case.
///
/// A plain `WebPPictureRescale` premultiplies RGB by alpha, which destroys the colour of
/// fully transparent pixels — exactly what `-exact` promises to preserve, and the
/// Electron app pairs `-exact` with every lossless encode. `cwebp` works around it by
/// rescaling an opaque copy for the colour channels and the real picture for alpha, then
/// reassembling. Without this, a lossless resize diverges from `cwebp` in every
/// transparent pixel.
fn resize_picture(picture: &mut Picture, settings: &EncodeSettings, config: &WebPConfig) -> Result<(), EncodeError> {
	let target_w = i32::try_from(settings.resize.width).map_err(|_| EncodeError::ImageTooLarge { width: settings.resize.width, height: settings.resize.height })?;
	let target_h = i32::try_from(settings.resize.height).map_err(|_| EncodeError::ImageTooLarge { width: settings.resize.width, height: settings.resize.height })?;

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
	use crate::settings::{AlphaFiltering, EncodeSettings, FilterType, Mode, Preset, Resize, TargetMetric};

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

	fn encode(settings: &EncodeSettings) -> Result<Vec<u8>, EncodeError> {
		let pixels = fixture(48, 32);
		encode_rgba(settings, &RgbaImage { width: 48, height: 32, pixels: &pixels })
	}

	/// Every control must reach the corresponding `WebPConfig` field. This is the parity
	/// claim at the config level; `tests/parity.rs` is the claim at the byte level.
	#[test]
	fn every_lossy_control_reaches_webpconfig() {
		let settings = EncodeSettings { mode: Mode::Lossy, quality: 42, alpha_quality: 33, alpha_filtering: Some(AlphaFiltering::Best), method: 5, segments: 2, partition_limit: 17, sns: 81, passes: 9, filter: FilterType::Strong, filter_strength: 64, filter_sharpness: 6, target: Some(TargetMetric::Size(12_345)), sharp_yuv: true, low_memory: true, multi_threading: true, ..Default::default() };
		let config = build_config(&settings).expect("config builds");
		assert!((config.quality - 42.0).abs() < f32::EPSILON);
		assert_eq!(config.alpha_quality, 33);
		assert_eq!(config.alpha_filtering, 2);
		assert_eq!(config.method, 5);
		assert_eq!(config.segments, 2);
		assert_eq!(config.partition_limit, 17);
		assert_eq!(config.sns_strength, 81);
		assert_eq!(config.pass, 9);
		assert_eq!(config.filter_type, 1, "strong filtering");
		assert_eq!(config.filter_strength, 64);
		assert_eq!(config.filter_sharpness, 6);
		assert_eq!(config.target_size, 12_345);
		assert_eq!(config.use_sharp_yuv, 1);
		assert_eq!(config.low_memory, 1);
		assert_eq!(config.thread_level, 1);
		assert_eq!(config.lossless, 0);
		assert_eq!(config.autofilter, 0, "an explicit filter type replaces autofilter");
	}

	#[test]
	fn auto_filter_sets_autofilter_and_leaves_strength_alone() {
		let config = build_config(&EncodeSettings { filter: FilterType::Auto, filter_strength: 99, ..Default::default() }).expect("config builds");
		assert_eq!(config.autofilter, 1);
		assert_ne!(config.filter_strength, 99, "auto filtering must not adopt the manual strength");
	}

	#[test]
	fn simple_filter_selects_filter_type_zero() {
		let config = build_config(&EncodeSettings { filter: FilterType::Simple, ..Default::default() }).expect("config builds");
		assert_eq!(config.filter_type, 0);
		assert_eq!(config.autofilter, 0);
	}

	/// The mapping that is invisible from the Electron app: `-near_lossless` implies
	/// `lossless = 1` in cwebp.c, and without it the encode is an ordinary lossy one.
	#[test]
	fn near_lossless_implies_lossless() {
		let config = build_config(&EncodeSettings { mode: Mode::NearLossless, quality: 60, ..Default::default() }).expect("config builds");
		assert_eq!(config.near_lossless, 60);
		assert_eq!(config.lossless, 1, "cwebp.c: use near-lossless only with lossless");
	}

	#[test]
	fn lossless_sets_exact() {
		let config = build_config(&EncodeSettings { mode: Mode::Lossless, ..Default::default() }).expect("config builds");
		assert_eq!(config.lossless, 1);
		assert_eq!(config.exact, 1);
	}

	#[test]
	fn jpeg_like_sets_emulate_jpeg_size() {
		assert_eq!(build_config(&EncodeSettings { mode: Mode::JpegLike, ..Default::default() }).expect("config builds").emulate_jpeg_size, 1);
	}

	/// A preset is a re-initialisation, so this pins libwebp's actual preset table
	/// (`config_enc.c`) rather than asserting that "something changed". Getting one of
	/// these wrong silently encodes a user's `-preset photo` as something else.
	#[test]
	fn presets_match_libwebps_own_table() {
		// (preset, sns_strength, filter_strength, filter_sharpness, segments)
		let expected = [(Preset::Default, 50, 60, 0, 4), (Preset::Picture, 80, 35, 4, 4), (Preset::Photo, 80, 30, 3, 4), (Preset::Drawing, 25, 10, 6, 4), (Preset::Icon, 0, 0, 0, 4), (Preset::Text, 0, 0, 0, 2)];
		for (preset, sns, strength, sharpness, segments) in expected {
			let config = build_config(&EncodeSettings { mode: Mode::Preset, preset: Some(preset), ..Default::default() }).expect("config builds");
			assert_eq!((config.sns_strength, config.filter_strength, config.filter_sharpness, config.segments), (sns, strength, sharpness, segments), "{preset:?}");
		}
	}

	/// `-q` follows `-preset` on the command line, so it must survive the
	/// re-initialisation the preset performs.
	#[test]
	fn quality_after_a_preset_still_wins() {
		let config = build_config(&EncodeSettings { mode: Mode::Preset, preset: Some(Preset::Icon), quality: 30, ..Default::default() }).expect("config builds");
		assert!((config.quality - 30.0).abs() < f32::EPSILON);
	}

	/// A preset must not have the Electron app's `-alpha_filter best` forced onto it: the
	/// app deliberately clears that flag in preset mode, leaving libwebp's default of 1.
	#[test]
	fn preset_mode_keeps_libwebps_alpha_filtering() {
		let config = build_config(&EncodeSettings { mode: Mode::Preset, preset: Some(Preset::Photo), alpha_filtering: Some(AlphaFiltering::Best), ..Default::default() }).expect("config builds");
		assert_eq!(config.alpha_filtering, 1, "preset mode must not override the preset's alpha filtering");
	}

	#[test]
	fn psnr_target_sets_show_compressed_like_cwebp() {
		let config = build_config(&EncodeSettings { target: Some(TargetMetric::Psnr(41)), ..Default::default() }).expect("config builds");
		assert!((config.target_PSNR - 41.0).abs() < f32::EPSILON);
		assert_eq!(config.show_compressed, 1, "cwebp sets show_compressed for -print_psnr");
		assert_eq!(config.target_size, 0);
	}

	#[test]
	fn invalid_settings_are_rejected_before_libwebp_sees_them() {
		let err = build_config(&EncodeSettings { method: 9, ..Default::default() }).expect_err("method 9 is out of range");
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

	#[test]
	fn a_default_encode_produces_a_webp() {
		let bytes = encode(&EncodeSettings::default()).expect("default settings encode");
		assert!(bytes.len() > 20, "suspiciously small output: {} bytes", bytes.len());
		assert_eq!(&bytes[0..4], b"RIFF");
		assert_eq!(&bytes[8..12], b"WEBP");
	}

	#[test]
	fn every_mode_encodes() {
		for mode in [Mode::Lossy, Mode::Lossless, Mode::NearLossless, Mode::JpegLike, Mode::Preset] {
			let settings = EncodeSettings { mode, preset: Some(Preset::Photo), ..Default::default() };
			let bytes = encode(&settings).unwrap_or_else(|e| panic!("{mode:?} failed to encode: {e}"));
			assert_eq!(&bytes[0..4], b"RIFF", "{mode:?} did not produce a RIFF container");
		}
	}

	/// Lossless output must decode back to the exact input, which is the only end-to-end
	/// check that the pixels actually survived the import path.
	#[test]
	fn lossless_round_trips_the_pixels() {
		let pixels = fixture(48, 32);
		let bytes = encode_rgba(&EncodeSettings { mode: Mode::Lossless, ..Default::default() }, &RgbaImage { width: 48, height: 32, pixels: &pixels }).expect("lossless encodes");
		let mut width = 0;
		let mut height = 0;
		let decoded = unsafe { libwebp_sys::WebPDecodeRGBA(bytes.as_ptr(), bytes.len(), &raw mut width, &raw mut height) };
		assert!(!decoded.is_null(), "lossless output must decode");
		assert_eq!((width, height), (48, 32));
		let round_tripped = unsafe { std::slice::from_raw_parts(decoded, pixels.len()) }.to_vec();
		unsafe { libwebp_sys::WebPFree(decoded.cast()) };
		assert_eq!(round_tripped, pixels, "lossless + exact must preserve every byte, including RGB under transparent pixels");
	}

	#[test]
	fn resize_changes_the_output_dimensions() {
		let pixels = fixture(48, 32);
		for (resize, expected) in [(Resize { width: 24, height: 16 }, (24, 16)), (Resize { width: 24, height: 0 }, (24, 16)), (Resize { width: 0, height: 16 }, (24, 16))] {
			let bytes = encode_rgba(&EncodeSettings { resize, ..Default::default() }, &RgbaImage { width: 48, height: 32, pixels: &pixels }).expect("resized encode");
			let mut width = 0;
			let mut height = 0;
			assert_ne!(unsafe { libwebp_sys::WebPGetInfo(bytes.as_ptr(), bytes.len(), &raw mut width, &raw mut height) }, 0);
			assert_eq!((width, height), expected, "resize {resize:?}");
		}
	}

	/// The `-exact` resize path is a different code path; make sure it still produces a
	/// decodable file at the right size rather than failing or corrupting.
	#[test]
	fn lossless_resize_uses_the_exact_path_and_still_decodes() {
		let pixels = fixture(48, 32);
		let bytes = encode_rgba(&EncodeSettings { mode: Mode::Lossless, resize: Resize { width: 24, height: 16 }, ..Default::default() }, &RgbaImage { width: 48, height: 32, pixels: &pixels }).expect("lossless resize encodes");
		let mut width = 0;
		let mut height = 0;
		assert_ne!(unsafe { libwebp_sys::WebPGetInfo(bytes.as_ptr(), bytes.len(), &raw mut width, &raw mut height) }, 0);
		assert_eq!((width, height), (24, 16));
	}

	#[test]
	fn a_malformed_buffer_is_rejected() {
		let err = encode_rgba(&EncodeSettings::default(), &RgbaImage { width: 4, height: 4, pixels: &[0; 10] }).expect_err("a short buffer must be rejected");
		assert!(matches!(err, EncodeError::MalformedImage { expected: 64, actual: 10, .. }), "got {err:?}");
	}

	#[test]
	fn zero_dimensions_are_rejected() {
		assert!(encode_rgba(&EncodeSettings::default(), &RgbaImage { width: 0, height: 4, pixels: &[] }).is_err());
		assert!(encode_rgba(&EncodeSettings::default(), &RgbaImage { width: 4, height: 0, pixels: &[] }).is_err());
	}

	/// The hook must actually fire, with sane percentages, and must not change the output.
	#[test]
	fn progress_is_reported_without_changing_the_output() {
		let pixels = fixture(96, 64);
		let image = RgbaImage { width: 96, height: 64, pixels: &pixels };
		let settings = EncodeSettings::default();

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
		let error = super::encode_rgba_with_progress(&EncodeSettings::default(), &image, &mut |_| {
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
		let result = super::encode_rgba_with_progress(&EncodeSettings { method: 6, ..Default::default() }, &image, &mut |_| {
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
		let low = encode(&EncodeSettings { quality: 5, ..Default::default() }).expect("q5 encodes");
		let high = encode(&EncodeSettings { quality: 95, ..Default::default() }).expect("q95 encodes");
		assert!(low.len() < high.len(), "q5 produced {} bytes, q95 produced {} bytes", low.len(), high.len());
	}
}
