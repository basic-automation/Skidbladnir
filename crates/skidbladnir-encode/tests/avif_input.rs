//! The wide AVIF-input gate: `source::decode_avif` (avif-decode over rav1d) against
//! libavif's own `avifdec`, across the colour encodings a real AVIF can carry.
//!
//! `tests/avif_reference.rs` already checks agreement on two files (8- and 10-bit). This
//! covers the rest of the space a user's camera or export tool can produce: `avifenc`
//! writes each fixture from one PNG in 8, 10 and 12 bits; 4:4:4, 4:2:2, 4:2:0 and
//! monochrome; full and limited range; the BT.601, BT.709, BT.2020 and identity matrices;
//! straight and premultiplied alpha; and lossless. Two correct decoders can round a
//! YUV-to-RGB conversion differently, so the bound is two-sided: close to `avifdec`
//! (mean ≤ 2, worst ≤ 20 levels) **and no further from the original image than libavif
//! lands**, give or take half a level of mean error. A wrong matrix, range, depth scaling or alpha rule is tens of levels out on
//! both (mutation-tested on a hand-written decoder when this gate was built: forcing full
//! range failed 4 files, BT.601-for-709 failed 4, ignoring premultiplication failed at a
//! mean of 41).
//!
//! What it established about the two decoders:
//!
//! - **Chroma upsampling.** avif-decode upsamples 4:2:0/4:2:2 nearest-neighbour, so
//!   `avifdec` is asked for `-u nearest`. Bilinear filters disagree about siting: at one
//!   hard edge, a bilinear decode, `avifdec -u bilinear` and `magick` gave 204, 170 and
//!   152 for the same red channel.
//! - **Saturated limited-range pixels.** The largest gaps (worst 13–18 levels) are at pure
//!   blue in limited-range 4:2:0 files, where libavif lands *further* from the original
//!   (237) than avif-decode (250).
//! - **Limited-range monochrome.** `avifdec`'s greyscale PNG — and `magick`'s, via
//!   libheif — hand back the raw limited-range luma, so black comes out as 16. avif-decode
//!   expands it to full range, as the same image encoded full-range decodes; that case is
//!   checked against libavif's raw Y plane (a y4m), expanded by `(Y - 16) * 255 / 219`.
//!
//! Needs `avifenc` and `avifdec` (libavif), via `SKIDBLADNIR_REFERENCE_AVIFENC` /
//! `SKIDBLADNIR_REFERENCE_AVIFDEC` or on `PATH`. Without them it prints loudly and
//! returns, or fails under `SKIDBLADNIR_REQUIRE_PARITY=1`.

use std::{
	env, fs, path::{Path, PathBuf}, process::Command
};

use skidbladnir_encode::source::decode_avif;

const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;

/// Colour ramps in every direction plus a saturated block, so every matrix coefficient
/// and both chroma planes matter; alpha falls off across the image when asked for.
fn source(alpha: bool) -> image::RgbaImage {
	image::RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
		let (r, g, b) = if (40..56).contains(&x) && (8..24).contains(&y) { (230, 30, 200) } else { (x * 4, y * 5, 255 - x * 2 - y) };
		let a = if alpha { 255 - (x * 3).min(255) } else { 255 };
		image::Rgba([r, g, b, a].map(|v| u8::try_from(v.min(255)).unwrap_or(255)))
	})
}

fn tool(variable: &str, name: &str) -> Option<PathBuf> {
	if let Some(path) = env::var_os(variable) {
		let path = PathBuf::from(path);
		return path.is_file().then_some(path);
	}
	Command::new(name).arg("--version").output().ok().filter(|out| out.status.success()).map(|_| PathBuf::from(name))
}

fn run(program: &Path, args: &[&str]) {
	let out = Command::new(program).args(args).output().expect("run a libavif tool");
	assert!(out.status.success(), "{} {} failed: {}", program.display(), args.join(" "), String::from_utf8_lossy(&out.stderr));
}

/// Mean and worst absolute difference over every channel.
fn difference(ours: &[u8], theirs: &[u8]) -> (f64, u8) {
	let total: u64 = ours.iter().zip(theirs).map(|(a, b)| u64::from(a.abs_diff(*b))).sum();
	let worst = ours.iter().zip(theirs).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
	#[expect(clippy::cast_precision_loss, reason = "a mean over a few thousand samples")]
	let mean = total as f64 / ours.len() as f64;
	(mean, worst)
}

/// libavif's decode of an 8-bit limited-range monochrome AVIF, as full-range grey RGBA:
/// its raw Y plane from a y4m, expanded with the standard limited-range formula. See the
/// module docs for why `avifdec`'s own greyscale PNG cannot be the reference here.
fn limited_grey_reference(avifdec: &Path, avif: &Path, y4m: &Path) -> image::RgbaImage {
	run(avifdec, &[&avif.to_string_lossy(), &y4m.to_string_lossy()]);
	let bytes = fs::read(y4m).expect("read avifdec's y4m");
	// "YUV4MPEG2 ...\nFRAME\n" then the Y plane; a monochrome y4m has nothing after it.
	let frame = bytes.windows(6).position(|w| w == b"FRAME\n").expect("a y4m frame") + 6;
	let luma = &bytes[frame..frame + (WIDTH * HEIGHT) as usize];
	image::RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
		let raw = u32::from(luma[(y * WIDTH + x) as usize]);
		let grey = u8::try_from((raw.saturating_sub(16).min(219) * 255 + 109) / 219).unwrap_or(255);
		image::Rgba([grey, grey, grey, 255])
	})
}

#[test]
fn matches_avifdec_across_colour_encodings() {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let (Some(avifenc), Some(avifdec)) = (tool("SKIDBLADNIR_REFERENCE_AVIFENC", "avifenc"), tool("SKIDBLADNIR_REFERENCE_AVIFDEC", "avifdec")) else {
		let message = "AVIF INPUT NOT CHECKED: avifenc and avifdec (libavif) are needed. The AVIF decoder is UNVERIFIED in this run.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};

	let dir = env::temp_dir().join(format!("skidbladnir-avif-input-{}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("create the scratch directory");
	for alpha in [false, true] {
		source(alpha).save(dir.join(format!("source-{alpha}.png"))).expect("write the source PNG");
	}

	// (name, alpha, avifenc arguments)
	let cases: &[(&str, bool, &[&str])] = &[("8-bit 4:4:4 full BT.601", false, &["-d", "8", "-y", "444", "-r", "full", "--cicp", "1/13/6"]), ("8-bit 4:2:0 limited BT.709", false, &["-d", "8", "-y", "420", "-r", "limited", "--cicp", "1/13/1"]), ("8-bit 4:2:2 full BT.601", false, &["-d", "8", "-y", "422", "-r", "full", "--cicp", "1/13/6"]), ("8-bit monochrome full", false, &["-d", "8", "-y", "400", "-r", "full"]), ("8-bit monochrome limited", false, &["-d", "8", "-y", "400", "-r", "limited"]), ("8-bit identity (GBR)", false, &["-d", "8", "-y", "444", "-r", "full", "--cicp", "1/13/0"]), ("10-bit 4:4:4 full BT.709", false, &["-d", "10", "-y", "444", "-r", "full", "--cicp", "1/13/1"]), ("10-bit 4:2:0 limited BT.2020", false, &["-d", "10", "-y", "420", "-r", "limited", "--cicp", "9/16/9"]), ("10-bit 4:2:2 full BT.601", false, &["-d", "10", "-y", "422", "-r", "full", "--cicp", "1/13/6"]), ("10-bit monochrome", false, &["-d", "10", "-y", "400", "-r", "full"]), ("12-bit 4:4:4 limited BT.601", false, &["-d", "12", "-y", "444", "-r", "limited", "--cicp", "1/13/6"]), ("12-bit 4:2:0 full BT.709", false, &["-d", "12", "-y", "420", "-r", "full", "--cicp", "1/13/1"]), ("10-bit identity (GBR)", false, &["-d", "10", "-y", "444", "-r", "full", "--cicp", "1/13/0"]), ("8-bit 4:2:0 with alpha", true, &["-d", "8", "-y", "420", "-r", "full", "--cicp", "1/13/6"]), ("10-bit 4:4:4 with alpha", true, &["-d", "10", "-y", "444", "-r", "full", "--cicp", "1/13/1"]), ("8-bit 4:4:4 premultiplied alpha", true, &["-d", "8", "-y", "444", "-r", "full", "--cicp", "1/13/6", "--premultiply"]), ("lossless", true, &["-l"])];

	let mut report = Vec::new();
	let mut failures = Vec::new();
	for (index, &(name, alpha, args)) in cases.iter().enumerate() {
		let avif = dir.join(format!("{index}.avif"));
		let png = dir.join(format!("{index}.png"));
		let source_png = dir.join(format!("source-{alpha}.png"));
		let mut encode_args: Vec<&str> = vec!["-s", "10"];
		// Lossless mode refuses a quality below 100; everything else is encoded at 90.
		if !args.contains(&"-l") {
			encode_args.extend(["-q", "90", "--qalpha", "90"]);
		}
		encode_args.extend_from_slice(args);
		let (source_str, avif_str, png_str) = (source_png.to_string_lossy().into_owned(), avif.to_string_lossy().into_owned(), png.to_string_lossy().into_owned());
		encode_args.extend([source_str.as_str(), avif_str.as_str()]);
		run(&avifenc, &encode_args);
		let theirs = if name.contains("monochrome limited") {
			limited_grey_reference(&avifdec, &avif, &dir.join(format!("{index}.y4m")))
		} else {
			run(&avifdec, &["-d", "8", "-u", "nearest", &avif_str, &png_str]);
			image::open(&png).expect("read avifdec's PNG").into_rgba8()
		};
		let (width, height, ours) = match decode_avif(&fs::read(&avif).expect("read the AVIF")) {
			Ok(decoded) => decoded,
			Err(error) => {
				failures.push(format!("{name}: our decoder refused it: {error}"));
				continue;
			}
		};
		if (width, height) != theirs.dimensions() {
			failures.push(format!("{name}: we read {width}x{height}, avifdec {:?}", theirs.dimensions()));
			continue;
		}
		let (mean, worst) = difference(&ours, theirs.as_raw());
		// How far each decoder lands from the image that was encoded. Monochrome discards
		// colour by design, so it is judged against avifdec alone.
		let original = source(alpha);
		let (ours_off, theirs_off) = (difference(&ours, original.as_raw()).0, difference(theirs.as_raw(), original.as_raw()).0);
		let monochrome = args.contains(&"400");
		report.push(format!("{name}: against avifdec mean {mean:.3} worst {worst}; from the original ours {ours_off:.3}, avifdec {theirs_off:.3}"));
		// Two decoders' fixed-point conversions differ by a level or two. The larger gaps,
		// measured, are all at saturated pixels of limited-range subsampled files, where
		// libavif clamps and lands further from the original (pure blue: avifdec 237, ours
		// 250). So the bound is loose-ish against avifdec but strict against the original:
		// never further from it than libavif is. A wrong matrix, range, depth scaling or alpha
		// rule is tens of levels out on both.
		// The slack is half a level of mean error: macOS CI's Homebrew libavif lands 0.27
		// closer to the original than ours on 8-bit 4:4:4 BT.601 (0.49 against 0.76), a
		// rounding difference; the wrong-matrix, wrong-range and wrong-depth mutations each
		// moved it by 8 or more.
		if mean > 2.0 || worst > 20 || (!monochrome && ours_off > theirs_off + 0.5) {
			failures.push(format!("{name}: against avifdec mean {mean:.3} worst {worst}; from the original ours {ours_off:.3} vs avifdec {theirs_off:.3}"));
		}
	}
	let _ = fs::remove_dir_all(&dir);

	eprintln!("{}", report.join("\n"));
	assert!(failures.is_empty(), "AVIF INPUT FAILED for {} of {} files:\n{}", failures.len(), cases.len(), failures.join("\n"));
	eprintln!("AVIF INPUT OK: {} files decoded within tolerance of avifdec", cases.len());
}

/// What libavif writes, this reads back — the app's own AVIF output is valid input.
#[test]
fn reads_back_our_own_avif_output() {
	let source = source(true);
	let job = skidbladnir_encode::settings::EncodeJob { format: skidbladnir_encode::settings::OutputFormat::Avif, avif: skidbladnir_encode::settings::AvifSettings { speed: Some(10), quality: Some(95), quality_alpha: Some(95), ..Default::default() }, ..Default::default() };
	let encoded = skidbladnir_encode::encode_rgba(&job, &skidbladnir_encode::RgbaImage { width: WIDTH, height: HEIGHT, pixels: source.as_raw() }).expect("encode");
	let (width, height, ours) = decode_avif(&encoded).expect("our own AVIF must decode");
	assert_eq!((width, height), (WIDTH, HEIGHT));
	let (mean, _) = difference(&ours, source.as_raw());
	assert!(mean < 3.0, "quality 95 should come back close, mean error {mean:.3}");
}

/// Garbage is an error, not a panic.
#[test]
fn refuses_what_is_not_an_avif() {
	assert!(decode_avif(b"not an avif at all").is_err());
	assert!(decode_avif(&[]).is_err());
}

/// An AVIF's crop (`clap`), rotation (`irot`) and mirror (`imir`) are applied, in that
/// order, exactly as libavif displays them.
///
/// The fixtures are **lossless**, so the expected pixels are known without any reference
/// decoder: the source PNG put through the `image` crate's own crop, rotate and flip — an
/// implementation independent of ours. `avifdec` must then agree exactly as well where it
/// applies the properties (libavif 1.4.2 does; 1.0.4, on Ubuntu 24.04, ignores them and is
/// reported rather than trusted). `irot` n is n quarter turns *anticlockwise*; `imir` 0 flips
/// top-to-bottom and 1 left-to-right (as `avifenc --help` documents them).
/// A transform as the `image` crate performs it, for the expected side of the comparison.
type Expected = fn(&image::RgbaImage) -> image::RgbaImage;

#[test]
fn honours_crop_rotation_and_mirror() {
	use image::imageops;

	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let (Some(avifenc), avifdec) = (tool("SKIDBLADNIR_REFERENCE_AVIFENC", "avifenc"), tool("SKIDBLADNIR_REFERENCE_AVIFDEC", "avifdec")) else {
		let message = "AVIF TRANSFORMS NOT CHECKED: avifenc (libavif) is needed.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};

	let dir = env::temp_dir().join(format!("skidbladnir-avif-transforms-{}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("create the scratch directory");
	let source = source(true);
	let source_png = dir.join("source.png");
	source.save(&source_png).expect("write the source PNG");

	// (name, avifenc flags, the same transform done by the `image` crate)
	let mut cases: Vec<(&str, &[&str], Expected)> = Vec::new();
	let mut add = |name: &'static str, flags: &'static [&'static str], expected: Expected| cases.push((name, flags, expected));
	add("irot 1", &["--irot", "1"], imageops::rotate270);
	add("irot 2", &["--irot", "2"], imageops::rotate180);
	add("irot 3", &["--irot", "3"], imageops::rotate90);
	add("imir 0", &["--imir", "0"], imageops::flip_vertical);
	add("imir 1", &["--imir", "1"], imageops::flip_horizontal);
	add("crop", &["--crop", "10,0,20,48"], |img| imageops::crop_imm(img, 10, 0, 20, 48).to_image());
	add("crop, irot 1, imir 0", &["--crop", "10,0,20,48", "--irot", "1", "--imir", "0"], |img| imageops::flip_vertical(&imageops::rotate270(&imageops::crop_imm(img, 10, 0, 20, 48).to_image())));

	let mut failures = Vec::new();
	let mut ignored = 0;
	for (index, (name, flags, expected)) in cases.iter().enumerate() {
		let avif = dir.join(format!("{index}.avif"));
		let mut args = vec!["-s", "10", "-l"];
		args.extend_from_slice(flags);
		let (source_str, avif_str) = (source_png.to_string_lossy().into_owned(), avif.to_string_lossy().into_owned());
		args.extend([source_str.as_str(), avif_str.as_str()]);
		run(&avifenc, &args);

		let expected = expected(&source);
		let (width, height, ours) = decode_avif(&fs::read(&avif).expect("read")).expect("decode");
		if (width, height) != expected.dimensions() || ours != *expected.as_raw() {
			failures.push(format!("{name}: we produced {width}x{height}, expected {:?}{}", expected.dimensions(), if (width, height) == expected.dimensions() { " with different pixels" } else { "" }));
		}
		if let Some(avifdec) = &avifdec {
			let png = dir.join(format!("{index}.png"));
			run(avifdec, &["-d", "8", &avif_str, &png.to_string_lossy()]);
			let theirs = image::open(&png).expect("read avifdec's PNG").into_rgba8();
			if theirs == source {
				// Older libavif (1.0.4, Ubuntu 24.04's) shows the stored image and ignores the
				// transforms entirely, so it cannot referee them; the exact check above still
				// holds. Newer ones (1.4.2) apply them and must agree.
				ignored += 1;
			} else if theirs.dimensions() != (width, height) || theirs.as_raw() != &ours {
				failures.push(format!("{name}: avifdec shows {:?}, we show {width}x{height}{}", theirs.dimensions(), if theirs.dimensions() == (width, height) { " with different pixels" } else { "" }));
			}
		}
	}
	let _ = fs::remove_dir_all(&dir);
	assert!(failures.is_empty(), "AVIF TRANSFORMS FAILED:\n{}", failures.join("\n"));
	let referee = match (&avifdec, ignored) {
		(None, _) => String::new(),
		(Some(_), 0) => ", matching avifdec".to_owned(),
		(Some(_), n) => format!("; this avifdec ignores transforms ({n} files shown untransformed), so only the exact check applied"),
	};
	eprintln!("AVIF TRANSFORMS OK: {} files shown with their crop, rotation and mirror, exactly{referee}", cases.len());
}

/// Grid (tiled) AVIFs decode whole: every tile in its place, cropped to the grid's output
/// size, with an alpha grid kept, and the grid item's own rotation still applied.
///
/// Lossless grids must come back exactly as their source; a lossy one must match `avifdec`
/// (same tolerance idea as the colour-encoding gate: a misplaced tile is off by tens of
/// levels over a whole tile).
#[test]
fn decodes_grid_avifs() {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let (Some(avifenc), avifdec) = (tool("SKIDBLADNIR_REFERENCE_AVIFENC", "avifenc"), tool("SKIDBLADNIR_REFERENCE_AVIFDEC", "avifdec")) else {
		let message = "AVIF GRIDS NOT CHECKED: avifenc (libavif) is needed.";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};
	let dir = env::temp_dir().join(format!("skidbladnir-avif-grid-{}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("create the scratch directory");
	// MIAF requires grid tiles of at least 64x64, so the fixtures are the usual source
	// scaled up to 256x192: 2x2 tiles of 128x96, and 4x3 tiles of 64x64.
	let big = |alpha: bool| image::imageops::resize(&source(alpha), 256, 192, image::imageops::FilterType::Triangle);
	let (opaque, translucent) = (big(false), big(true));
	let (opaque_png, translucent_png) = (dir.join("opaque.png"), dir.join("translucent.png"));
	opaque.save(&opaque_png).expect("write");
	translucent.save(&translucent_png).expect("write");

	// (name, source, avifenc flags, expected pixels: None = compare with avifdec)
	let rotated = image::imageops::rotate270(&opaque);
	let cases: Vec<(&str, &Path, Vec<&str>, Option<&image::RgbaImage>)> = vec![("2x2 lossless", &opaque_png, vec!["-l", "--grid", "2x2"], Some(&opaque)), ("4x3 lossless with alpha", &translucent_png, vec!["-l", "--grid", "4x3"], Some(&translucent)), ("2x2 lossless, the grid rotated", &opaque_png, vec!["-l", "--grid", "2x2", "--irot", "1"], Some(&rotated)), ("2x2 lossy 4:2:0", &opaque_png, vec!["-q", "80", "-y", "420", "--grid", "2x2"], None)];

	let mut failures = Vec::new();
	for (index, (name, png, flags, expected)) in cases.iter().enumerate() {
		let avif = dir.join(format!("{index}.avif"));
		let mut args = vec!["-s", "10"];
		args.extend(flags.iter().copied());
		let (png_str, avif_str) = (png.to_string_lossy().into_owned(), avif.to_string_lossy().into_owned());
		args.extend([png_str.as_str(), avif_str.as_str()]);
		run(&avifenc, &args);
		let bytes = fs::read(&avif).expect("read");
		// Otherwise a libavif that quietly ignored `--grid` would pass this gate untested.
		if !skidbladnir_encode::avif_grid::is_grid(&bytes) {
			failures.push(format!("{name}: avifenc did not write a grid"));
			continue;
		}
		let (width, height, ours) = match decode_avif(&bytes) {
			Ok(decoded) => decoded,
			Err(error) => {
				failures.push(format!("{name}: refused: {error}"));
				continue;
			}
		};
		if let Some(expected) = expected {
			if (width, height) != expected.dimensions() || ours != *expected.as_raw() {
				failures.push(format!("{name}: {width}x{height} {}", if (width, height) == expected.dimensions() { "with different pixels" } else { "at the wrong size" }));
			}
		} else if let Some(avifdec) = &avifdec {
			let out = dir.join(format!("{index}.png"));
			run(avifdec, &["-d", "8", "-u", "nearest", &avif_str, &out.to_string_lossy()]);
			let theirs = image::open(&out).expect("read").into_rgba8();
			let (mean, worst) = if theirs.dimensions() == (width, height) { difference(&ours, theirs.as_raw()) } else { (f64::MAX, u8::MAX) };
			if mean > 2.0 || worst > 20 {
				failures.push(format!("{name}: against avifdec mean {mean:.3} worst {worst}"));
			}
		}
	}
	let _ = fs::remove_dir_all(&dir);
	assert!(failures.is_empty(), "AVIF GRIDS FAILED:\n{}", failures.join("\n"));
	eprintln!("AVIF GRIDS OK: {} grid AVIFs decoded whole", cases.len());
}
