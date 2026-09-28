//! The HEIC gate: what we write, decoded by libheif's own `heif-dec`.
//!
//! Like AVIF and JPEG XL, there is no reference encoder output to match byte for byte, so
//! the question is the one a user cares about: does a reference decoder read the file, at
//! the right size, with the right pixels and the right transparency? It also checks HEIC
//! **input**: our decode must agree with `heif-dec`'s.
//!
//! The `heif-dec` on a CI runner is the distribution's libheif, a separate build from the
//! one this app ships, which is what makes it a check rather than a tautology.
//!
//! It needs `heif-dec`, found via `SKIDBLADNIR_REFERENCE_HEIFDEC` or on `PATH`. Without one
//! it prints loudly and returns; with `SKIDBLADNIR_REQUIRE_PARITY=1` (set by CI's parity
//! step, where `heif-dec` is installed) a missing decoder is a failure instead.

use std::{env, fs, path::PathBuf, process::Command};

use skidbladnir_encode::{
	RgbaImage, encode_rgba, heic, settings::{EncodeJob, HeicSettings, OutputFormat, Resize}
};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;

fn reference_heifdec() -> Option<PathBuf> {
	if let Some(path) = env::var_os("SKIDBLADNIR_REFERENCE_HEIFDEC") {
		let path = PathBuf::from(path);
		return path.is_file().then_some(path);
	}
	// Older libheif releases call the tool `heif-convert`.
	["heif-dec", "heif-convert"].into_iter().find(|tool| Command::new(tool).arg("--version").output().is_ok_and(|out| out.status.success() || !out.stdout.is_empty())).map(PathBuf::from)
}

fn fixture() -> Vec<u8> {
	let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
	for y in 0..HEIGHT {
		for x in 0..WIDTH {
			let transparent = x < WIDTH / 4;
			pixels.extend_from_slice(&[u8::try_from(x * 255 / WIDTH).unwrap_or(0), u8::try_from(y * 255 / HEIGHT).unwrap_or(0), 128, if transparent { 0 } else { 255 }]);
		}
	}
	pixels
}

fn decode(heifdec: &PathBuf, file: &[u8], name: &str) -> (u32, u32, Vec<u8>) {
	let dir = env::temp_dir().join(format!("skidbladnir-heic-ref-{}-{name}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("create the scratch directory");
	let (input, output) = (dir.join("in.heic"), dir.join("out.png"));
	fs::write(&input, file).expect("write the HEIC");
	// The job's library path points at the libheif this app ships; the reference decoder
	// must use its own, or the check compares our build with itself.
	let run = Command::new(heifdec).arg(&input).arg(&output).env_remove("LD_LIBRARY_PATH").env_remove("DYLD_LIBRARY_PATH").output().expect("run heif-dec");
	assert!(run.status.success(), "heif-dec could not decode our `{name}` output: {}{}", String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr));
	let image = image::open(&output).expect("read heif-dec's PNG").into_rgba8();
	let _ = fs::remove_dir_all(&dir);
	(image.width(), image.height(), image.into_raw())
}

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
	if squared == 0.0 { f64::INFINITY } else { 10.0 * (255.0 * 255.0 * samples / squared).log10() }
}

fn encode(settings: HeicSettings, resize: Resize, pixels: &[u8]) -> Vec<u8> {
	let job = EncodeJob { format: OutputFormat::Heic, resize, heic: settings, ..Default::default() };
	encode_rgba(&job, &RgbaImage { width: WIDTH, height: HEIGHT, pixels }).expect("our HEIC encode")
}

fn heifdec_or_skip(what: &str) -> Option<PathBuf> {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let found = reference_heifdec();
	if found.is_none() {
		let message = format!("HEIC REFERENCE NOT RUN: no heif-dec found. Set SKIDBLADNIR_REFERENCE_HEIFDEC or put heif-dec on PATH. {what} is UNVERIFIED by a reference decoder in this run.");
		assert!(!require, "{message}");
		eprintln!("{message}");
	}
	found
}

#[test]
fn reference_heifdec_reads_our_heic_correctly() {
	let Some(heifdec) = heifdec_or_skip("HEIC output") else { return };
	let pixels = fixture();

	let (width, height, rgba) = decode(&heifdec, &encode(HeicSettings::default(), Resize::default(), &pixels), "default");
	assert_eq!((width, height), (WIDTH, HEIGHT));
	let default_psnr = opaque_psnr(&pixels, &rgba);
	assert!(default_psnr > 30.0, "default-quality HEIC decodes at only {default_psnr:.1} dB PSNR");
	for (s, d) in pixels.as_chunks::<4>().0.iter().zip(rgba.as_chunks::<4>().0) {
		if s[3] == 0 {
			assert!(d[3] < 16, "a transparent source pixel decoded with alpha {}", d[3]);
		} else {
			assert!(d[3] > 239, "an opaque source pixel decoded with alpha {}", d[3]);
		}
	}

	let low = opaque_psnr(&pixels, &decode(&heifdec, &encode(HeicSettings { quality: 20 }, Resize::default(), &pixels), "q20").2);
	let high = opaque_psnr(&pixels, &decode(&heifdec, &encode(HeicSettings { quality: 95 }, Resize::default(), &pixels), "q95").2);
	assert!(high > low, "quality 95 decodes at {high:.1} dB but quality 20 at {low:.1} dB");

	let (width, height, _) = decode(&heifdec, &encode(HeicSettings::default(), Resize { width: 32, height: 0, no_enlarge: false }, &pixels), "resize");
	assert_eq!((width, height), (32, 24));

	eprintln!("HEIC REFERENCE OK: heif-dec read every file at the right size and alpha (libheif {}; default {default_psnr:.1} dB, q20 {low:.1} dB, q95 {high:.1} dB).", heic::linked_version());
}

/// HEIC input: our decode must agree with `heif-dec`'s on the same file.
#[test]
fn our_heic_decoder_agrees_with_heifdec() {
	let Some(heifdec) = heifdec_or_skip("HEIC input") else { return };
	let file = encode(HeicSettings { quality: 80 }, Resize::default(), &fixture());
	let (_, _, reference) = decode(&heifdec, &file, "agree");
	let (width, height, ours) = heic::decode(&file).expect("our decoder reads our own file");
	assert_eq!((width, height), (WIDTH, HEIGHT));
	let agreement = opaque_psnr(&reference, &ours);
	let worst_alpha = ours.as_chunks::<4>().0.iter().zip(reference.as_chunks::<4>().0).map(|(a, b)| a[3].abs_diff(b[3])).max().unwrap_or(0);
	assert!(agreement > 40.0, "our decode and heif-dec's agree at only {agreement:.1} dB");
	assert!(worst_alpha <= 2, "alpha differs from heif-dec's by up to {worst_alpha}");
	eprintln!("HEIC DECODE OK: agreement {agreement:.1} dB, alpha within {worst_alpha}.");
}
