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
	RgbaImage, cwebp_args, encode_rgba, settings::{AlphaFiltering, Crop, EncodeJob, FilterType, ImageHint, Preset, Resize, ResizeMode, TargetMetric, WebpMetadata, WebpSettings}
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

/// Every setting worth comparing: each `cwebp` flag on its own across its range, then the
/// combinations the old five-mode window could not express, which are the point of the
/// flag-for-field model.
fn cases() -> Vec<(String, EncodeJob)> {
	let mut cases: Vec<(String, EncodeJob)> = Vec::new();
	let mut add = |name: &str, settings: EncodeJob| cases.push((name.to_owned(), settings));
	let webp = |settings: WebpSettings| EncodeJob::from(settings);
	let d = WebpSettings::default;

	add("default", EncodeJob::default());
	add("libwebp defaults", webp(WebpSettings::libwebp_defaults()));

	for quality in [0.0_f32, 1.0, 50.0, 62.5, 99.9, 100.0] {
		add(&format!("quality {quality}"), webp(WebpSettings { quality, ..d() }));
	}
	for alpha_quality in [0_u8, 50, 100] {
		add(&format!("alpha quality {alpha_quality}"), webp(WebpSettings { alpha_quality, ..d() }));
	}
	add("alpha method 0", webp(WebpSettings { alpha_compression: false, ..d() }));
	for method in 0..=6_u8 {
		add(&format!("method {method}"), webp(WebpSettings { method, ..d() }));
	}
	for segments in 1..=4_u8 {
		add(&format!("segments {segments}"), webp(WebpSettings { segments, ..d() }));
	}
	for sns in [0_u8, 25, 50, 100] {
		add(&format!("sns {sns}"), webp(WebpSettings { sns, ..d() }));
	}
	for passes in [1_u8, 6, 10] {
		add(&format!("passes {passes}"), webp(WebpSettings { passes, ..d() }));
	}
	for partition_limit in [0_u8, 50, 100] {
		add(&format!("partition limit {partition_limit}"), webp(WebpSettings { partition_limit, ..d() }));
	}
	for (qmin, qmax) in [(0_u8, 100_u8), (20, 60), (50, 50)] {
		add(&format!("qrange {qmin} {qmax}"), webp(WebpSettings { qmin, qmax, ..d() }));
	}
	for preprocessing in 0..=7_u8 {
		add(&format!("pre {preprocessing}"), webp(WebpSettings { preprocessing, ..d() }));
	}
	for (strength, sharpness) in [(0_u8, 0_u8), (20, 3), (100, 7)] {
		for filter_type in [FilterType::Strong, FilterType::Simple] {
			add(&format!("{filter_type:?} filter {strength}/{sharpness}"), webp(WebpSettings { autofilter: false, filter_type, filter_strength: strength, filter_sharpness: sharpness, ..d() }));
		}
		add(&format!("autofilter with -f {strength} -sharpness {sharpness}"), webp(WebpSettings { autofilter: true, filter_strength: strength, filter_sharpness: sharpness, ..d() }));
	}
	add("simple autofilter", webp(WebpSettings { autofilter: true, filter_type: FilterType::Simple, ..d() }));

	for bytes in [512_u32, 2_048, 16_384] {
		add(&format!("target size {bytes}"), webp(WebpSettings { target: Some(TargetMetric::Size(bytes)), ..d() }));
	}
	for psnr in [30.0_f32, 42.0, 42.5, 50.0] {
		add(&format!("target psnr {psnr}"), webp(WebpSettings { target: Some(TargetMetric::Psnr(psnr)), ..d() }));
	}
	add("target size with one pass", webp(WebpSettings { passes: 1, target: Some(TargetMetric::Size(2_048)), ..d() }));

	add("sharp yuv", webp(WebpSettings { sharp_yuv: true, ..d() }));
	add("low memory", webp(WebpSettings { low_memory: true, ..d() }));
	add("no multi-threading", webp(WebpSettings { multi_threading: false, ..d() }));
	add("jpeg-like", webp(WebpSettings { jpeg_like: true, ..d() }));
	add("jpeg-like with sns 20 and 2 segments", webp(WebpSettings { jpeg_like: true, sns: 20, segments: 2, ..d() }));
	add("exact lossy", webp(WebpSettings { exact: true, ..d() }));
	for alpha_filtering in [AlphaFiltering::Off, AlphaFiltering::Fast, AlphaFiltering::Best] {
		add(&format!("alpha filter {}", alpha_filtering.as_cwebp_str()), webp(WebpSettings { alpha_filtering, ..d() }));
	}
	add("no alpha", webp(WebpSettings { keep_alpha: false, ..d() }));
	for colour in [0x00ff_ffff_u32, 0x0012_3456, 0] {
		add(&format!("blend alpha {colour:06x}"), webp(WebpSettings { blend_alpha: Some(colour), ..d() }));
	}

	lossless_cases(&mut cases);
	geometry_cases(&mut cases);
	cases
}

/// Lossless, near-lossless, `-z` and the presets.
fn lossless_cases(cases: &mut Vec<(String, EncodeJob)>) {
	let mut add = |name: &str, settings: EncodeJob| cases.push((name.to_owned(), settings));
	let webp = |settings: WebpSettings| EncodeJob::from(settings);
	let d = WebpSettings::default;

	add("lossless", webp(WebpSettings { lossless: true, ..d() }));
	add("lossless exact", webp(WebpSettings { lossless: true, exact: true, ..d() }));
	for quality in [0.0_f32, 50.0, 100.0] {
		add(&format!("lossless effort {quality}"), webp(WebpSettings { lossless: true, quality, ..d() }));
	}
	for hint in [ImageHint::Picture, ImageHint::Photo, ImageHint::Graph] {
		add(&format!("lossless hint {hint:?}"), webp(WebpSettings { lossless: true, image_hint: hint, ..d() }));
	}
	for level in [0_u8, 40, 60, 80, 99] {
		add(&format!("near-lossless {level}"), webp(WebpSettings { lossless: true, near_lossless: level, ..d() }));
	}
	add("near-lossless 60, q 90, m 6", webp(WebpSettings { lossless: true, near_lossless: 60, quality: 90.0, method: 6, ..d() }));
	add("lossless alpha method 0", webp(WebpSettings { lossless: true, alpha_compression: false, ..d() }));
	add("lossless no alpha", webp(WebpSettings { lossless: true, keep_alpha: false, ..d() }));
	for level in 0..=9_u8 {
		let mut settings = d();
		settings.apply_lossless_preset(level).expect("a valid level");
		add(&format!("-z {level}"), webp(settings));
	}
	for preset in [Preset::Default, Preset::Photo, Preset::Picture, Preset::Drawing, Preset::Icon, Preset::Text] {
		let mut settings = d();
		settings.apply_preset(preset);
		add(&format!("preset {}", preset.as_cwebp_str()), webp(settings.clone()));
		settings.sns = 30;
		settings.filter_strength = 45;
		add(&format!("preset {} then -sns 30 -f 45", preset.as_cwebp_str()), webp(settings));
	}
}

/// Crop and resize, including the -exact path and the resize modes, and everything at once.
fn geometry_cases(cases: &mut Vec<(String, EncodeJob)>) {
	let mut add = |name: &str, settings: EncodeJob| cases.push((name.to_owned(), settings));
	let d = WebpSettings::default;

	for resize in [Resize::to(32, 24), Resize::to(32, 0), Resize::to(0, 24), Resize::to(128, 96)] {
		add(&format!("resize {}x{}", resize.width, resize.height), EncodeJob { resize, ..Default::default() });
		add(&format!("lossless exact resize {}x{}", resize.width, resize.height), EncodeJob { resize, webp: WebpSettings { lossless: true, exact: true, ..d() }, ..Default::default() });
	}
	for mode in [ResizeMode::DownOnly, ResizeMode::UpOnly] {
		for (width, height) in [(32, 0), (128, 0), (32, 96), (80, 40)] {
			add(&format!("resize {width}x{height} {mode:?}"), EncodeJob { resize: Resize { width, height, mode }, ..Default::default() });
		}
	}
	add("down-only declined still takes the ARGB path", EncodeJob { resize: Resize { width: 128, height: 0, mode: ResizeMode::DownOnly }, webp: WebpSettings { lossless: false, ..d() }, ..Default::default() });
	for crop in [Crop { x: 0, y: 0, width: 64, height: 48 }, Crop { x: 5, y: 3, width: 31, height: 17 }, Crop { x: 63, y: 47, width: 1, height: 1 }] {
		add(&format!("crop {crop:?}"), EncodeJob { crop: Some(crop), ..Default::default() });
		add(&format!("crop {crop:?} then resize 40x0"), EncodeJob { crop: Some(crop), resize: Resize::to(40, 0), ..Default::default() });
	}

	// The full surface at once, lossy and lossless.
	add("everything lossy", EncodeJob { crop: Some(Crop { x: 2, y: 2, width: 60, height: 40 }), resize: Resize::to(40, 0), webp: WebpSettings { quality: 61.5, alpha_quality: 77, alpha_compression: true, alpha_filtering: AlphaFiltering::Fast, method: 5, segments: 3, partition_limit: 22, sns: 66, passes: 4, filter_type: FilterType::Simple, autofilter: false, filter_strength: 44, filter_sharpness: 2, target: Some(TargetMetric::Size(4_096)), qmin: 5, qmax: 95, preprocessing: 1, jpeg_like: true, sharp_yuv: true, low_memory: true, multi_threading: true, exact: true, blend_alpha: Some(0x0080_8080), ..d() }, ..Default::default() });
	add("everything lossless", EncodeJob { crop: Some(Crop { x: 2, y: 2, width: 60, height: 40 }), resize: Resize { width: 30, height: 30, mode: ResizeMode::DownOnly }, webp: WebpSettings { lossless: true, near_lossless: 70, exact: true, quality: 33.0, method: 2, image_hint: ImageHint::Photo, alpha_quality: 40, alpha_filtering: AlphaFiltering::Best, keep_alpha: true, ..d() }, ..Default::default() });
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

/// The same comparison, but through a **PNG file** rather than raw pixels.
///
/// The main test deliberately removes the image decoder from the comparison so a mismatch
/// can only be the encoder. This one puts it back: our pipeline decodes the PNG with the
/// `image` crate, `cwebp` decodes it with libpng, and both then encode. Passing means the
/// whole file path agrees, not just the encoder — which is what a user actually exercises.
///
/// A failure here is therefore a *decoder* difference, not an encoder one, and the message
/// says so; the two are worth telling apart.
#[test]
fn matches_reference_cwebp_through_a_png_file() {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let Some(cwebp) = reference_cwebp() else {
		let message = "PNG PARITY NOT RUN: no reference cwebp found.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};
	let linked = skidbladnir_encode::encoder::linked_encoder_version();
	if reference_version(&cwebp) != Some(linked) {
		let message = "PNG PARITY NOT RUN: reference cwebp and the linked libwebp are different versions.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	}

	let (width, height) = (64_u32, 48_u32);
	let pixels = fixture(width, height);
	let dir = env::temp_dir().join(format!("skidbladnir-parity-png-{}", std::process::id()));
	fs::create_dir_all(&dir).expect("create the scratch directory");
	let png = dir.join("fixture.png");
	image::RgbaImage::from_raw(width, height, pixels).expect("buffer matches the dimensions").save_with_format(&png, image::ImageFormat::Png).expect("write the PNG fixture");

	let mut mismatches: Vec<String> = Vec::new();
	// A representative slice rather than every case: this test is about the decoder agreeing,
	// and the encoder surface is already covered above.
	let subset = [("default lossy", EncodeJob::default()), ("lossless", EncodeJob::from(WebpSettings { lossless: true, ..Default::default() })), ("near-lossless 60", EncodeJob::from(WebpSettings { lossless: true, near_lossless: 60, ..Default::default() })), ("quality 30", EncodeJob::from(WebpSettings { quality: 30.0, ..Default::default() })), ("sharp yuv", EncodeJob::from(WebpSettings { sharp_yuv: true, ..Default::default() })), ("resize 32x0", EncodeJob { resize: Resize::to(32, 0), ..Default::default() }), ("no alpha", EncodeJob::from(WebpSettings { keep_alpha: false, ..Default::default() })), ("crop", EncodeJob { crop: Some(Crop { x: 3, y: 4, width: 20, height: 20 }), ..Default::default() })];

	for (name, settings) in &subset {
		let theirs_path = dir.join(format!("cwebp-{}.webp", name.replace(' ', "-")));
		let args = cwebp_args(settings, &png, &theirs_path);
		let run = Command::new(&cwebp).args(&args).output().expect("run the reference cwebp");
		assert!(run.status.success(), "reference cwebp failed for `{name}`: {}", String::from_utf8_lossy(&run.stderr));
		let expected = fs::read(&theirs_path).expect("read the reference output");

		let ours_path = dir.join(format!("ours-{}.webp", name.replace(' ', "-")));
		skidbladnir_encode::source::encode_file(settings, &png, &ours_path).expect("our pipeline encodes");
		let actual = fs::read(&ours_path).expect("read our output");

		if actual != expected {
			mismatches.push(format!("`{name}`: ours {} bytes, cwebp {} bytes", actual.len(), expected.len()));
		}
	}

	let _ = fs::remove_dir_all(&dir);
	assert!(mismatches.is_empty(), "{} of {} PNG-file conversions diverged. The raw-pixel parity test covers the encoder, so this is a DECODER difference between the `image` crate and libpng:\n  {}", mismatches.len(), subset.len(), mismatches.join("\n  "));
	eprintln!("PNG PARITY OK: {} PNG-file conversions matched the reference cwebp end to end.", subset.len());
}

/// Parity across PNG **colour types and bit depths**, not just 8-bit RGBA.
///
/// This exists because a real divergence hid here. `cwebp` reads PNGs through libpng with
/// `png_set_strip_16`, which discards the low byte of a 16-bit sample, while the `image`
/// crate scales it — so 16-bit input encoded to different bytes on the two paths (216 vs
/// 212 for RGBA, 168 vs 166 for RGB) while every 8-bit type matched. The reduction was
/// changed to truncate; this test is what stops it drifting back.
#[test]
fn matches_reference_cwebp_across_png_colour_types() {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let Some(cwebp) = reference_cwebp() else {
		let message = "COLOUR-TYPE PARITY NOT RUN: no reference cwebp found.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};
	if reference_version(&cwebp) != Some(skidbladnir_encode::encoder::linked_encoder_version()) {
		let message = "COLOUR-TYPE PARITY NOT RUN: reference cwebp and the linked libwebp are different versions.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	}

	let dir = env::temp_dir().join(format!("skidbladnir-parity-colour-{}", std::process::id()));
	fs::create_dir_all(&dir).expect("create the scratch directory");
	let (width, height) = (48_u32, 32_u32);

	// One fixture per colour type the `image` crate can write.
	let mut fixtures: Vec<(&str, PathBuf)> = Vec::new();

	let path = dir.join("rgba16.png");
	let mut buffer = image::ImageBuffer::<image::Rgba<u16>, Vec<u16>>::new(width, height);
	for (x, y, pixel) in buffer.enumerate_pixels_mut() {
		*pixel = image::Rgba([u16::try_from((x * 1300) % 65536).unwrap_or(0), u16::try_from((y * 2000) % 65536).unwrap_or(0), 30_000, if x % 3 == 0 { 20_000 } else { u16::MAX }]);
	}
	buffer.save_with_format(&path, image::ImageFormat::Png).expect("write rgba16");
	fixtures.push(("16-bit RGBA", path));

	let path = dir.join("rgb16.png");
	let mut buffer = image::ImageBuffer::<image::Rgb<u16>, Vec<u16>>::new(width, height);
	for (x, y, pixel) in buffer.enumerate_pixels_mut() {
		*pixel = image::Rgb([u16::try_from((x * 1300) % 65536).unwrap_or(0), u16::try_from((y * 2000) % 65536).unwrap_or(0), 30_000]);
	}
	buffer.save_with_format(&path, image::ImageFormat::Png).expect("write rgb16");
	fixtures.push(("16-bit RGB", path));

	let path = dir.join("gray8.png");
	let mut buffer = image::GrayImage::new(width, height);
	for (x, y, pixel) in buffer.enumerate_pixels_mut() {
		*pixel = image::Luma([u8::try_from((x * 5 + y * 3) % 256).unwrap_or(0)]);
	}
	buffer.save_with_format(&path, image::ImageFormat::Png).expect("write gray8");
	fixtures.push(("8-bit grayscale", path));

	let path = dir.join("graya8.png");
	let mut buffer = image::GrayAlphaImage::new(width, height);
	for (x, y, pixel) in buffer.enumerate_pixels_mut() {
		*pixel = image::LumaA([u8::try_from((x * 7 + y) % 256).unwrap_or(0), if x % 4 == 0 { 60 } else { 255 }]);
	}
	buffer.save_with_format(&path, image::ImageFormat::Png).expect("write graya8");
	fixtures.push(("8-bit grayscale + alpha", path));

	let settings = EncodeJob::default();
	let mut mismatches: Vec<String> = Vec::new();
	for (name, input) in &fixtures {
		let theirs_path = dir.join(format!("cwebp-{}.webp", name.replace([' ', '+', '-'], "_")));
		let run = Command::new(&cwebp).args(cwebp_args(&settings, input, &theirs_path)).output().expect("run the reference cwebp");
		assert!(run.status.success(), "reference cwebp failed for `{name}`: {}", String::from_utf8_lossy(&run.stderr));

		let ours_path = dir.join(format!("ours-{}.webp", name.replace([' ', '+', '-'], "_")));
		skidbladnir_encode::source::encode_file(&settings, input, &ours_path).expect("our pipeline encodes");

		let (expected, actual) = (fs::read(&theirs_path).expect("read reference"), fs::read(&ours_path).expect("read ours"));
		if actual != expected {
			mismatches.push(format!("`{name}`: ours {} bytes, cwebp {} bytes", actual.len(), expected.len()));
		}
	}

	let _ = fs::remove_dir_all(&dir);
	assert!(mismatches.is_empty(), "{} of {} PNG colour types diverged. This is a DECODER difference — check how `image` reduces the sample depth against what libpng does:\n  {}", mismatches.len(), fixtures.len(), mismatches.join("\n  "));
	eprintln!("COLOUR-TYPE PARITY OK: {} PNG colour types matched the reference cwebp.", fixtures.len());
}

/// `-metadata`: a PNG carrying an ICC profile, Exif and XMP, copied by `cwebp` and by us,
/// every combination of the three, lossy and lossless. The metadata reader and the
/// `VP8X` writer are ours, so this is the test that they are `cwebp`'s.
#[test]
fn matches_reference_cwebp_copying_metadata() {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let Some(cwebp) = reference_cwebp() else {
		let message = "METADATA PARITY NOT RUN: no reference cwebp found.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};
	if reference_version(&cwebp) != Some(skidbladnir_encode::encoder::linked_encoder_version()) {
		let message = "METADATA PARITY NOT RUN: reference cwebp and the linked libwebp are different versions.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	}

	let dir = env::temp_dir().join(format!("skidbladnir-parity-metadata-{}", std::process::id()));
	fs::create_dir_all(&dir).expect("create the scratch directory");
	let (width, height) = (40_u32, 30_u32);
	let mut png = Vec::new();
	{
		let mut encoder = png::Encoder::new(&mut png, width, height);
		encoder.set_color(png::ColorType::Rgba);
		encoder.set_depth(png::BitDepth::Eight);
		encoder.add_itxt_chunk("XML:com.adobe.xmp".to_owned(), "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/>".to_owned()).expect("add XMP");
		let mut writer = encoder.write_header().expect("header");
		let mut iccp = b"test\0\0".to_vec();
		iccp.extend(zlib(&minimal_icc()));
		writer.write_chunk(png::chunk::ChunkType(*b"iCCP"), &iccp).expect("iCCP");
		writer.write_chunk(png::chunk::ChunkType(*b"eXIf"), b"MM\0*\0\0\0\x08\0\0\0\0\0").expect("eXIf");
		writer.write_image_data(&fixture(width, height)).expect("pixels");
	}
	let input = dir.join("metadata.png");
	fs::write(&input, &png).expect("write the fixture");

	let mut mismatches: Vec<String> = Vec::new();
	let mut total = 0;
	for lossless in [false, true] {
		for bits in 1..8_u8 {
			let metadata = WebpMetadata { exif: bits & 1 != 0, icc: bits & 2 != 0, xmp: bits & 4 != 0 };
			let job = EncodeJob::from(WebpSettings { lossless, metadata, ..WebpSettings::default() });
			let name = format!("lossless {lossless}, {metadata:?}");
			let theirs = dir.join(format!("cwebp-{total}.webp"));
			let run = Command::new(&cwebp).args(cwebp_args(&job, &input, &theirs)).output().expect("run the reference cwebp");
			assert!(run.status.success(), "reference cwebp failed for `{name}`: {}", String::from_utf8_lossy(&run.stderr));
			let ours = dir.join(format!("ours-{total}.webp"));
			skidbladnir_encode::source::encode_file(&job, &input, &ours).expect("our pipeline encodes");
			let (expected, actual) = (fs::read(&theirs).expect("read reference"), fs::read(&ours).expect("read ours"));
			if actual != expected {
				mismatches.push(format!("`{name}`: ours {} bytes, cwebp {} bytes", actual.len(), expected.len()));
			}
			total += 1;
		}
	}
	let _ = fs::remove_dir_all(&dir);
	assert!(mismatches.is_empty(), "{} of {total} metadata copies diverged from cwebp:\n  {}", mismatches.len(), mismatches.join("\n  "));
	eprintln!("METADATA PARITY OK: {total} metadata copies matched the reference cwebp.");
}

/// An RGB display profile with one tag and enough incompressible data that its `iCCP`
/// chunk clears libpng's minimum size. libpng silently drops profiles it does not accept,
/// so the fixture has to be one it does.
fn minimal_icc() -> Vec<u8> {
	let mut icc = vec![0_u8; 132 + 12 + 200];
	icc[0..4].copy_from_slice(&344_u32.to_be_bytes());
	icc[8..12].copy_from_slice(&0x0210_0000_u32.to_be_bytes());
	icc[12..16].copy_from_slice(b"mntr");
	icc[16..20].copy_from_slice(b"RGB ");
	icc[20..24].copy_from_slice(b"XYZ ");
	icc[36..40].copy_from_slice(b"acsp");
	icc[68..80].copy_from_slice(&[0, 0, 0xf6, 0xd6, 0, 1, 0, 0, 0, 0, 0xd3, 0x2d]);
	icc[128..132].copy_from_slice(&1_u32.to_be_bytes());
	icc[132..136].copy_from_slice(b"desc");
	icc[136..140].copy_from_slice(&144_u32.to_be_bytes());
	icc[140..144].copy_from_slice(&200_u32.to_be_bytes());
	let mut state = 0x1234_5678_u32;
	for byte in &mut icc[144..] {
		state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
		*byte = state.to_be_bytes()[0];
	}
	icc
}

fn zlib(data: &[u8]) -> Vec<u8> {
	use std::io::Write as _;
	let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
	encoder.write_all(data).expect("compress");
	encoder.finish().expect("compress")
}

/// JPEG input: `cwebp` decodes through libjpeg-turbo, and so do we, so the whole file path
/// must agree at every chroma sampling, and the metadata `cwebp -metadata` copies out of
/// a JPEG with it. The fixtures are the ones the other formats' parity tests share (see
/// `tests/common/mod.rs`); a CMYK JPEG is not here because `cwebp` refuses it (see
/// `tests/cmyk_jpeg.rs`).
#[test]
fn matches_reference_cwebp_through_jpeg_files() {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let Some(cwebp) = reference_cwebp() else {
		let message = "JPEG PARITY NOT RUN: no reference cwebp found.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};
	if reference_version(&cwebp) != Some(skidbladnir_encode::encoder::linked_encoder_version()) {
		let message = "JPEG PARITY NOT RUN: reference cwebp and the linked libwebp are different versions.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	}
	let jpegs: [(&str, &[u8]); 9] = [("4:2:0", include_bytes!("fixtures/jpeg-420.jpg")), ("4:4:4", include_bytes!("fixtures/jpeg-444.jpg")), ("4:2:2", include_bytes!("fixtures/jpeg-422.jpg")), ("4:4:0", include_bytes!("fixtures/jpeg-440.jpg")), ("4:1:1", include_bytes!("fixtures/jpeg-411.jpg")), ("gray", include_bytes!("fixtures/jpeg-gray.jpg")), ("progressive", include_bytes!("fixtures/jpeg-progressive.jpg")), ("RGB-coded", include_bytes!("fixtures/jpeg-rgb.jpg")), ("4:2:0, odd size", include_bytes!("fixtures/jpeg-420-odd.jpg"))];
	// Exif, XMP and a one-segment ICC profile, inserted after SOI.
	let segment = |marker: u8, data: &[u8]| [&[0xff, marker][..], &u16::try_from(data.len() + 2).expect("small").to_be_bytes(), data].concat();
	let icc = [b"ICC_PROFILE\0\x01\x01".as_slice(), &[0_u8; 132]].concat();
	let exif = b"Exif\0\0MM\0*\0\0\0\x08\0\0\0\0\0\0";
	let tagged = [&jpegs[0].1[..2], &segment(0xe1, exif), &segment(0xe1, b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta/>"), &segment(0xe2, &icc), &jpegs[0].1[2..]].concat();

	let dir = env::temp_dir().join(format!("skidbladnir-parity-jpeg-{}", std::process::id()));
	fs::create_dir_all(&dir).expect("create the scratch directory");
	let cases = [("default lossy", EncodeJob::default()), ("lossless", EncodeJob::from(WebpSettings { lossless: true, ..Default::default() })), ("sharp yuv", EncodeJob::from(WebpSettings { sharp_yuv: true, ..Default::default() })), ("resize 32x0", EncodeJob { resize: Resize::to(32, 0), ..Default::default() }), ("-metadata all", EncodeJob::from(WebpSettings { metadata: WebpMetadata { exif: true, icc: true, xmp: true }, ..Default::default() }))];
	let mut mismatches: Vec<String> = Vec::new();
	let mut total = 0;
	for (name, bytes) in jpegs.iter().map(|(name, bytes)| (*name, *bytes)).chain([("4:2:0 with Exif, XMP and ICC", tagged.as_slice())]) {
		let input = dir.join(format!("{}.jpg", name.replace([' ', ':', ','], "_")));
		fs::write(&input, bytes).expect("write the fixture");
		for (label, settings) in &cases {
			total += 1;
			let theirs = dir.join(format!("cwebp-{total}.webp"));
			let run = Command::new(&cwebp).args(cwebp_args(settings, &input, &theirs)).output().expect("run the reference cwebp");
			assert!(run.status.success(), "reference cwebp failed for `{name}, {label}`: {}", String::from_utf8_lossy(&run.stderr));
			let ours = dir.join(format!("ours-{total}.webp"));
			skidbladnir_encode::source::encode_file(settings, &input, &ours).expect("our pipeline encodes");
			let (expected, actual) = (fs::read(&theirs).expect("read reference"), fs::read(&ours).expect("read ours"));
			if actual != expected {
				mismatches.push(format!("`{name}, {label}`: ours {} bytes, cwebp {} bytes", actual.len(), expected.len()));
			}
		}
	}
	let _ = fs::remove_dir_all(&dir);
	assert!(mismatches.is_empty(), "{} of {total} JPEG conversions diverged from cwebp:\n  {}", mismatches.len(), mismatches.join("\n  "));
	eprintln!("JPEG PARITY OK: {total} JPEG conversions matched the reference cwebp end to end.");
}
