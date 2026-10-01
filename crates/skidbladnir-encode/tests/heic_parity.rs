//! The HEIC parity gate: our encoder against a reference `heif-enc -e kvazaar`, byte for
//! byte.
//!
//! The reference must use the same libheif and Kvazaar as ours: Kvazaar writes its version
//! and settings into every file. `scripts/build-libheif.sh --with-heif-enc --prefix <dir>`
//! builds one; point `SKIDBLADNIR_REFERENCE_HEIF_ENC` at `<dir>/bin/heif-enc`. It is run
//! with `<dir>/lib` first on the library path, so it cannot pick up a system libheif. A
//! distribution's `heif-enc` has no Kvazaar and is skipped rather than compared.
//!
//! PNG fixtures are built chunk by chunk, since `heif-enc`'s result depends on the PNG's
//! channels and metadata as well as its pixels. JPEG fixtures cover each chroma sampling,
//! gray, progressive and CMYK, with and without metadata: `heif-enc` keeps a JPEG's YCbCr
//! planes, and so do we, through its own reader. 16-bit PNGs are not compared: `heif-enc`
//! hands Kvazaar 10 bits and Kvazaar refuses them.
//!
//! `SKIDBLADNIR_REQUIRE_PARITY=1` turns "could not run" into a failure.

mod common;

use std::{
	env, fs, path::{Path, PathBuf}, process::Command
};

use common::{H, JPEGS, W, chunk, exif, icc, jpeg_metadata, jpeg_with, png, raw_profile, rows, zlib};
use skidbladnir_encode::{
	heic, heif_enc::heif_enc_args, settings::{ChromaDownsampling, ColorProfile, EncodeJob, HeicSettings, OmafProjection, Orientation, OutputFormat}, source::encode_file
};

/// The reference and its library directory, if it is built the way ours is.
fn reference() -> Result<(PathBuf, PathBuf), String> {
	let path = env::var_os("SKIDBLADNIR_REFERENCE_HEIF_ENC").map(PathBuf::from).ok_or("SKIDBLADNIR_REFERENCE_HEIF_ENC is not set (build one with scripts/build-libheif.sh --with-heif-enc)")?;
	let lib = path.parent().and_then(Path::parent).map(|prefix| prefix.join("lib")).ok_or("the reference is not in a <prefix>/bin directory")?;
	let run = |arg: &str| Command::new(&path).arg(arg).env("LD_LIBRARY_PATH", &lib).env("DYLD_LIBRARY_PATH", &lib).output().map(|out| String::from_utf8_lossy(&out.stdout).into_owned()).map_err(|error| format!("could not run {}: {error}", path.display()));
	let version = run("--version")?;
	let ours = heic::linked_version();
	if !version.contains(&format!("libheif: {ours}")) {
		return Err(format!("the reference is not libheif {ours}: {version}"));
	}
	let encoders = run("--list-encoders")?;
	if !encoders.contains("- kvazaar =") {
		return Err(format!("the reference has no Kvazaar: {encoders}"));
	}
	Ok((path, lib))
}

struct Run {
	heif_enc: PathBuf,
	lib: PathBuf,
	dir: PathBuf,
	mismatches: Vec<String>,
	total: usize,
}

impl Run {
	fn compare(&mut self, name: &str, input: &Path, settings: &HeicSettings) {
		self.compare_job(name, input, &EncodeJob { format: OutputFormat::Heic, heic: settings.clone(), ..Default::default() });
	}

	fn compare_job(&mut self, name: &str, input: &Path, job: &EncodeJob) {
		let job = job.clone();
		let theirs = self.dir.join(format!("heif-enc-{}.heic", self.total));
		let ours = self.dir.join(format!("ours-{}.heic", self.total));
		self.total += 1;
		let args = heif_enc_args(&job, input, &theirs);
		let run = Command::new(&self.heif_enc).args(&args).env("LD_LIBRARY_PATH", &self.lib).env("DYLD_LIBRARY_PATH", &self.lib).output().expect("run heif-enc");
		let reference = run.status.success().then(|| fs::read(&theirs).ok()).flatten();
		let result = encode_file(&job, input, &ours).map(|_| fs::read(&ours).expect("read ours"));
		let command = || args.iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" ");
		match (reference, result) {
			(Some(expected), Ok(actual)) if expected == actual => {}
			(Some(expected), Ok(actual)) => {
				let first = actual.iter().zip(&expected).position(|(a, b)| a != b).map_or_else(|| "length only".to_owned(), |at| format!("byte {at}"));
				self.mismatches.push(format!("`{name}`: ours {} bytes, heif-enc {} bytes, first difference at {first}\n    heif-enc {}", actual.len(), expected.len(), command()));
			}
			(Some(_), Err(error)) => self.mismatches.push(format!("`{name}`: heif-enc succeeded but ours failed: {error}")),
			(None, _) => self.mismatches.push(format!("`{name}`: heif-enc refused it: {}\n    heif-enc {}", String::from_utf8_lossy(&run.stderr).trim(), command())),
		}
	}
}

fn prepare() -> Option<Run> {
	// The claim is the standard edition's: Kvazaar, as `heif-enc -e kvazaar`. The GPL
	// edition encodes with x265 instead, which this reference is not.
	if skidbladnir_encode::settings::HEIC_X265 {
		eprintln!("HEIC PARITY NOT RUN: this is the GPL edition, whose HEIC encoder is x265; the heif-enc parity gate is the standard edition's (Kvazaar).");
		return None;
	}
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let (heif_enc, lib) = match reference() {
		Ok(found) => found,
		Err(why) => {
			let message = format!("HEIC PARITY NOT RUN: {why}");
			assert!(!require, "{message}");
			eprintln!("{message}");
			return None;
		}
	};
	let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
	let dir = env::temp_dir().join(format!("skidbladnir-heic-parity-{}-{nanos}", std::process::id()));
	fs::create_dir_all(&dir).expect("create the scratch directory");
	Some(Run { heif_enc, lib, dir, mismatches: Vec::new(), total: 0 })
}

fn finish(run: &Run, what: &str) {
	let _ = fs::remove_dir_all(&run.dir);
	assert!(run.mismatches.is_empty(), "{} of {} {what} diverged from heif-enc:\n  {}", run.mismatches.len(), run.total, run.mismatches.join("\n  "));
	eprintln!("HEIC PARITY OK: {} {what} matched heif-enc byte for byte.", run.total);
}

fn d() -> HeicSettings {
	HeicSettings::default()
}

/// An XMP packet in an `iTXt` chunk.
fn itxt(key: &str, value: &[u8]) -> Vec<u8> {
	chunk(*b"iTXt", &[key.as_bytes(), &[0, 0, 0, 0, 0], value].concat())
}

#[test]
fn matches_heif_enc_across_png_inputs() {
	let Some(mut run) = prepare() else { return };
	let rgb8 = rows(3, 8);
	let text = |kind: [u8; 4], key: &str, value: &[u8]| chunk(kind, &[key.as_bytes(), &[0], value].concat());
	let ztxt = |key: &str, value: &[u8]| chunk(*b"zTXt", &[key.as_bytes(), &[0, 0], &zlib(value)].concat());
	let palette: Vec<u8> = (0..=255_u8).flat_map(|i| [i, 255 - i, i.wrapping_mul(7)]).collect();
	let palette_rows: Vec<u8> = (0..H).flat_map(|y| (0..W).map(move |x| u8::try_from((x * 5 + y * 3) % 256).expect("byte"))).collect();
	let trns: Vec<u8> = (0..=255_u8).map(|i| if i % 5 == 0 { 0 } else { 255 - i / 2 }).collect();
	let mut exif_pointer = exif();
	// A second Exif block with the orientation in a SHORT of IFD0, little-endian.
	let exif_le: Vec<u8> = [b"II*\0".as_slice(), &[8, 0, 0, 0], &[1, 0], &[0x12, 1, 3, 0, 1, 0, 0, 0, 8, 0, 0, 0], &[0, 0, 0, 0]].concat();
	exif_pointer.extend_from_slice(b"tail");
	let (odd_w, odd_h) = (61_u32, 45_u32);
	let odd_rows: Vec<u8> = (0..odd_h).flat_map(|y| (0..odd_w).flat_map(move |x| [u8::try_from((x * 3 + y) % 256).expect("byte"), u8::try_from(x * 4 % 256).expect("byte"), u8::try_from(y * 5 % 256).expect("byte"), u8::try_from((x + y) * 2 % 256).expect("byte")])).collect();
	let fixtures: Vec<(&str, Vec<u8>)> = vec![("rgb8", png(W, H, 8, 2, &rgb8, &[], &[])), ("rgba8", png(W, H, 8, 6, &rows(4, 8), &[], &[])), ("gray8", png(W, H, 8, 0, &rows(1, 8), &[], &[])), ("graya8", png(W, H, 8, 4, &rows(2, 8), &[], &[])), ("gray4", png(W, H, 4, 0, &rows(1, 4), &[], &[])), ("gray1", png(W, H, 1, 0, &rows(1, 1), &[], &[])), ("palette", png(W, H, 8, 3, &palette_rows, &[chunk(*b"PLTE", &palette)], &[])), ("palette+tRNS", png(W, H, 8, 3, &palette_rows, &[chunk(*b"PLTE", &palette), chunk(*b"tRNS", &trns)], &[])), ("palette+opaque tRNS", png(W, H, 8, 3, &palette_rows, &[chunk(*b"PLTE", &palette), chunk(*b"tRNS", &[255; 4])], &[])), ("rgb+tRNS (ignored)", png(W, H, 8, 2, &rgb8, &[chunk(*b"tRNS", &[0, 0, 0, 0, 0, 0])], &[])), ("gray+tRNS (ignored)", png(W, H, 8, 0, &rows(1, 8), &[chunk(*b"tRNS", &[0, 0])], &[])), ("odd size rgba", png(odd_w, odd_h, 8, 6, &odd_rows, &[], &[])), ("iCCP", png(W, H, 8, 2, &rgb8, &[chunk(*b"iCCP", &[b"test\0\0".as_slice(), &zlib(&icc(false))].concat())], &[])), ("gray iCCP", png(W, H, 8, 0, &rows(1, 8), &[chunk(*b"iCCP", &[b"test\0\0".as_slice(), &zlib(&icc(true))].concat())], &[])), ("gray image, RGB iCCP (libpng drops it)", png(W, H, 8, 0, &rows(1, 8), &[chunk(*b"iCCP", &[b"test\0\0".as_slice(), &zlib(&icc(false))].concat())], &[])), ("eXIf, orientation reset", png(W, H, 8, 2, &rgb8, &[chunk(*b"eXIf", &exif())], &[])), ("little-endian eXIf", png(W, H, 8, 2, &rgb8, &[chunk(*b"eXIf", &exif_le)], &[])), ("eXIf after the image", png(W, H, 8, 2, &rgb8, &[], &[chunk(*b"eXIf", &exif_pointer)])), ("invalid eXIf, then a valid one", png(W, H, 8, 2, &rgb8, &[chunk(*b"eXIf", b"XXXXjunk")], &[chunk(*b"eXIf", &exif())])), ("XMP in iTXt", png(W, H, 8, 2, &rgb8, &[itxt("XML:com.adobe.xmp", b"<x:xmpmeta xmlns:x='adobe:ns:meta/'/>")], &[])), ("XMP in tEXt and zTXt, the last wins", png(W, H, 8, 2, &rgb8, &[text(*b"tEXt", "XML:com.adobe.xmp", b"<first/>")], &[ztxt("XML:com.adobe.xmp", b"<last/>")])), ("raw profile Exif and XMP (heif-enc reads neither)", png(W, H, 8, 2, &rgb8, &[text(*b"tEXt", "Raw profile type exif", &raw_profile("exif", &[b"Exif\0\0".as_slice(), &exif()].concat())), text(*b"tEXt", "Raw profile type xmp", &raw_profile("xmp", b"<x:xmpmeta/>"))], &[])), ("every kind of metadata", png(W, H, 8, 6, &rows(4, 8), &[chunk(*b"iCCP", &[b"test\0\0".as_slice(), &zlib(&icc(false))].concat()), chunk(*b"eXIf", &exif()), itxt("XML:com.adobe.xmp", b"<x:xmpmeta/>")], &[]))];
	for (name, bytes) in fixtures {
		let input = run.dir.join(format!("{}.png", name.replace([' ', '+', '(', ')', ','], "_")));
		fs::write(&input, &bytes).expect("write the fixture");
		run.compare(&format!("{name}, defaults"), &input, &d());
		run.compare(&format!("{name}, -q 90 --color-profile auto"), &input, &HeicSettings { quality: 90, color_profile: ColorProfile::Auto, ..d() });
	}
	finish(&run, "PNG inputs");
}

#[test]
fn matches_heif_enc_across_the_option_surface() {
	let Some(mut run) = prepare() else { return };
	let input = run.dir.join("rgba.png");
	fs::write(&input, png(W, H, 8, 6, &rows(4, 8), &[chunk(*b"eXIf", &exif()), itxt("XML:com.adobe.xmp", b"<x:xmpmeta/>")], &[])).expect("write the fixture");
	let input_icc = run.dir.join("rgb-icc.png");
	fs::write(&input_icc, png(W, H, 8, 2, &rows(3, 8), &[chunk(*b"iCCP", &[b"test\0\0".as_slice(), &zlib(&icc(false))].concat())], &[])).expect("write the fixture");
	let gray = run.dir.join("gray.png");
	fs::write(&gray, png(W, H, 8, 4, &rows(2, 8), &[], &[])).expect("write the fixture");

	let mut cases: Vec<(String, PathBuf, HeicSettings)> = Vec::new();
	let mut add = |name: &str, input: &Path, settings: HeicSettings| cases.push((name.to_owned(), input.to_path_buf(), settings));
	for quality in [0_u8, 1, 30, 75, 100] {
		add(&format!("-q {quality}"), &input, HeicSettings { quality, ..d() });
	}
	add("-p lossless=true", &input, HeicSettings { lossless: true, ..d() });
	add("-q 20 -p lossless=true", &input, HeicSettings { quality: 20, lossless: true, ..d() });
	add("--no-alpha", &input, HeicSettings { alpha: false, ..d() });
	add("--premultiplied-alpha", &input, HeicSettings { premultiplied_alpha: true, ..d() });
	add("-t 32", &input, HeicSettings { thumbnail: Some(32), ..d() });
	add("-t 50 --no-thumb-alpha", &input, HeicSettings { thumbnail: Some(50), thumbnail_alpha: false, ..d() });
	add("-t 32 --no-alpha", &input, HeicSettings { thumbnail: Some(32), alpha: false, ..d() });
	add("-t 32 --premultiplied-alpha", &input, HeicSettings { thumbnail: Some(32), premultiplied_alpha: true, ..d() });
	add("-t 32 with an ICC profile", &input_icc, HeicSettings { thumbnail: Some(32), ..d() });
	for algorithm in [ChromaDownsampling::NearestNeighbor, ChromaDownsampling::Average, ChromaDownsampling::SharpYuv] {
		add(&format!("-C {}", algorithm.as_heif_enc_str()), &input, HeicSettings { chroma_downsampling: Some(algorithm), ..d() });
	}
	for preset in [ColorProfile::Auto, ColorProfile::Bt601, ColorProfile::Bt709, ColorProfile::Compatible, ColorProfile::Bt2020] {
		add(&format!("--color-profile {}", preset.as_heif_enc_str()), &input, HeicSettings { color_profile: preset, ..d() });
		add(&format!("gray, --color-profile {}", preset.as_heif_enc_str()), &gray, HeicSettings { color_profile: preset, ..d() });
	}
	for (matrix_coefficients, colour_primaries, transfer_characteristics, full_range) in [(6_u16, 1_u16, 13_u16, false), (1, 1, 1, true), (9, 9, 16, true), (0, 1, 13, true), (2, 2, 2, true), (5, 12, 18, false), (12, 22, 8, true)] {
		add(&format!("custom {matrix_coefficients}/{colour_primaries}/{transfer_characteristics}/{full_range}"), &input, HeicSettings { color_profile: ColorProfile::Custom { matrix_coefficients, colour_primaries, transfer_characteristics, full_range }, ..d() });
	}
	add("an ICC profile", &input_icc, d());
	add("--enable-two-colr-boxes", &input_icc, HeicSettings { two_colr_boxes: true, ..d() });
	add("--clli 1000,400", &input, HeicSettings { clli: Some([1000, 400]), ..d() });
	add("--pasp 4,3", &input, HeicSettings { pasp: Some([4, 3]), ..d() });
	for orientation in [Orientation::FlipHorizontally, Orientation::Rotate180, Orientation::FlipVertically, Orientation::Rotate90CwThenFlipHorizontally, Orientation::Rotate90Cw, Orientation::Rotate90CwThenFlipVertically, Orientation::Rotate270Cw] {
		add(&orientation.as_heif_enc_args().join(" "), &input, HeicSettings { orientation, ..d() });
	}
	add("--cut-tiles 32", &input, HeicSettings { cut_tiles: Some(32), ..d() });
	add("--cut-tiles 20 -t 16 --rotate-cw 90", &input, HeicSettings { cut_tiles: Some(20), thumbnail: Some(16), orientation: Orientation::Rotate90Cw, ..d() });
	add("--cut-tiles 64 --premultiplied-alpha", &input, HeicSettings { cut_tiles: Some(64), premultiplied_alpha: true, ..d() });
	add("--cut-tiles 16 with an ICC profile", &input_icc, HeicSettings { cut_tiles: Some(16), ..d() });
	for projection in [OmafProjection::Equirectangular, OmafProjection::CubeMap] {
		add(&format!("--omaf-image-projection {}", projection.as_heif_enc_str()), &input, HeicSettings { omaf_projection: Some(projection), ..d() });
	}
	add("--pitm-description", &input, HeicSettings { description: "A test image, ünïcode".into(), ..d() });
	add("--add-compatible-brand", &input, HeicSettings { compatible_brands: vec!["abcd".into()], ..d() });
	add("--add-compatible-brand twice, one a duplicate", &input, HeicSettings { compatible_brands: vec!["mif1".into(), "wxyz".into()], ..d() });
	add("--unif", &input, HeicSettings { unif: true, ..d() });
	add("--unif --pitm-description -t 32", &input, HeicSettings { unif: true, description: "unified".into(), thumbnail: Some(32), ..d() });
	add("--mini", &input, HeicSettings { mini: true, ..d() });
	add("--mini, no alpha", &input_icc, HeicSettings { mini: true, ..d() });
	add("--mini -t 32 (no mini then)", &input, HeicSettings { mini: true, thumbnail: Some(32), ..d() });
	add("everything at once", &input, HeicSettings { quality: 70, premultiplied_alpha: true, thumbnail: Some(24), thumbnail_alpha: false, chroma_downsampling: Some(ChromaDownsampling::Average), color_profile: ColorProfile::Bt709, clli: Some([500, 200]), pasp: Some([1, 2]), orientation: Orientation::FlipVertically, description: "all".into(), compatible_brands: vec!["test".into()], unif: true, ..d() });
	for (name, input, settings) in cases {
		run.compare(&name, &input, &settings);
	}
	finish(&run, "option settings");
}

#[test]
fn matches_heif_enc_across_jpeg_inputs() {
	let Some(mut run) = prepare() else { return };
	let add = |run: &mut Run, name: &str, bytes: &[u8], cases: &[(&str, HeicSettings)]| {
		let input = run.dir.join(format!("{}.jpg", name.replace([' ', ':', ',', '(', ')'], "_")));
		fs::write(&input, bytes).expect("write the fixture");
		for (label, settings) in cases {
			run.compare(&format!("{name}, {label}"), &input, settings);
		}
	};
	let common_cases = [("defaults", d()), ("--color-profile auto (converted)", HeicSettings { color_profile: ColorProfile::Auto, ..d() }), ("-C sharp-yuv --color-profile 709", HeicSettings { chroma_downsampling: Some(ChromaDownsampling::SharpYuv), color_profile: ColorProfile::Bt709, ..d() }), ("-q 85 -t 24", HeicSettings { quality: 85, thumbnail: Some(24), ..d() })];
	for (name, bytes) in JPEGS {
		// heif-enc refuses an RGB-coded JPEG; see below.
		if name != "RGB-coded" {
			add(&mut run, name, bytes, &common_cases);
		}
	}
	let tagged = jpeg_with(JPEGS[0].1, &jpeg_metadata(false));
	add(&mut run, "4:2:0 with Exif (orientation 6), XMP and ICC", &tagged, &[("defaults", d()), ("--rotate-cw 90 after the Exif's", HeicSettings { orientation: Orientation::Rotate90Cw, ..d() }), ("--enable-two-colr-boxes", HeicSettings { two_colr_boxes: true, ..d() }), ("--cut-tiles 32 -t 16", HeicSettings { cut_tiles: Some(32), thumbnail: Some(16), ..d() }), ("--mini", HeicSettings { mini: true, ..d() })]);
	let gray_tagged = jpeg_with(JPEGS[5].1, &jpeg_metadata(true));
	add(&mut run, "gray with metadata", &gray_tagged, &[("defaults", d()), ("--color-profile 2020", HeicSettings { color_profile: ColorProfile::Bt2020, ..d() })]);
	add(&mut run, "Adobe CMYK", include_bytes!("fixtures/cmyk-adobe.jpg"), &[("defaults", d()), ("--color-profile auto", HeicSettings { color_profile: ColorProfile::Auto, ..d() })]);

	// An RGB-coded JPEG: heif-enc's reader refuses it, so heif-enc writes nothing; we encode
	// its pixels rather than refuse too.
	let rgb = run.dir.join("rgb-coded.jpg");
	fs::write(&rgb, JPEGS[7].1).expect("write the fixture");
	let job = EncodeJob { format: OutputFormat::Heic, ..Default::default() };
	let theirs = run.dir.join("rgb-heif-enc.heic");
	let refused = !Command::new(&run.heif_enc).args(heif_enc_args(&job, &rgb, &theirs)).env("LD_LIBRARY_PATH", &run.lib).env("DYLD_LIBRARY_PATH", &run.lib).output().expect("run heif-enc").status.success();
	assert!(refused, "heif-enc now reads RGB-coded JPEGs: compare them above instead");
	encode_file(&job, &rgb, &run.dir.join("rgb-ours.heic")).expect("an RGB-coded JPEG still encodes");
	finish(&run, "JPEG inputs");
}

/// TIFF input, which `heif-enc` reads with libtiff (`heifio/decoder_tiff.cc`). 8-bit only:
/// Kvazaar refuses the deeper input `heif-enc` makes of a 16-bit TIFF.
#[test]
fn matches_heif_enc_across_tiff_inputs() {
	let Some(mut run) = prepare() else { return };
	let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
	for name in ["tiff-rgb8.tif", "tiff-rgb8-lzw.tif", "tiff-rgba8.tif"] {
		for (setting, settings) in [("default", d()), ("quality 30", HeicSettings { quality: 30, ..d() })] {
			run.compare(&format!("{name}, {setting}"), &fixtures.join(name), &settings);
		}
	}
	finish(&run, "TIFF inputs");
}

/// WebP input, which `heif-enc` reads with libwebp (`heifio/decoder_webp.cc`): lossless and
/// lossy, with and without alpha, written here by libwebp itself.
#[test]
fn matches_heif_enc_across_webp_inputs() {
	use skidbladnir_encode::{RgbaImage, encode_rgba, settings::WebpSettings};

	let Some(mut run) = prepare() else { return };
	let (width, height) = (W, H);
	let rgba = rows(4, 8);
	let opaque: Vec<u8> = rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
	for (name, pixels, settings) in [("lossless, alpha", &rgba, WebpSettings { lossless: true, exact: true, ..WebpSettings::default() }), ("lossless, opaque", &opaque, WebpSettings { lossless: true, ..WebpSettings::default() }), ("lossy, alpha", &rgba, WebpSettings::default()), ("lossy, opaque", &opaque, WebpSettings::default())] {
		let webp = encode_rgba(&settings.into(), &RgbaImage { width, height, pixels }).expect("libwebp encodes the fixture");
		let input = run.dir.join(format!("{}.webp", name.replace([',', ' '], "_")));
		fs::write(&input, webp).expect("write the fixture");
		for (setting, heic) in [("default", d()), ("quality 30", HeicSettings { quality: 30, ..d() })] {
			run.compare(&format!("{name}, {setting}"), &input, &heic);
		}
	}
	// Odd dimensions, so the 4:2:0 planes round up.
	let (odd_w, odd_h) = (61_u32, 45_u32);
	let odd: Vec<u8> = (0..odd_h).flat_map(|y| (0..odd_w).flat_map(move |x| [u8::try_from(x * 4).expect("byte"), u8::try_from(y * 5).expect("byte"), 120, u8::try_from(255 - x).expect("byte")])).collect();
	let webp = encode_rgba(&WebpSettings::default().into(), &RgbaImage { width: odd_w, height: odd_h, pixels: &odd }).expect("libwebp encodes the fixture");
	let input = run.dir.join("odd.webp");
	fs::write(&input, webp).expect("write the fixture");
	run.compare("lossy, alpha, 61x45", &input, &d());
	finish(&run, "WebP inputs");
}

/// HEIC input, which `heif-enc` decodes with libheif in the file's own colourspace
/// (`heifio/decoder_heif.cc`). The sources are written by `heif-enc` itself, from PNGs with
/// and without alpha, gray, an odd size, and every kind of metadata.
#[test]
fn matches_heif_enc_across_heic_inputs() {
	use skidbladnir_encode::settings::HeicMetadata;

	let Some(mut run) = prepare() else { return };
	let (odd_w, odd_h) = (61_u32, 45_u32);
	let odd_rows: Vec<u8> = (0..odd_h).flat_map(|y| (0..odd_w).flat_map(move |x| [u8::try_from((x * 3 + y) % 256).expect("byte"), u8::try_from(x * 4 % 256).expect("byte"), u8::try_from(y * 5 % 256).expect("byte"), u8::try_from((x + y) * 2 % 256).expect("byte")])).collect();
	let pngs: Vec<(&str, Vec<u8>)> = vec![("rgb", png(W, H, 8, 2, &rows(3, 8), &[], &[])), ("rgba", png(W, H, 8, 6, &rows(4, 8), &[], &[])), ("gray", png(W, H, 8, 0, &rows(1, 8), &[], &[])), ("odd size rgba", png(odd_w, odd_h, 8, 6, &odd_rows, &[], &[])), ("every kind of metadata", png(W, H, 8, 6, &rows(4, 8), &[chunk(*b"iCCP", &[b"test\0\0".as_slice(), &zlib(&icc(false))].concat()), chunk(*b"eXIf", &exif()), itxt("XML:com.adobe.xmp", b"<x:xmpmeta/>")], &[]))];
	for (name, bytes) in pngs {
		let png_path = run.dir.join("source.png");
		fs::write(&png_path, &bytes).expect("write the PNG");
		let heic = run.dir.join(format!("{}.heic", name.replace(' ', "_")));
		let made = Command::new(&run.heif_enc).args(["-e", "kvazaar", "-q", "70"]).arg(&png_path).arg("-o").arg(&heic).env("LD_LIBRARY_PATH", &run.lib).env("DYLD_LIBRARY_PATH", &run.lib).output().expect("run heif-enc");
		assert!(made.status.success(), "heif-enc could not make the {name} source: {}", String::from_utf8_lossy(&made.stderr));
		run.compare(&format!("{name}, defaults"), &heic, &d());
		run.compare(&format!("{name}, -q 30 --color-profile auto"), &heic, &HeicSettings { quality: 30, color_profile: ColorProfile::Auto, ..d() });
		// heif-enc keeps every block it reads; leaving them out is the window's choice, not a
		// heif-enc option, so it is checked directly: the blocks are there, then gone.
		if name == "every kind of metadata" {
			let has = |settings: &HeicSettings, needle: &[u8]| {
				let out = run.dir.join("metadata.heic");
				skidbladnir_encode::source::encode_file(&EncodeJob { format: OutputFormat::Heic, heic: settings.clone(), ..Default::default() }, &heic, &out).expect("our conversion");
				fs::read(&out).expect("read ours").windows(needle.len()).any(|window| window == needle)
			};
			let none = HeicSettings { metadata: HeicMetadata { icc: true, exif: false, xmp: false }, ..d() };
			assert!(has(&d(), b"<x:xmpmeta/>") && has(&d(), &exif()[..8]), "a HEIC source's XMP and Exif are kept by default");
			assert!(!has(&none, b"<x:xmpmeta/>") && !has(&none, &exif()[..8]), "a HEIC source's XMP and Exif are left out when asked");
		}
	}
	finish(&run, "HEIC inputs");
}

/// An 8-bit RGBA TIFF, little-endian, one uncompressed strip, with `ExtraSamples` `extra`.
fn rgba_tiff(width: u32, height: u32, raster: &[u8], extra: u16) -> Vec<u8> {
	let raster_len = u32::try_from(raster.len()).expect("small");
	let bits_at = 8 + raster_len;
	let mut out = b"II*\0".to_vec();
	out.extend_from_slice(&(bits_at + 8).to_le_bytes());
	out.extend_from_slice(raster);
	out.extend([8_u16; 4].iter().flat_map(|b| b.to_le_bytes()));
	let short = |tag: u16, value: u16| [tag.to_le_bytes().as_slice(), &3_u16.to_le_bytes(), &1_u32.to_le_bytes(), &value.to_le_bytes(), &[0, 0]].concat();
	let long = |tag: u16, kind: u16, count: u32, value: u32| [tag.to_le_bytes().as_slice(), &kind.to_le_bytes(), &count.to_le_bytes(), &value.to_le_bytes()].concat();
	let entries = [long(256, 4, 1, width), long(257, 4, 1, height), long(258, 3, 4, bits_at), short(259, 1), short(262, 2), long(273, 4, 1, 8), short(277, 4), long(278, 4, 1, height), long(279, 4, 1, raster_len), short(284, 1), short(338, extra)];
	out.extend_from_slice(&u16::try_from(entries.len()).expect("few").to_le_bytes());
	for entry in entries {
		out.extend(entry);
	}
	out.extend_from_slice(&0_u32.to_le_bytes());
	out
}

/// A TIFF with premultiplied alpha: by default Skidbladnir un-multiplies it (the colours
/// the file means), where `heif-enc` takes the stored samples as straight colour; with
/// `tiff_alpha_like_reference` on, it hands them over as stored and matches `heif-enc`
/// byte for byte.
#[test]
fn matches_heif_enc_through_a_premultiplied_tiff_when_asked() {
	let Some(mut run) = prepare() else { return };
	let straight = rows(4, 8);
	let premultiply = |c: u8, a: u8| u8::try_from((u32::from(c) * u32::from(a) + 127) / 255).expect("byte");
	let stored: Vec<u8> = straight.chunks(4).flat_map(|p| [premultiply(p[0], p[3]), premultiply(p[1], p[3]), premultiply(p[2], p[3]), p[3]]).collect();
	assert!(stored.chunks(4).any(|p| p[3] > 0 && p[3] < 255 && p[0] > 0), "the fixture must have semi-transparent colour");
	let input = run.dir.join("premultiplied.tif");
	fs::write(&input, rgba_tiff(W, H, &stored, 1)).expect("write the fixture");
	for (name, heic) in [("default", d()), ("quality 30", HeicSettings { quality: 30, ..d() })] {
		run.compare_job(&format!("premultiplied TIFF, {name}, like heif-enc"), &input, &EncodeJob { format: OutputFormat::Heic, heic, tiff_alpha_like_reference: true, ..Default::default() });
	}
	// The default differs from heif-enc: it keeps the true colours.
	let job = EncodeJob { format: OutputFormat::Heic, ..Default::default() };
	let (theirs, ours) = (run.dir.join("premultiplied-heif-enc.heic"), run.dir.join("premultiplied-ours.heic"));
	let made = Command::new(&run.heif_enc).args(heif_enc_args(&job, &input, &theirs)).env("LD_LIBRARY_PATH", &run.lib).env("DYLD_LIBRARY_PATH", &run.lib).output().expect("run heif-enc");
	assert!(made.status.success(), "heif-enc reads the fixture");
	encode_file(&job, &input, &ours).expect("our conversion");
	assert_ne!(fs::read(&ours).expect("read ours"), fs::read(&theirs).expect("read heif-enc's"), "by default the true colours are kept, unlike heif-enc");
	finish(&run, "premultiplied TIFF inputs");
}
