//! The JPEG XL gate: what we write, decoded by libjxl's own `djxl`.
//!
//! As with AVIF, there is no reference *encoder* output to match byte for byte: our
//! encoder is libjxl, but through its API rather than `cjxl`'s command line, whose own
//! input handling differs. So the question asked is the one a user cares about: does the
//! reference decoder read the file, at the right size, with the right pixels and alpha,
//! and does a recompressed JPEG come back as the original JPEG, bit for bit?
//!
//! It also checks JPEG XL **input**: `jxl-oxide` must decode our files the way `djxl`
//! does.
//!
//! It needs `djxl`, found via `SKIDBLADNIR_REFERENCE_DJXL` or on `PATH`. Without one it
//! prints loudly and returns; with `SKIDBLADNIR_REQUIRE_PARITY=1` (set by CI's parity
//! step, where `djxl` is installed) a missing decoder is a failure instead.

use std::{env, fs, path::PathBuf, process::Command};

use skidbladnir_encode::{
	RgbaImage, encode_rgba, jxl, settings::{EncodeJob, JxlSettings, OutputFormat, Resize}
};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;

fn reference_djxl() -> Option<PathBuf> {
	if let Some(path) = env::var_os("SKIDBLADNIR_REFERENCE_DJXL") {
		let path = PathBuf::from(path);
		return path.is_file().then_some(path);
	}
	Command::new("djxl").arg("--version").output().ok().filter(|out| out.status.success()).map(|_| PathBuf::from("djxl"))
}

/// Ramps plus a transparent left quarter, with colour under the transparent pixels.
fn fixture() -> Vec<u8> {
	let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
	for y in 0..HEIGHT {
		for x in 0..WIDTH {
			let transparent = x < WIDTH / 4;
			pixels.extend_from_slice(&[u8::try_from(x * 255 / WIDTH).unwrap_or(0), u8::try_from(y * 255 / HEIGHT).unwrap_or(0), u8::try_from((x + y) * 2).unwrap_or(0), if transparent { 0 } else { 255 }]);
		}
	}
	pixels
}

fn scratch(name: &str) -> PathBuf {
	let dir = env::temp_dir().join(format!("skidbladnir-jxl-ref-{}-{name}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("create the scratch directory");
	dir
}

/// Run `djxl` on `jxl`, writing `extension`, and return the output bytes.
fn run_djxl(djxl: &PathBuf, jxl: &[u8], name: &str, extension: &str) -> Vec<u8> {
	let dir = scratch(name);
	let (input, output) = (dir.join("in.jxl"), dir.join(format!("out.{extension}")));
	fs::write(&input, jxl).expect("write the JPEG XL");
	let run = Command::new(djxl).arg(&input).arg(&output).output().expect("run djxl");
	assert!(run.status.success(), "djxl could not decode our `{name}` output: {}{}", String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr));
	let bytes = fs::read(&output).expect("read djxl's output");
	let _ = fs::remove_dir_all(&dir);
	bytes
}

/// `djxl`'s decode of a file, as 8-bit RGBA.
fn decode(djxl: &PathBuf, jxl: &[u8], name: &str) -> (u32, u32, Vec<u8>) {
	let png = run_djxl(djxl, jxl, name, "png");
	let image = image::load_from_memory(&png).expect("read djxl's PNG").into_rgba8();
	(image.width(), image.height(), image.into_raw())
}

/// PSNR of the colour channels over the pixels that are opaque in the source.
fn opaque_psnr(source: &[u8], decoded: &[u8]) -> f64 {
	let (mut squared, mut samples) = (0.0_f64, 0.0_f64);
	for (s, d) in source.as_chunks::<4>().0.iter().zip(decoded.as_chunks::<4>().0) {
		if s[3] == 255 {
			for channel in 0..3 {
				let difference = f64::from(s[channel]) - f64::from(d[channel]);
				squared += difference * difference;
				samples += 1.0;
			}
		}
	}
	let mse = squared / samples;
	if mse == 0.0 { f64::INFINITY } else { 10.0 * (255.0 * 255.0 / mse).log10() }
}

fn encode(settings: JxlSettings, resize: Resize, pixels: &[u8]) -> Vec<u8> {
	let job = EncodeJob { format: OutputFormat::Jxl, resize, jxl: settings, ..Default::default() };
	encode_rgba(&job, &RgbaImage { width: WIDTH, height: HEIGHT, pixels }).expect("our JPEG XL encode")
}

fn djxl_or_skip(what: &str) -> Option<PathBuf> {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let found = reference_djxl();
	if found.is_none() {
		let message = format!("JPEG XL REFERENCE NOT RUN: no djxl found. Set SKIDBLADNIR_REFERENCE_DJXL or put djxl on PATH. {what} is UNVERIFIED by a reference decoder in this run.");
		assert!(!require, "{message}");
		eprintln!("{message}");
	}
	found
}

#[test]
fn reference_djxl_reads_our_jpeg_xl_correctly() {
	let Some(djxl) = djxl_or_skip("JPEG XL output") else { return };
	let pixels = fixture();
	let fast = JxlSettings { effort: 3, ..JxlSettings::default() };

	// Default settings: the right size, the right colour, the right alpha.
	let (width, height, rgba) = decode(&djxl, &encode(fast.clone(), Resize::default(), &pixels), "default");
	assert_eq!((width, height), (WIDTH, HEIGHT));
	let default_psnr = opaque_psnr(&pixels, &rgba);
	assert!(default_psnr > 35.0, "default-quality JPEG XL decodes at only {default_psnr:.1} dB PSNR");
	for (s, d) in pixels.as_chunks::<4>().0.iter().zip(rgba.as_chunks::<4>().0) {
		if s[3] == 0 {
			assert!(d[3] < 16, "a transparent source pixel decoded with alpha {}", d[3]);
		} else {
			assert!(d[3] > 239, "an opaque source pixel decoded with alpha {}", d[3]);
		}
	}

	// Quality means something: higher quality, closer pixels.
	let low = opaque_psnr(&pixels, &decode(&djxl, &encode(JxlSettings { quality: 30, ..fast.clone() }, Resize::default(), &pixels), "q30").2);
	let high = opaque_psnr(&pixels, &decode(&djxl, &encode(JxlSettings { quality: 98, ..fast.clone() }, Resize::default(), &pixels), "q98").2);
	assert!(high > low, "quality 98 decodes at {high:.1} dB but quality 30 at {low:.1} dB");

	// Lossless is lossless to the reference decoder too, colour under transparency included.
	let (_, _, exact) = decode(&djxl, &encode(JxlSettings { lossless: true, ..fast.clone() }, Resize::default(), &pixels), "lossless");
	assert_eq!(exact, pixels, "djxl's decode of a lossless file must be the source, byte for byte");

	// Every effort level and single-threaded encoding still produce readable files.
	for effort in [1, 5, 9] {
		let (width, height, rgba) = decode(&djxl, &encode(JxlSettings { effort, multi_threading: effort != 5, ..fast.clone() }, Resize::default(), &pixels), &format!("effort{effort}"));
		assert_eq!((width, height), (WIDTH, HEIGHT));
		assert!(opaque_psnr(&pixels, &rgba) > 30.0, "effort {effort} decodes badly");
	}

	// The shared resize.
	let (width, height, _) = decode(&djxl, &encode(fast, Resize::to(32, 0), &pixels), "resize");
	assert_eq!((width, height), (32, 24));

	eprintln!("JPEG XL REFERENCE OK: djxl read every file at the right size and alpha (default {default_psnr:.1} dB, q30 {low:.1} dB, q98 {high:.1} dB, lossless exact).");
}

/// A recompressed JPEG must come back from the reference decoder as the original JPEG.
#[test]
fn reference_djxl_rebuilds_a_recompressed_jpeg_bit_for_bit() {
	let Some(djxl) = djxl_or_skip("Lossless JPEG recompression") else { return };
	let rgb: Vec<u8> = fixture().as_chunks::<4>().0.iter().flat_map(|&[r, g, b, _]| [r, g, b]).collect();
	let mut jpeg = Vec::new();
	image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 88).encode(&rgb, WIDTH, HEIGHT, image::ExtendedColorType::Rgb8).expect("write a JPEG");
	let jxl = jxl::recompress_jpeg(&JxlSettings { effort: 3, ..JxlSettings::default() }, &jpeg, &mut |_| true).expect("recompress").expect("libjxl takes a baseline JPEG");
	let rebuilt = run_djxl(&djxl, &jxl, "jpeg", "jpg");
	assert_eq!(rebuilt, jpeg, "djxl must reconstruct the original JPEG exactly");
	eprintln!("JPEG XL JPEG RECOMPRESSION OK: {} byte JPEG -> {} byte JPEG XL -> the same JPEG, per djxl.", jpeg.len(), jxl.len());
}

/// JPEG XL input: our decoder (`jxl-oxide`) must read our files as `djxl` does.
#[test]
fn our_jpeg_xl_decoder_agrees_with_djxl() {
	let Some(djxl) = djxl_or_skip("JPEG XL input") else { return };
	let pixels = fixture();
	for (name, settings) in [("lossy", JxlSettings { effort: 3, ..JxlSettings::default() }), ("lossless", JxlSettings { effort: 3, lossless: true, ..JxlSettings::default() })] {
		let file = encode(settings, Resize::default(), &pixels);
		let (_, _, reference) = decode(&djxl, &file, &format!("agree-{name}"));
		let (width, height, ours) = jxl::decode(&file).expect("our decoder reads our own file");
		assert_eq!((width, height), (WIDTH, HEIGHT), "{name}");
		let (mut squared, mut worst_alpha) = (0.0_f64, 0_u8);
		for (a, b) in ours.as_chunks::<4>().0.iter().zip(reference.as_chunks::<4>().0) {
			for channel in 0..3 {
				let difference = f64::from(a[channel]) - f64::from(b[channel]);
				squared += difference * difference;
			}
			worst_alpha = worst_alpha.max(a[3].abs_diff(b[3]));
		}
		let samples = f64::from(WIDTH * HEIGHT * 3);
		let agreement = if squared == 0.0 { f64::INFINITY } else { 10.0 * (255.0 * 255.0 * samples / squared).log10() };
		assert!(agreement > 45.0, "{name}: our decode and djxl's agree at only {agreement:.1} dB");
		assert!(worst_alpha <= 1, "{name}: alpha differs from djxl's by up to {worst_alpha}");
		eprintln!("JPEG XL DECODE OK ({name}): agreement {agreement:.1} dB, alpha within {worst_alpha}.");
	}
}
