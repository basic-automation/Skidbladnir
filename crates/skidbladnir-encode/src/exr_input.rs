//! `OpenEXR` input, read the way `cjxl` reads it (`lib/extras/dec/exr.cc`), with the `exr`
//! crate (pure Rust) standing in for the `OpenEXR` library's `InputFile`.
//!
//! What `cjxl` takes from a file: its first part's largest level; the R, G and B channels —
//! unprefixed, or else the first `layer.R`/`.G`/`.B` triple in channel order — or, with no
//! such triple, the first channel as gray; that prefix's `A` as alpha, which EXR stores
//! premultiplied; half or float samples as stored; the display window as the image, with
//! the data window's pixels placed in it and zero elsewhere; linear light with sRGB
//! primaries and a D65 white unless the file names its chromaticities; and its white
//! luminance as the intensity target. It refuses `UINT` and subsampled channels, and colour
//! or alpha channels of mixed types. Any other channels become extra channels in `cjxl`'s
//! output; Skidbladnir leaves them out (see [`Exr::extra_channels`]).

/// The EXR magic number, the first four bytes of every file.
pub const MAGIC: [u8; 4] = [0x76, 0x2f, 0x31, 0x01];

/// An EXR image as `cjxl` hands it to libjxl.
#[derive(Clone, Debug, PartialEq)]
pub struct Exr {
	/// The display window's width.
	pub width: u32,
	/// The display window's height.
	pub height: u32,
	/// One colour channel rather than three.
	pub gray: bool,
	/// Whether there is an alpha channel (premultiplied, as EXR stores it).
	pub alpha: bool,
	/// Whether the samples were 16-bit half floats rather than 32-bit floats.
	pub half: bool,
	/// Interleaved samples, gray or RGB, then alpha, row by row; halves widened exactly.
	pub samples: Vec<f32>,
	/// The file's chromaticities, red, green, blue and white `(x, y)`, if it names them.
	pub chromaticities: Option<[[f32; 2]; 4]>,
	/// The file's white luminance, or 0 when it has none.
	pub intensity_target: f32,
	/// How many channels beyond the colour and alpha the file has, which `cjxl` encodes as
	/// extra channels and Skidbladnir does not.
	pub extra_channels: usize,
}

impl Exr {
	/// The channels per pixel in [`Self::samples`].
	#[must_use]
	pub const fn channels(&self) -> usize {
		(if self.gray { 1 } else { 3 }) + if self.alpha { 1 } else { 0 }
	}
}

/// `FindColorLayerPrefix`: "" when R, G and B exist unprefixed, else the prefix (with its
/// dot) of the first channel, in channel order, that ends in `.R`, `.G` or `.B` and whose
/// prefix has all three.
fn colour_prefix(names: &[&str]) -> String {
	let has = |name: &str| names.contains(&name);
	if has("R") && has("G") && has("B") {
		return String::new();
	}
	for name in names {
		let Some(dot) = name.rfind('.') else { continue };
		if !matches!(&name[dot + 1..], "R" | "G" | "B") {
			continue;
		}
		let prefix = &name[..=dot];
		if ["R", "G", "B"].iter().all(|suffix| has(&format!("{prefix}{suffix}"))) {
			return prefix.to_owned();
		}
	}
	String::new()
}

/// Decode an EXR file as `cjxl` reads it.
///
/// # Errors
///
/// A description of why the file cannot be read, in `cjxl`'s terms where it refuses too.
pub fn decode(bytes: &[u8]) -> Result<Exr, String> {
	use exr::prelude::{FlatSamples, ReadChannels as _, ReadLayers as _};

	let image = exr::image::read::read().no_deep_data().largest_resolution_level().all_channels().first_valid_layer().all_attributes().from_buffered(std::io::Cursor::new(bytes)).map_err(|error| format!("not an EXR file exr can read: {error}"))?;
	let layer = &image.layer_data;
	let channels = &layer.channel_data.list;
	for channel in channels {
		if matches!(channel.sample_data, FlatSamples::U32(_)) {
			return Err("OpenEXR files with UINT channels are not supported, as cjxl does not read them".to_owned());
		}
		if channel.sampling.x() != 1 || channel.sampling.y() != 1 {
			return Err("OpenEXR files with subsampled channels are not supported, as cjxl does not read them".to_owned());
		}
	}
	let names: Vec<String> = channels.iter().map(|channel| channel.name.to_string()).collect();
	let names: Vec<&str> = names.iter().map(String::as_str).collect();
	let index = |name: &str| names.iter().position(|candidate| *candidate == name);
	let prefix = colour_prefix(&names);
	let rgb = [index(&format!("{prefix}R")), index(&format!("{prefix}G")), index(&format!("{prefix}B"))];
	let gray = rgb.iter().any(Option::is_none);
	let colour: Vec<usize> = if gray { vec![0] } else { rgb.iter().flatten().copied().collect() };
	if channels.is_empty() {
		return Err("the EXR file has no channels".to_owned());
	}
	let alpha = index(&format!("{prefix}A")).filter(|&alpha| !(gray && alpha == 0));
	let half = matches!(channels[colour[0]].sample_data, FlatSamples::F16(_));
	let is_half = |at: usize| matches!(channels[at].sample_data, FlatSamples::F16(_));
	if colour.iter().chain(alpha.iter()).any(|&at| is_half(at) != half) {
		return Err("OpenEXR color channels with different types are not supported, as cjxl does not read them".to_owned());
	}
	let used: Vec<usize> = colour.iter().chain(alpha.iter()).copied().collect();

	let display = image.attributes.display_window;
	let (display_x, display_y) = (display.position.x(), display.position.y());
	let (width, height) = (display.size.x(), display.size.y());
	let (data_x, data_y) = (layer.attributes.layer_position.x(), layer.attributes.layer_position.y());
	let (data_width, data_height) = (layer.size.x(), layer.size.y());
	if width == 0 || height == 0 || data_width == 0 || data_height == 0 {
		return Err("the EXR file's display or data window is empty".to_owned());
	}
	let pixels = width.checked_mul(height).filter(|&pixels| pixels <= 1 << 30).ok_or("the EXR image is too large")?;
	let per_pixel = used.len();
	let mut samples = vec![0.0_f32; pixels * per_pixel];
	// The data window's pixels inside the display window; anything else stays zero.
	let to_i64 = |value: usize| i64::try_from(value).unwrap_or(i64::MAX);
	let x1 = i64::from(data_x).max(i64::from(display_x));
	let x2 = (i64::from(data_x) + to_i64(data_width)).min(i64::from(display_x) + to_i64(width));
	let y1 = i64::from(data_y).max(i64::from(display_y));
	let y2 = (i64::from(data_y) + to_i64(data_height)).min(i64::from(display_y) + to_i64(height));
	let sample = |at: usize, offset: usize| -> f32 {
		match &channels[at].sample_data {
			FlatSamples::F16(values) => values[offset].to_f32(),
			FlatSamples::F32(values) => values[offset],
			FlatSamples::U32(_) => 0.0,
		}
	};
	let index_of = |value: i64| usize::try_from(value).unwrap_or(0);
	for y in y1..y2 {
		for x in x1..x2 {
			let source = index_of(y - i64::from(data_y)) * data_width + index_of(x - i64::from(data_x));
			let target = (index_of(y - i64::from(display_y)) * width + index_of(x - i64::from(display_x))) * per_pixel;
			for (channel, &at) in used.iter().enumerate() {
				samples[target + channel] = sample(at, source);
			}
		}
	}

	let chromaticities = image.attributes.chromaticities.map(|c| [[c.red.x(), c.red.y()], [c.green.x(), c.green.y()], [c.blue.x(), c.blue.y()], [c.white.x(), c.white.y()]]);
	Ok(Exr { width: u32::try_from(width).map_err(|_| "the EXR image is too wide")?, height: u32::try_from(height).map_err(|_| "the EXR image is too tall")?, gray, alpha: alpha.is_some(), half, samples, chromaticities, intensity_target: layer.attributes.white_luminance.unwrap_or(0.0), extra_channels: channels.len() - used.len() })
}

/// The image as 8- and 16-bit RGBA for the formats whose tools do not read EXR: the alpha
/// un-premultiplied, the linear light given the sRGB curve, clipped to the displayable
/// range (an EXR's brighter-than-white values have no place in an 8- or 16-bit sRGB image).
/// A file's own chromaticities are not converted.
#[must_use]
pub fn rgba(image: &Exr) -> (Vec<u8>, Vec<u16>) {
	let channels = image.channels();
	let encode = |linear: f32| -> f32 {
		let linear = if linear.is_nan() { 0.0 } else { linear.clamp(0.0, 1.0) };
		if linear <= 0.003_130_8 { linear * 12.92 } else { 1.055 * linear.powf(1.0 / 2.4) - 0.055 }
	};
	#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "rounded from 0..=65535")]
	let to_u16 = |value: f32| (value.clamp(0.0, 1.0) * 65535.0).round() as u16;
	let mut deep = Vec::with_capacity(image.samples.len() / channels * 4);
	for pixel in image.samples.chunks_exact(channels) {
		let alpha = if image.alpha { pixel[channels - 1] } else { 1.0 };
		let alpha = if alpha.is_nan() { 0.0 } else { alpha.clamp(0.0, 1.0) };
		let straight = |value: f32| if alpha > 0.0 { value / alpha } else { 0.0 };
		let (r, g, b) = if image.gray { (pixel[0], pixel[0], pixel[0]) } else { (pixel[0], pixel[1], pixel[2]) };
		deep.extend([to_u16(encode(straight(r))), to_u16(encode(straight(g))), to_u16(encode(straight(b))), to_u16(alpha)]);
	}
	let pixels = deep.iter().map(|&value| u8::try_from((u32::from(value) * 255 + 32_767) / 65_535).unwrap_or(u8::MAX)).collect();
	(pixels, deep)
}

#[cfg(test)]
mod tests {
	use super::colour_prefix;

	#[test]
	fn finds_the_colour_layer_as_cjxl_does() {
		assert_eq!(colour_prefix(&["A", "B", "G", "R"]), "");
		assert_eq!(colour_prefix(&["diffuse.B", "diffuse.G", "diffuse.R", "spec.B", "spec.G", "spec.R"]), "diffuse.");
		assert_eq!(colour_prefix(&["a.B", "a.G", "b.B", "b.G", "b.R"]), "b.");
		assert_eq!(colour_prefix(&["Y"]), "");
	}
}
