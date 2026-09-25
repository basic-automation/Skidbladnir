//! The Phase 2 parity gate: our encoder against the real `cwebp`, byte for byte.
//!
//! This is the test the whole migration rests on. Skidbladnir's reason to exist is that
//! it drives `cwebp`'s full control surface, so a port that quietly changes what comes
//! out of the encoder has failed even if every button still works. Reading the code and
//! concluding it looks right is not evidence; this is.
//!
//! # How it isolates the encoder
//!
//! The fixture is written as a **PAM** file (`P7`, `TUPLTYPE RGB_ALPHA`), which `cwebp`
//! reads with its own built-in PNM reader — no libpng, no libjpeg, no decoder of ours.
//! The bytes `cwebp` encodes are therefore bit-identical to the ones handed to
//! [`encode_rgba`], so any difference in the output is the encoder configuration and
//! nothing else. A PNG fixture would have left two independent decoders in the
//! comparison and made a mismatch ambiguous.
//!
//! # When it runs
//!
//! It needs a reference `cwebp`, found via `SKIDBLADNIR_REFERENCE_CWEBP` or on `PATH`.
//! Byte parity is only meaningful between equal libwebp versions, so it also compares the
//! reference's version against the version of the libwebp this crate is linked to and
//! declines to compare across a mismatch rather than reporting a false failure. In both
//! of those cases it prints loudly and returns. Set `SKIDBLADNIR_REQUIRE_PARITY=1` to
//! turn "could not run" into a failure, which is what CI does on the platforms where a
//! reference encoder is installed.

use std::{
	env, ffi::OsString, fs, path::{Path, PathBuf}, process::Command
};

use skidbladnir_encode::{
	RgbaImage, cwebp_args, encode_rgba, settings::{AlphaFiltering, EncodeSettings, FilterType, Mode, Preset, Resize, TargetMetric}
};

/// Locate a reference `cwebp`, preferring an explicitly configured one.
fn reference_cwebp() -> Option<PathBuf> {
	if let Some(path) = env::var_os("SKIDBLADNIR_REFERENCE_CWEBP") {
		let path = PathBuf::from(path);
		return path.is_file().then_some(path);
	}
	// Fall back to PATH. Running it is the only portable test that it works.
	Command::new("cwebp").arg("-version").output().ok().filter(|out| out.status.success()).map(|_| PathBuf::from("cwebp"))
}

/// Parse the version `cwebp -version` prints on its first line, e.g. `1.6.0`.
fn reference_version(cwebp: &Path) -> Option<(i32, i32, i32)> {
	let out = Command::new(cwebp).arg("-version").output().ok()?;
	let text = String::from_utf8_lossy(&out.stdout);
	let first = text.lines().next()?.trim();
	let mut parts = first.split('.').map(str::parse::<i32>);
	match (parts.next(), parts.next(), parts.next()) {
		(Some(Ok(major)), Some(Ok(minor)), Some(Ok(revision))) => Some((major, minor, revision)),
		_ => None,
	}
}

/// A fixture with smooth colour ramps, high-frequency detail and a real alpha pattern, so
/// SNS, the deblocking filter and the alpha controls all have something to act on. A flat
/// image would make most of the control surface untestable.
fn fixture(width: u32, height: u32) -> Vec<u8> {
	let mut pixels = Vec::with_capacity((width as usize) * (height as usize) * 4);
	for y in 0..height {
		for x in 0..width {
			// A diagonal ripple: enough local detail that the filter controls matter.
			let ripple = ((x * 13 + y * 7) % 97) * 255 / 97;
			pixels.push(u8::try_from(ripple).expect("ripple is under 256"));
			pixels.push(u8::try_from(x * 255 / width.max(1)).expect("ramp is under 256"));
			pixels.push(u8::try_from(y * 255 / height.max(1)).expect("ramp is under 256"));
			// Blocks of full transparency, a ramp, and opacity.
			pixels.push(match (x / 8 + y / 8) % 3 {
				0 => 0,
				1 => u8::try_from(x * 255 / width.max(1)).expect("ramp is under 256"),
				_ => 255,
			});
		}
	}
	pixels
}

/// Write the fixture as a PAM file, the format `cwebp` reads without any image library.
fn write_pam(path: &Path, pixels: &[u8], width: u32, height: u32) {
	let mut out = format!("P7\nWIDTH {width}\nHEIGHT {height}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n").into_bytes();
	out.extend_from_slice(pixels);
	fs::write(path, out).expect("write the PAM fixture");
}

/// Every setting combination worth comparing: the whole control surface, one case per
/// control, plus the mode-specific paths.
fn cases() -> Vec<(String, EncodeSettings)> {
	let mut cases: Vec<(String, EncodeSettings)> = Vec::new();
	let mut add = |name: &str, settings: EncodeSettings| cases.push((name.to_owned(), settings));

	add("default lossy", EncodeSettings::default());

	for quality in [0_u8, 1, 50, 99, 100] {
		add(&format!("quality {quality}"), EncodeSettings { quality, ..Default::default() });
	}
	for alpha_quality in [0_u8, 50, 100] {
		add(&format!("alpha quality {alpha_quality}"), EncodeSettings { alpha_quality, ..Default::default() });
	}
	for method in 0..=6_u8 {
		add(&format!("method {method}"), EncodeSettings { method, ..Default::default() });
	}
	for segments in 1..=4_u8 {
		add(&format!("segments {segments}"), EncodeSettings { segments, ..Default::default() });
	}
	for sns in [0_u8, 25, 50, 100] {
		add(&format!("sns {sns}"), EncodeSettings { sns, ..Default::default() });
	}
	for passes in [1_u8, 6, 10] {
		add(&format!("passes {passes}"), EncodeSettings { passes, ..Default::default() });
	}
	for partition_limit in [0_u8, 50, 100] {
		add(&format!("partition limit {partition_limit}"), EncodeSettings { partition_limit, ..Default::default() });
	}

	add("auto filter", EncodeSettings { filter: FilterType::Auto, ..Default::default() });
	for (strength, sharpness) in [(0_u8, 0_u8), (20, 3), (100, 7)] {
		add(&format!("strong filter {strength}/{sharpness}"), EncodeSettings { filter: FilterType::Strong, filter_strength: strength, filter_sharpness: sharpness, ..Default::default() });
		add(&format!("simple filter {strength}/{sharpness}"), EncodeSettings { filter: FilterType::Simple, filter_strength: strength, filter_sharpness: sharpness, ..Default::default() });
	}

	for bytes in [512_u32, 2_048, 16_384] {
		add(&format!("target size {bytes}"), EncodeSettings { target: Some(TargetMetric::Size(bytes)), ..Default::default() });
	}
	for psnr in [30_u32, 42, 50] {
		add(&format!("target psnr {psnr}"), EncodeSettings { target: Some(TargetMetric::Psnr(psnr)), ..Default::default() });
	}

	add("sharp yuv", EncodeSettings { sharp_yuv: true, ..Default::default() });
	add("low memory", EncodeSettings { low_memory: true, ..Default::default() });
	add("no multi-threading", EncodeSettings { multi_threading: false, ..Default::default() });
	add("sharp yuv + low memory + no mt", EncodeSettings { sharp_yuv: true, low_memory: true, multi_threading: false, ..Default::default() });

	for alpha_filtering in [AlphaFiltering::Off, AlphaFiltering::Fast, AlphaFiltering::Best] {
		add(&format!("alpha filter {}", alpha_filtering.as_cwebp_str()), EncodeSettings { alpha_filtering: Some(alpha_filtering), ..Default::default() });
	}
	add("no alpha filter flag", EncodeSettings { alpha_filtering: None, ..Default::default() });

	add("lossless", EncodeSettings { mode: Mode::Lossless, ..Default::default() });
	for quality in [0_u8, 50, 100] {
		add(&format!("lossless effort {quality}"), EncodeSettings { mode: Mode::Lossless, quality, ..Default::default() });
	}
	for quality in [0_u8, 40, 60, 80, 100] {
		add(&format!("near-lossless {quality}"), EncodeSettings { mode: Mode::NearLossless, quality, ..Default::default() });
	}
	add("jpeg-like", EncodeSettings { mode: Mode::JpegLike, ..Default::default() });
	for preset in [Preset::Default, Preset::Photo, Preset::Picture, Preset::Drawing, Preset::Icon, Preset::Text] {
		add(&format!("preset {}", preset.as_cwebp_str()), EncodeSettings { mode: Mode::Preset, preset: Some(preset), ..Default::default() });
	}

	// Resize, including the -exact path that lossless takes.
	for resize in [Resize { width: 32, height: 24 }, Resize { width: 32, height: 0 }, Resize { width: 0, height: 24 }, Resize { width: 128, height: 96 }] {
		add(&format!("resize {}x{}", resize.width, resize.height), EncodeSettings { resize, ..Default::default() });
		add(&format!("lossless resize {}x{}", resize.width, resize.height), EncodeSettings { mode: Mode::Lossless, resize, ..Default::default() });
	}

	// A few combinations, because controls interact: target size with a manual filter,
	// sharp YUV with a preset-free lossy encode, and the full advanced set at once.
	add("everything at once", EncodeSettings { mode: Mode::Lossy, quality: 61, alpha_quality: 77, alpha_filtering: Some(AlphaFiltering::Fast), method: 5, segments: 3, partition_limit: 22, sns: 66, passes: 4, filter: FilterType::Strong, filter_strength: 44, filter_sharpness: 2, target: Some(TargetMetric::Size(4_096)), sharp_yuv: true, low_memory: true, multi_threading: true, resize: Resize { width: 40, height: 0 }, preset: None });

	cases
}

#[test]
fn matches_reference_cwebp_across_the_whole_control_surface() {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");

	let Some(cwebp) = reference_cwebp() else {
		let message = "PARITY NOT RUN: no reference cwebp found. Set SKIDBLADNIR_REFERENCE_CWEBP or put cwebp on PATH. The encode path is therefore UNVERIFIED in this run.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};

	let linked = skidbladnir_encode::encoder::linked_encoder_version();
	let Some(reference) = reference_version(&cwebp) else {
		let message = format!("PARITY NOT RUN: could not read a version from `{} -version`.", cwebp.display());
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};
	if reference != linked {
		// Two different libwebp versions can legitimately produce different bytes for the
		// same configuration, so comparing across them would report a false failure.
		let message = format!("PARITY NOT RUN: reference cwebp is {}.{}.{} but this crate links libwebp {}.{}.{}. Byte parity is only meaningful between equal versions, so the comparison was skipped rather than reported as a failure. The encode path is UNVERIFIED in this run.", reference.0, reference.1, reference.2, linked.0, linked.1, linked.2);
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	}

	let (width, height) = (64_u32, 48_u32);
	let pixels = fixture(width, height);
	let dir = env::temp_dir().join(format!("skidbladnir-parity-{}", std::process::id()));
	fs::create_dir_all(&dir).expect("create the scratch directory");
	let input = dir.join("fixture.pam");
	write_pam(&input, &pixels, width, height);

	let image = RgbaImage { width, height, pixels: &pixels };
	let mut mismatches: Vec<String> = Vec::new();
	let cases = cases();
	let total = cases.len();

	for (index, (name, settings)) in cases.into_iter().enumerate() {
		let output = dir.join(format!("case-{index}.webp"));
		let args: Vec<OsString> = cwebp_args(&settings, &input, &output);
		let run = Command::new(&cwebp).args(&args).output().expect("run the reference cwebp");
		assert!(run.status.success(), "reference cwebp failed for `{name}`: {}", String::from_utf8_lossy(&run.stderr));
		let expected = fs::read(&output).expect("read the reference output");

		let actual = match encode_rgba(&settings, &image) {
			Ok(bytes) => bytes,
			Err(error) => {
				mismatches.push(format!("`{name}`: our encoder failed: {error}"));
				continue;
			}
		};

		if actual != expected {
			let first_difference = actual.iter().zip(&expected).position(|(a, b)| a != b).map_or_else(|| "length only".to_owned(), |at| format!("byte {at}"));
			mismatches.push(format!("`{name}`: ours {} bytes, cwebp {} bytes, first difference at {first_difference}\n    cwebp {} {}", actual.len(), expected.len(), cwebp.display(), args.iter().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" ")));
		}

		let _ = fs::remove_file(&output);
	}

	let _ = fs::remove_dir_all(&dir);

	assert!(mismatches.is_empty(), "{} of {total} settings diverged from the reference cwebp {}.{}.{}:\n  {}", mismatches.len(), linked.0, linked.1, linked.2, mismatches.join("\n  "));
	eprintln!("PARITY OK: {total} settings matched the reference cwebp {}.{}.{} byte for byte.", linked.0, linked.1, linked.2);
}
