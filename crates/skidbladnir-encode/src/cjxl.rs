//! Turning an [`EncodeJob`] into the `cjxl` command line that makes the same file.
//!
//! The JPEG XL half of the parity claim, as [`crate::cwebp`] is the WebP half:
//! `tests/jxl_parity.rs` runs `cjxl` with these arguments and compares its file with ours.
//! Every [`JxlSettings`] field has its flag here; a field at `cjxl`'s default is still
//! written out where `cjxl` would accept it, so the command line never depends on the two
//! programs' defaults agreeing.
//!
//! [`JxlSettings`]: crate::settings::JxlSettings

use std::{ffi::OsString, path::Path};

use crate::settings::{EncodeJob, JxlTarget, MetadataSource, Tristate};

/// Build the `cjxl` argument list for these settings, encoding `input` to `output`.
#[must_use]
pub fn cjxl_args(job: &EncodeJob, input: &Path, output: &Path) -> Vec<OsString> {
	let s = &job.jxl;
	let mut args: Vec<OsString> = vec![input.as_os_str().to_os_string(), output.as_os_str().to_os_string()];
	let mut push = |arg: String| args.push(OsString::from(arg));

	match s.target {
		JxlTarget::Default => {}
		JxlTarget::Distance(distance) => push(format!("--distance={distance}")),
		JxlTarget::Quality(quality) => push(format!("--quality={quality}")),
	}
	if let Some(alpha) = s.alpha_distance {
		push(format!("--alpha_distance={alpha}"));
	}
	if s.allow_expert_options {
		push("--allow_expert_options".to_owned());
	}
	push(format!("--effort={}", s.effort));
	push(format!("--brotli_effort={}", s.brotli_effort));
	for (on, flag) in [(s.progressive, "--progressive"), (s.progressive_ac, "--progressive_ac"), (s.qprogressive_ac, "--qprogressive_ac"), (s.already_downsampled, "--already_downsampled"), (s.disable_perceptual_optimizations, "--disable_perceptual_optimizations"), (s.streaming_output, "--streaming_output"), (s.modular_lossy_palette, "--modular_lossy_palette")] {
		if on {
			push(flag.to_owned());
		}
	}
	for (value, flag) in [(s.group_order, "group_order"), (s.container, "container"), (s.compress_boxes, "compress_boxes"), (s.modular, "modular"), (s.keep_invisible, "keep_invisible"), (s.gaborish, "gaborish"), (s.noise, "noise"), (s.jpeg_reconstruction_cfl, "jpeg_reconstruction_cfl"), (s.dots, "dots"), (s.patches, "patches")] {
		match value {
			Tristate::Default => {}
			Tristate::Off => push(format!("--{flag}=0")),
			Tristate::On => push(format!("--{flag}=1")),
		}
	}
	push(format!("--lossless_jpeg={}", u8::from(s.lossless_jpeg)));
	push(format!("--allow_jpeg_reconstruction={}", u8::from(s.allow_jpeg_reconstruction)));
	push(format!("--photon_noise_iso={}", s.photon_noise_iso));
	// `cjxl` accepts only a positive target; 0, its default, is spelled by leaving it out.
	if s.intensity_target > 0.0 {
		push(format!("--intensity_target={}", s.intensity_target));
	}
	push(format!("--codestream_level={}", s.codestream_level));
	push(format!("--buffering={}", s.buffering));
	push(format!("--faster_decoding={}", s.faster_decoding));
	push(format!("--premultiply={}", s.premultiply));
	push(format!("--center_x={}", s.center_x));
	push(format!("--center_y={}", s.center_y));
	push(format!("--progressive_dc={}", s.progressive_dc));
	push(format!("--resampling={}", s.resampling));
	push(format!("--ec_resampling={}", s.ec_resampling));
	push(format!("--upsampling_mode={}", s.upsampling_mode));
	push(format!("--epf={}", s.epf));
	push(format!("--override_bitdepth={}", s.override_bitdepth));
	push(format!("--output_mode={}", s.output_mode));
	if s.frame_index_box {
		push("--frame_indexing=1".to_owned());
	}
	push(format!("--iterations={}", s.iterations));
	push(format!("--modular_colorspace={}", s.modular_colorspace));
	push(format!("--modular_group_size={}", s.modular_group_size));
	push(format!("--modular_predictor={}", s.modular_predictor));
	push(format!("--modular_nb_prev_channels={}", s.modular_nb_prev_channels));
	push(format!("--modular_palette_colors={}", s.modular_palette_colors));
	push(format!("--pre-compact={}", s.pre_compact));
	push(format!("--post-compact={}", s.post_compact));
	if let Some(responsive) = s.responsive {
		push(format!("--responsive={}", u8::from(responsive)));
	}
	if !s.multi_threading {
		push("--num_threads=0".to_owned());
	}
	if let Some(space) = s.color_space {
		push("-x".to_owned());
		push(format!("color_space={}", space.description()));
	}
	if let Some(icc) = &s.icc_file {
		push("-x".to_owned());
		push(format!("icc_pathname={}", icc.display()));
	}
	for (source, kind) in [(&s.exif, "exif"), (&s.xmp, "xmp"), (&s.jumbf, "jumbf")] {
		match source {
			MetadataSource::Keep => {}
			MetadataSource::Strip => {
				push("-x".to_owned());
				push(format!("strip={kind}"));
			}
			MetadataSource::File(path) => {
				push("-x".to_owned());
				push(format!("{kind}={}", path.display()));
			}
		}
	}
	args
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use super::cjxl_args;
	use crate::settings::{EncodeJob, JxlSettings, JxlTarget, MetadataSource, OutputFormat, Tristate};

	fn line(job: &EncodeJob) -> String {
		cjxl_args(job, Path::new("in.png"), Path::new("out.jxl")).iter().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" ")
	}

	#[test]
	fn every_setting_reaches_the_command_line() {
		let job = EncodeJob { format: OutputFormat::Jxl, jxl: JxlSettings { target: JxlTarget::Distance(1.5), progressive: true, modular: Tristate::On, container: Tristate::Off, responsive: Some(false), multi_threading: false, exif: MetadataSource::Strip, ..Default::default() }, ..Default::default() };
		let rendered = line(&job);
		for expected in ["in.png out.jxl", "--distance=1.5", "--effort=7", "--progressive", "--modular=1", "--container=0", "--responsive=0", "--num_threads=0", "-x strip=exif", "--lossless_jpeg=1"] {
			assert!(rendered.contains(expected), "missing `{expected}` in {rendered}");
		}
		assert!(!rendered.contains("--gaborish"), "a default tri-state is left to cjxl: {rendered}");
	}
}
