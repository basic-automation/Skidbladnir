//! Turning [`EncodeSettings`] into a `cwebp` command line.
//!
//! This is not dead weight in a port that intends to call libwebp directly. It has two
//! jobs that outlive the subprocess:
//!
//! 1. **It drives the reference encoder in the parity test.** The Phase 2 gate is that
//!    our own encoder and the real `cwebp` produce the same bytes for the same settings,
//!    and something has to build the `cwebp` side of that comparison.
//! 2. **It is the sidecar fallback, kept working.** If binding libwebp ever stops being
//!    viable on a platform, shipping the real `cwebp` as a Tauri sidecar is the
//!    documented escape hatch, and it needs exactly this function.
//!
//! The flag order and the per-mode flag *selection* are deliberately byte-for-byte what
//! the Electron app produces: `index.html` decides which values to send for the chosen
//! mode, and `main.js` concatenates them in a fixed order. Both behaviours are
//! reproduced here, quirks included, because this is the thing that is supposed to be a
//! faithful reading of the old app.

use std::{ffi::OsString, path::Path};

use crate::settings::{EncodeSettings, Mode, TargetMetric};

/// Build the `cwebp` argument list for these settings, encoding `input` to `output`.
///
/// The returned vector excludes the `cwebp` program name itself, so it can be handed
/// straight to [`std::process::Command::args`].
///
/// The order matches `main.js`'s concatenation: preset, near-lossless, JPEG-like,
/// lossless, autofilter, multi-threading, filter strength, filter sharpness, sharp YUV,
/// alpha filtering, quality, alpha quality, method, low memory, segments, partition
/// limit, target size, target PSNR, passes, SNS, resize, then the input path and `-o`.
///
/// Which of those appear depends on the mode, because the Electron renderer only sends
/// the values that mode uses — the advanced lossy controls reach the encoder in
/// [`Mode::Lossy`] alone, and [`Mode::NearLossless`] sends nothing but its own level.
#[must_use]
pub fn cwebp_args(settings: &EncodeSettings, input: &Path, output: &Path) -> Vec<OsString> {
	let mut args: Vec<OsString> = Vec::new();
	let mut push = |flag: &str| args.push(OsString::from(flag));

	match settings.mode {
		Mode::Preset => {
			// An unset preset is a validation error, not something to guess at; emitting
			// no -preset would silently encode with the plain lossy defaults instead.
			if let Some(preset) = settings.preset {
				push("-preset");
				push(preset.as_cwebp_str());
			}
		}
		Mode::NearLossless => {
			// The near-lossless level is the quality slider's value: the Electron UI
			// relabels the same control rather than adding a second one.
			push("-near_lossless");
			push(&settings.quality.to_string());
		}
		Mode::JpegLike => push("-jpeg_like"),
		// -exact is not a separate control: the Electron app always pairs it with
		// -lossless, preserving RGB under fully transparent pixels.
		Mode::Lossless => {
			push("-lossless");
			push("-exact");
		}
		Mode::Lossy => {}
	}

	// The filter selection is sent for the lossy mode only.
	if settings.mode.uses_lossy_options() {
		match settings.filter {
			crate::settings::FilterType::Auto => push("-af"),
			crate::settings::FilterType::Simple => push("-nostrong"),
			crate::settings::FilterType::Strong => push("-strong"),
		}
	}

	if settings.multi_threading {
		push("-mt");
	}

	if settings.mode.uses_lossy_options() && settings.filter.uses_manual_strength() {
		push("-f");
		push(&settings.filter_strength.to_string());
		push("-sharpness");
		push(&settings.filter_sharpness.to_string());
	}

	if settings.mode.uses_lossy_options() && settings.sharp_yuv {
		push("-sharp_yuv");
	}

	// A preset carries its own alpha filtering choice, so the Electron app clears this
	// flag in preset mode rather than overriding the preset.
	if settings.mode != Mode::Preset
		&& let Some(alpha_filtering) = settings.alpha_filtering
	{
		push("-alpha_filter");
		push(alpha_filtering.as_cwebp_str());
	}

	// Near-lossless is the one mode that does not send an explicit quality: its level
	// went out as -near_lossless above.
	if settings.mode != Mode::NearLossless {
		push("-q");
		push(&settings.quality.to_string());
	}

	if matches!(settings.mode, Mode::Lossy | Mode::Lossless | Mode::JpegLike) {
		push("-alpha_q");
		push(&settings.alpha_quality.to_string());
		push("-m");
		push(&settings.method.to_string());
	}

	if settings.mode.uses_lossy_options() {
		if settings.low_memory {
			push("-low_memory");
		}
		push("-segments");
		push(&settings.segments.to_string());
		push("-partition_limit");
		push(&settings.partition_limit.to_string());

		match settings.target {
			Some(TargetMetric::Size(bytes)) => {
				push("-size");
				push(&bytes.to_string());
			}
			Some(TargetMetric::Psnr(psnr)) => {
				// The Electron app asks for the measurement as well as the target, so
				// the PSNR it achieved is reported back.
				push("-print_psnr");
				push("-psnr");
				push(&psnr.to_string());
			}
			None => {}
		}

		push("-pass");
		push(&settings.passes.to_string());
		push("-sns");
		push(&settings.sns.to_string());
	}

	if !settings.resize.is_noop() {
		push("-resize");
		push(&settings.resize.width.to_string());
		push(&settings.resize.height.to_string());
	}

	args.push(input.as_os_str().to_os_string());
	args.push(OsString::from("-o"));
	args.push(output.as_os_str().to_os_string());
	args
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use super::cwebp_args;
	use crate::settings::{AlphaFiltering, EncodeSettings, FilterType, Mode, Preset, Resize, TargetMetric};

	/// Render an argument list as a space-joined string, so a failing assertion reads like
	/// the command line it is about rather than a vector of `OsString`.
	fn line(settings: &EncodeSettings) -> String {
		cwebp_args(settings, Path::new("in.png"), Path::new("out.webp")).iter().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" ")
	}

	/// The Electron app's default lossy encode, minus its `-size 1` bug. Every flag and
	/// its position is what `main.js` concatenates.
	#[test]
	fn default_lossy_matches_the_electron_command_line() {
		assert_eq!(line(&EncodeSettings::default()), "-af -mt -alpha_filter best -q 75 -alpha_q 100 -m 4 -segments 4 -partition_limit 0 -pass 6 -sns 50 in.png -o out.webp");
	}

	#[test]
	fn lossless_pairs_exact_and_skips_the_advanced_controls() {
		let settings = EncodeSettings { mode: Mode::Lossless, ..Default::default() };
		// No -af: the renderer sends the filter selection for the lossy mode only. No
		// -segments / -sns / -pass / -partition_limit either.
		assert_eq!(line(&settings), "-lossless -exact -mt -alpha_filter best -q 75 -alpha_q 100 -m 4 in.png -o out.webp");
	}

	#[test]
	fn near_lossless_sends_the_quality_slider_as_its_level_and_no_q() {
		let settings = EncodeSettings { mode: Mode::NearLossless, quality: 60, ..Default::default() };
		assert_eq!(line(&settings), "-near_lossless 60 -mt -alpha_filter best in.png -o out.webp");
	}

	#[test]
	fn jpeg_like_is_lossy_with_the_basic_controls_only() {
		let settings = EncodeSettings { mode: Mode::JpegLike, ..Default::default() };
		assert_eq!(line(&settings), "-jpeg_like -mt -alpha_filter best -q 75 -alpha_q 100 -m 4 in.png -o out.webp");
	}

	/// A preset clears the alpha-filter flag so the preset's own choice stands, and sends
	/// quality but not alpha quality or method.
	#[test]
	fn preset_mode_clears_alpha_filter() {
		let settings = EncodeSettings { mode: Mode::Preset, preset: Some(Preset::Drawing), ..Default::default() };
		assert_eq!(line(&settings), "-preset drawing -mt -q 75 in.png -o out.webp");
	}

	#[test]
	fn every_preset_reaches_the_command_line() {
		for (preset, expected) in [(Preset::Default, "default"), (Preset::Photo, "photo"), (Preset::Picture, "picture"), (Preset::Drawing, "drawing"), (Preset::Icon, "icon"), (Preset::Text, "text")] {
			let settings = EncodeSettings { mode: Mode::Preset, preset: Some(preset), ..Default::default() };
			assert_eq!(line(&settings), format!("-preset {expected} -mt -q 75 in.png -o out.webp"));
		}
	}

	/// An unselected preset must not silently fall through to the lossy defaults.
	#[test]
	fn preset_mode_without_a_preset_emits_no_preset_flag() {
		let settings = EncodeSettings { mode: Mode::Preset, preset: None, ..Default::default() };
		assert!(!line(&settings).contains("-preset"), "an unselected preset must be caught by validate(), not guessed at");
		assert_eq!(settings.validate(), Err(crate::settings::ValidationError::MissingPreset));
	}

	#[test]
	fn manual_filtering_adds_strength_and_sharpness() {
		let strong = EncodeSettings { filter: FilterType::Strong, filter_strength: 65, filter_sharpness: 3, ..Default::default() };
		assert_eq!(line(&strong), "-strong -mt -f 65 -sharpness 3 -alpha_filter best -q 75 -alpha_q 100 -m 4 -segments 4 -partition_limit 0 -pass 6 -sns 50 in.png -o out.webp");
		let simple = EncodeSettings { filter: FilterType::Simple, filter_strength: 10, filter_sharpness: 7, ..Default::default() };
		assert_eq!(line(&simple), "-nostrong -mt -f 10 -sharpness 7 -alpha_filter best -q 75 -alpha_q 100 -m 4 -segments 4 -partition_limit 0 -pass 6 -sns 50 in.png -o out.webp");
	}

	/// Under auto filtering the encoder picks the strength, so the user's slider values
	/// must not be sent — the Electron UI disables both sliders in that state.
	#[test]
	fn auto_filtering_withholds_the_strength_sliders() {
		let settings = EncodeSettings { filter: FilterType::Auto, filter_strength: 99, filter_sharpness: 5, ..Default::default() };
		let rendered = line(&settings);
		assert!(rendered.contains("-af"));
		assert!(!rendered.contains("-f 99"), "auto filtering must not send a manual strength: {rendered}");
		assert!(!rendered.contains("-sharpness"), "auto filtering must not send a sharpness: {rendered}");
	}

	#[test]
	fn target_size_and_psnr_are_mutually_exclusive() {
		let sized = EncodeSettings { target: Some(TargetMetric::Size(51_200)), ..Default::default() };
		let rendered = line(&sized);
		assert!(rendered.contains("-size 51200"), "{rendered}");
		assert!(!rendered.contains("-psnr"), "{rendered}");

		let psnr = EncodeSettings { target: Some(TargetMetric::Psnr(42)), ..Default::default() };
		let rendered = line(&psnr);
		assert!(rendered.contains("-print_psnr -psnr 42"), "{rendered}");
		assert!(!rendered.contains("-size"), "{rendered}");
	}

	/// The regression guard for the divergence: a default encode must not carry a size
	/// target at all.
	#[test]
	fn default_encode_carries_no_size_target() {
		assert!(!line(&EncodeSettings::default()).contains("-size"), "the Electron app's `-size 1` default must not come back");
	}

	#[test]
	fn optional_lossy_switches_appear_only_when_set() {
		let all_on = EncodeSettings { sharp_yuv: true, low_memory: true, ..Default::default() };
		let rendered = line(&all_on);
		assert!(rendered.contains("-sharp_yuv"), "{rendered}");
		assert!(rendered.contains("-low_memory"), "{rendered}");

		let rendered = line(&EncodeSettings::default());
		assert!(!rendered.contains("-sharp_yuv"), "{rendered}");
		assert!(!rendered.contains("-low_memory"), "{rendered}");
	}

	/// `-sharp_yuv` is a lossy-only control: the Electron renderer sends it in the lossy
	/// branch alone, so the other modes must not pick it up from a stale settings value.
	#[test]
	fn sharp_yuv_does_not_leak_into_other_modes() {
		for mode in [Mode::Lossless, Mode::NearLossless, Mode::JpegLike, Mode::Preset] {
			let settings = EncodeSettings { mode, preset: Some(Preset::Photo), sharp_yuv: true, low_memory: true, ..Default::default() };
			let rendered = line(&settings);
			assert!(!rendered.contains("-sharp_yuv"), "{mode:?} must not send -sharp_yuv: {rendered}");
			assert!(!rendered.contains("-low_memory"), "{mode:?} must not send -low_memory: {rendered}");
		}
	}

	#[test]
	fn multi_threading_can_be_turned_off() {
		let settings = EncodeSettings { multi_threading: false, ..Default::default() };
		assert!(!line(&settings).contains("-mt"));
	}

	#[test]
	fn alpha_filtering_variants_and_absence() {
		for (choice, expected) in [(AlphaFiltering::Off, "none"), (AlphaFiltering::Fast, "fast"), (AlphaFiltering::Best, "best")] {
			let settings = EncodeSettings { alpha_filtering: Some(choice), ..Default::default() };
			assert!(line(&settings).contains(&format!("-alpha_filter {expected}")));
		}
		let settings = EncodeSettings { alpha_filtering: None, ..Default::default() };
		assert!(!line(&settings).contains("-alpha_filter"));
	}

	/// Resize is sent in every mode, and only when at least one dimension is non-zero.
	#[test]
	fn resize_is_emitted_in_every_mode_when_set() {
		for mode in [Mode::Lossy, Mode::Lossless, Mode::NearLossless, Mode::JpegLike, Mode::Preset] {
			let settings = EncodeSettings { mode, preset: Some(Preset::Icon), resize: Resize { width: 1024, height: 0 }, ..Default::default() };
			assert!(line(&settings).contains("-resize 1024 0"), "{mode:?} must carry the resize");
		}
		assert!(!line(&EncodeSettings::default()).contains("-resize"));
	}

	/// Resize lands immediately before the input path, as the last option.
	#[test]
	fn resize_is_the_last_option_before_the_paths() {
		let settings = EncodeSettings { resize: Resize { width: 0, height: 480 }, ..Default::default() };
		assert!(line(&settings).ends_with("-resize 0 480 in.png -o out.webp"), "{}", line(&settings));
	}

	/// Paths go through as `OsString` rather than being formatted into a shell string, so
	/// a space or a quote in a filename cannot break the invocation the way it can in the
	/// Electron app, which builds one shell line and spawns it with `shell: true`.
	#[test]
	fn paths_are_passed_as_arguments_not_shell_text() {
		let args = cwebp_args(&EncodeSettings::default(), Path::new("/tmp/a b/holiday \"photo\".png"), Path::new("/tmp/out dir/holiday.webp"));
		let tail = &args[args.len() - 3..];
		assert_eq!(tail[0], std::ffi::OsString::from("/tmp/a b/holiday \"photo\".png"));
		assert_eq!(tail[1], std::ffi::OsString::from("-o"));
		assert_eq!(tail[2], std::ffi::OsString::from("/tmp/out dir/holiday.webp"));
	}
}
