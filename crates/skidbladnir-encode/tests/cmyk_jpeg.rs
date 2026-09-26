//! CMYK JPEG input — the one input path where Skidbladnir and `cwebp` knowingly differ.
//!
//! `cwebp` 1.6.0 reads JPEGs through libjpeg and asks for `JCS_RGB` output, which libjpeg
//! cannot produce from a four-channel file: a CMYK JPEG is refused with "libjpeg error:
//! Unsupported color conversion request". There is therefore no reference *output* to hold
//! this path to. What can be checked is that our decoder turns CMYK into the right
//! colours, and that the difference from `cwebp` is still a refusal on its side rather
//! than a second, disagreeing conversion.
//!
//! # The fixture
//!
//! `tests/fixtures/cmyk-adobe.jpg` (439 bytes, SHA-256
//! `ed4afda9d4e74e1c195eead6c886bfc0790b9e6d493c953ca4d401a69a053fbb`) was made with
//! `magick` 7.1.2-31:
//!
//! ```text
//! magick -size 32x16 gradient:red-blue -colorspace CMYK -quality 90 -strip cmyk-adobe.jpg
//! ```
//!
//! It carries an Adobe APP14 marker with transform 2 (YCCK), which is how Photoshop writes
//! CMYK and means the channels are stored **inverted** — the case a naive decoder gets
//! visibly wrong. The `image` crate cannot write CMYK, which is why this is a file rather
//! than a fixture generated at test time.

use std::{env, fs, path::PathBuf, process::Command};

use skidbladnir_encode::{
	settings::EncodeJob, source::{SourceFormat, encode_file, load}
};

const CMYK_JPEG: &[u8] = include_bytes!("fixtures/cmyk-adobe.jpg");

/// Write the fixture into a fresh per-test directory and return its path.
fn fixture_on_disk(test: &str) -> (PathBuf, PathBuf) {
	let dir = env::temp_dir().join(format!("skidbladnir-cmyk-{test}-{}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("create the scratch directory");
	let path = dir.join("cmyk-adobe.jpg");
	fs::write(&path, CMYK_JPEG).expect("write the fixture");
	(dir, path)
}

/// The decoded colours must be the ones the file describes, not their inverse.
///
/// The expected values are `magick`'s own CMYK→sRGB conversion of the same file
/// (`magick cmyk-adobe.jpg -colorspace sRGB`), sampled at both ends and the middle of the
/// red-to-blue ramp. An inverted-CMYK mistake would turn pure red into cyan, so the
/// tolerance of a few levels is for JPEG rounding, not for doubt about the direction.
#[test]
fn adobe_cmyk_jpeg_decodes_to_the_right_colours() {
	let (dir, path) = fixture_on_disk("colours");
	let image = load(&path).expect("a CMYK JPEG must load");
	let _ = fs::remove_dir_all(&dir);

	assert_eq!(image.format, SourceFormat::Jpeg);
	assert_eq!((image.width, image.height), (32, 16));

	for ((x, y), expected) in [((0_u32, 0_u32), [255_u8, 0, 0]), ((16, 8), [120, 0, 136]), ((31, 15), [0, 0, 255])] {
		let at = usize::try_from((y * image.width + x) * 4).expect("fits in usize");
		let actual = &image.pixels[at..at + 4];
		for channel in 0..3 {
			assert!(actual[channel].abs_diff(expected[channel]) <= 3, "pixel ({x}, {y}) decoded as {actual:?}, ImageMagick reads {expected:?}");
		}
		assert_eq!(actual[3], 255, "a JPEG has no alpha, so it must decode opaque");
	}
}

/// The whole pipeline, not just the decoder: a CMYK JPEG converts to a WebP.
#[test]
fn adobe_cmyk_jpeg_converts() {
	let (dir, path) = fixture_on_disk("converts");
	let output = dir.join("cmyk-adobe.webp");
	let conversion = encode_file(&EncodeJob::default(), &path, &output).expect("a CMYK JPEG must convert");
	assert_eq!((conversion.width, conversion.height), (32, 16));
	assert!(fs::read(&output).expect("read the output").starts_with(b"RIFF"), "the output must be a WebP");
	let _ = fs::remove_dir_all(&dir);
}

/// The divergence from `cwebp` must stay a refusal on its side.
///
/// If a future libwebp starts converting CMYK itself, this fails — and that is the point:
/// the two paths would then be *both* producing output, and whether they agree becomes a
/// parity question that has to be tested rather than left documented as a refusal.
///
/// Before trusting the refusal it checks that the reference encoder reads JPEG at all, so
/// a `cwebp` built without libjpeg cannot make this pass for the wrong reason.
#[test]
fn reference_cwebp_still_refuses_cmyk_jpeg() {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let cwebp = match env::var_os("SKIDBLADNIR_REFERENCE_CWEBP") {
		Some(path) => PathBuf::from(path),
		None => PathBuf::from("cwebp"),
	};
	if Command::new(&cwebp).arg("-version").output().ok().is_none_or(|out| !out.status.success()) {
		let message = "CMYK REFUSAL NOT CHECKED: no reference cwebp found.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	}

	let (dir, path) = fixture_on_disk("reference");

	// An RGB JPEG it must accept, written by the `image` crate.
	let rgb = dir.join("rgb.jpg");
	image::RgbImage::from_pixel(8, 8, image::Rgb([200, 40, 90])).save_with_format(&rgb, image::ImageFormat::Jpeg).expect("write the RGB JPEG");
	let accepted = Command::new(&cwebp).arg(&rgb).arg("-o").arg(dir.join("rgb.webp")).output().expect("run the reference cwebp");
	if !accepted.status.success() {
		let _ = fs::remove_dir_all(&dir);
		let message = format!("CMYK REFUSAL NOT CHECKED: the reference cwebp cannot read JPEG at all, so its refusal of CMYK would prove nothing: {}", String::from_utf8_lossy(&accepted.stderr));
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	}

	let refused = Command::new(&cwebp).arg(&path).arg("-o").arg(dir.join("cmyk.webp")).output().expect("run the reference cwebp");
	let _ = fs::remove_dir_all(&dir);
	assert!(!refused.status.success(), "the reference cwebp now converts CMYK JPEG. Skidbladnir and cwebp both produce output for it, so their agreement must now be parity-tested instead of documented as a refusal.");
	eprintln!("CMYK REFUSAL OK: the reference cwebp refuses CMYK JPEG ({}), and Skidbladnir converts it.", String::from_utf8_lossy(&refused.stderr).lines().next().unwrap_or_default().trim());
}
