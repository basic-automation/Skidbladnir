//! Turning an [`EncodeJob`] into the `cwebp` command line that makes the same file.
//!
//! This is the other half of the parity claim. [`crate::encoder`] builds a `WebPConfig`
//! from the settings; this builds the `cwebp` arguments from the same settings; and
//! `tests/parity.rs` runs both and compares the bytes. If a field of [`WebpSettings`] had
//! no flag here, or a flag had no field, one side or the other would be unreachable.
//!
//! Every setting is written out, including the ones at their default, so the command line
//! is fully determined by the settings and never depends on `cwebp`'s defaults agreeing
//! with ours. It is also a record: a user can see the exact `cwebp` invocation their
//! settings stand for.
//!
//! [`WebpSettings`]: crate::settings::WebpSettings

use std::{ffi::OsString, path::Path};

use crate::settings::{EncodeJob, FilterType, ResizeMode, TargetMetric};

/// Build the `cwebp` argument list for these settings, encoding `input` to `output`.
///
/// The returned vector excludes the `cwebp` program name itself, so it can be handed
/// straight to [`std::process::Command::args`]. No flag here depends on another's position:
/// `-preset` and `-z`, the two that do, are never emitted, because the settings already
/// hold the values they would have set.
#[must_use]
pub fn cwebp_args(job: &EncodeJob, input: &Path, output: &Path) -> Vec<OsString> {
	let s = &job.webp;
	let mut args: Vec<String> = Vec::new();
	let mut flag = |name: &str, values: &[String]| {
		args.push(name.to_owned());
		args.extend(values.iter().cloned());
	};

	if s.lossless {
		flag("-lossless", &[]);
		if s.near_lossless < 100 {
			flag("-near_lossless", &[s.near_lossless.to_string()]);
		}
	}
	if s.exact {
		flag("-exact", &[]);
	}
	// `{}` prints the shortest decimal that reads back as the same f32.
	flag("-q", &[s.quality.to_string()]);
	flag("-alpha_q", &[s.alpha_quality.to_string()]);
	flag("-alpha_method", &[u8::from(s.alpha_compression).to_string()]);
	flag("-alpha_filter", &[s.alpha_filtering.as_cwebp_str().to_owned()]);
	flag("-m", &[s.method.to_string()]);
	if let Some(hint) = s.image_hint.as_cwebp_str() {
		flag("-hint", &[hint.to_owned()]);
	}
	match s.target {
		Some(TargetMetric::Size(bytes)) => flag("-size", &[bytes.to_string()]),
		Some(TargetMetric::Psnr(psnr)) => flag("-psnr", &[psnr.to_string()]),
		None => {}
	}
	flag("-segments", &[s.segments.to_string()]);
	flag("-sns", &[s.sns.to_string()]);
	flag("-f", &[s.filter_strength.to_string()]);
	flag("-sharpness", &[s.filter_sharpness.to_string()]);
	flag(
		match s.filter_type {
			FilterType::Simple => "-nostrong",
			FilterType::Strong => "-strong",
		},
		&[],
	);
	if s.autofilter {
		flag("-af", &[]);
	}
	flag("-pass", &[s.passes.to_string()]);
	flag("-qrange", &[s.qmin.to_string(), s.qmax.to_string()]);
	flag("-pre", &[s.preprocessing.to_string()]);
	flag("-partition_limit", &[s.partition_limit.to_string()]);
	for (on, name) in [(s.jpeg_like, "-jpeg_like"), (s.sharp_yuv, "-sharp_yuv"), (s.low_memory, "-low_memory"), (s.multi_threading, "-mt"), (!s.keep_alpha, "-noalpha")] {
		if on {
			flag(name, &[]);
		}
	}
	if let Some(colour) = s.blend_alpha {
		flag("-blend_alpha", &[format!("0x{colour:06x}")]);
	}
	if s.metadata.any() {
		let kinds: Vec<&str> = [(s.metadata.exif, "exif"), (s.metadata.icc, "icc"), (s.metadata.xmp, "xmp")].into_iter().filter_map(|(on, name)| on.then_some(name)).collect();
		flag("-metadata", &[kinds.join(",")]);
	}
	if let Some(crop) = job.crop {
		flag("-crop", &[crop.x.to_string(), crop.y.to_string(), crop.width.to_string(), crop.height.to_string()]);
	}
	if !job.resize.is_noop() {
		flag("-resize", &[job.resize.width.to_string(), job.resize.height.to_string()]);
		match job.resize.mode {
			ResizeMode::Always => {}
			ResizeMode::DownOnly => flag("-resize_mode", &["down_only".to_owned()]),
			ResizeMode::UpOnly => flag("-resize_mode", &["up_only".to_owned()]),
		}
	}

	let mut args: Vec<OsString> = args.into_iter().map(OsString::from).collect();
	args.push(input.as_os_str().to_os_string());
	args.push(OsString::from("-o"));
	args.push(output.as_os_str().to_os_string());
	args
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use super::cwebp_args;
	use crate::settings::{AlphaFiltering, Crop, EncodeJob, FilterType, ImageHint, Resize, ResizeMode, TargetMetric, WebpMetadata, WebpSettings};

	/// Render an argument list as a space-joined string, so a failing assertion reads like
	/// the command line it is about rather than a vector of `OsString`.
	fn line(job: &EncodeJob) -> String {
		cwebp_args(job, Path::new("in.png"), Path::new("out.webp")).iter().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" ")
	}

	/// The app's default encode, every setting spelled out.
	#[test]
	fn default_encode_spells_out_every_setting() {
		assert_eq!(line(&EncodeJob::default()), "-q 75 -alpha_q 100 -alpha_method 1 -alpha_filter best -m 4 -segments 4 -sns 50 -f 60 -sharpness 0 -strong -af -pass 6 -qrange 0 100 -pre 0 -partition_limit 0 -mt in.png -o out.webp");
	}

	#[test]
	fn near_lossless_is_its_own_level_beside_quality_and_method() {
		let job = EncodeJob::from(WebpSettings { lossless: true, near_lossless: 60, quality: 90.0, method: 6, ..WebpSettings::default() });
		let rendered = line(&job);
		assert!(rendered.starts_with("-lossless -near_lossless 60 -q 90 "), "{rendered}");
		assert!(rendered.contains("-m 6"), "{rendered}");
		// Off is 100, which cwebp spells by leaving the flag out.
		assert!(!line(&EncodeJob::from(WebpSettings { lossless: true, ..WebpSettings::default() })).contains("-near_lossless"));
	}

	#[test]
	fn every_switch_reaches_the_command_line() {
		let job = EncodeJob { crop: Some(Crop { x: 1, y: 2, width: 30, height: 20 }), resize: Resize { width: 16, height: 0, mode: ResizeMode::DownOnly }, webp: WebpSettings { exact: true, quality: 42.5, alpha_compression: false, alpha_filtering: AlphaFiltering::Off, image_hint: ImageHint::Graph, target: Some(TargetMetric::Psnr(41.5)), filter_type: FilterType::Simple, autofilter: false, qmin: 10, qmax: 90, preprocessing: 3, jpeg_like: true, sharp_yuv: true, low_memory: true, multi_threading: false, keep_alpha: false, blend_alpha: Some(0x00ff_8000), metadata: WebpMetadata { exif: true, icc: false, xmp: true }, ..WebpSettings::default() }, ..Default::default() };
		let rendered = line(&job);
		for expected in ["-exact", "-q 42.5", "-alpha_method 0", "-alpha_filter none", "-hint graph", "-psnr 41.5", "-nostrong", "-qrange 10 90", "-pre 3", "-jpeg_like", "-sharp_yuv", "-low_memory", "-noalpha", "-blend_alpha 0xff8000", "-metadata exif,xmp", "-crop 1 2 30 20", "-resize 16 0 -resize_mode down_only"] {
			assert!(rendered.contains(expected), "missing `{expected}` in {rendered}");
		}
		for absent in ["-af", "-mt", "-lossless", "-size"] {
			assert!(!rendered.split(' ').any(|word| word == absent), "unexpected `{absent}` in {rendered}");
		}
	}

	/// Paths go through as `OsString` rather than being formatted into a shell string, so
	/// a space or a quote in a filename cannot break the invocation.
	#[test]
	fn paths_are_passed_as_arguments_not_shell_text() {
		let args = cwebp_args(&EncodeJob::default(), Path::new("/tmp/a b/holiday \"photo\".png"), Path::new("/tmp/out dir/holiday.webp"));
		let tail = &args[args.len() - 3..];
		assert_eq!(tail[0], std::ffi::OsString::from("/tmp/a b/holiday \"photo\".png"));
		assert_eq!(tail[1], std::ffi::OsString::from("-o"));
		assert_eq!(tail[2], std::ffi::OsString::from("/tmp/out dir/holiday.webp"));
	}
}
