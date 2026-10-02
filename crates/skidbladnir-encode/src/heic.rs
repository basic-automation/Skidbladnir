//! HEIC output and input, through libheif — for output, the way `heif-enc -e kvazaar`
//! drives it.
//!
//! The HEVC encoder depends on the edition. The standard edition's is **Kvazaar**
//! (BSD-3-Clause); the GPL edition's, with the `x265` feature, is **x265**
//! (GPL-2.0-or-later). Either is built *into* one shared libheif (LGPL-3.0) by
//! `scripts/build-libheif.sh`, with the **libde265** decoder and with libheif's plugin
//! loading switched off. The app links that libheif dynamically and ships it beside itself,
//! which is how an ISC app uses an LGPL library, and with no plugin loading the standard
//! edition can never pick up x265 from the user's system. `build.rs` refuses a libheif
//! built for the other edition. x265's controls are what `heif-enc -e x265 -p` sets; its
//! lossless is `heif-enc -L`.
//!
//! The encode is `native/heic_shim.c`: `heif-enc`'s still-image path, the libheif calls it
//! makes after reading one image, as one C function. For a PNG, this module does what
//! `heif-enc`'s reader (libheif's `heifio/`) does before that: the samples laid out as the
//! image it builds (gray, gray and alpha, RGB or RGBA), the ICC profile, Exif and XMP by its
//! rules. A JPEG goes to `heif-enc`'s own JPEG reader, compiled into the shim
//! (`native/heic_jpeg.cc`), which keeps the JPEG's YCbCr planes.
//! `tests/heic_parity.rs` compares the result with a reference `heif-enc` built from the
//! same libheif, byte for byte.
//!
//! The encoder is asked for **by name**. A dev host without the vendored build links the
//! system libheif instead (see build.rs); only then does this fall back to whatever HEVC
//! encoder that libheif offers, so a release can never silently encode with x265.
//!
//! Kvazaar encodes 8-bit 4:2:0 (or 4:0:0) only, so a deeper source is encoded from its
//! high bytes; `heif-enc` refuses one. Like AVIF, it reports no progress, so progress moves
//! at the start and end of a file.
//!
//! The declarations for decoding are written from libheif 1.23's headers. `heif_error` is
//! returned by value, and every enum is `c_int`.

use std::{
	borrow::Cow, ffi::{CStr, CString, c_char, c_int, c_void}, ptr, sync::Once
};

use crate::{
	encoder::{EncodeError, transformed}, metadata::{jpeg_icc, jpeg_markers, libpng_iccp, png_chunks, png_text}, settings::{ChromaDownsampling, ColorProfile, EncodeJob, HEIC_X265, HeicBitDepth, HeicPreset, HeicSettings, OmafProjection}, source::{SourceFormat, SourceImage}
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

const HEIF_ERROR_OK: c_int = 0;
const HEIF_COLORSPACE_RGB: c_int = 1;
const HEIF_CHROMA_INTERLEAVED_RGBA: c_int = 11;
const HEIF_CHANNEL_INTERLEAVED: c_int = 10;

unsafe extern "C" {
	fn heif_get_version() -> *const c_char;
	fn heif_init(params: *const c_void) -> HeifError;
	fn heif_context_alloc() -> *mut Opaque;
	fn heif_context_free(ctx: *mut Opaque);
	fn heif_image_get_plane_readonly2(image: *const Opaque, channel: c_int, out_stride: *mut usize) -> *const u8;
	fn heif_image_get_width(image: *const Opaque, channel: c_int) -> c_int;
	fn heif_image_get_height(image: *const Opaque, channel: c_int) -> c_int;
	fn heif_image_release(image: *const Opaque);
	fn heif_context_read_from_memory_without_copy(ctx: *mut Opaque, data: *const c_void, size: usize, options: *const c_void) -> HeifError;
	fn heif_context_get_primary_image_handle(ctx: *mut Opaque, out: *mut *mut Opaque) -> HeifError;
	fn heif_image_handle_get_width(handle: *const Opaque) -> c_int;
	fn heif_image_handle_get_height(handle: *const Opaque) -> c_int;
	fn heif_image_handle_release(handle: *const Opaque);
	fn heif_decode_image(handle: *const Opaque, out: *mut *mut Opaque, colorspace: c_int, chroma: c_int, options: *const c_void) -> HeifError;
	fn heif_context_get_encoder_descriptors(ctx: *mut Opaque, format: c_int, name_filter: *const c_char, out: *mut *const Opaque, count: c_int) -> c_int;
	fn heif_encoder_descriptor_get_name(descriptor: *const Opaque) -> *const c_char;
}

const HEIF_COMPRESSION_HEVC: c_int = 1;

#[repr(C)]
struct SkidHeicInput {
	width: u32,
	height: u32,
	layout: c_int,
	has_alpha: c_int,
	planes: [*const u8; 4],
	strides: [usize; 4],
	icc: *const u8,
	icc_size: usize,
	exif: *const u8,
	exif_size: usize,
	xmp: *const u8,
	xmp_size: usize,
	orientation: c_int,
	jpeg: *const u8,
	jpeg_size: usize,
	ten_bit: c_int,
	heif: *const u8,
	heif_size: usize,
	keep_exif: c_int,
	keep_xmp: c_int,
}

#[repr(C)]
struct SkidHeicParameter {
	name: *const c_char,
	value: *const c_char,
}

#[repr(C)]
struct SkidHeicSettings {
	encoder: *const c_char,
	quality: c_int,
	lossless: c_int,
	parameters: *const SkidHeicParameter,
	parameter_count: usize,
	alpha: c_int,
	premultiplied: c_int,
	thumbnail: c_int,
	thumbnail_alpha: c_int,
	chroma_downsampling: c_int,
	color_profile: c_int,
	matrix_coefficients: c_int,
	colour_primaries: c_int,
	transfer_characteristics: c_int,
	full_range: c_int,
	two_colr_boxes: c_int,
	clli_set: c_int,
	clli: [u16; 2],
	pasp_set: c_int,
	pasp: [u32; 2],
	orientation: c_int,
	cut_tiles: c_int,
	omaf_projection: c_int,
	description: *const c_char,
	brands: *const c_char,
	brand_count: usize,
	unif: c_int,
	mini: c_int,
}

unsafe extern "C" {
	fn skid_heic_encode(input: *const SkidHeicInput, settings: *const SkidHeicSettings, out: *mut *mut u8, out_size: *mut usize, error: *mut c_char, error_size: usize) -> c_int;
	fn skid_heic_free(data: *mut u8);
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

/// The name of the HEVC encoder this edition uses, and asks libheif for by name.
const ENCODER: &CStr = if HEIC_X265 { c"x265" } else { c"kvazaar" };

/// The descriptor of the HEVC encoder called `name`, or `None` if the linked libheif lacks it.
fn descriptor(ctx: &Context, name: &CStr) -> Option<*const Opaque> {
	let mut descriptor: *const Opaque = ptr::null();
	// SAFETY: `ctx` is live, the filter is a NUL-terminated string, and one slot is offered.
	let found = unsafe { heif_context_get_encoder_descriptors(ctx.0, HEIF_COMPRESSION_HEVC, name.as_ptr(), &raw mut descriptor, 1) };
	(found > 0 && !descriptor.is_null()).then_some(descriptor)
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

/// How the samples are laid out, as the `heif_image` `heif-enc`'s readers build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
	/// A `Y` plane, and an alpha plane when `alpha`.
	Gray { alpha: bool },
	/// Interleaved RGB, or RGBA when `alpha`.
	Rgb { alpha: bool },
	/// YCbCr 4:2:0 planes (`Y`, `Cb`, `Cr`), and an alpha plane when `alpha`: a lossy WebP.
	Yuv420 { alpha: bool },
}

/// What `heif-enc`'s readers make of an input file, before the encoder sees it.
#[derive(Debug)]
struct Input {
	layout: Layout,
	/// The planes: gray and alpha, or the one interleaved plane. Tightly packed.
	planes: Vec<Vec<u8>>,
	icc: Option<Vec<u8>>,
	exif: Option<Vec<u8>>,
	xmp: Option<Vec<u8>>,
	/// The Exif orientation of a JPEG, 1 to 8; libheif treats anything else as 1.
	orientation: u16,
}

impl Input {
	/// Whether the image built from this input has an alpha channel.
	const fn has_alpha(&self) -> bool {
		matches!(self.layout, Layout::Gray { alpha: true } | Layout::Rgb { alpha: true } | Layout::Yuv420 { alpha: true })
	}

	/// The source's samples in this layout.
	fn samples(source: &SourceImage, layout: Layout) -> Vec<Vec<u8>> {
		let pixels = source.pixels.as_chunks::<4>().0;
		match layout {
			Layout::Gray { alpha } => {
				let mut planes = vec![pixels.iter().map(|p| p[0]).collect()];
				if alpha {
					planes.push(pixels.iter().map(|p| p[3]).collect());
				}
				planes
			}
			Layout::Rgb { alpha: true } => vec![source.pixels.clone()],
			Layout::Rgb { alpha: false } => vec![pixels.iter().flat_map(|p| [p[0], p[1], p[2]]).collect()],
			// Only a lossy WebP is laid out this way, from its own planes (`Self::webp`).
			Layout::Yuv420 { .. } => Vec::new(),
		}
	}

	/// The layout for a source `heif-enc` cannot read: its own channels.
	fn layout(source: &SourceImage) -> Layout {
		if source.gray { Layout::Gray { alpha: source.has_alpha } } else { Layout::Rgb { alpha: source.has_alpha } }
	}

	/// `heifio/decoder_png.cc`, `loadPNG`. libpng expands palettes (a palette's `tRNS`
	/// becoming alpha) and low-depth gray, and ignores a gray or RGB image's `tRNS`. The
	/// ICC profile is libpng's `iCCP`; Exif the first `eXIf` libpng accepts, with its
	/// orientation set to 1, since a PNG's orientation is not applied; XMP the last
	/// `XML:com.adobe.xmp` text chunk.
	fn png(source: &SourceImage) -> Self {
		let chunks = png_chunks(&source.bytes).unwrap_or_default();
		let head = &chunks[..chunks.iter().position(|(kind, _)| kind == b"IDAT").unwrap_or(chunks.len())];
		let colour_type = chunks.iter().find(|(kind, _)| kind == b"IHDR").and_then(|(_, data)| data.get(9).copied()).unwrap_or(6);
		let layout = match colour_type {
			0 => Layout::Gray { alpha: false },
			4 => Layout::Gray { alpha: true },
			2 => Layout::Rgb { alpha: false },
			3 => {
				// png_handle_tRNS: after PLTE, one to as many entries as the palette has.
				let entries = head.iter().find(|(kind, _)| kind == b"PLTE").map(|(_, data)| data.len() / 3);
				let plte = head.iter().position(|(kind, _)| kind == b"PLTE");
				let trns = head.iter().position(|(kind, _)| kind == b"tRNS");
				let alpha = match (plte, trns, entries) {
					(Some(plte), Some(trns), Some(entries)) => trns > plte && (1..=entries.min(256)).contains(&head[trns].1.len()),
					_ => false,
				};
				Layout::Rgb { alpha }
			}
			_ => Layout::Rgb { alpha: true },
		};
		let icc = head.iter().find(|(kind, _)| kind == b"iCCP").and_then(|(_, chunk)| libpng_iccp(chunk, colour_type & 2 != 0));
		let exif = chunks.iter().find(|(kind, data)| kind == b"eXIf" && data.len() >= 4 && (data.starts_with(b"II*\0") || data.starts_with(b"MM\0*"))).map(|(_, data)| {
			let mut exif = data.to_vec();
			set_exif_orientation(&mut exif, 1);
			exif
		});
		let xmp = chunks.iter().filter_map(|(kind, data)| png_text(*kind, data)).rfind(|(keyword, text)| keyword == "XML:com.adobe.xmp" && !text.is_empty()).map(|(_, text)| text);
		Self { layout, planes: Self::samples(source, layout), icc, exif, xmp, orientation: 1 }
	}

	/// A cropped or resized JPEG (or one `heif-enc`'s reader refuses), whose pixels are no
	/// longer the file's: they go through RGB, with the file's metadata read the way
	/// `heifio/decoder_jpeg.cc` reads it. The ICC profile is reassembled from its APP2
	/// segments; Exif and XMP are the first APP1 of each kind, the Exif kept whole and its
	/// orientation applied to the file's `irot`/`imir`. A JPEG as it is goes to that reader
	/// itself, in the shim.
	fn jpeg(source: &SourceImage) -> Self {
		let markers = jpeg_markers(&source.bytes);
		let layout = if source.gray { Layout::Gray { alpha: false } } else { Layout::Rgb { alpha: false } };
		let app1 = |signature: &[u8], skip: usize| markers.iter().find(|(marker, data)| *marker == 0xe1 && data.len() >= skip && data.starts_with(signature)).map(|(_, data)| data[skip..].to_vec());
		let exif = app1(b"Exif\0\0", 6);
		let xmp = app1(b"http://ns.adobe.com/xap/1.0/", 29);
		let orientation = exif.as_deref().map_or(1, exif_orientation);
		Self { layout, planes: Self::samples(source, layout), icc: jpeg_icc(&markers), exif: exif.filter(|exif| !exif.is_empty()), xmp: xmp.filter(|xmp| !xmp.is_empty()), orientation }
	}

	/// `heifio/decoder_webp.cc`: a lossy WebP decoded straight to its YCbCr 4:2:0 planes and
	/// alpha, never through RGB; a lossless one to RGB(A), which is [`Self::other`]. The
	/// metadata is the file's `ICCP`, `EXIF` and `XMP ` chunks either way. A cropped or
	/// resized WebP is no longer the file's planes, and goes through RGB too.
	fn webp(source: &SourceImage) -> Self {
		let lossy = crate::inspect::inspect_webp(&source.bytes).is_some_and(|info| info.compression == crate::inspect::WebpCompression::Lossy && !info.has_animation);
		match lossy.then(|| crate::encoder::webp_yuv420_planes(&source.bytes)).flatten() {
			Some((width, height, alpha, planes)) if (width, height) == (source.width, source.height) => {
				let metadata = crate::metadata::webp_chunks(&source.bytes);
				Self { layout: Layout::Yuv420 { alpha }, planes, icc: metadata.icc, exif: metadata.exif, xmp: metadata.xmp, orientation: 1 }
			}
			_ => Self::other(source),
		}
	}

	/// `heifio/decoder_y4m.cc`: `heif-enc` reads a Y4M's first frame as 8-bit 4:2:0 planes,
	/// whatever its header says, from a header whose `W` and `H` it parses and a frame line
	/// that is exactly `FRAME`. Where that reading is the file's (8-bit 4:2:0, which is
	/// also a header with no `C` tag) the planes go in as they are, as `heif-enc` puts them;
	/// any other Y4M, which `heif-enc` misreads or refuses, is converted to RGB instead.
	fn y4m(source: &SourceImage) -> Self {
		y4m_420_planes(&source.bytes).filter(|(width, height, _)| (*width, *height) == (source.width, source.height)).map_or_else(|| Self::other(source), |(_, _, planes)| Self { layout: Layout::Yuv420 { alpha: false }, planes, icc: None, exif: None, xmp: None, orientation: 1 })
	}

	/// A format `heif-enc` does not read here: its samples as they are, and the metadata
	/// its container carries.
	fn other(source: &SourceImage) -> Self {
		let metadata = crate::metadata::Metadata::read_like_cwebp(&source.bytes, source.format);
		let layout = Self::layout(source);
		Self { layout, planes: Self::samples(source, layout), icc: metadata.icc, exif: metadata.exif, xmp: metadata.xmp, orientation: 1 }
	}
}

/// Where the Orientation tag's entry is in a TIFF-header Exif block, and its byte order:
/// `heifio/exif.cc`'s `find_exif_tag`, which searches IFD0, the Exif IFD it points to, and
/// the IFDs chained after them, at most five deep.
fn find_exif_orientation(exif: &[u8]) -> Option<(usize, bool)> {
	fn search(exif: &[u8], offset: u32, little: bool, depth: u32) -> Option<usize> {
		let size = u32::try_from(exif.len()).ok()?;
		if depth > 5 || offset == 0 || size < 6 || size - 6 < offset {
			return None;
		}
		let read16 = |at: u32| exif.get(at as usize..at as usize + 2).map(|b| if little { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) });
		let read32 = |at: u32| exif.get(at as usize..at as usize + 4).map(|b| if little { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) } else { u32::from_be_bytes([b[0], b[1], b[2], b[3]]) });
		let count = u32::from(read16(offset)?);
		let table = 2 + count * 12 + 4;
		if table > size || size - table < offset {
			return None;
		}
		for i in 0..count {
			let entry = offset + 2 + i * 12;
			match read16(entry)? {
				0x0112 => return Some(entry as usize),
				0x8769 => {
					if let Some(found) = search(exif, read32(entry + 8)?, little, depth + 1) {
						return Some(found);
					}
				}
				_ => {}
			}
		}
		search(exif, read32(offset + 2 + count * 12)?, little, depth + 1)
	}
	if exif.len() < 8 || !matches!(exif[0], b'I' | b'M') || !matches!(exif[1], b'I' | b'M') {
		return None;
	}
	let little = exif[0] == b'I';
	let first = if little { u32::from_le_bytes([exif[4], exif[5], exif[6], exif[7]]) } else { u32::from_be_bytes([exif[4], exif[5], exif[6], exif[7]]) };
	search(exif, first, little, 1).map(|at| (at, little))
}

/// The Orientation entry's type and count, when it holds one SHORT: `(value position,
/// little-endian)`.
fn exif_orientation_value(exif: &[u8]) -> Option<(usize, bool)> {
	let (at, little) = find_exif_orientation(exif)?;
	let entry = exif.get(at..at + 12)?;
	let (kind, count) = if little { (u16::from_le_bytes([entry[2], entry[3]]), u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]])) } else { (u16::from_be_bytes([entry[2], entry[3]]), u32::from_be_bytes([entry[4], entry[5], entry[6], entry[7]])) };
	(kind == 3 && count == 1).then_some((at + 8, little))
}

/// `read_exif_orientation_tag`: the orientation, or 1.
fn exif_orientation(exif: &[u8]) -> u16 {
	exif_orientation_value(exif).map_or(1, |(at, little)| if little { u16::from_le_bytes([exif[at], exif[at + 1]]) } else { u16::from_be_bytes([exif[at], exif[at + 1]]) })
}

/// `modify_exif_orientation_tag_if_it_exists`.
fn set_exif_orientation(exif: &mut [u8], orientation: u16) {
	if let Some((at, little)) = exif_orientation_value(exif) {
		exif[at..at + 2].copy_from_slice(&if little { orientation.to_le_bytes() } else { orientation.to_be_bytes() });
	}
}

/// Encode a source to HEIC as `heif-enc -e kvazaar` would with these settings.
///
/// # Errors
///
/// Returns [`EncodeError`] if the settings are out of range, libheif fails (as it does for
/// an Exif block with no TIFF header, which `heif-enc` refuses too), or `on_progress` asked
/// to stop.
pub fn encode(job: &EncodeJob, source: &SourceImage, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Vec<u8>, EncodeError> {
	let settings = &job.heic;
	settings.validate()?;
	let source = transformed(source, job.crop, job.resize)?;
	if !on_progress(0) {
		return Err(EncodeError::Cancelled);
	}
	let ten_bit = HEIC_X265 && settings.bit_depth == HeicBitDepth::Ten && !settings.lossless;
	let mut input = match source.format {
		_ if source.bytes.is_empty() => Input::other(&source),
		SourceFormat::Png => Input::png(&source),
		SourceFormat::Jpeg => Input::jpeg(&source),
		// 10-bit input is made from pixels, which `heif-enc` cannot do.
		SourceFormat::Webp if !ten_bit => Input::webp(&source),
		SourceFormat::Y4m if !ten_bit && matches!(source, Cow::Borrowed(_)) => Input::y4m(&source),
		_ => Input::other(&source),
	};
	// Metadata left out is metadata the input never had.
	let keep = settings.metadata;
	for (kept, slot) in [(keep.icc, &mut input.icc), (keep.exif, &mut input.exif), (keep.xmp, &mut input.xmp)] {
		if !kept {
			*slot = None;
		}
	}
	// libheif encodes transparency with a second x265 that does not get the TU inter depth
	// fixed for a small image (see `x265_parameters`), and placebo's is one x265 refuses.
	if HEIC_X265 && input.has_alpha() && settings.preset == HeicPreset::Placebo && source.width.min(source.height) < 32 {
		return Err(EncodeError::Heic("x265's placebo preset cannot encode the transparency of an image under 32 pixels on a side; use veryslow".to_owned()));
	}
	// A JPEG as it is on disk goes to `heif-enc`'s own reader, with any metadata left out
	// removed from it first. Where that reader refuses it (an RGB-coded JPEG, say),
	// `heif-enc` writes nothing, and the pixels are encoded instead. 10-bit input is made
	// from pixels, which `heif-enc` cannot do.
	let jpeg = (source.format == SourceFormat::Jpeg && !source.bytes.is_empty() && matches!(source, Cow::Borrowed(_)) && !ten_bit).then(|| without_jpeg_metadata(&source.bytes, keep));
	// A HEIC as it is goes to `heif-enc`'s HEIF reader (`heifio/decoder_heif.cc`), decoded in
	// its own colourspace. That reader cannot leave the colour profile out, so with ICC
	// dropped the pixels are encoded instead.
	let heif = (source.format == SourceFormat::Heic && !source.bytes.is_empty() && matches!(source, Cow::Borrowed(_)) && !ten_bit && keep.icc).then_some(source.bytes.as_slice());
	let file = jpeg.as_deref().map(SourceFile::Jpeg).or(heif.map(SourceFile::Heif));
	let output = match encode_input(settings, source.width, source.height, &input, file, ten_bit) {
		Err(Refusal::Jpeg(_)) => encode_input(settings, source.width, source.height, &input, None, ten_bit),
		other => other,
	}
	.map_err(|refusal| EncodeError::Heic(refusal.into_message()))?;
	if !on_progress(100) {
		return Err(EncodeError::Cancelled);
	}
	Ok(output)
}

/// Why the shim wrote nothing.
/// A file handed to one of `heif-enc`'s own readers in the shim, rather than as samples.
#[derive(Clone, Copy)]
enum SourceFile<'a> {
	Jpeg(&'a [u8]),
	Heif(&'a [u8]),
}

enum Refusal {
	/// `heif-enc`'s JPEG or HEIF reader could not read the file.
	Jpeg(String),
	/// Anything else.
	Other(String),
}

impl Refusal {
	fn into_message(self) -> String {
		match self {
			Self::Jpeg(message) | Self::Other(message) => message,
		}
	}
}

/// A JPEG with the metadata segments left out removed: ICC `APP2`, Exif and XMP `APP1`.
fn without_jpeg_metadata(jpeg: &[u8], keep: crate::settings::HeicMetadata) -> Cow<'_, [u8]> {
	if keep.icc && keep.exif && keep.xmp {
		return Cow::Borrowed(jpeg);
	}
	let drop = |marker: u8, data: &[u8]| (!keep.icc && marker == 0xe2 && data.starts_with(b"ICC_PROFILE\0")) || (!keep.exif && marker == 0xe1 && data.starts_with(b"Exif\0\0")) || (!keep.xmp && marker == 0xe1 && data.starts_with(b"http://ns.adobe.com/xap/1.0/"));
	let mut out = jpeg[..2].to_vec();
	let mut at = 2;
	while at + 4 <= jpeg.len() && jpeg[at] == 0xff {
		let marker = jpeg[at + 1];
		if marker == 0xda || marker == 0xd9 || marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
			break;
		}
		let end = at + 2 + usize::from(u16::from_be_bytes([jpeg[at + 2], jpeg[at + 3]]));
		let Some(segment) = jpeg.get(at..end) else { break };
		if !drop(marker, &segment[4..]) {
			out.extend_from_slice(segment);
		}
		at = end;
	}
	out.extend_from_slice(&jpeg[at..]);
	Cow::Owned(out)
}

/// x265's controls as `heif-enc -p` parameters, for an image this size.
pub(crate) fn x265_parameters(s: &HeicSettings, (width, height): (u32, u32)) -> Vec<(String, String)> {
	let tenths = |value: u16| format!("{}.{}", value / 10, value % 10);
	let mut parameters = Vec::new();
	// -L sets the chroma itself.
	if !s.lossless {
		parameters.push(("chroma".to_owned(), s.chroma.parameter().to_owned()));
	}
	parameters.push(("preset".to_owned(), s.preset.parameter().to_owned()));
	parameters.push(("tune".to_owned(), s.tune.parameter().to_owned()));
	// libheif codes an image under 32 pixels on a side in 16-pixel CTUs, where x265 allows
	// transform units at most 3 levels deep, for inter prediction too. The preset's inter
	// depth (4 for placebo) goes unused in a still image, but x265 still checks it.
	let small = width.min(height) < 32;
	parameters.push(("tu-intra-depth".to_owned(), (if small { s.tu_intra_depth.min(3) } else { s.tu_intra_depth }).to_string()));
	if small {
		parameters.push(("x265:tu-inter-depth".to_owned(), "3".to_owned()));
	}
	// Straight to x265. libheif applies these after its own settings and the preset and
	// tune, so they override all of them.
	parameters.push(("x265:aq-mode".to_owned(), s.aq_mode.parameter().to_owned()));
	parameters.push(("x265:aq-strength".to_owned(), tenths(u16::from(s.aq_strength))));
	parameters.push(("x265:psy-rd".to_owned(), tenths(u16::from(s.psy_rd))));
	parameters.push(("x265:psy-rdoq".to_owned(), tenths(s.psy_rdoq)));
	parameters.push(("x265:deblock".to_owned(), if s.deblock { format!("{}:{}", s.deblock_strength, s.deblock_threshold) } else { "false".to_owned() }));
	parameters.push(("x265:sao".to_owned(), (if s.sao { "true" } else { "false" }).to_owned()));
	parameters
}

fn encode_input(s: &HeicSettings, width: u32, height: u32, input: &Input, file: Option<SourceFile<'_>>, ten_bit: bool) -> Result<Vec<u8>, Refusal> {
	let (jpeg, heif) = match file {
		Some(SourceFile::Jpeg(bytes)) => (Some(bytes), None),
		Some(SourceFile::Heif(bytes)) => (None, Some(bytes)),
		None => (None, None),
	};
	if c_int::try_from(width).is_err() || c_int::try_from(height).is_err() {
		return Err(Refusal::Other(format!("{width}x{height} is too large for HEIC")));
	}
	init();
	let description = CString::new(s.description.clone()).map_err(|_| Refusal::Other("the description contains a NUL character".to_owned()))?;
	let brands: Vec<u8> = s.compatible_brands.iter().flat_map(|brand| brand.bytes()).collect();
	let bytes = |data: Option<&Vec<u8>>| data.map_or((ptr::null(), 0), |data| (data.as_ptr(), data.len()));
	let (icc, icc_size) = bytes(input.icc.as_ref());
	let (exif, exif_size) = bytes(input.exif.as_ref());
	let (xmp, xmp_size) = bytes(input.xmp.as_ref());
	let (w, uv) = (width as usize, width.div_ceil(2) as usize);
	let (layout, has_alpha, strides) = match input.layout {
		Layout::Gray { alpha } => (0, alpha, [w, w, 0, 0]),
		Layout::Rgb { alpha } => (1, alpha, [w * if alpha { 4 } else { 3 }, w, 0, 0]),
		Layout::Yuv420 { alpha } => (2, alpha, [w, uv, uv, w]),
	};
	let plane = |i: usize| input.planes.get(i).map_or(ptr::null(), Vec::as_ptr);
	let raw = SkidHeicInput { width, height, layout, has_alpha: c_int::from(has_alpha), planes: [plane(0), plane(1), plane(2), plane(3)], strides, icc, icc_size, exif, exif_size, xmp, xmp_size, orientation: c_int::from(input.orientation), jpeg: jpeg.map_or(ptr::null(), <[u8]>::as_ptr), jpeg_size: jpeg.map_or(0, <[u8]>::len), ten_bit: c_int::from(ten_bit), heif: heif.map_or(ptr::null(), <[u8]>::as_ptr), heif_size: heif.map_or(0, <[u8]>::len), keep_exif: c_int::from(s.metadata.exif), keep_xmp: c_int::from(s.metadata.xmp) };

	// Kvazaar's lossless is its `-p lossless=true`; x265's is `-L`.
	let parameters: Vec<(String, String)> = if HEIC_X265 {
		x265_parameters(s, (width, height))
	} else if s.lossless {
		vec![("lossless".to_owned(), "true".to_owned())]
	} else {
		Vec::new()
	};
	let parameters: Vec<(CString, CString)> = parameters.into_iter().map(|(name, value)| Ok((CString::new(name)?, CString::new(value)?))).collect::<Result<_, std::ffi::NulError>>().map_err(|_| Refusal::Other("an encoder parameter contains a NUL character".to_owned()))?;
	let parameter_pointers: Vec<SkidHeicParameter> = parameters.iter().map(|(name, value)| SkidHeicParameter { name: name.as_ptr(), value: value.as_ptr() }).collect();

	let (color_profile, [matrix_coefficients, colour_primaries, transfer_characteristics], full_range) = match s.color_profile {
		ColorProfile::Custom { matrix_coefficients, colour_primaries, transfer_characteristics, full_range } => (0, [matrix_coefficients, colour_primaries, transfer_characteristics].map(c_int::from), full_range),
		ColorProfile::Auto => (1, [0; 3], true),
		ColorProfile::Bt601 => (2, [0; 3], true),
		ColorProfile::Bt709 | ColorProfile::Compatible => (3, [0; 3], true),
		ColorProfile::Bt2020 => (4, [0; 3], true),
	};
	let size = |value: Option<u32>| value.map_or(0, |value| c_int::try_from(value).unwrap_or(c_int::MAX));
	let settings = SkidHeicSettings { encoder: if SYSTEM_LIBHEIF { ptr::null() } else { ENCODER.as_ptr() }, quality: c_int::from(s.quality), lossless: c_int::from(HEIC_X265 && s.lossless), parameters: parameter_pointers.as_ptr(), parameter_count: parameter_pointers.len(), alpha: c_int::from(s.alpha), premultiplied: c_int::from(s.premultiplied_alpha), thumbnail: size(s.thumbnail), thumbnail_alpha: c_int::from(s.thumbnail_alpha), chroma_downsampling: s.chroma_downsampling.map_or(0, ChromaDownsampling::as_libheif), color_profile, matrix_coefficients, colour_primaries, transfer_characteristics, full_range: c_int::from(full_range), two_colr_boxes: c_int::from(s.two_colr_boxes), clli_set: c_int::from(s.clli.is_some()), clli: s.clli.unwrap_or([0; 2]), pasp_set: c_int::from(s.pasp.is_some()), pasp: s.pasp.unwrap_or([0; 2]), orientation: s.orientation.as_libheif(), cut_tiles: size(s.cut_tiles), omaf_projection: s.omaf_projection.map_or(-1, OmafProjection::as_libheif), description: description.as_ptr(), brands: brands.as_ptr().cast(), brand_count: s.compatible_brands.len(), unif: c_int::from(s.unif), mini: c_int::from(s.mini) };

	let mut out: *mut u8 = ptr::null_mut();
	let mut out_size = 0_usize;
	let mut error = [0 as c_char; 512];
	// SAFETY: every pointer in `raw` and `settings` addresses a live buffer of the stated
	// length for the whole call (each plane is `height` rows of its stride); the shim copies
	// what it keeps. On success `out` is a `malloc`ed buffer of `out_size` bytes, released
	// below with `skid_heic_free`.
	let ok = unsafe { skid_heic_encode(&raw const raw, &raw const settings, &raw mut out, &raw mut out_size, error.as_mut_ptr(), error.len()) };
	if ok != 1 {
		// SAFETY: the shim NUL-terminates the message within the buffer.
		let message = unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy().into_owned();
		return Err(if ok == 2 { Refusal::Jpeg(message) } else { Refusal::Other(message) });
	}
	// SAFETY: as above; copied before it is freed, exactly once.
	let file = unsafe { std::slice::from_raw_parts(out, out_size) }.to_vec();
	unsafe { skid_heic_free(out) };
	Ok(file)
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

/// A Y4M's first frame as `heif-enc` reads it, when that reading is the file's own: its
/// size and its Y, Cb and Cr planes, for an 8-bit 4:2:0 file (a `C420*` tag or none) with a
/// bare `FRAME` line. `None` for anything else.
fn y4m_420_planes(bytes: &[u8]) -> Option<(u32, u32, Vec<Vec<u8>>)> {
	let header_end = bytes.iter().position(|&b| b == b'\n')?;
	let header = std::str::from_utf8(&bytes[..header_end]).ok()?.strip_prefix("YUV4MPEG2 ")?;
	let (mut width, mut height) = (None, None);
	for field in header.split(' ').filter(|field| !field.is_empty()) {
		let (tag, value) = field.split_at(1);
		match tag {
			"W" => width = value.parse::<u32>().ok(),
			"H" => height = value.parse::<u32>().ok(),
			"C" if !matches!(value, "420" | "420jpeg" | "420paldv" | "420mpeg2") => return None,
			_ => {}
		}
	}
	let (width, height) = (width?, height?);
	let rest = &bytes[header_end + 1..];
	let frame_end = rest.iter().position(|&b| b == b'\n')?;
	if &rest[..frame_end] != b"FRAME" {
		return None;
	}
	let data = &rest[frame_end + 1..];
	let (w, h) = (width as usize, height as usize);
	let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
	let luma = w.checked_mul(h)?;
	let chroma = cw.checked_mul(ch)?;
	if data.len() < luma + 2 * chroma {
		return None;
	}
	Some((width, height, vec![data[..luma].to_vec(), data[luma..luma + chroma].to_vec(), data[luma + chroma..luma + 2 * chroma].to_vec()]))
}

#[cfg(test)]
mod tests {
	use super::{SYSTEM_LIBHEIF, decode, dimensions, encode, encoder_name, exif_orientation, has_encoder, linked_version, set_exif_orientation, y4m_420_planes};
	use crate::{
		encoder::{EncodeError, RgbaImage}, settings::{EncodeJob, HEIC_X265, HeicSettings, OutputFormat, Resize}, source::SourceImage
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

	fn run(settings: HeicSettings, resize: Resize, width: u32, height: u32, pixels: &[u8], on_progress: &mut dyn FnMut(u32) -> bool) -> Result<Vec<u8>, EncodeError> {
		let job = EncodeJob { format: OutputFormat::Heic, resize, heic: settings, ..EncodeJob::default() };
		encode(&job, &SourceImage::from_rgba(&RgbaImage { width, height, pixels }), on_progress)
	}

	#[test]
	fn links_libheif_1_23_or_newer() {
		let version = linked_version();
		let mut parts = version.split('.').map(|part| part.parse::<u32>().unwrap_or(0));
		let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
		assert!(major > 1 || minor >= 23, "linked libheif is {version}");
	}

	#[test]
	fn writes_a_heic_file_that_reads_back_at_the_right_size_with_alpha() {
		let pixels = fixture(64, 48, true);
		let bytes = run(HeicSettings::default(), Resize::default(), 64, 48, &pixels, &mut |_| true).expect("encode");
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
		let measure = |quality| {
			let bytes = run(HeicSettings { quality, ..HeicSettings::default() }, Resize::default(), 64, 64, &pixels, &mut |_| true).expect("encode");
			let (_, _, rgba) = decode(&bytes).expect("decode");
			(bytes.len(), psnr(&pixels, &rgba))
		};
		let (low_size, low_psnr) = measure(20);
		let (high_size, high_psnr) = measure(90);
		assert!(high_psnr > low_psnr, "quality 90 at {high_psnr:.1} dB vs quality 20 at {low_psnr:.1} dB");
		assert!(high_size > low_size, "quality 90 is {high_size} bytes vs quality 20 at {low_size}");
	}

	#[test]
	fn applies_the_shared_resize() {
		let pixels = fixture(64, 48, false);
		let bytes = run(HeicSettings::default(), Resize::to(32, 0), 64, 48, &pixels, &mut |_| true).expect("encode");
		assert_eq!(dimensions(&bytes), Some((32, 24)));
	}

	#[test]
	fn a_cancel_returns_nothing_and_bad_settings_are_refused() {
		let pixels = fixture(8, 8, false);
		assert_eq!(run(HeicSettings::default(), Resize::default(), 8, 8, &pixels, &mut |_| false), Err(EncodeError::Cancelled));
		assert!(run(HeicSettings { quality: 101, ..HeicSettings::default() }, Resize::default(), 8, 8, &pixels, &mut |_| true).is_err());
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

	/// Kvazaar's lossless coding (`-p lossless=true`) keeps the 4:2:0 image exactly: what
	/// decodes differs from the source only by the conversion to 4:2:0 and back, which the
	/// lossy encode adds to.
	#[test]
	#[cfg(not(feature = "x265"))]
	fn kvazaars_lossless_coding_is_closer_than_its_best_lossy() {
		let pixels = fixture(64, 48, false);
		let closeness = |settings: HeicSettings| psnr(&pixels, &decode(&run(settings, Resize::default(), 64, 48, &pixels, &mut |_| true).expect("encode")).expect("decode").2);
		let lossless = closeness(HeicSettings { lossless: true, ..HeicSettings::default() });
		let best = closeness(HeicSettings { quality: 100, ..HeicSettings::default() });
		assert!(lossless >= best, "lossless at {lossless:.1} dB, quality 100 at {best:.1} dB");
	}

	#[test]
	fn a_file_that_is_not_heif_is_refused() {
		assert!(decode(b"not a heif file").is_err());
		assert_eq!(dimensions(b"not a heif file"), None);
	}

	/// A big-endian Exif block whose IFD0 holds an Orientation SHORT.
	fn exif_with_orientation(orientation: u16) -> Vec<u8> {
		let mut exif = b"MM\0*\0\0\0\x08".to_vec();
		exif.extend_from_slice(&1_u16.to_be_bytes());
		exif.extend_from_slice(&[0x01, 0x12, 0, 3, 0, 0, 0, 1]);
		exif.extend_from_slice(&orientation.to_be_bytes());
		exif.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
		exif
	}

	#[test]
	fn the_exif_orientation_is_read_and_reset_as_heifio_does() {
		let mut exif = exif_with_orientation(6);
		assert_eq!(exif_orientation(&exif), 6);
		set_exif_orientation(&mut exif, 1);
		assert_eq!(exif_orientation(&exif), 1);
		assert_eq!(exif_orientation(b"not exif"), 1, "no TIFF header reads as the default");
	}

	/// x265's controls, only in the GPL edition.
	#[cfg(feature = "x265")]
	mod x265 {
		use std::{
			ffi::{c_int, c_void}, ptr
		};

		use super::{fixture, psnr, run};
		use crate::{
			heic::{Context, HeifError, Opaque, check, decode}, settings::{EncodeJob, HeicAqMode, HeicBitDepth, HeicChroma, HeicPreset, HeicSettings, HeicTune, OutputFormat, Resize}, source::{SourceFormat, SourceImage}
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
			run(settings.clone(), Resize::default(), width, height, pixels, &mut |_| true).expect("encode")
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

		/// An ICC profile labels the pixels; it must not cost lossless its identity matrix.
		#[test]
		fn lossless_with_an_icc_profile_is_still_exact() {
			let pixels = fixture(64, 48, false);
			let mut png = Vec::new();
			{
				let mut info = png::Info::with_size(64, 48);
				info.color_type = png::ColorType::Rgba;
				info.bit_depth = png::BitDepth::Eight;
				info.icc_profile = Some(std::borrow::Cow::Owned(crate::metadata::tests::test_icc(*b"RGB ")));
				png::Encoder::with_info(&mut png, info).expect("a valid header").write_header().expect("header").write_image_data(&pixels).expect("pixels");
			}
			let source = SourceImage { format: SourceFormat::Png, bytes: png, ..SourceImage::from_rgba(&crate::encoder::RgbaImage { width: 64, height: 48, pixels: &pixels }) };
			let job = EncodeJob { format: OutputFormat::Heic, heic: HeicSettings { lossless: true, ..HeicSettings::default() }, ..EncodeJob::default() };
			let bytes = crate::heic::encode(&job, &source, &mut |_| true).expect("encode");
			let (_, _, rgba) = decode(&bytes).expect("decode");
			assert_eq!(rgba, pixels, "lossless with an ICC profile changed pixels");
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
						match run(settings.clone(), Resize::default(), width, height, &pixels, &mut |_| true) {
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

	/// `heif-enc`'s Y4M reading is taken only where it is the file's own: 8-bit 4:2:0 with a
	/// bare `FRAME` line. Everything else goes through RGB.
	#[test]
	fn y4m_planes_only_for_what_heif_enc_reads_correctly() {
		let file = |header: &str, frame: &str, size: usize| [header.as_bytes(), b"\n", frame.as_bytes(), b"\n", &vec![128; size]].concat();
		let planes = y4m_420_planes(&file("YUV4MPEG2 W4 H2 F25:1 C420jpeg", "FRAME", 12)).expect("4:2:0 is read as planes");
		assert_eq!((planes.0, planes.1, planes.2.iter().map(Vec::len).collect::<Vec<_>>()), (4, 2, vec![8, 2, 2]));
		assert!(y4m_420_planes(&file("YUV4MPEG2 W4 H2", "FRAME", 12)).is_some(), "no C tag is 4:2:0");
		assert!(y4m_420_planes(&file("YUV4MPEG2 W4 H2 C444", "FRAME", 24)).is_none(), "heif-enc misreads 4:4:4");
		assert!(y4m_420_planes(&file("YUV4MPEG2 W4 H2 C420p10", "FRAME", 24)).is_none(), "and 10-bit");
		assert!(y4m_420_planes(&file("YUV4MPEG2 W4 H2", "FRAME Ixyz", 12)).is_none(), "and refuses frame parameters");
		assert!(y4m_420_planes(&file("YUV4MPEG2 W4 H2", "FRAME", 11)).is_none(), "a short frame is not read");
	}
}
