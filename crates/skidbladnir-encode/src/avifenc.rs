//! Turning an [`EncodeJob`] into the `avifenc` command line that makes the same file.
//!
//! The AVIF half of the parity claim, as [`crate::cwebp`] and [`crate::cjxl`] are for
//! their formats: `tests/avif_parity.rs` runs a reference `avifenc` with these arguments
//! and compares its file with ours. Options `avifenc` treats as "not given" are left out
//! when the setting is `None`, because giving them changes more than their own value.

use std::{ffi::OsString, path::Path};

use crate::settings::{CleanAperture, EncodeJob, MetadataSource, Tiling, YuvFormat};

/// Build the `avifenc` argument list for these settings, encoding `input` to `output`.
#[must_use]
pub fn avifenc_args(job: &EncodeJob, input: &Path, output: &Path) -> Vec<OsString> {
	let s = &job.avif;
	let mut args: Vec<String> = Vec::new();
	let push = |args: &mut Vec<String>, arg: &str| args.push(arg.to_owned());
	let pair = |args: &mut Vec<String>, flag: &str, value: String| args.extend([flag.to_owned(), value]);

	pair(&mut args, "--jobs", s.jobs.map_or_else(|| "all".to_owned(), |jobs| jobs.to_string()));
	pair(&mut args, "--speed", s.speed.map_or_else(|| "default".to_owned(), |speed| speed.to_string()));
	if let Some(quality) = s.quality {
		pair(&mut args, "--qcolor", quality.to_string());
	}
	if let Some(quality) = s.quality_alpha {
		pair(&mut args, "--qalpha", quality.to_string());
	}
	if s.lossless {
		push(&mut args, "--lossless");
	}
	match (s.depth, s.depth_extension) {
		(Some(depth), Some(extension)) => pair(&mut args, "--depth", format!("{depth},{extension}")),
		(Some(depth), None) => pair(&mut args, "--depth", depth.to_string()),
		_ => {}
	}
	if s.yuv != YuvFormat::Auto {
		pair(&mut args, "--yuv", s.yuv.as_avifenc_str().to_owned());
	}
	if s.premultiply {
		push(&mut args, "--premultiply");
	}
	if s.sharp_yuv {
		push(&mut args, "--sharpyuv");
	}
	if let Some(cicp) = s.cicp {
		pair(&mut args, "--cicp", format!("{}/{}/{}", cicp.primaries, cicp.transfer, cicp.matrix));
	}
	if s.limited_range {
		pair(&mut args, "--range", "limited".to_owned());
	}
	if let Some(size) = s.target_size {
		pair(&mut args, "--target-size", size.to_string());
	}
	if s.progressive {
		push(&mut args, "--progressive");
	}
	if let Some(grid) = s.grid {
		pair(&mut args, "--grid", format!("{}x{}", grid.columns, grid.rows));
	}
	if let Some(q) = s.quantizer {
		pair(&mut args, "--min", q.min.to_string());
		pair(&mut args, "--max", q.max.to_string());
	}
	if let Some(q) = s.alpha_quantizer {
		pair(&mut args, "--minalpha", q.min.to_string());
		pair(&mut args, "--maxalpha", q.max.to_string());
	}
	if let Tiling::Manual { rows_log2, cols_log2 } = s.tiling {
		pair(&mut args, "--tilerowslog2", rows_log2.to_string());
		pair(&mut args, "--tilecolslog2", cols_log2.to_string());
	}
	if let Some(fraction) = s.scaling_mode {
		pair(&mut args, "--scaling-mode", format!("{}/{}", fraction.numerator, fraction.denominator));
	}
	for option in &s.codec_options {
		pair(&mut args, "--advanced", if option.value.is_empty() { option.key.clone() } else { format!("{}={}", option.key, option.value) });
	}
	if let Some([h, v]) = s.pasp {
		pair(&mut args, "--pasp", format!("{h},{v}"));
	}
	match s.clean_aperture {
		Some(CleanAperture::Crop(values)) => pair(&mut args, "--crop", values.map(|v| v.to_string()).join(",")),
		Some(CleanAperture::Raw(values)) => pair(&mut args, "--clap", values.map(|v| v.to_string()).join(",")),
		None => {}
	}
	if let Some(irot) = s.irot {
		pair(&mut args, "--irot", irot.to_string());
	}
	if let Some(imir) = s.imir {
		pair(&mut args, "--imir", imir.to_string());
	}
	if let Some([max_cll, max_pall]) = s.clli {
		pair(&mut args, "--clli", format!("{max_cll},{max_pall}"));
	}
	for (source, ignore, flag) in [(&s.icc, "--ignore-icc", "--icc"), (&s.exif, "--ignore-exif", "--exif"), (&s.xmp, "--ignore-xmp", "--xmp")] {
		match source {
			MetadataSource::Keep => {}
			MetadataSource::Strip => push(&mut args, ignore),
			MetadataSource::File(path) => pair(&mut args, flag, path.display().to_string()),
		}
	}
	let mut args: Vec<OsString> = args.into_iter().map(OsString::from).collect();
	args.push(input.as_os_str().to_os_string());
	args.push(output.as_os_str().to_os_string());
	args
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use super::avifenc_args;
	use crate::settings::{AvifSettings, CodecOption, EncodeJob, MetadataSource, OutputFormat, QuantizerRange, Tiling, YuvFormat};

	fn line(job: &EncodeJob) -> String {
		avifenc_args(job, Path::new("in.png"), Path::new("out.avif")).iter().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" ")
	}

	#[test]
	fn defaults_leave_avifenc_to_its_own() {
		assert_eq!(line(&EncodeJob { format: OutputFormat::Avif, ..Default::default() }), "--jobs all --speed 6 in.png out.avif");
	}

	#[test]
	fn every_setting_reaches_the_command_line() {
		let job = EncodeJob { format: OutputFormat::Avif, avif: AvifSettings { quality: Some(40), speed: None, depth: Some(12), depth_extension: Some(4), yuv: YuvFormat::Yuv420, quantizer: Some(QuantizerRange { min: 10, max: 30 }), tiling: Tiling::Manual { rows_log2: 1, cols_log2: 2 }, codec_options: vec![CodecOption { key: "tune".into(), value: "ssim".into() }, CodecOption { key: "color:enable-chroma-deltaq".into(), value: String::new() }], icc: MetadataSource::Strip, ..Default::default() }, ..Default::default() };
		let rendered = line(&job);
		for expected in ["--speed default", "--qcolor 40", "--depth 12,4", "--yuv 420", "--min 10 --max 30", "--tilerowslog2 1 --tilecolslog2 2", "--advanced tune=ssim", "--advanced color:enable-chroma-deltaq ", "--ignore-icc"] {
			assert!(rendered.contains(expected), "missing `{expected}` in {rendered}");
		}
	}
}
