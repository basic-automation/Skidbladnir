//! The animated-WebP gate: our animation encoder against libwebp's own `img2webp`.
//!
//! `img2webp` is libwebp's reference tool for building an animation from still frames. It
//! drives the same `WebPAnimEncoder` this crate does, so for any configuration both can
//! express, the two must produce the same bytes. Anything else means the per-frame config
//! or the animation options have drifted.
//!
//! # What it covers, and what it cannot
//!
//! `img2webp` exposes lossless/lossy, quality, method, exact, near-lossless, sharp YUV and
//! the loop count — so those are compared byte for byte. It has no flag for segments, SNS,
//! the filter, partitions, targets, presets or alpha, so for those this test cannot hold
//! the animation path to a reference. What keeps them honest is that the per-frame config
//! is built by the very function `tests/parity.rs` gates against `cwebp` across the whole
//! surface; the test below at least proves each one reaches the frames (the output
//! changes when the control does).
//!
//! # How it isolates the encoder
//!
//! As in `tests/parity.rs`, frames are handed to `img2webp` as **PAM** files, which its
//! built-in PNM reader takes without libpng, so both sides encode identical pixels.
//!
//! # When it runs
//!
//! The round-trip tests always run. The comparison needs a reference `img2webp` (via
//! `SKIDBLADNIR_REFERENCE_IMG2WEBP` or on `PATH`) of the same libwebp version this crate
//! links; otherwise it prints loudly and returns, or fails under
//! `SKIDBLADNIR_REQUIRE_PARITY=1`.

use std::{
	env, fs, path::{Path, PathBuf}, process::Command
};

use skidbladnir_encode::{
	animation::{self, Animation, Frame}, settings::{AlphaFiltering, FilterType, Mode, Resize, WebpSettings}
};

const WIDTH: u32 = 48;
const HEIGHT: u32 = 32;

/// Four frames of a square moving across a gradient, with a translucent band, so the
/// encoder has sub-rectangles to find, blending to decide and alpha to carry. Durations
/// differ per frame so a timestamp mistake shows.
fn fixture(loop_count: u32) -> Animation {
	let frames = (0..4_u32)
		.map(|index| {
			let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
			for y in 0..HEIGHT {
				for x in 0..WIDTH {
					let inside = (index * 8..index * 8 + 12).contains(&x) && (10..22).contains(&y);
					let [r, g, b] = if inside { [240, 200, 30] } else { [u8::try_from(x * 5).unwrap_or(255), u8::try_from(y * 7).unwrap_or(255), 120] };
					let a = if y < 4 { 128 } else { 255 };
					pixels.extend_from_slice(&[r, g, b, a]);
				}
			}
			Frame { pixels, duration_ms: 70 + index * 30 }
		})
		.collect();
	Animation { width: WIDTH, height: HEIGHT, loop_count, background: 0xffff_ffff, frames }
}

fn encode(settings: &WebpSettings, animation: &Animation) -> Vec<u8> {
	animation::encode(settings, Resize::default(), animation, &mut |_| true).expect("the animation must encode")
}

/// A lossless re-encode gives back exactly the frames, durations, loop count and
/// background it was given — decoding and encoding are each other's inverse.
#[test]
fn lossless_round_trip_is_exact() {
	let original = fixture(3);
	let settings = WebpSettings { mode: Mode::Lossless, ..Default::default() };
	let decoded = animation::decode(&encode(&settings, &original)).expect("our output must decode");
	assert_eq!(decoded, original);
}

/// A lossy re-encode keeps the timing and dimensions, and the pixels stay close.
#[test]
fn lossy_round_trip_keeps_the_timing() {
	let original = fixture(0);
	let decoded = animation::decode(&encode(&WebpSettings::default(), &original)).expect("our output must decode");
	assert_eq!((decoded.width, decoded.height, decoded.loop_count), (WIDTH, HEIGHT, 0));
	assert_eq!(decoded.frames.iter().map(|f| f.duration_ms).collect::<Vec<_>>(), [70, 100, 130, 160]);
	for (ours, theirs) in decoded.frames.iter().zip(&original.frames) {
		// Mean rather than worst: the square's hard yellow edge on a gradient is exactly where
		// lossy WebP rings, so a single channel can be far off in a frame that is fine.
		let total: u64 = ours.pixels.iter().zip(&theirs.pixels).map(|(a, b)| u64::from(a.abs_diff(*b))).sum();
		let mean = total / theirs.pixels.len() as u64;
		assert!(mean <= 4, "a default-quality frame should be recognisably the same image, mean channel error {mean}");
	}
}

/// A file written by libwebp's own `img2webp`, not by us, decodes to the frames, timing
/// and loop count it was built from — so the decoder is not only ever tested against our
/// own encoder.
///
/// `tests/fixtures/animated.webp` (348 bytes, SHA-256
/// `1814118985636bc723fd3283502ff50b4bcc1ea3e2fa187c9907d13a5eb173ad`) was made with
/// libwebp 1.6.0's `img2webp -lossless -loop 2 -d 80 f0.pam -d 120 f1.pam -d 160 f2.pam`
/// from three 24x16 frames drawn by the pattern in [`fixture_frame`]. The smoke test
/// converts the same file through the running window.
#[test]
fn a_reference_animation_decodes_as_built() {
	let decoded = animation::decode(include_bytes!("fixtures/animated.webp")).expect("the img2webp fixture must decode");
	assert_eq!((decoded.width, decoded.height, decoded.loop_count), (24, 16, 2));
	assert_eq!(decoded.frames.iter().map(|frame| frame.duration_ms).collect::<Vec<_>>(), [80, 120, 160]);
	for (index, frame) in decoded.frames.iter().enumerate() {
		let expected = fixture_frame(u32::try_from(index).expect("small"));
		for (at, (ours, theirs)) in frame.pixels.as_chunks::<4>().0.iter().zip(expected.as_chunks::<4>().0).enumerate() {
			// Lossless without -exact may rewrite the colour under fully transparent pixels,
			// which no one can see; only the alpha is guaranteed there.
			if theirs[3] == 0 {
				assert_eq!(ours[3], 0, "frame {index} pixel {at} must stay transparent");
			} else {
				assert_eq!(ours, theirs, "frame {index} pixel {at}");
			}
		}
	}
}

/// Frame `index` of the `animated.webp` fixture: a red block moving over a gradient, with
/// the top two rows transparent.
fn fixture_frame(index: u32) -> Vec<u8> {
	let mut pixels = Vec::new();
	for y in 0..16_u32 {
		for x in 0..24_u32 {
			let inside = (index * 6..index * 6 + 8).contains(&x) && (4..12).contains(&y);
			let pixel = if inside { [230, 60, 90, 255] } else { [u8::try_from(x * 10).unwrap_or(255), u8::try_from(y * 15).unwrap_or(255), 200, if y > 1 { 255 } else { 0 }] };
			pixels.extend_from_slice(&pixel);
		}
	}
	pixels
}

/// A still WebP decodes as a one-frame animation, so the decoder is not a second path
/// that only animations exercise.
#[test]
fn a_still_webp_decodes_as_one_frame() {
	let still = skidbladnir_encode::encode_rgba(&WebpSettings { mode: Mode::Lossless, ..Default::default() }.into(), &skidbladnir_encode::RgbaImage { width: WIDTH, height: HEIGHT, pixels: &fixture(0).frames[0].pixels }).expect("encode a still");
	let decoded = animation::decode(&still).expect("a still WebP must decode");
	assert_eq!(decoded.frames.len(), 1);
	assert_eq!(decoded.frames[0].pixels, fixture(0).frames[0].pixels);
}

/// The resize applies to every frame, with the still path's aspect-ratio rule.
#[test]
fn resize_applies_to_every_frame() {
	let resized = animation::encode(&WebpSettings::default(), Resize { width: 24, height: 0 }, &fixture(0), &mut |_| true).expect("encode");
	let decoded = animation::decode(&resized).expect("decode");
	assert_eq!((decoded.width, decoded.height, decoded.frames.len()), (24, 16, 4));
}

/// Cancelling between frames stops the encode, and progress reaches 100 on completion.
#[test]
fn progress_and_cancel() {
	let mut seen = Vec::new();
	animation::encode(&WebpSettings::default(), Resize::default(), &fixture(0), &mut |p| {
		seen.push(p);
		true
	})
	.expect("encode");
	assert_eq!(seen, [25, 50, 75, 100]);

	let error = animation::encode(&WebpSettings::default(), Resize::default(), &fixture(0), &mut |p| p < 50).expect_err("cancel");
	assert_eq!(error, skidbladnir_encode::EncodeError::Cancelled);
}

/// Controls `img2webp` cannot express still reach the frames: changing one changes the
/// output. Not parity — see the module docs — but it catches a control being dropped on
/// the animation path.
#[test]
fn controls_without_an_img2webp_flag_reach_the_frames() {
	let animation = fixture(0);
	let base = WebpSettings { filter: FilterType::Strong, filter_strength: 60, ..Default::default() };
	let baseline = encode(&base, &animation);
	let mut variants: Vec<(&str, WebpSettings)> = Vec::new();
	let mut add = |name: &'static str, settings: WebpSettings| variants.push((name, settings));
	add("segments", WebpSettings { segments: 1, ..base.clone() });
	add("sns", WebpSettings { sns: 0, ..base.clone() });
	add("filter strength", WebpSettings { filter_strength: 0, ..base.clone() });
	add("filter sharpness", WebpSettings { filter_sharpness: 7, ..base.clone() });
	add("alpha quality", WebpSettings { alpha_quality: 10, ..base.clone() });
	for (name, settings) in variants {
		assert_ne!(encode(&settings, &animation), baseline, "changing {name} did not change the animation");
	}
}

fn reference_img2webp() -> Option<PathBuf> {
	if let Some(path) = env::var_os("SKIDBLADNIR_REFERENCE_IMG2WEBP") {
		let path = PathBuf::from(path);
		return path.is_file().then_some(path);
	}
	Command::new("img2webp").arg("-version").output().ok().filter(|out| out.status.success()).map(|_| PathBuf::from("img2webp"))
}

/// Parse `WebP Encoder version: 1.6.0` from `img2webp -version`.
fn reference_version(img2webp: &Path) -> Option<(i32, i32, i32)> {
	let out = Command::new(img2webp).arg("-version").output().ok()?;
	let text = String::from_utf8_lossy(&out.stdout);
	let version = text.lines().find_map(|line| line.strip_prefix("WebP Encoder version:"))?.trim();
	let mut parts = version.split('.').map(str::parse::<i32>);
	match (parts.next(), parts.next(), parts.next()) {
		(Some(Ok(major)), Some(Ok(minor)), Some(Ok(revision))) => Some((major, minor, revision)),
		_ => None,
	}
}

fn write_pam(path: &Path, pixels: &[u8]) {
	let mut out = format!("P7\nWIDTH {WIDTH}\nHEIGHT {HEIGHT}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n").into_bytes();
	out.extend_from_slice(pixels);
	fs::write(path, out).expect("write a PAM frame");
}

/// Our settings, and the `img2webp` flags that ask libwebp for the same per-frame config.
///
/// The lossy cases pin three controls to libwebp's own defaults, because `img2webp` has no
/// flag for any of them: the filter (the app defaults to auto; libwebp to strong, strength
/// 60), the pass count (the app 6; libwebp 1) and alpha filtering (the app "best"; libwebp
/// "fast" — found by `tests/gif.rs`, where it moved mostly transparent frames by 2 bytes). The pass count is not inert
/// without a target, as one might assume — at `-q 95 -m 1` it changes the output, found
/// by this test.
fn cases() -> Vec<(&'static str, WebpSettings, Vec<&'static str>)> {
	let lossy = WebpSettings { filter: FilterType::Strong, filter_strength: 60, passes: 1, alpha_filtering: Some(AlphaFiltering::Fast), ..Default::default() };
	let mut cases = Vec::new();
	let mut add = |name: &'static str, settings: WebpSettings, flags: Vec<&'static str>| cases.push((name, settings, flags));
	add("lossless", WebpSettings { mode: Mode::Lossless, ..Default::default() }, vec!["-lossless", "-exact", "-q", "75", "-m", "4"]);
	add("lossless q100 m6", WebpSettings { mode: Mode::Lossless, quality: 100, method: 6, ..Default::default() }, vec!["-lossless", "-exact", "-q", "100", "-m", "6"]);
	add("lossless q0 m0", WebpSettings { mode: Mode::Lossless, quality: 0, method: 0, ..Default::default() }, vec!["-lossless", "-exact", "-q", "0", "-m", "0"]);
	add("lossy q75", lossy.clone(), vec!["-lossy", "-q", "75", "-m", "4"]);
	add("lossy q20 m6", WebpSettings { quality: 20, method: 6, ..lossy.clone() }, vec!["-lossy", "-q", "20", "-m", "6"]);
	add("lossy q95 m1", WebpSettings { quality: 95, method: 1, ..lossy.clone() }, vec!["-lossy", "-q", "95", "-m", "1"]);
	add("lossy sharp yuv", WebpSettings { sharp_yuv: true, ..lossy.clone() }, vec!["-lossy", "-sharp_yuv", "-q", "75", "-m", "4"]);
	add("near-lossless 60", WebpSettings { mode: Mode::NearLossless, quality: 60, ..Default::default() }, vec!["-near_lossless", "60"]);
	add("near-lossless 0", WebpSettings { mode: Mode::NearLossless, quality: 0, ..Default::default() }, vec!["-near_lossless", "0"]);
	cases
}

#[test]
fn matches_reference_img2webp() {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let Some(img2webp) = reference_img2webp() else {
		let message = "ANIMATION PARITY NOT RUN: no reference img2webp found. Set SKIDBLADNIR_REFERENCE_IMG2WEBP or put img2webp on PATH.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};
	let linked = skidbladnir_encode::encoder::linked_encoder_version();
	if reference_version(&img2webp) != Some(linked) {
		let message = format!("ANIMATION PARITY NOT RUN: `{}` is not libwebp {}.{}.{}, the version this crate links.", img2webp.display(), linked.0, linked.1, linked.2);
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	}

	let dir = env::temp_dir().join(format!("skidbladnir-animation-{}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("create the scratch directory");

	let mut compared = 0;
	let mut mismatches = Vec::new();
	for loop_count in [0_u32, 3] {
		let animation = fixture(loop_count);
		let mut frame_args = Vec::new();
		for (index, frame) in animation.frames.iter().enumerate() {
			let path = dir.join(format!("frame{index}.pam"));
			write_pam(&path, &frame.pixels);
			frame_args.push(("-d".to_owned(), frame.duration_ms.to_string(), path));
		}

		for (name, settings, flags) in cases() {
			let output = dir.join("reference.webp");
			let mut command = Command::new(&img2webp);
			command.args(&flags);
			if loop_count > 0 {
				command.args(["-loop", &loop_count.to_string()]);
			}
			// Frame options go before the frame they apply to; the encoder flags are
			// repeated for none of them because they carry over from frame to frame.
			for (flag, value, path) in &frame_args {
				command.arg(flag).arg(value).arg(path);
			}
			let run = command.arg("-o").arg(&output).output().expect("run the reference img2webp");
			assert!(run.status.success(), "img2webp failed for {name}: {}", String::from_utf8_lossy(&run.stderr));

			let theirs = fs::read(&output).expect("read the reference output");
			let ours = encode(&settings, &animation);
			compared += 1;
			if ours != theirs {
				mismatches.push(format!("{name} (loop {loop_count}): ours {} bytes, img2webp {} bytes", ours.len(), theirs.len()));
			}
		}
	}
	let _ = fs::remove_dir_all(&dir);

	assert!(mismatches.is_empty(), "ANIMATION PARITY FAILED for {} of {compared} cases:\n{}", mismatches.len(), mismatches.join("\n"));
	eprintln!("ANIMATION PARITY OK: {compared} cases byte-identical to img2webp");
}
