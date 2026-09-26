//! The AVIF gate: what we write, decoded by libavif's own `avifdec`.
//!
//! WebP has a byte-for-byte gate because `cwebp` defines what the output should be. AVIF
//! has no such reference encoder to match — Skidbladnir's AVIF comes from `ravif`, and
//! libavif's `avifenc` would make different (not wrong) choices. So the question this
//! asks is the one a user actually cares about: does a *reference decoder* read the file,
//! at the right size, with the right pixels and the right transparency?
//!
//! "Right pixels" is measured, not eyeballed: PSNR of the decoded colour against the
//! source over the opaque pixels, which must clear a floor and must rise with quality.
//!
//! It needs `avifdec`, found via `SKIDBLADNIR_REFERENCE_AVIFDEC` or on `PATH`. Without one
//! it prints loudly and returns; with `SKIDBLADNIR_REQUIRE_PARITY=1` (set by CI's parity
//! step, where `avifdec` is installed) a missing decoder is a failure instead.

use std::{env, fs, path::PathBuf, process::Command};

use skidbladnir_encode::{
	RgbaImage, encode_rgba, settings::{AvifBitDepth, AvifColorModel, AvifSettings, EncodeJob, OutputFormat, Resize}
};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;

fn reference_avifdec() -> Option<PathBuf> {
	if let Some(path) = env::var_os("SKIDBLADNIR_REFERENCE_AVIFDEC") {
		let path = PathBuf::from(path);
		return path.is_file().then_some(path);
	}
	Command::new("avifdec").arg("--version").output().ok().filter(|out| out.status.success()).map(|_| PathBuf::from("avifdec"))
}

/// Smooth ramps plus a transparent left quarter, so both colour fidelity and alpha have
/// something to be wrong about.
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

/// What `avifdec` made of one of our files.
struct Decoded {
	width: u32,
	height: u32,
	rgba: Vec<u8>,
}

fn decode(avifdec: &PathBuf, avif: &[u8], name: &str) -> Decoded {
	let dir = env::temp_dir().join(format!("skidbladnir-avif-ref-{}-{name}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("create the scratch directory");
	let (input, output) = (dir.join("in.avif"), dir.join("out.png"));
	fs::write(&input, avif).expect("write the AVIF");
	let run = Command::new(avifdec).args(["-d", "8"]).arg(&input).arg(&output).output().expect("run avifdec");
	assert!(run.status.success(), "avifdec could not decode our `{name}` output: {}{}", String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr));
	let image = image::open(&output).expect("read avifdec's PNG").into_rgba8();
	let _ = fs::remove_dir_all(&dir);
	Decoded { width: image.width(), height: image.height(), rgba: image.into_raw() }
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

fn encode(settings: AvifSettings, resize: Resize, pixels: &[u8]) -> Vec<u8> {
	let job = EncodeJob { format: OutputFormat::Avif, resize, avif: settings, ..Default::default() };
	encode_rgba(&job, &RgbaImage { width: WIDTH, height: HEIGHT, pixels }).expect("our AVIF encode")
}

#[test]
fn reference_avifdec_reads_our_avif_correctly() {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let Some(avifdec) = reference_avifdec() else {
		let message = "AVIF REFERENCE NOT RUN: no avifdec found. Set SKIDBLADNIR_REFERENCE_AVIFDEC or put avifdec on PATH. AVIF output is UNVERIFIED by a reference decoder in this run.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};
	let pixels = fixture();
	// Speed 8 keeps the test quick without being rav1e's least representative preset.
	let fast = AvifSettings { speed: 8, ..AvifSettings::default() };
	let mut checked = 0;

	// Default settings, 10-bit: the right size, the right colour, the right alpha.
	let decoded = decode(&avifdec, &encode(fast.clone(), Resize::default(), &pixels), "default");
	assert_eq!((decoded.width, decoded.height), (WIDTH, HEIGHT));
	let default_psnr = opaque_psnr(&pixels, &decoded.rgba);
	assert!(default_psnr > 35.0, "default-quality AVIF decodes at only {default_psnr:.1} dB PSNR");
	for (s, d) in pixels.as_chunks::<4>().0.iter().zip(decoded.rgba.as_chunks::<4>().0) {
		if s[3] == 0 {
			assert!(d[3] < 16, "a transparent source pixel decoded with alpha {}", d[3]);
		} else {
			assert!(d[3] > 239, "an opaque source pixel decoded with alpha {}", d[3]);
		}
	}
	checked += 1;

	// Quality must actually mean something: higher quality, closer pixels.
	let low = opaque_psnr(&pixels, &decode(&avifdec, &encode(AvifSettings { quality: 20, ..fast.clone() }, Resize::default(), &pixels), "q20").rgba);
	let high = opaque_psnr(&pixels, &decode(&avifdec, &encode(AvifSettings { quality: 95, ..fast.clone() }, Resize::default(), &pixels), "q95").rgba);
	assert!(high > low, "quality 95 decodes at {high:.1} dB but quality 20 at {low:.1} dB");
	assert!(high > default_psnr - 0.5, "quality 95 ({high:.1} dB) is no better than the default ({default_psnr:.1} dB)");
	checked += 2;

	// The other storage options must still produce files a reference decoder reads well.
	for (name, settings) in [("8-bit", AvifSettings { bit_depth: AvifBitDepth::Eight, ..fast.clone() }), ("rgb model", AvifSettings { color_model: AvifColorModel::Rgb, ..fast.clone() }), ("single thread", AvifSettings { multi_threading: false, ..fast.clone() })] {
		let decoded = decode(&avifdec, &encode(settings, Resize::default(), &pixels), name);
		assert_eq!((decoded.width, decoded.height), (WIDTH, HEIGHT), "{name}");
		let psnr = opaque_psnr(&pixels, &decoded.rgba);
		assert!(psnr > 30.0, "`{name}` decodes at only {psnr:.1} dB PSNR");
		checked += 1;
	}

	// The shared resize, with the height derived from the aspect ratio.
	let decoded = decode(&avifdec, &encode(fast, Resize { width: 32, height: 0 }, &pixels), "resize");
	assert_eq!((decoded.width, decoded.height), (32, 24));
	checked += 1;

	eprintln!("AVIF REFERENCE OK: avifdec decoded {checked} of our AVIF files at the right size, colour and transparency (default {default_psnr:.1} dB, q20 {low:.1} dB, q95 {high:.1} dB).");
}
