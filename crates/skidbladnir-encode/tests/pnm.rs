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

/// A PFM file: `PF` or `Pf`, the size, the scale (its sign is the byte order), then rows
/// bottom to top. The samples run a little outside 0 to 1, as real PFMs do.
fn pfm(gray: bool, scale: &str) -> Vec<u8> {
	let channels = if gray { 1 } else { 3 };
	let little = scale.starts_with('-');
	let mut out = format!("{}\n{W} {H}\n{scale}\n", if gray { "Pf" } else { "PF" }).into_bytes();
	for y in (0..H).rev() {
		for x in 0..W {
			for c in 0..channels {
				#[expect(clippy::cast_precision_loss, reason = "small integers")]
				let value = sample(x, y, c, 1000) as f32 / 900.0 - 0.05;
				out.extend_from_slice(&if little { value.to_le_bytes() } else { value.to_be_bytes() });
			}
		}
	}
	out
}

#[test]
fn matches_cjxl_reading_pfm() {
	let Some((cjxl, dir)) = prepare("PFM cjxl", tool("SKIDBLADNIR_REFERENCE_CJXL", "cjxl", "--version", jxl::linked_version())) else { return };
	let fixtures = [("PF big-endian", pfm(false, "1.0")), ("PF little-endian", pfm(false, "-1.0")), ("Pf gray", pfm(true, "-1")), ("PF scale 2.5 (ignored)", pfm(false, "2.5"))];
	let mut mismatches = Vec::new();
	let mut total = 0;
	for (fixture, bytes) in fixtures {
		let input = dir.join("input.pfm");
		fs::write(&input, &bytes).expect("write the fixture");
		for settings in [JxlSettings { effort: 3, ..JxlSettings::default() }, JxlSettings { effort: 7, target: JxlTarget::Distance(0.0), ..JxlSettings::default() }, JxlSettings { effort: 7, target: JxlTarget::Distance(2.0), ..JxlSettings::default() }] {
			let job = EncodeJob { format: OutputFormat::Jxl, jxl: settings.clone(), ..EncodeJob::default() };
			let theirs = dir.join("cjxl.jxl");
			let ours = dir.join("ours.jxl");
			let run = Command::new(&cjxl).args(cjxl_args(&job, &input, &theirs)).output().expect("run cjxl");
			assert!(run.status.success(), "cjxl refused {fixture}: {}", String::from_utf8_lossy(&run.stderr));
			encode_file(&job, &input, &ours).unwrap_or_else(|error| panic!("ours failed on {fixture}: {error}"));
			total += 1;
			let (expected, actual) = (fs::read(&theirs).expect("read cjxl's"), fs::read(&ours).expect("read ours"));
			if expected != actual {
				mismatches.push(format!("{fixture}, {:?}: ours {} bytes, cjxl {} bytes", settings.target, actual.len(), expected.len()));
			}
		}
	}
	// libjxl 0.12.0 fails a lossless float encode at effort 3 ("JxlEncoderProcessOutput
	// failed" from cjxl itself). Both sides must refuse it; if cjxl starts accepting it, it
	// belongs in the cases above.
	let job = EncodeJob { format: OutputFormat::Jxl, jxl: JxlSettings { effort: 3, target: JxlTarget::Distance(0.0), ..JxlSettings::default() }, ..EncodeJob::default() };
	let input = dir.join("input.pfm");
	fs::write(&input, pfm(false, "1.0")).expect("write the fixture");
	let run = Command::new(&cjxl).args(cjxl_args(&job, &input, &dir.join("cjxl.jxl"))).output().expect("run cjxl");
	assert!(!run.status.success(), "cjxl now encodes a lossless float PFM at effort 3: make it a parity case");
	assert!(encode_file(&job, &input, &dir.join("ours.jxl")).is_err(), "libjxl refuses it on our side too");
	let _ = fs::remove_dir_all(&dir);
	assert!(mismatches.is_empty(), "{} of {total} PFM conversions diverged from cjxl:\n  {}", mismatches.len(), mismatches.join("\n  "));
	eprintln!("PFM PARITY OK: {total} PFM-to-JPEG XL conversions matched cjxl byte for byte.");
}

/// PFM to the other formats has no reference (only `cjxl` reads PFM): it converts, clamped
/// to the nominal range, at the file's size.
#[test]
fn pfm_converts_to_every_format() {
	let dir = env::temp_dir().join(format!("skidbladnir-pfm-formats-{}", std::process::id()));
	fs::create_dir_all(&dir).expect("create the scratch directory");
	let input = dir.join("input.pfm");
	fs::write(&input, pfm(false, "-1")).expect("write the fixture");
	for format in [OutputFormat::Webp, OutputFormat::Avif, OutputFormat::Jxl, OutputFormat::Heic] {
		let output = dir.join(format!("out.{}", format.extension()));
		let conversion = encode_file(&EncodeJob { format, ..EncodeJob::default() }, &input, &output).unwrap_or_else(|error| panic!("{format:?}: {error}"));
		assert_eq!((conversion.width, conversion.height), (W, H), "{format:?}");
	}
	let _ = fs::remove_dir_all(&dir);
}

/// A PGX: `header` then `samples` in the byte order its `ML` (big-endian) or `LM` says.
fn pgx(header: &str, samples: &[u16], wide: bool, big_endian: bool) -> Vec<u8> {
	let mut out = header.as_bytes().to_vec();
	for &sample in samples {
		match (wide, big_endian) {
			(false, _) => out.push(u8::try_from(sample).expect("an 8-bit sample")),
			(true, true) => out.extend_from_slice(&sample.to_be_bytes()),
			(true, false) => out.extend_from_slice(&sample.to_le_bytes()),
		}
	}
	out
}

/// A 21x13 gradient up to `max`.
fn pgx_samples(max: u32) -> Vec<u16> {
	(0..13_u32).flat_map(|y| (0..21_u32).map(move |x| u16::try_from((x * 11 + y * 17) * max / (20 * 11 + 12 * 17)).expect("fits"))).collect()
}

/// PGX into JPEG XL, as `cjxl` reads it (`lib/extras/dec/pgx.cc`), at the two depths it
/// reads correctly: 8 and 16 bits, either byte order, with `\r\n` and the optional space
/// after the sign. At any other depth `cjxl` hands libjxl the samples at the full range of
/// their container (a 12-bit 4095 becomes 256 of 4095), so Skidbladnir reads those as
/// meant instead, and this checks that it does and that the result then differs from
/// `cjxl`'s. Signed and over-16-bit files are refused by both.
#[test]
fn matches_cjxl_reading_pgx() {
	use skidbladnir_encode::source::load;

	let Some((cjxl, dir)) = prepare("PGX cjxl", tool("SKIDBLADNIR_REFERENCE_CJXL", "cjxl", "--version", jxl::linked_version())) else { return };
	let fixtures = [("8-bit ML", pgx("PG ML + 8 21 13\n", &pgx_samples(255), false, true)), ("8-bit LM, +8, CRLF", pgx("PG LM +8 21 13\r\n", &pgx_samples(255), false, false)), ("16-bit ML", pgx("PG ML + 16 21 13\n", &pgx_samples(65_535), true, true)), ("16-bit LM", pgx("PG LM + 16 21 13\n", &pgx_samples(65_535), true, false))];
	let settings = [JxlSettings::default(), JxlSettings { effort: 3, ..JxlSettings::default() }, JxlSettings { effort: 7, target: JxlTarget::Distance(0.0), ..JxlSettings::default() }, JxlSettings { effort: 7, target: JxlTarget::Distance(2.0), ..JxlSettings::default() }];
	let mut mismatches = Vec::new();
	let mut total = 0;
	let compare = |input: &Path, settings: &JxlSettings| -> (Vec<u8>, Vec<u8>) {
		let job = EncodeJob { format: OutputFormat::Jxl, jxl: settings.clone(), ..EncodeJob::default() };
		let (theirs, ours) = (dir.join("cjxl.jxl"), dir.join("ours.jxl"));
		let run = Command::new(&cjxl).args(cjxl_args(&job, input, &theirs)).output().expect("run cjxl");
		assert!(run.status.success(), "cjxl refused {}: {}", input.display(), String::from_utf8_lossy(&run.stderr));
		encode_file(&job, input, &ours).unwrap_or_else(|error| panic!("ours failed on {}: {error}", input.display()));
		(fs::read(&theirs).expect("read cjxl's"), fs::read(&ours).expect("read ours"))
	};
	for (fixture, bytes) in &fixtures {
		let input = dir.join("input.pgx");
		fs::write(&input, bytes).expect("write the fixture");
		for settings in &settings {
			let (expected, actual) = compare(&input, settings);
			total += 1;
			if expected != actual {
				mismatches.push(format!("{fixture}, {:?} e{}: ours {} bytes, cjxl {} bytes", settings.target, settings.effort, actual.len(), expected.len()));
			}
		}
	}

	// 12 bits: read as meant, so not as cjxl reads it.
	let input = dir.join("twelve.pgx");
	fs::write(&input, pgx("PG ML + 12 21 13\n", &pgx_samples(4095), true, true)).expect("write the fixture");
	let image = load(&input).expect("a 12-bit PGX loads");
	let brightest = image.pixels.chunks(4).map(|p| p[0]).max().expect("pixels");
	assert_eq!(brightest, 255, "the 12-bit maximum is white, not a sixteenth of it");
	let (expected, actual) = compare(&input, &JxlSettings { effort: 7, target: JxlTarget::Distance(0.0), ..JxlSettings::default() });
	assert_ne!(expected, actual, "cjxl now reads a 12-bit PGX at its depth: make 12 bits a parity case");

	for (name, bytes) in [("signed", b"PG ML - 8 2 1\n\x01\x02".to_vec()), ("17 bits", b"PG ML + 17 1 1\n\x00\x00\x00\x00".to_vec())] {
		let input = dir.join("refused.pgx");
		fs::write(&input, bytes).expect("write the fixture");
		let job = EncodeJob { format: OutputFormat::Jxl, ..EncodeJob::default() };
		let run = Command::new(&cjxl).args(cjxl_args(&job, &input, &dir.join("refused.jxl"))).output().expect("run cjxl");
		assert!(!run.status.success(), "cjxl now reads a {name} PGX");
		assert!(encode_file(&job, &input, &dir.join("refused-ours.jxl")).is_err(), "we read a {name} PGX that cjxl refuses");
	}
	let _ = fs::remove_dir_all(&dir);
	assert!(mismatches.is_empty(), "{} of {total} PGX conversions diverged from cjxl:\n  {}", mismatches.len(), mismatches.join("\n  "));
	eprintln!("PGX PARITY OK: {total} PGX-to-JPEG XL conversions matched cjxl byte for byte.");
}
