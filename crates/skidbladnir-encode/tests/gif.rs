//! The GIF gate: GIF-to-WebP conversions against libwebp's own `gif2webp`, byte for byte.
//!
//! `gif2webp` is libwebp's reference GIF converter. `src/gif_input.rs` reproduces its
//! reading of a GIF — canvas, disposal, timing, loop count, background — and the animation
//! encoder is given its keyframe spacing, so for every setting `gif2webp` can express the
//! two must write identical files.
//!
//! The GIFs are built here, with the `gif` crate's encoder, to exercise what a GIF can do
//! that a plain frame list cannot: frames smaller than the canvas and offset within it, a
//! transparent index, all three disposal methods, local palettes, an interlaced frame,
//! delays at and under the 10 ms floor, each loop-count form, and a single-frame still.
//!
//! Needs a `gif2webp` (via `SKIDBLADNIR_REFERENCE_GIF2WEBP` or on `PATH`) of the libwebp
//! version this crate links. Without one it prints loudly and returns, or fails under
//! `SKIDBLADNIR_REQUIRE_PARITY=1`. The decode tests always run.

use std::{
	borrow::Cow, env, fs, path::{Path, PathBuf}, process::Command
};

use gif::{DisposalMethod, Encoder, Repeat};
use skidbladnir_encode::{
	animation::{self, Keyframes}, gif_input, settings::{AlphaFiltering, FilterType, Mode, Resize, WebpSettings}
};

const WIDTH: u16 = 40;
const HEIGHT: u16 = 30;

/// A 16-colour global palette: a ramp plus a few saturated entries.
fn global_palette() -> Vec<u8> {
	(0..16_u8).flat_map(|i| [i * 16, 255 - i * 12, if i % 2 == 0 { 40 } else { 220 }]).collect()
}

/// One frame: a `width` x `height` rectangle at (`left`, `top`) filled by `paint`.
struct Spec {
	left: u16,
	top: u16,
	width: u16,
	height: u16,
	delay: u16,
	dispose: DisposalMethod,
	transparent: Option<u8>,
	palette: Option<Vec<u8>>,
	interlaced: bool,
	paint: fn(u16, u16) -> u8,
}

fn build(frames: &[Spec], repeat: Option<Repeat>) -> Vec<u8> {
	let mut out = Vec::new();
	{
		let mut encoder = Encoder::new(&mut out, WIDTH, HEIGHT, &global_palette()).expect("start a GIF");
		if let Some(repeat) = repeat {
			encoder.set_repeat(repeat).expect("write the loop count");
		}
		for spec in frames {
			let buffer: Vec<u8> = (0..spec.height).flat_map(|y| (0..spec.width).map(move |x| (spec.paint)(x, y))).collect();
			let frame = gif::Frame { left: spec.left, top: spec.top, width: spec.width, height: spec.height, delay: spec.delay, dispose: spec.dispose, transparent: spec.transparent, palette: spec.palette.clone(), interlaced: spec.interlaced, buffer: Cow::Owned(buffer), ..gif::Frame::default() };
			encoder.write_frame(&frame).expect("write a GIF frame");
		}
	}
	out
}

fn full(delay: u16, paint: fn(u16, u16) -> u8) -> Spec {
	Spec { left: 0, top: 0, width: WIDTH, height: HEIGHT, delay, dispose: DisposalMethod::Keep, transparent: None, palette: None, interlaced: false, paint }
}

/// The fixtures, by name.
fn fixtures() -> Vec<(&'static str, Vec<u8>)> {
	let background: fn(u16, u16) -> u8 = |x, y| u8::try_from((x / 5 + y / 5) % 16).unwrap_or(0);
	let square: fn(u16, u16) -> u8 = |x, y| if (2..10).contains(&x) && (2..10).contains(&y) { 12 } else { 0 };
	let stripes: fn(u16, u16) -> u8 = |x, _| u8::try_from(x % 4).unwrap_or(0);
	let local = Some((0..8_u8).flat_map(|i| [255 - i * 30, i * 30, 128]).collect::<Vec<_>>());

	vec![
		("moving sprite, transparent, kept", build(&[full(5, background), Spec { left: 4, top: 4, width: 12, height: 12, delay: 0, transparent: Some(0), ..full(0, square) }, Spec { left: 14, top: 8, width: 12, height: 12, delay: 20, transparent: Some(0), ..full(0, square) }, Spec { left: 24, top: 12, width: 12, height: 12, delay: 7, transparent: Some(0), ..full(0, square) }], Some(Repeat::Infinite))),
		("every disposal, local palette, interlaced, loop 2", build(&[full(10, background), Spec { left: 6, top: 6, width: 12, height: 12, delay: 15, dispose: DisposalMethod::Background, transparent: Some(0), ..full(0, square) }, Spec { left: 20, top: 2, width: 16, height: 10, delay: 15, dispose: DisposalMethod::Previous, palette: local.clone(), ..full(0, stripes) }, Spec { left: 0, top: 10, width: 40, height: 20, delay: 25, interlaced: true, ..full(0, background) }, Spec { left: 10, top: 10, width: 8, height: 8, delay: 30, dispose: DisposalMethod::Background, transparent: Some(3), palette: local, ..full(0, stripes) }], Some(Repeat::Finite(2)))),
		("no loop extension (plays once)", build(&[full(12, background), full(12, stripes)], None)),
		("a still GIF", build(&[full(0, background)], Some(Repeat::Infinite))),
		// Long enough that keyframe spacing decides the output: more than gif2webp's lossless
		// maximum of 17 frames between keyframes, and far more than the lossy 5.
		("24 frames, so keyframe spacing matters", build(&(0..24_u16).map(|i| Spec { left: i, top: i / 2, width: 12, height: 12, delay: 4, transparent: Some(0), dispose: if i % 3 == 0 { DisposalMethod::Background } else { DisposalMethod::Keep }, ..full(0, square) }).collect::<Vec<_>>(), Some(Repeat::Infinite))),
		// Written by a different encoder: ImageMagick 7.1.2-31, three 24x16 frames with local
		// palettes, dispose-to-background and a transparent third frame:
		//   magick -size 24x16 -delay 12 gradient:red-blue \( -size 24x16 gradient:green-yellow
		//     -fill white -draw "rectangle 4,4 12,10" \) \( -size 24x16 xc:none -fill orange
		//     -draw "circle 12,8 12,3" \) -loop 0 -dispose background animated.gif
		// 478 bytes, SHA-256 399f03a98124c9f42b6e40c6bc0c14f9e9de2f5d10e16c15ebe25961bb078417.
		// The smoke test converts the same file through the running window.
		("written by ImageMagick", include_bytes!("fixtures/animated.gif").to_vec()),
		("background is the transparent index", build(&[Spec { transparent: Some(0), ..full(8, square) }, Spec { transparent: Some(0), ..full(8, background) }], Some(Repeat::Infinite))),
	]
}

/// The GIF rules `gif_input` promises, checked on the decoded frames directly.
#[test]
fn decodes_as_gif2webp_reads_a_gif() {
	let fixtures = fixtures();
	let moving = gif_input::decode(&fixtures[0].1).expect("decode");
	assert_eq!((moving.width, moving.height, moving.loop_count), (40, 30, 0), "infinite loops stay infinite");
	assert_eq!(moving.frames.iter().map(|f| f.duration_ms).collect::<Vec<_>>(), [50, 100, 200, 70], "10 ms or less becomes 100 ms; 50 ms stays");
	// The sprite's transparent surround leaves the background showing, and "keep" leaves
	// the earlier sprite in place under the later ones.
	let pixel = |frame: usize, x: usize, y: usize| moving.frames[frame].pixels[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4].to_vec();
	assert_eq!(pixel(1, 4, 4), pixel(0, 4, 4), "a transparent index shows the canvas beneath");
	assert_eq!(pixel(3, 7, 7), pixel(1, 7, 7), "a kept sprite stays");

	let disposals = gif_input::decode(&fixtures[1].1).expect("decode");
	assert_eq!(disposals.loop_count, 3, "NETSCAPE 2 repeats is 3 plays in WebP");
	let pixel = |frame: usize, x: usize, y: usize| disposals.frames[frame].pixels[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4].to_vec();
	assert_eq!(pixel(2, 9, 9)[3], 0, "dispose-to-background clears to transparent, not to the background colour");
	assert_eq!(pixel(3, 25, 5), pixel(1, 25, 5), "dispose-to-previous restores what was there");

	assert_eq!(gif_input::decode(&fixtures[2].1).expect("decode").loop_count, 1, "no extension plays once");
	let still = gif_input::decode(&fixtures[3].1).expect("decode");
	assert_eq!((still.frames.len(), still.loop_count), (1, 0));
	assert_eq!(gif_input::decode(&fixtures[6].1).expect("decode").background, 0, "a transparent background index is a transparent hint");
	let magick = gif_input::decode(&fixtures[5].1).expect("decode the ImageMagick GIF");
	assert_eq!((magick.width, magick.height, magick.frames.len(), magick.loop_count), (24, 16, 3, 0));
	assert!(gif_input::decode(b"GIF89a").is_err(), "a truncated GIF is an error, not a panic");
}

fn reference_gif2webp() -> Option<PathBuf> {
	if let Some(path) = env::var_os("SKIDBLADNIR_REFERENCE_GIF2WEBP") {
		let path = PathBuf::from(path);
		return path.is_file().then_some(path);
	}
	Command::new("gif2webp").arg("-version").output().ok().filter(|out| out.status.success()).map(|_| PathBuf::from("gif2webp"))
}

fn reference_version(tool: &Path) -> Option<(i32, i32, i32)> {
	let out = Command::new(tool).arg("-version").output().ok()?;
	let text = String::from_utf8_lossy(&out.stdout);
	let version = text.lines().find_map(|line| line.strip_prefix("WebP Encoder version:"))?.trim();
	let mut parts = version.split('.').map(str::parse::<i32>);
	match (parts.next(), parts.next(), parts.next()) {
		(Some(Ok(major)), Some(Ok(minor)), Some(Ok(revision))) => Some((major, minor, revision)),
		_ => None,
	}
}

/// Our settings and the `gif2webp` flags for the same per-frame config. As in
/// `tests/animation.rs`, the lossy cases pin the filter, pass count and alpha filtering to
/// libwebp's defaults, which `gif2webp` has no flags for. Alpha filtering is the one that
/// showed: the app's default ("best") left mostly transparent lossy frames 2 bytes off.
fn cases() -> Vec<(&'static str, WebpSettings, Vec<&'static str>)> {
	let lossy = WebpSettings { filter: FilterType::Strong, filter_strength: 60, passes: 1, alpha_filtering: Some(AlphaFiltering::Fast), ..Default::default() };
	let mut cases = Vec::new();
	let mut add = |name: &'static str, settings: WebpSettings, flags: Vec<&'static str>| cases.push((name, settings, flags));
	add("lossless (the default)", WebpSettings { mode: Mode::Lossless, ..Default::default() }, vec![]);
	add("lossless q100 m6", WebpSettings { mode: Mode::Lossless, quality: 100, method: 6, ..Default::default() }, vec!["-q", "100", "-m", "6"]);
	add("lossy q75", lossy.clone(), vec!["-lossy"]);
	add("lossy q30 m6", WebpSettings { quality: 30, method: 6, ..lossy.clone() }, vec!["-lossy", "-q", "30", "-m", "6"]);
	add("lossy filter 25", WebpSettings { filter_strength: 25, ..lossy.clone() }, vec!["-lossy", "-f", "25"]);
	add("lossy sharp yuv", WebpSettings { sharp_yuv: true, ..lossy }, vec!["-lossy", "-sharp_yuv"]);
	add("near-lossless 40", WebpSettings { mode: Mode::NearLossless, quality: 40, ..Default::default() }, vec!["-near_lossless", "40"]);
	cases
}

#[test]
fn matches_reference_gif2webp() {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let Some(gif2webp) = reference_gif2webp() else {
		let message = "GIF PARITY NOT RUN: no reference gif2webp found. Set SKIDBLADNIR_REFERENCE_GIF2WEBP or put gif2webp on PATH.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};
	let linked = skidbladnir_encode::encoder::linked_encoder_version();
	if reference_version(&gif2webp) != Some(linked) {
		let message = format!("GIF PARITY NOT RUN: `{}` is not libwebp {}.{}.{}, the version this crate links.", gif2webp.display(), linked.0, linked.1, linked.2);
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	}

	let dir = env::temp_dir().join(format!("skidbladnir-gif-{}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("create the scratch directory");

	let mut compared = 0;
	let mut mismatches = Vec::new();
	for (fixture_name, gif_bytes) in fixtures() {
		let input = dir.join("input.gif");
		fs::write(&input, &gif_bytes).expect("write the GIF");
		let decoded = gif_input::decode(&gif_bytes).expect("decode the fixture");
		for (name, settings, flags) in cases() {
			let output = dir.join("reference.webp");
			let run = Command::new(&gif2webp).args(&flags).arg(&input).arg("-o").arg(&output).output().expect("run gif2webp");
			assert!(run.status.success(), "gif2webp failed on {fixture_name} / {name}: {}", String::from_utf8_lossy(&run.stderr));
			let theirs = fs::read(&output).expect("read the reference output");
			let ours = animation::encode_with(&settings, Resize::default(), &decoded, Keyframes::Gif, &mut |_| true).expect("encode");
			compared += 1;
			if ours != theirs {
				mismatches.push(format!("{fixture_name} / {name}: ours {} bytes, gif2webp {} bytes", ours.len(), theirs.len()));
			}
		}
	}
	let _ = fs::remove_dir_all(&dir);

	assert!(mismatches.is_empty(), "GIF PARITY FAILED for {} of {compared} conversions:\n{}", mismatches.len(), mismatches.join("\n"));
	eprintln!("GIF PARITY OK: {compared} conversions byte-identical to gif2webp");
}
