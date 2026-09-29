//! The AVIF parity gate: our encoder against a reference `avifenc`, byte for byte.
//!
//! The reference must be built like ours — the same pinned libavif and libaom, no libyuv,
//! libwebp's sharpyuv — which `scripts/build-reference-avifenc.sh` does; point
//! `SKIDBLADNIR_REFERENCE_AVIFENC` at it. A distribution's `avifenc` links libyuv and is
//! skipped rather than compared. As with JPEG XL, every fixture is a PNG built chunk by
//! chunk, because `avifenc`'s result depends on the PNG's depth, channels, colour chunks
//! and metadata as well as its pixels. JPEG input is not compared: `avifenc` copies a
//! JPEG's YCbCr planes, which Skidbladnir does not yet (ROADMAP.md Phase 8).
//!
//! `SKIDBLADNIR_REQUIRE_PARITY=1` turns "could not run" into a failure.

mod common;

use std::{
	env, fs, path::{Path, PathBuf}, process::Command
};

use common::{H, W, chunk, exif, icc, png, raw_profile, rows, zlib};
use skidbladnir_encode::{
	avif, avifenc::avifenc_args, settings::{AvifSettings, Cicp, CleanAperture, CodecOption, EncodeJob, Fraction, Grid, MetadataSource, OutputFormat, QuantizerRange, Tiling, YuvFormat}, source::encode_file
};

/// The reference, if there is one built the way ours is.
fn reference() -> Result<PathBuf, String> {
	let path = env::var_os("SKIDBLADNIR_REFERENCE_AVIFENC").map(PathBuf::from).ok_or("SKIDBLADNIR_REFERENCE_AVIFENC is not set (build one with scripts/build-reference-avifenc.sh)")?;
	let out = Command::new(&path).arg("--version").output().map_err(|error| format!("could not run {}: {error}", path.display()))?;
	let text = String::from_utf8_lossy(&out.stdout).into_owned();
	let ours = avif::linked_version();
	if !text.contains(&format!("Version: {ours} ")) {
		return Err(format!("the reference is not libavif {ours}: {text}"));
	}
	if !text.contains("libyuv : unavailable") {
		return Err(format!("the reference links libyuv, which ours does not, so their RGB-to-YUV conversions differ: {text}"));
	}
	for codec in avif::linked_codecs().split(", ").filter(|codec| codec.starts_with("aom")) {
		if !text.contains(codec) {
			return Err(format!("the reference's libaom is not ours ({codec}): {text}"));
		}
	}
	Ok(path)
}

struct Run {
	avifenc: PathBuf,
	dir: PathBuf,
	mismatches: Vec<String>,
	total: usize,
}

impl Run {
	fn compare(&mut self, name: &str, input: &Path, settings: &AvifSettings) {
		let job = EncodeJob { format: OutputFormat::Avif, avif: settings.clone(), ..Default::default() };
		let theirs = self.dir.join(format!("avifenc-{}.avif", self.total));
		let ours = self.dir.join(format!("ours-{}.avif", self.total));
		self.total += 1;
		let args = avifenc_args(&job, input, &theirs);
		let run = Command::new(&self.avifenc).args(&args).output().expect("run avifenc");
		let reference = run.status.success().then(|| fs::read(&theirs).ok()).flatten();
		let result = encode_file(&job, input, &ours).map(|_| fs::read(&ours).expect("read ours"));
		let command = || args.iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" ");
		match (reference, result) {
			(Some(expected), Ok(actual)) if expected == actual => {}
			(Some(expected), Ok(actual)) => {
				let first = actual.iter().zip(&expected).position(|(a, b)| a != b).map_or_else(|| "length only".to_owned(), |at| format!("byte {at}"));
				self.mismatches.push(format!("`{name}`: ours {} bytes, avifenc {} bytes, first difference at {first}\n    avifenc {}", actual.len(), expected.len(), command()));
			}
			(Some(_), Err(error)) => self.mismatches.push(format!("`{name}`: avifenc succeeded but ours failed: {error}")),
			(None, _) => self.mismatches.push(format!("`{name}`: avifenc refused it: {}\n    avifenc {}", String::from_utf8_lossy(&run.stderr).trim(), command())),
		}
	}
}

fn prepare() -> Option<Run> {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let avifenc = match reference() {
		Ok(path) => path,
		Err(why) => {
			let message = format!("AVIF PARITY NOT RUN: {why}");
			assert!(!require, "{message}");
			eprintln!("{message}");
			return None;
		}
	};
	let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
	let dir = env::temp_dir().join(format!("skidbladnir-avif-parity-{}-{nanos}", std::process::id()));
	fs::create_dir_all(&dir).expect("create the scratch directory");
	Some(Run { avifenc, dir, mismatches: Vec::new(), total: 0 })
}

fn finish(run: &Run, what: &str) {
	let _ = fs::remove_dir_all(&run.dir);
	assert!(run.mismatches.is_empty(), "{} of {} {what} diverged from avifenc:\n  {}", run.mismatches.len(), run.total, run.mismatches.join("\n  "));
	eprintln!("AVIF PARITY OK: {} {what} matched avifenc byte for byte.", run.total);
}

/// Fast settings: the parity is in the flags, not the speed.
fn fast() -> AvifSettings {
	AvifSettings { speed: Some(9), ..AvifSettings::default() }
}

#[test]
fn matches_avifenc_across_png_inputs() {
	let Some(mut run) = prepare() else { return };
	let rgb8 = rows(3, 8);
	let text = |key: &str, value: &[u8]| chunk(*b"tEXt", &[key.as_bytes(), &[0], value].concat());
	let itxt = |key: &str, value: &[u8]| chunk(*b"iTXt", &[key.as_bytes(), &[0, 0, 0, 0, 0], value].concat());
	let palette: Vec<u8> = (0..=255_u8).flat_map(|i| [i, 255 - i, i.wrapping_mul(7)]).collect();
	let palette_rows: Vec<u8> = (0..H).flat_map(|y| (0..W).map(move |x| u8::try_from((x * 5 + y * 3) % 256).expect("byte"))).collect();
	let trns: Vec<u8> = (0..=255_u8).map(|i| if i % 5 == 0 { 0 } else { 255 - i / 2 }).collect();
	let fixtures: Vec<(&str, Vec<u8>)> = vec![("rgb8", png(W, H, 8, 2, &rgb8, &[], &[])), ("rgba8", png(W, H, 8, 6, &rows(4, 8), &[], &[])), ("gray8", png(W, H, 8, 0, &rows(1, 8), &[], &[])), ("graya8", png(W, H, 8, 4, &rows(2, 8), &[], &[])), ("gray4", png(W, H, 4, 0, &rows(1, 4), &[], &[])), ("rgb16", png(W, H, 16, 2, &rows(3, 16), &[], &[])), ("rgba16", png(W, H, 16, 6, &rows(4, 16), &[], &[])), ("gray16", png(W, H, 16, 0, &rows(1, 16), &[], &[])), ("palette+tRNS", png(W, H, 8, 3, &palette_rows, &[chunk(*b"PLTE", &palette), chunk(*b"tRNS", &trns)], &[])), ("rgb+tRNS", png(W, H, 8, 2, &rgb8, &[chunk(*b"tRNS", &[0, 0, 0, 0, 0, 0])], &[])), ("sRGB", png(W, H, 8, 2, &rgb8, &[chunk(*b"sRGB", &[0])], &[])), ("gAMA 2.2", png(W, H, 8, 2, &rgb8, &[chunk(*b"gAMA", &45_455_u32.to_be_bytes())], &[])), ("gAMA 1.8 + cHRM (a generated ICC)", png(W, H, 8, 2, &rgb8, &[chunk(*b"gAMA", &55_555_u32.to_be_bytes()), chunk(*b"cHRM", &[31270_u32, 32900, 64000, 33000, 30000, 60000, 15000, 6000].iter().flat_map(|v| v.to_be_bytes()).collect::<Vec<_>>())], &[])), ("gray gAMA 1.8", png(W, H, 8, 0, &rows(1, 8), &[chunk(*b"gAMA", &55_555_u32.to_be_bytes())], &[])), ("iCCP", png(W, H, 8, 2, &rgb8, &[chunk(*b"iCCP", &[b"test\0\0".as_slice(), &zlib(&icc(false))].concat())], &[])), ("gray iCCP", png(W, H, 8, 0, &rows(1, 8), &[chunk(*b"iCCP", &[b"test\0\0".as_slice(), &zlib(&icc(true))].concat())], &[])), ("cICP P3", png(W, H, 8, 2, &rgb8, &[chunk(*b"cICP", &[12, 13, 0, 1])], &[])), ("eXIf + XMP", png(W, H, 8, 2, &rgb8, &[chunk(*b"eXIf", &exif()), itxt("XML:com.adobe.xmp", b"<x:xmpmeta xmlns:x='adobe:ns:meta/'/>\0")], &[])), ("raw profile exif after the image", png(W, H, 8, 2, &rgb8, &[], &[text("Raw profile type exif", &raw_profile("exif", &[b"Exif\0\0".as_slice(), &exif()].concat()))])), ("raw profile APP1 XMP", png(W, H, 8, 2, &rgb8, &[text("Raw profile type APP1", &raw_profile("APP1", b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta/>"))], &[]))];
	for (name, bytes) in fixtures {
		let input = run.dir.join(format!("{}.png", name.replace([' ', '+', '(', ')'], "_")));
		fs::write(&input, &bytes).expect("write the fixture");
		run.compare(&format!("{name}, defaults"), &input, &fast());
		run.compare(&format!("{name}, lossless"), &input, &AvifSettings { lossless: true, ..fast() });
	}
	finish(&run, "PNG inputs");
}

#[test]
fn matches_avifenc_across_the_option_surface() {
	let Some(mut run) = prepare() else { return };
	let input = run.dir.join("rgba.png");
	fs::write(&input, png(W, H, 8, 6, &rows(4, 8), &[chunk(*b"eXIf", &exif())], &[])).expect("write the fixture");
	let input_icc = run.dir.join("rgb-icc.png");
	fs::write(&input_icc, png(W, H, 8, 2, &rows(3, 8), &[chunk(*b"iCCP", &[b"test\0\0".as_slice(), &zlib(&icc(false))].concat()), chunk(*b"iTXt", &[b"XML:com.adobe.xmp".as_slice(), &[0, 0, 0, 0, 0], b"<x:xmpmeta/>"].concat())], &[])).expect("write the fixture");
	let input16 = run.dir.join("rgba16.png");
	fs::write(&input16, png(W, H, 16, 6, &rows(4, 16), &[], &[])).expect("write the fixture");
	let big = run.dir.join("big.png");
	let (big_w, big_h) = (200_u32, 150_u32);
	let big_rows: Vec<u8> = (0..big_h).flat_map(|y| (0..big_w).flat_map(move |x| [u8::try_from((x * 3 + y) % 256).expect("byte"), u8::try_from(x % 256).expect("byte"), u8::try_from(y % 256).expect("byte")])).collect();
	fs::write(&big, png(big_w, big_h, 8, 2, &big_rows, &[], &[])).expect("write the fixture");
	let icc_file = run.dir.join("override.icc");
	fs::write(&icc_file, icc(false)).expect("write");
	let exif_file = run.dir.join("override.exif");
	fs::write(&exif_file, exif()).expect("write");
	let xmp_file = run.dir.join("override.xmp");
	fs::write(&xmp_file, b"<x:xmpmeta xmlns:x='adobe:ns:meta/'><o/></x:xmpmeta>").expect("write");

	let d = fast;
	let mut cases: Vec<(String, PathBuf, AvifSettings)> = Vec::new();
	let mut add = |name: &str, input: &Path, settings: AvifSettings| cases.push((name.to_owned(), input.to_path_buf(), settings));
	for quality in [0_u8, 30, 60, 90, 100] {
		add(&format!("-q {quality}"), &input, AvifSettings { quality: Some(quality), ..d() });
	}
	add("--qalpha 20", &input, AvifSettings { quality_alpha: Some(20), ..d() });
	add("-q 40 --qalpha 90", &input, AvifSettings { quality: Some(40), quality_alpha: Some(90), ..d() });
	for speed in [4_u8, 8, 10] {
		add(&format!("-s {speed}"), &input, AvifSettings { speed: Some(speed), ..d() });
	}
	add("-s default", &input, AvifSettings { speed: None, quality: Some(30), ..d() });
	for depth in [8_u8, 10, 12] {
		add(&format!("-d {depth}"), &input, AvifSettings { depth: Some(depth), ..d() });
		add(&format!("16-bit source, -d {depth}"), &input16, AvifSettings { depth: Some(depth), ..d() });
	}
	add("16-bit source, auto depth", &input16, d());
	for (depth, extension) in [(8_u8, 8_u8), (12, 4), (12, 8)] {
		add(&format!("-d {depth},{extension}"), &input16, AvifSettings { depth: Some(depth), depth_extension: Some(extension), ..d() });
	}
	for yuv in [YuvFormat::Yuv444, YuvFormat::Yuv422, YuvFormat::Yuv420, YuvFormat::Yuv400] {
		add(&format!("-y {}", yuv.as_avifenc_str()), &input, AvifSettings { yuv, ..d() });
	}
	add("-p", &input, AvifSettings { premultiply: true, ..d() });
	add("--sharpyuv -y 420", &input, AvifSettings { sharp_yuv: true, yuv: YuvFormat::Yuv420, ..d() });
	for (p, t, m) in [(1_u16, 13_u16, 6_u16), (9, 16, 9), (12, 13, 1), (1, 13, 0), (2, 2, 2)] {
		add(&format!("--cicp {p}/{t}/{m}"), &input, AvifSettings { cicp: Some(Cicp { primaries: p, transfer: t, matrix: m }), ..d() });
	}
	add("--cicp 1/13/16 -l (YCgCo-Re)", &input, AvifSettings { cicp: Some(Cicp { primaries: 1, transfer: 13, matrix: 16 }), lossless: true, ..d() });
	add("-r limited", &input, AvifSettings { limited_range: true, ..d() });
	add("-r limited -y 420 -d 10", &input, AvifSettings { limited_range: true, yuv: YuvFormat::Yuv420, depth: Some(10), ..d() });
	add("--target-size 1500", &input, AvifSettings { target_size: Some(1500), ..d() });
	add("--target-size 1500 --qalpha 80", &input, AvifSettings { target_size: Some(1500), quality_alpha: Some(80), ..d() });
	add("--target-size 2500 with the input's ICC and XMP", &input_icc, AvifSettings { target_size: Some(2500), ..d() });
	add("--progressive", &big, AvifSettings { progressive: true, ..d() });
	add("--progressive -q 80", &big, AvifSettings { progressive: true, quality: Some(80), ..d() });
	add("-g 2x2", &big, AvifSettings { grid: Some(Grid { columns: 2, rows: 2 }), ..d() });
	add("-g 3x2 -y 420", &big, AvifSettings { grid: Some(Grid { columns: 3, rows: 2 }), yuv: YuvFormat::Yuv420, ..d() });
	add("--min 10 --max 40", &input, AvifSettings { quantizer: Some(QuantizerRange { min: 10, max: 40 }), ..d() });
	add("--minalpha 0 --maxalpha 20", &input, AvifSettings { alpha_quantizer: Some(QuantizerRange { min: 0, max: 20 }), ..d() });
	add("tiles 1x1 log2", &big, AvifSettings { tiling: Tiling::Manual { rows_log2: 1, cols_log2: 1 }, ..d() });
	add("--scaling-mode 1/2", &big, AvifSettings { scaling_mode: Some(Fraction { numerator: 1, denominator: 2 }), ..d() });
	for (key, value) in [("tune", "ssim"), ("tune", "psnr"), ("sharpness", "3"), ("color:aq-mode", "1"), ("alpha:end-usage", "q"), ("enable-chroma-deltaq", "1"), ("color:denoise-noise-level", "10")] {
		add(&format!("-a {key}={value}"), &input, AvifSettings { codec_options: vec![CodecOption { key: key.to_owned(), value: value.to_owned() }], ..d() });
	}
	add("--pasp 4,3", &input, AvifSettings { pasp: Some([4, 3]), ..d() });
	add("--crop 4,4,40,30", &input, AvifSettings { clean_aperture: Some(CleanAperture::Crop([4, 4, 40, 30])), ..d() });
	add("--clap 60,1,40,1,0,1,0,1", &input, AvifSettings { clean_aperture: Some(CleanAperture::Raw([60, 1, 40, 1, 0, 1, 0, 1])), ..d() });
	add("--irot 1 --imir 1", &input, AvifSettings { irot: Some(1), imir: Some(1), ..d() });
	add("--clli 1000,400", &input, AvifSettings { clli: Some([1000, 400]), ..d() });
	add("--ignore-exif", &input, AvifSettings { exif: MetadataSource::Strip, ..d() });
	add("--icc --exif --xmp from files", &input, AvifSettings { icc: MetadataSource::File(icc_file.clone()), exif: MetadataSource::File(exif_file.clone()), xmp: MetadataSource::File(xmp_file.clone()), ..d() });
	add("-j 1", &input, AvifSettings { jobs: Some(1), ..d() });
	for (name, input, settings) in cases {
		run.compare(&name, &input, &settings);
	}
	finish(&run, "option settings");
}
