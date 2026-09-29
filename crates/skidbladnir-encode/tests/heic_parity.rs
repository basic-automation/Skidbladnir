//! The HEIC parity gate: our encoder against a reference `heif-enc -e kvazaar`, byte for
//! byte.
//!
//! The reference must use the same libheif and Kvazaar as ours: Kvazaar writes its version
//! and settings into every file. `scripts/build-libheif.sh --with-heif-enc --prefix <dir>`
//! builds one; point `SKIDBLADNIR_REFERENCE_HEIF_ENC` at `<dir>/bin/heif-enc`. It is run
//! with `<dir>/lib` first on the library path, so it cannot pick up a system libheif. A
//! distribution's `heif-enc` has no Kvazaar and is skipped rather than compared.
//!
//! Every fixture is a PNG built chunk by chunk, since `heif-enc`'s result depends on the
//! PNG's channels and metadata as well as its pixels. JPEG input is not compared:
//! `heif-enc` copies a JPEG's YCbCr planes, which Skidbladnir does not yet (ROADMAP.md
//! Phase 8). Neither are 16-bit PNGs, which `heif-enc` hands Kvazaar at 10 bits and
//! Kvazaar refuses.
//!
//! `SKIDBLADNIR_REQUIRE_PARITY=1` turns "could not run" into a failure.

mod common;

use std::{
	env, fs, path::{Path, PathBuf}, process::Command
};

use common::{H, W, chunk, exif, icc, png, raw_profile, rows, zlib};
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
		let job = EncodeJob { format: OutputFormat::Heic, heic: settings.clone(), ..Default::default() };
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
	let fixtures: Vec<(&str, Vec<u8>)> = vec![
		("rgb8", png(W, H, 8, 2, &rgb8, &[], &[])),
		("rgba8", png(W, H, 8, 6, &rows(4, 8), &[], &[])),
		("gray8", png(W, H, 8, 0, &rows(1, 8), &[], &[])),
		("graya8", png(W, H, 8, 4, &rows(2, 8), &[], &[])),
		("gray4", png(W, H, 4, 0, &rows(1, 4), &[], &[])),
		("gray1", png(W, H, 1, 0, &rows(1, 1), &[], &[])),
		("palette", png(W, H, 8, 3, &palette_rows, &[chunk(*b"PLTE", &palette)], &[])),
		("palette+tRNS", png(W, H, 8, 3, &palette_rows, &[chunk(*b"PLTE", &palette), chunk(*b"tRNS", &trns)], &[])),
		("palette+opaque tRNS", png(W, H, 8, 3, &palette_rows, &[chunk(*b"PLTE", &palette), chunk(*b"tRNS", &[255; 4])], &[])),
		("rgb+tRNS (ignored)", png(W, H, 8, 2, &rgb8, &[chunk(*b"tRNS", &[0, 0, 0, 0, 0, 0])], &[])),
		("gray+tRNS (ignored)", png(W, H, 8, 0, &rows(1, 8), &[chunk(*b"tRNS", &[0, 0])], &[])),
		("odd size rgba", png(odd_w, odd_h, 8, 6, &odd_rows, &[], &[])),
		("iCCP", png(W, H, 8, 2, &rgb8, &[chunk(*b"iCCP", &[b"test\0\0".as_slice(), &zlib(&icc(false))].concat())], &[])),
		("gray iCCP", png(W, H, 8, 0, &rows(1, 8), &[chunk(*b"iCCP", &[b"test\0\0".as_slice(), &zlib(&icc(true))].concat())], &[])),
		("gray image, RGB iCCP (libpng drops it)", png(W, H, 8, 0, &rows(1, 8), &[chunk(*b"iCCP", &[b"test\0\0".as_slice(), &zlib(&icc(false))].concat())], &[])),
		("eXIf, orientation reset", png(W, H, 8, 2, &rgb8, &[chunk(*b"eXIf", &exif())], &[])),
		("little-endian eXIf", png(W, H, 8, 2, &rgb8, &[chunk(*b"eXIf", &exif_le)], &[])),
		("eXIf after the image", png(W, H, 8, 2, &rgb8, &[], &[chunk(*b"eXIf", &exif_pointer)])),
		("invalid eXIf, then a valid one", png(W, H, 8, 2, &rgb8, &[chunk(*b"eXIf", b"XXXXjunk")], &[chunk(*b"eXIf", &exif())])),
		("XMP in iTXt", png(W, H, 8, 2, &rgb8, &[itxt("XML:com.adobe.xmp", b"<x:xmpmeta xmlns:x='adobe:ns:meta/'/>")], &[])),
		("XMP in tEXt and zTXt, the last wins", png(W, H, 8, 2, &rgb8, &[text(*b"tEXt", "XML:com.adobe.xmp", b"<first/>")], &[ztxt("XML:com.adobe.xmp", b"<last/>")])),
		("raw profile Exif and XMP (heif-enc reads neither)", png(W, H, 8, 2, &rgb8, &[text(*b"tEXt", "Raw profile type exif", &raw_profile("exif", &[b"Exif\0\0".as_slice(), &exif()].concat())), text(*b"tEXt", "Raw profile type xmp", &raw_profile("xmp", b"<x:xmpmeta/>"))], &[])),
		("every kind of metadata", png(W, H, 8, 6, &rows(4, 8), &[chunk(*b"iCCP", &[b"test\0\0".as_slice(), &zlib(&icc(false))].concat()), chunk(*b"eXIf", &exif()), itxt("XML:com.adobe.xmp", b"<x:xmpmeta/>")], &[])),
	];
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
