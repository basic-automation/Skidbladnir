//! Turning an [`EncodeJob`] into the `heif-enc` command line that makes the same file.
//!
//! The HEIC half of the parity claim, as [`crate::cwebp`], [`crate::cjxl`] and
//! [`crate::avifenc`] are for theirs: `tests/heic_parity.rs` runs a reference `heif-enc`
//! with these arguments and compares its file with ours. The encoder is always named, since
//! `heif-enc` otherwise takes the first HEVC encoder it finds.
//!
//! In the GPL edition the encoder is x265, and its controls are `-L` and `-p` parameters.
//! Those are written for an image at least 32 pixels on each side: below that the encoder
//! also caps the transform depth, which a command line cannot know without the image.

use std::{ffi::OsString, path::Path};

use crate::settings::{ColorProfile, EncodeJob, HEIC_X265};

/// Build the `heif-enc` argument list for these settings, encoding `input` to `output`.
#[must_use]
pub fn heif_enc_args(job: &EncodeJob, input: &Path, output: &Path) -> Vec<OsString> {
	let s = &job.heic;
	let mut args: Vec<String> = vec!["-e".into(), (if HEIC_X265 { "x265" } else { "kvazaar" }).into(), "-q".into(), s.quality.to_string()];
	let mut push = |parts: &[&str]| args.extend(parts.iter().map(|&part| part.to_owned()));
	if HEIC_X265 {
		if s.lossless {
			push(&["-L"]);
		}
		for (name, value) in crate::heic::x265_parameters(s, (u32::MAX, u32::MAX)) {
			push(&["-p", &format!("{name}={value}")]);
		}
	} else if s.lossless {
		push(&["-p", "lossless=true"]);
	}
	if !s.alpha {
		push(&["--no-alpha"]);
	}
	if s.premultiplied_alpha {
		push(&["--premultiplied-alpha"]);
	}
	if let Some(size) = s.thumbnail {
		push(&["-t", &size.to_string()]);
	}
	if !s.thumbnail_alpha {
		push(&["--no-thumb-alpha"]);
	}
	if let Some(algorithm) = s.chroma_downsampling {
		push(&["-C", algorithm.as_heif_enc_str()]);
	}
	push(&["--color-profile", s.color_profile.as_heif_enc_str()]);
	if let ColorProfile::Custom { matrix_coefficients, colour_primaries, transfer_characteristics, full_range } = s.color_profile {
		push(&["--matrix_coefficients", &matrix_coefficients.to_string(), "--colour_primaries", &colour_primaries.to_string(), "--transfer_characteristic", &transfer_characteristics.to_string(), "--full_range_flag", if full_range { "1" } else { "0" }]);
	}
	if s.two_colr_boxes {
		push(&["--enable-two-colr-boxes"]);
	}
	if let Some([max_cll, max_pall]) = s.clli {
		push(&["--clli", &format!("{max_cll},{max_pall}")]);
	}
	if let Some([h, v]) = s.pasp {
		push(&["--pasp", &format!("{h},{v}")]);
	}
	push(s.orientation.as_heif_enc_args());
	if let Some(size) = s.cut_tiles {
		push(&["--cut-tiles", &size.to_string()]);
	}
	if let Some(projection) = s.omaf_projection {
		push(&["--omaf-image-projection", projection.as_heif_enc_str()]);
	}
	if !s.description.is_empty() {
		push(&["--pitm-description", &s.description]);
	}
	for brand in &s.compatible_brands {
		push(&["--add-compatible-brand", brand]);
	}
	if s.unif {
		push(&["--unif"]);
	}
	if s.mini {
		push(&["--mini"]);
	}
	let mut args: Vec<OsString> = args.into_iter().map(OsString::from).collect();
	args.push("-o".into());
	args.push(output.as_os_str().to_os_string());
	args.push(input.as_os_str().to_os_string());
	args
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use super::heif_enc_args;
	use crate::settings::{ChromaDownsampling, ColorProfile, EncodeJob, HEIC_X265, HeicSettings, OmafProjection, Orientation, OutputFormat};

	fn line(settings: HeicSettings) -> String {
		let job = EncodeJob { format: OutputFormat::Heic, heic: settings, ..EncodeJob::default() };
		heif_enc_args(&job, Path::new("in.png"), Path::new("out.heic")).iter().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" ")
	}

	#[test]
	fn defaults_are_heif_enc_defaults_with_the_encoder_named() {
		let x265 = if HEIC_X265 { " -p chroma=420 -p preset=slow -p tune=ssim -p tu-intra-depth=2 -p x265:aq-mode=1 -p x265:aq-strength=1.0 -p x265:psy-rd=1.0 -p x265:psy-rdoq=1.0 -p x265:deblock=0:0 -p x265:sao=true" } else { "" };
		assert_eq!(line(HeicSettings::default()), format!("-e {} -q 50{x265} --color-profile custom --matrix_coefficients 6 --colour_primaries 1 --transfer_characteristic 13 --full_range_flag 1 -o out.heic in.png", if HEIC_X265 { "x265" } else { "kvazaar" }));
	}

	#[test]
	fn every_setting_reaches_the_command_line() {
		let rendered = line(HeicSettings { quality: 80, lossless: true, alpha: false, premultiplied_alpha: true, thumbnail: Some(32), thumbnail_alpha: false, chroma_downsampling: Some(ChromaDownsampling::SharpYuv), color_profile: ColorProfile::Bt2020, two_colr_boxes: true, clli: Some([1000, 400]), pasp: Some([4, 3]), orientation: Orientation::Rotate90CwThenFlipVertically, cut_tiles: Some(64), omaf_projection: Some(OmafProjection::CubeMap), description: "a view".into(), compatible_brands: vec!["abcd".into()], unif: true, mini: true, ..HeicSettings::default() });
		for expected in ["-q 80", if HEIC_X265 { "-L" } else { "-p lossless=true" }, "--no-alpha", "--premultiplied-alpha", "-t 32", "--no-thumb-alpha", "-C sharp-yuv", "--color-profile 2020", "--enable-two-colr-boxes", "--clli 1000,400", "--pasp 4,3", "--rotate-cw 90 --flip-v", "--cut-tiles 64", "--omaf-image-projection cube-map", "--pitm-description a view", "--add-compatible-brand abcd", "--unif", "--mini"] {
			assert!(rendered.contains(expected), "missing `{expected}` in {rendered}");
		}
		assert!(!rendered.contains("--matrix_coefficients"), "a preset takes no code points: {rendered}");
	}
}
