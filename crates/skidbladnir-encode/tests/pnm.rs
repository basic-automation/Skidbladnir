//! PNM input against the two reference tools that read it: `cwebp` and `cjxl`.
//!
//! Each fixture is written here, so its header and samples are exactly what the case
//! needs: binary graymaps and pixmaps, PAM with and without alpha and with and without a
//! `TUPLTYPE`, 8 and 16 bits per sample, maximum values that are not 255 (which `cwebp`
//! rescales and `cjxl` keeps), and a header with comment and blank lines. Every case
//! converts the file through [`encode_file`] and through the reference tool and compares
//! the two outputs byte for byte, so it checks our reading of the file, not only the
//! encoder.
//!
//! Each tool is found through `SKIDBLADNIR_REFERENCE_CWEBP` / `SKIDBLADNIR_REFERENCE_CJXL`
//! or on `PATH`, and must be the version this crate links; otherwise the test says so and
//! returns, and `SKIDBLADNIR_REQUIRE_PARITY=1` makes that a failure.

use std::{
	env, fs, path::{Path, PathBuf}, process::Command
};

use skidbladnir_encode::{
	cjxl::cjxl_args, cwebp_args, jxl, settings::{Crop, EncodeJob, JxlSettings, JxlTarget, OutputFormat, Resize, ResizeMode, WebpSettings}, source::encode_file
};

const W: u32 = 61;
const H: u32 = 43;

/// Sample `c` of pixel `(x, y)` scaled to `max`: ramps, a ripple and blocks of
/// transparency, with detail in the low byte of a 16-bit sample.
fn sample(x: u32, y: u32, c: u32, max: u32) -> u32 {
	let unit = match c {
		0 => (x * 13 + y * 7) % 97 * 1000 / 96,
		1 => x * 1000 / (W - 1),
		2 => y * 1000 / (H - 1),
		_ if (x / 8 + y / 8).is_multiple_of(3) => 0,
		_ => 1000 - x * 1000 / (2 * W),
	};
	(unit * max + 500) / 1000
}

/// The raster of a `channels`-sample image at maximum value `max`; a gray-and-alpha image
/// takes its alpha from the RGBA alpha ramp.
fn raster(channels: u32, max: u32) -> Vec<u8> {
	let mut out = Vec::new();
	for y in 0..H {
		for x in 0..W {
			for c in 0..channels {
				let value = sample(x, y, if channels == 2 && c == 1 { 3 } else { c }, max);
				if max > 255 {
					out.extend_from_slice(&u16::try_from(value).expect("16-bit").to_be_bytes());
				} else {
					out.push(u8::try_from(value).expect("8-bit"));
				}
			}
		}
	}
	out
}

/// A PNM file: the header as given, then the raster.
fn file(header: &str, channels: u32, max: u32) -> Vec<u8> {
	[header.as_bytes(), &raster(channels, max)].concat()
}

fn pam(channels: u32, max: u32, tuple: Option<&str>) -> Vec<u8> {
	let tuple = tuple.map_or_else(String::new, |tuple| format!("TUPLTYPE {tuple}\n"));
	file(&format!("P7\nWIDTH {W}\nHEIGHT {H}\nDEPTH {channels}\nMAXVAL {max}\n{tuple}ENDHDR\n"), channels, max)
}

/// Every fixture: a name, the file, and whether `cjxl` reads it (it refuses a maximum that
/// is not one less than a power of two).
fn fixtures() -> Vec<(&'static str, Vec<u8>, bool)> {
	vec![("P5 8-bit", file(&format!("P5\n{W} {H}\n255\n"), 1, 255), true), ("P6 8-bit", file(&format!("P6\n{W} {H}\n255\n"), 3, 255), true), ("P6 16-bit", file(&format!("P6\n{W} {H}\n65535\n"), 3, 65_535), true), ("P5 10-bit", file(&format!("P5\n{W} {H}\n1023\n"), 1, 1023), true), ("P6 4-bit", file(&format!("P6\n{W} {H}\n15\n"), 3, 15), true), ("P6 maximum 1000", file(&format!("P6\n{W} {H}\n1000\n"), 3, 1000), false), ("P6 with comments", file(&format!("P6\n# written by hand\n\n{W} {H}\n# the maximum\n255\n"), 3, 255), true), ("PAM RGB_ALPHA", pam(4, 255, Some("RGB_ALPHA")), true), ("PAM GRAYSCALE_ALPHA", pam(2, 255, Some("GRAYSCALE_ALPHA")), true), ("PAM RGB_ALPHA 16-bit", pam(4, 65_535, Some("RGB_ALPHA")), true), ("PAM depth 3, no TUPLTYPE", pam(3, 255, None), true)]
}

fn reference(variable: &str, program: &str, version_flag: &str) -> Option<PathBuf> {
	if let Some(path) = env::var_os(variable) {
		let path = PathBuf::from(path);
		return path.is_file().then_some(path);
	}
	Command::new(program).arg(version_flag).output().ok().filter(|out| out.status.success()).map(|_| PathBuf::from(program))
}

/// The first `a.b.c` in a tool's version output.
fn version(tool: &Path, flag: &str) -> Option<(u32, u32, u32)> {
	let out = Command::new(tool).arg(flag).output().ok()?;
	let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
	text.split_whitespace().find_map(|word| {
		let mut parts = word.trim_start_matches('v').split('.').map(str::parse::<u32>);
		match (parts.next(), parts.next(), parts.next()) {
			(Some(Ok(a)), Some(Ok(b)), Some(Ok(c))) => Some((a, b, c)),
			_ => None,
		}
	})
}

/// The tool, if it is present and the version this crate links; otherwise a message.
fn tool(variable: &str, program: &str, flag: &str, linked: (u32, u32, u32)) -> Result<PathBuf, String> {
	let path = reference(variable, program, flag).ok_or_else(|| format!("no reference {program} found. Set {variable} or put {program} on PATH."))?;
	match version(&path, flag) {
		Some(found) if found == linked => Ok(path),
		found => Err(format!("reference {program} is {found:?} but this crate links {linked:?}.")),
	}
}

/// Run the gate, or say why it cannot run.
fn prepare(what: &str, found: Result<PathBuf, String>) -> Option<(PathBuf, PathBuf)> {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	match found {
		Ok(path) => {
			let dir = env::temp_dir().join(format!("skidbladnir-pnm-{what}-{}", std::process::id()));
			fs::create_dir_all(&dir).expect("create the scratch directory");
			Some((path, dir))
		}
		Err(message) => {
			let message = format!("PNM {what} PARITY NOT RUN: {message}");
			assert!(!require, "{message}");
			eprintln!("{message}");
			None
		}
	}
}

/// WebP settings that reach every way the decoded samples are used: the YUV and ARGB paths,
/// alpha kept, dropped and blended, and the crop and resize that follow the read.
fn webp_cases() -> Vec<(&'static str, EncodeJob)> {
	let webp = |settings: WebpSettings| EncodeJob::from(settings);
	let d = WebpSettings::default;
	vec![("default", EncodeJob::default()), ("quality 10", webp(WebpSettings { quality: 10.0, ..d() })), ("quality 95", webp(WebpSettings { quality: 95.0, ..d() })), ("alpha quality 50", webp(WebpSettings { alpha_quality: 50, ..d() })), ("lossless", webp(WebpSettings { lossless: true, ..d() })), ("lossless exact", webp(WebpSettings { lossless: true, exact: true, ..d() })), ("near-lossless 60", webp(WebpSettings { lossless: true, near_lossless: 60, ..d() })), ("sharp yuv", webp(WebpSettings { sharp_yuv: true, ..d() })), ("no alpha", webp(WebpSettings { keep_alpha: false, ..d() })), ("blend alpha", webp(WebpSettings { blend_alpha: Some(0x0012_3456), ..d() })), ("method 6", webp(WebpSettings { method: 6, ..d() })), ("crop", EncodeJob { crop: Some(Crop { x: 3, y: 2, width: 40, height: 30 }), ..EncodeJob::default() }), ("resize", EncodeJob { resize: Resize { width: 32, height: 0, mode: ResizeMode::Always }, ..EncodeJob::default() })]
}

#[test]
fn matches_cwebp_reading_pnm() {
	let (major, minor, revision) = skidbladnir_encode::encoder::linked_encoder_version();
	let linked = (major.unsigned_abs(), minor.unsigned_abs(), revision.unsigned_abs());
	let Some((cwebp, dir)) = prepare("cwebp", tool("SKIDBLADNIR_REFERENCE_CWEBP", "cwebp", "-version", linked)) else { return };

	let mut mismatches = Vec::new();
	let mut total = 0;
	for (fixture, bytes, _) in fixtures() {
		let input = dir.join(format!("{}.pnm", fixture.replace([' ', ','], "_")));
		fs::write(&input, &bytes).expect("write the fixture");
		for (name, job) in webp_cases() {
			total += 1;
			let theirs = dir.join("cwebp.webp");
			let ours = dir.join("ours.webp");
			let run = Command::new(&cwebp).args(cwebp_args(&job, &input, &theirs)).output().expect("run cwebp");
			assert!(run.status.success(), "cwebp refused {fixture}, `{name}`: {}", String::from_utf8_lossy(&run.stderr));
			encode_file(&job, &input, &ours).unwrap_or_else(|error| panic!("ours failed on {fixture}, `{name}`: {error}"));
			let (expected, actual) = (fs::read(&theirs).expect("read cwebp's"), fs::read(&ours).expect("read ours"));
			if expected != actual {
				mismatches.push(format!("{fixture}, `{name}`: ours {} bytes, cwebp {} bytes", actual.len(), expected.len()));
			}
			let _ = fs::remove_file(&ours);
		}
	}
	let _ = fs::remove_dir_all(&dir);
	assert!(mismatches.is_empty(), "{} of {total} PNM conversions diverged from cwebp:\n  {}", mismatches.len(), mismatches.join("\n  "));
	eprintln!("PNM PARITY OK: {total} PNM-to-WebP conversions matched cwebp byte for byte.");
}

/// What `cwebp` cannot read, Skidbladnir refuses too: the ASCII and bitmap forms, a header
/// on one line, a truncated raster. Checked against `cwebp` first, so the list cannot hold
/// a file it reads.
#[test]
fn refuses_what_cwebp_refuses() {
	let (major, minor, revision) = skidbladnir_encode::encoder::linked_encoder_version();
	let linked = (major.unsigned_abs(), minor.unsigned_abs(), revision.unsigned_abs());
	let Some((cwebp, dir)) = prepare("cwebp refusal", tool("SKIDBLADNIR_REFERENCE_CWEBP", "cwebp", "-version", linked)) else { return };
	let refused: [(&str, &[u8]); 4] = [("plain pixmap", b"P3\n1 1\n255\n1 2 3\n"), ("bitmap", b"P4\n8 1\n\xff"), ("one-line header", b"P6 1 1 255\n\x01\x02\x03"), ("truncated", b"P6\n2 2\n255\n\x01\x02\x03")];
	for (name, bytes) in refused {
		let input = dir.join("refused.pnm");
		fs::write(&input, bytes).expect("write the fixture");
		let run = Command::new(&cwebp).args(cwebp_args(&EncodeJob::default(), &input, &dir.join("cwebp.webp"))).output().expect("run cwebp");
		assert!(!run.status.success(), "cwebp reads the {name} file, so it belongs in the parity fixtures instead");
		assert!(encode_file(&EncodeJob::default(), &input, &dir.join("ours.webp")).is_err(), "the {name} file must be refused");
	}
	let _ = fs::remove_dir_all(&dir);
}

#[test]
fn matches_cjxl_reading_pnm() {
	let Some((cjxl, dir)) = prepare("cjxl", tool("SKIDBLADNIR_REFERENCE_CJXL", "cjxl", "--version", jxl::linked_version())) else { return };

	let mut mismatches = Vec::new();
	let mut total = 0;
	for (fixture, bytes, cjxl_reads) in fixtures() {
		let input = dir.join(format!("{}.pnm", fixture.replace([' ', ','], "_")));
		fs::write(&input, &bytes).expect("write the fixture");
		for settings in [JxlSettings { effort: 3, ..JxlSettings::default() }, JxlSettings { effort: 3, target: JxlTarget::Distance(0.0), ..JxlSettings::default() }] {
			let job = EncodeJob { format: OutputFormat::Jxl, jxl: settings.clone(), ..EncodeJob::default() };
			let theirs = dir.join("cjxl.jxl");
			let ours = dir.join("ours.jxl");
			let _ = fs::remove_file(&theirs);
			let run = Command::new(&cjxl).args(cjxl_args(&job, &input, &theirs)).output().expect("run cjxl");
			encode_file(&job, &input, &ours).unwrap_or_else(|error| panic!("ours failed on {fixture}: {error}"));
			if !cjxl_reads {
				// Not a parity case: cjxl refuses the file, and Skidbladnir converts it the way
				// it converts any input cjxl cannot read.
				assert!(!run.status.success(), "cjxl now reads {fixture}: make it a parity case");
				continue;
			}
			total += 1;
			assert!(run.status.success(), "cjxl refused {fixture}: {}", String::from_utf8_lossy(&run.stderr));
			let (expected, actual) = (fs::read(&theirs).expect("read cjxl's"), fs::read(&ours).expect("read ours"));
			if expected != actual {
				mismatches.push(format!("{fixture}, {:?}: ours {} bytes, cjxl {} bytes", settings.target, actual.len(), expected.len()));
			}
		}
	}
	let _ = fs::remove_dir_all(&dir);
	assert!(mismatches.is_empty(), "{} of {total} PNM conversions diverged from cjxl:\n  {}", mismatches.len(), mismatches.join("\n  "));
	eprintln!("PNM PARITY OK: {total} PNM-to-JPEG XL conversions matched cjxl byte for byte.");
}
