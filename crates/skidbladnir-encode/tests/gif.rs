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
	animation::{self, Keyframes}, gif_input, settings::{Resize, WebpSettings}
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
/// `tests/animation.rs`, the lossy cases start from libwebp's defaults, since `gif2webp` has
/// no flags for the filter, pass count and alpha filtering the app sets differently. Alpha filtering is the one that
/// showed: the app's default ("best") left mostly transparent lossy frames 2 bytes off.
fn cases() -> Vec<(&'static str, WebpSettings, Vec<&'static str>)> {
	let lossy = WebpSettings { multi_threading: true, ..WebpSettings::libwebp_defaults() };
	let mut cases = Vec::new();
	let mut add = |name: &'static str, settings: WebpSettings, flags: Vec<&'static str>| cases.push((name, settings, flags));
	add("lossless (the default)", WebpSettings { lossless: true, exact: true, ..Default::default() }, vec![]);
	add("lossless q100 m6", WebpSettings { lossless: true, exact: true, quality: 100.0, method: 6, ..Default::default() }, vec!["-q", "100", "-m", "6"]);
	add("lossy q75", lossy.clone(), vec!["-lossy"]);
	add("lossy q30 m6", WebpSettings { quality: 30.0, method: 6, ..lossy.clone() }, vec!["-lossy", "-q", "30", "-m", "6"]);
	add("lossy filter 25", WebpSettings { filter_strength: 25, ..lossy.clone() }, vec!["-lossy", "-f", "25"]);
	add("lossy sharp yuv", WebpSettings { sharp_yuv: true, ..lossy }, vec!["-lossy", "-sharp_yuv"]);
	add("near-lossless 40", WebpSettings { lossless: true, near_lossless: 40, ..Default::default() }, vec!["-near_lossless", "40"]);
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

/// An application extension carrying `payload` the way its kind is written into a GIF: an
/// ICC profile in length-prefixed sub-blocks, an XMP packet raw, followed by XMP's 258-byte
/// "magic trailer" (`0x01`, `0xff` down to `0x00`, then the block terminator), which lets the
/// packet's own bytes double as sub-block lengths.
fn app_extension(identifier: &[u8; 11], payload: &[u8], xmp: bool) -> Vec<u8> {
	let mut out = vec![0x21, 0xff, 11];
	out.extend_from_slice(identifier);
	if xmp {
		out.extend_from_slice(payload);
		out.push(0x01);
		out.extend((0..=255_u8).rev());
		out.push(0);
	} else {
		for block in payload.chunks(255) {
			out.push(u8::try_from(block.len()).expect("at most 255"));
			out.extend_from_slice(block);
		}
		out.push(0);
	}
	out
}

/// `gif` with `extensions` placed after its header and global colour table, or after its
/// first image when `after_first_image` is set (`gif2webp` reads extensions anywhere).
fn with_extensions(gif: &[u8], extensions: &[Vec<u8>], after_first_image: bool) -> Vec<u8> {
	let flags = gif[10];
	let mut at = 13 + if flags & 0x80 == 0 { 0 } else { 3 << ((flags & 7) + 1) };
	if after_first_image {
		// Skip extensions up to the first image, then the image itself.
		loop {
			match gif[at] {
				0x21 => {
					at += 2;
					while gif[at] != 0 {
						at += 1 + usize::from(gif[at]);
					}
					at += 1;
				}
				0x2c => {
					let local = gif[at + 9];
					at += 10 + if local & 0x80 == 0 { 0 } else { 3 << ((local & 7) + 1) } + 1;
					while gif[at] != 0 {
						at += 1 + usize::from(gif[at]);
					}
					at += 1;
					break;
				}
				other => panic!("unexpected GIF block {other:#x}"),
			}
		}
	}
	[&gif[..at], &extensions.concat(), &gif[at..]].concat()
}

/// `gif2webp -metadata`: the ICC profile and XMP a GIF carries in application extensions,
/// kept or not, through the whole conversion (`encode_file`), against `gif2webp` itself.
#[test]
fn matches_reference_gif2webp_keeping_metadata() {
	use skidbladnir_encode::{
		settings::{EncodeJob, WebpMetadata}, source::encode_file
	};

	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let linked = skidbladnir_encode::encoder::linked_encoder_version();
	let Some(gif2webp) = reference_gif2webp().filter(|tool| reference_version(tool) == Some(linked)) else {
		let message = "GIF METADATA PARITY NOT RUN: no gif2webp of the linked libwebp version.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};

	let profile: Vec<u8> = (0..700_u32).map(|i| u8::try_from(i * 7 % 251).expect("byte")).collect();
	let other_profile = vec![9_u8; 40];
	let packet = b"<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?><x:xmpmeta xmlns:x='adobe:ns:meta/'><rdf:RDF/></x:xmpmeta><?xpacket end='w'?>".to_vec();
	let icc = |payload: &[u8]| app_extension(b"ICCRGBG1012", payload, false);
	let xmp = |payload: &[u8]| app_extension(b"XMP DataXMP", payload, true);
	let base = fixtures();
	let (still, animated) = (&base.iter().find(|(name, _)| name.contains("still")).expect("a still fixture").1, &base[0].1);
	let sources = [("ICC and XMP, animated", with_extensions(animated, &[icc(&profile), xmp(&packet)], false)), ("XMP then ICC after the first frame", with_extensions(animated, &[xmp(&packet), icc(&profile)], true)), ("two of each: the first wins", with_extensions(still, &[icc(&profile), xmp(&packet), icc(&other_profile), xmp(b"<x/>")], false)), ("ICC only", with_extensions(still, &[icc(&profile)], false))];
	let keep = |exif: bool, icc: bool, xmp: bool| WebpMetadata { exif, icc, xmp };
	let settings = [("none", keep(false, false, false)), ("icc", keep(false, true, false)), ("xmp", keep(false, false, true)), ("all", keep(false, true, true)), ("none", keep(true, false, false))];

	let dir = env::temp_dir().join(format!("skidbladnir-gif-metadata-{}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("create the scratch directory");
	let mut compared = 0;
	let mut mismatches = Vec::new();
	for (source_name, gif_bytes) in &sources {
		let input = dir.join("input.gif");
		fs::write(&input, gif_bytes).expect("write the GIF");
		for (flag, metadata) in settings {
			let job = EncodeJob::from(WebpSettings { lossless: true, exact: true, metadata, ..Default::default() });
			let theirs = dir.join("reference.webp");
			let run = Command::new(&gif2webp).args(["-metadata", flag]).arg(&input).arg("-o").arg(&theirs).output().expect("run gif2webp");
			assert!(run.status.success(), "gif2webp failed on {source_name} / -metadata {flag}: {}", String::from_utf8_lossy(&run.stderr));
			let ours = dir.join("ours.webp");
			encode_file(&job, &input, &ours).expect("our conversion");
			let (theirs, ours) = (fs::read(&theirs).expect("read theirs"), fs::read(&ours).expect("read ours"));
			compared += 1;
			if ours != theirs {
				mismatches.push(format!("{source_name} / -metadata {flag} ({metadata:?}): ours {} bytes, gif2webp {} bytes", ours.len(), theirs.len()));
			}
			// The chunks are really there when asked for, so the gate cannot pass vacuously.
			let has = |fourcc: &[u8]| theirs.windows(4).any(|window| window == fourcc);
			let carries_xmp = !source_name.contains("ICC only");
			assert_eq!((has(b"ICCP"), has(b"XMP ")), (metadata.icc, metadata.xmp && carries_xmp), "{source_name} / -metadata {flag}: gif2webp's chunks");
		}
	}
	let _ = fs::remove_dir_all(&dir);
	assert!(mismatches.is_empty(), "GIF METADATA PARITY FAILED for {} of {compared} conversions:\n{}", mismatches.len(), mismatches.join("\n"));
	eprintln!("GIF METADATA PARITY OK: {compared} conversions byte-identical to gif2webp -metadata");
}

/// The reader on its own: the first of each kind, a profile without its length bytes, a
/// packet without its trailer.
#[test]
fn reads_gif_metadata_as_gif2webp_does() {
	let base = fixtures();
	let gif = with_extensions(&base[0].1, &[app_extension(b"ICCRGBG1012", &[1; 300], false), app_extension(b"XMP DataXMP", b"<x/>", true), app_extension(b"ICCRGBG1012", &[2; 3], false)], true);
	let metadata = gif_input::metadata(&gif);
	assert_eq!(metadata.icc.as_deref(), Some(&[1_u8; 300][..]));
	assert_eq!(metadata.xmp.as_deref(), Some(&b"<x/>"[..]));
	assert_eq!(metadata.exif, None);
	assert_eq!(gif_input::metadata(&base[0].1), skidbladnir_encode::metadata::Metadata::default(), "a GIF without the extensions has none");
	assert_eq!(gif_input::metadata(&gif[..gif.len() - 1]).icc.as_deref(), Some(&[1_u8; 300][..]), "a truncated GIF keeps what was read");
}

/// A still GIF into JPEG XL, against `cjxl`, which reads GIF with its own reader
/// (`lib/extras/dec/gif.cc`) and counts it a lossy input, so its default is lossless.
/// Animated GIFs are refused for JPEG XL output (still images only), as for AVIF and HEIC.
#[test]
fn matches_cjxl_reading_a_still_gif() {
	use skidbladnir_encode::{
		cjxl::cjxl_args, jxl, settings::{EncodeJob, JxlSettings, JxlTarget, OutputFormat}, source::encode_file
	};

	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let cjxl = env::var_os("SKIDBLADNIR_REFERENCE_CJXL").map(PathBuf::from).filter(|path| path.is_file()).or_else(|| Command::new("cjxl").arg("--version").output().ok().filter(|out| out.status.success()).map(|_| PathBuf::from("cjxl")));
	let version = cjxl.as_ref().and_then(|tool| {
		let out = Command::new(tool).arg("--version").output().ok()?;
		let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
		let version = text.split_whitespace().find_map(|word| word.strip_prefix('v'))?.to_owned();
		let mut parts = version.split('.').map(str::parse::<u32>);
		match (parts.next(), parts.next(), parts.next()) {
			(Some(Ok(a)), Some(Ok(b)), Some(Ok(c))) => Some((a, b, c)),
			_ => None,
		}
	});
	let Some(cjxl) = cjxl.filter(|_| version == Some(jxl::linked_version())) else {
		let message = "GIF-TO-JXL PARITY NOT RUN: no cjxl of the linked libjxl version.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};

	let background: fn(u16, u16) -> u8 = |x, y| u8::try_from((x / 5 + y / 5) % 16).unwrap_or(0);
	let stills = [("opaque", build(&[full(0, background)], None)), ("transparent index", build(&[Spec { transparent: Some(3), ..full(0, background) }], None)), ("local palette, interlaced", build(&[Spec { palette: Some((0..16_u8).flat_map(|i| [i * 15, 90, 255 - i * 15]).collect()), interlaced: true, ..full(0, background) }], None)), ("with a loop extension", build(&[full(0, background)], Some(Repeat::Infinite)))];
	// `JxlTarget::Default` is no -d or -q at all, where cjxl makes a GIF lossless.
	let settings = [JxlSettings { effort: 3, ..JxlSettings::default() }, JxlSettings { effort: 7, target: JxlTarget::Default, ..JxlSettings::default() }, JxlSettings { effort: 3, target: JxlTarget::Default, ..JxlSettings::default() }, JxlSettings { effort: 3, target: JxlTarget::Distance(1.5), ..JxlSettings::default() }, JxlSettings { effort: 3, target: JxlTarget::Quality(80.0), ..JxlSettings::default() }];

	let dir = env::temp_dir().join(format!("skidbladnir-gif-jxl-{}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("create the scratch directory");
	let mut compared = 0;
	let mut mismatches = Vec::new();
	for (name, bytes) in &stills {
		let input = dir.join("still.gif");
		fs::write(&input, bytes).expect("write the GIF");
		for settings in &settings {
			let job = EncodeJob { format: OutputFormat::Jxl, jxl: settings.clone(), ..EncodeJob::default() };
			let (theirs, ours) = (dir.join("cjxl.jxl"), dir.join("ours.jxl"));
			let run = Command::new(&cjxl).args(cjxl_args(&job, &input, &theirs)).output().expect("run cjxl");
			assert!(run.status.success(), "cjxl refused the {name} GIF: {}", String::from_utf8_lossy(&run.stderr));
			encode_file(&job, &input, &ours).expect("our conversion");
			compared += 1;
			let (expected, actual) = (fs::read(&theirs).expect("read cjxl's"), fs::read(&ours).expect("read ours"));
			if expected != actual {
				mismatches.push(format!("{name}, effort {} {:?}: ours {} bytes, cjxl {} bytes", settings.effort, settings.target, actual.len(), expected.len()));
			}
		}
	}
	let _ = fs::remove_dir_all(&dir);
	assert!(mismatches.is_empty(), "GIF-TO-JXL PARITY FAILED for {} of {compared}:\n{}", mismatches.len(), mismatches.join("\n"));
	eprintln!("GIF-TO-JXL PARITY OK: {compared} still GIFs byte-identical to cjxl");
}
