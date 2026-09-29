//! The metadata gate: keeping ICC, EXIF and XMP must write what `cwebp -metadata` writes.
//!
//! Every fixture is built here, with its metadata put in by hand, so nothing depends on
//! the host's tools or colour profiles: JPEGs with `APP1`/`APP2` segments (an ICC profile
//! big enough to need three segments, stored out of order), PNGs with `iCCP` and `eXIf`
//! or with `ImageMagick`-style hex "raw profile" and XMP text chunks, a TIFF with an ICC
//! tag, and WebP files written by `cwebp -metadata all` itself.
//!
//! Every conversion must be **byte for byte** `cwebp`'s: the metadata chunks, their order,
//! the `VP8X` header, and the image itself (both sides decode each source the same way —
//! see the JPEG- and WebP-source parity tests in `parity.rs`).
//!
//! Needs the reference `cwebp` (`SKIDBLADNIR_REFERENCE_CWEBP` or `PATH`), as the parity
//! gate does; `SKIDBLADNIR_REQUIRE_PARITY=1` makes its absence a failure.

use std::{
	env, fmt::Write as _, fs, path::{Path, PathBuf}, process::Command
};

use skidbladnir_encode::{
	EncodeJob, cwebp_args, encode_file, metadata, settings::{KeepMetadata, Mode, WebpSettings}
};

fn reference_cwebp() -> Option<PathBuf> {
	if let Some(path) = env::var_os("SKIDBLADNIR_REFERENCE_CWEBP") {
		let path = PathBuf::from(path);
		return path.is_file().then_some(path);
	}
	Command::new("cwebp").arg("-version").output().ok().filter(|out| out.status.success()).map(|_| PathBuf::from("cwebp"))
}

/// A small image with gradients, and alpha if asked.
fn pixels(alpha: bool) -> image::RgbaImage {
	image::RgbaImage::from_fn(40, 28, |x, y| {
		let a = if alpha { u8::try_from((x * 255) / 39).unwrap_or(255) } else { 255 };
		image::Rgba([u8::try_from(x * 6).unwrap_or(0), u8::try_from(y * 9).unwrap_or(0), u8::try_from((x + y) * 3).unwrap_or(0), a])
	})
}

/// A structurally valid ICC v2 RGB display profile of exactly `length` bytes (a multiple
/// of 4), with a D50 illuminant and no tags, followed by a recognisable fill — enough for
/// libpng's checks, and `cwebp` does not look inside.
fn icc_profile(length: usize) -> Vec<u8> {
	let mut icc = vec![0_u8; length];
	icc[0..4].copy_from_slice(&u32::try_from(length).unwrap_or(0).to_be_bytes());
	icc[8..12].copy_from_slice(&[2, 0x10, 0, 0]);
	icc[12..16].copy_from_slice(b"mntr");
	icc[16..20].copy_from_slice(b"RGB ");
	icc[20..24].copy_from_slice(b"XYZ ");
	icc[36..40].copy_from_slice(b"acsp");
	icc[68..80].copy_from_slice(&[0, 0, 0xf6, 0xd6, 0, 1, 0, 0, 0, 0, 0xd3, 0x2d]);
	for (index, byte) in icc.iter_mut().enumerate().skip(132) {
		*byte = u8::try_from(index % 251).unwrap_or(0);
	}
	icc
}

/// A plausible EXIF block (big-endian TIFF header, then filler), odd-length on purpose so
/// the chunk padding is exercised.
fn exif_block() -> Vec<u8> {
	let mut exif = b"MM\0\x2a\0\0\0\x08".to_vec();
	exif.extend((0..301).map(|i| u8::try_from(i % 200).unwrap_or(0)));
	exif
}

fn xmp_packet() -> String {
	"<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?><x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description xmlns:dc=\"http://purl.org/dc/elements/1.1/\" dc:format=\"image/jpeg\"/></rdf:RDF></x:xmpmeta><?xpacket end=\"w\"?>".to_owned()
}

/// A JPEG of `pixels(false)` with the given segments inserted straight after SOI.
fn jpeg_with(segments: &[(u8, Vec<u8>)]) -> Vec<u8> {
	let mut plain = Vec::new();
	image::codecs::jpeg::JpegEncoder::new_with_quality(&mut plain, 90).encode_image(&image::DynamicImage::ImageRgba8(pixels(false)).to_rgb8()).expect("encode a JPEG");
	let mut out = plain[..2].to_vec();
	for (marker, data) in segments {
		out.extend_from_slice(&[0xff, *marker]);
		out.extend_from_slice(&u16::try_from(data.len() + 2).expect("segment fits").to_be_bytes());
		out.extend_from_slice(data);
	}
	out.extend_from_slice(&plain[2..]);
	out
}

/// `APP2` segments carrying `icc`, `per` bytes each, listed in `order` (1-based).
fn icc_segments(icc: &[u8], per: usize, order: &[usize]) -> Vec<(u8, Vec<u8>)> {
	let chunks: Vec<&[u8]> = icc.chunks(per).collect();
	order.iter()
		.map(|&seq| {
			let mut data = b"ICC_PROFILE\0".to_vec();
			data.extend_from_slice(&[u8::try_from(seq).expect("seq"), u8::try_from(chunks.len()).expect("count")]);
			data.extend_from_slice(chunks[seq - 1]);
			(0xe2, data)
		})
		.collect()
}

fn app1(signature: &[u8], payload: &[u8]) -> (u8, Vec<u8>) {
	(0xe1, [signature, payload].concat())
}

/// A PNG of `pixels(alpha)` with an ICC profile and an eXIf chunk, written by `image`.
fn png_with_icc_and_exif(alpha: bool) -> Vec<u8> {
	use image::ImageEncoder;
	let mut out = Vec::new();
	let mut encoder = image::codecs::png::PngEncoder::new(&mut out);
	encoder.set_icc_profile(icc_profile(3000)).expect("PNG takes an ICC profile");
	encoder.set_exif_metadata(exif_block()).expect("PNG takes EXIF");
	let img = pixels(alpha);
	encoder.write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8).expect("encode a PNG");
	out
}

/// A PNG carrying EXIF as `ImageMagick`'s hex "Raw profile type exif" (compressed text) and
/// XMP as an `XML:com.adobe.xmp` international text chunk, written by `png` itself.
fn png_with_text_chunks() -> Vec<u8> {
	let exif = exif_block();
	let mut hex = String::new();
	for (index, byte) in exif.iter().enumerate() {
		if index % 36 == 0 {
			hex.push('\n');
		}
		let _ = write!(hex, "{byte:02x}");
	}
	let raw = format!("\nexif\n{:8}{hex}\n", exif.len());
	let img = pixels(false);
	let mut out = Vec::new();
	{
		let mut encoder = png::Encoder::new(&mut out, img.width(), img.height());
		encoder.set_color(png::ColorType::Rgba);
		encoder.add_ztxt_chunk("Raw profile type exif".to_owned(), raw).expect("add zTXt");
		encoder.add_itxt_chunk("XML:com.adobe.xmp".to_owned(), xmp_packet()).expect("add iTXt");
		let mut writer = encoder.write_header().expect("write the header");
		writer.write_image_data(img.as_raw()).expect("write the pixels");
	}
	out
}

/// An RGB TIFF with an ICC profile tag.
fn tiff_with_icc() -> Vec<u8> {
	use image::ImageEncoder;
	let mut out = std::io::Cursor::new(Vec::new());
	let mut encoder = image::codecs::tiff::TiffEncoder::new(&mut out);
	encoder.set_icc_profile(icc_profile(1024)).expect("TIFF takes an ICC profile");
	let img = image::DynamicImage::ImageRgba8(pixels(false)).to_rgb8();
	encoder.write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgb8).expect("encode a TIFF");
	out.into_inner()
}

/// `cwebp -metadata`'s spelling: a comma-separated list of `icc`, `exif`, `xmp`, or `all`.
fn keep(names: &str) -> KeepMetadata {
	let has = |kind: &str| names.split(',').any(|name| name == kind || name == "all");
	KeepMetadata { icc: has("icc"), exif: has("exif"), xmp: has("xmp") }
}

/// Our file and `cwebp`'s for the same job, or a note of why one refused.
fn both(cwebp: &Path, job: &EncodeJob, input: &Path, dir: &Path, tag: &str) -> (Result<Vec<u8>, String>, Result<Vec<u8>, String>) {
	let (ours, theirs) = (dir.join(format!("{tag}.ours.webp")), dir.join(format!("{tag}.cwebp.webp")));
	let ours = encode_file(job, input, &ours).map_err(|error| error.to_string()).and_then(|_| fs::read(&ours).map_err(|error| error.to_string()));
	let out = Command::new(cwebp).arg("-quiet").args(cwebp_args(job, input, &theirs)).output().expect("run cwebp");
	let theirs = if out.status.success() { fs::read(&theirs).map_err(|error| error.to_string()) } else { Err(String::from_utf8_lossy(&out.stderr).into_owned()) };
	(ours, theirs)
}

#[test]
fn keeps_metadata_exactly_as_cwebp_does() {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let Some(cwebp) = reference_cwebp() else {
		let message = "METADATA NOT CHECKED: no reference cwebp (set SKIDBLADNIR_REFERENCE_CWEBP).";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};
	let dir = env::temp_dir().join(format!("skidbladnir-metadata-{}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("create the scratch directory");

	let big_icc = icc_profile(140_000);
	let exif = exif_block();
	let xmp = xmp_packet();
	let mut sources: Vec<(&str, PathBuf)> = Vec::new();
	let mut add = |name: &'static str, bytes: Vec<u8>, extension: &str| {
		let path = dir.join(format!("{name}.{extension}"));
		fs::write(&path, bytes).expect("write a fixture");
		sources.push((name, path));
	};
	add("jpeg, ICC in 3 segments out of order + EXIF + XMP", jpeg_with(&[icc_segments(&big_icc, 65_519, &[2, 3, 1]), vec![app1(b"Exif\0\0", &exif), app1(b"http://ns.adobe.com/xap/1.0/\0", xmp.as_bytes()), app1(b"Exif\0\0", b"a second EXIF, ignored")]].concat()), "jpg");
	add("jpeg, EXIF only", jpeg_with(&[app1(b"Exif\0\0", &exif)]), "jpg");
	add("png, iCCP + eXIf", png_with_icc_and_exif(false), "png");
	add("png with alpha, iCCP + eXIf", png_with_icc_and_exif(true), "png");
	add("png, raw-profile EXIF + iTXt XMP", png_with_text_chunks(), "png");
	add("tiff, ICC", tiff_with_icc(), "tif");
	// WebP input carrying all three, written by cwebp itself from a PNG.
	let seed = dir.join("seed.png");
	fs::write(&seed, png_with_icc_and_exif(true)).expect("write");
	let webp_source = dir.join("webp, ICC + EXIF (cwebp-written).webp");
	let status = Command::new(&cwebp).args(["-quiet", "-lossless", "-metadata", "all"]).arg(&seed).arg("-o").arg(&webp_source).status().expect("run cwebp");
	assert!(status.success(), "cwebp must write the WebP fixture");
	sources.push(("webp, ICC + EXIF (cwebp-written)", webp_source));

	let jobs = [("lossy", WebpSettings::default()), ("lossless", WebpSettings { mode: Mode::Lossless, ..WebpSettings::default() })];
	let kinds = ["all", "icc", "exif", "xmp", "exif,xmp", "none"];
	let mut failures = Vec::new();
	let mut compared = 0;
	for (name, input) in &sources {
		let source_bytes = fs::read(input).expect("read");
		let found = metadata::extract(&source_bytes).expect("the fixtures' metadata is well formed");
		assert!(!found.is_empty(), "{name}: the fixture must carry metadata, or this compares nothing");
		for (mode, webp) in &jobs {
			for kind in kinds {
				let job = EncodeJob { webp: webp.clone(), metadata: keep(kind), ..EncodeJob::default() };
				let tag = format!("{compared}");
				compared += 1;
				let case = format!("{name}, {mode}, -metadata {kind}");
				let (ours, theirs) = match both(&cwebp, &job, input, &dir, &tag) {
					(Ok(ours), Ok(theirs)) => (ours, theirs),
					(ours, theirs) => {
						failures.push(format!("{case}: ours {:?}, cwebp {:?}", ours.map(|b| b.len()), theirs.map(|b| b.len())));
						continue;
					}
				};
				if ours != theirs {
					failures.push(format!("{case}: {} bytes, cwebp {} — not identical", ours.len(), theirs.len()));
				}
				// And what was kept is exactly the source's, so the gate is not vacuous.
				let kept = metadata::extract(&ours).expect("our output's metadata reads back");
				let expected = |wanted: bool, value: &Option<Vec<u8>>| if wanted { value.clone() } else { None };
				let want = keep(kind);
				if (kept.icc, kept.exif, kept.xmp) != (expected(want.icc, &found.icc), expected(want.exif, &found.exif), expected(want.xmp, &found.xmp)) {
					failures.push(format!("{case}: the kept metadata is not the source's"));
				}
			}
		}
	}

	// Malformed ICC segmentation: cwebp refuses the file, and so do we, by name.
	let broken = dir.join("broken-icc.jpg");
	fs::write(&broken, jpeg_with(&icc_segments(&big_icc, 65_519, &[1, 3]))).expect("write");
	let job = EncodeJob { metadata: keep("icc"), ..EncodeJob::default() };
	match both(&cwebp, &job, &broken, &dir, "broken") {
		(Err(ours), Err(_)) if ours.contains("could not keep the metadata") => {}
		(ours, theirs) => failures.push(format!("a JPEG missing an ICC segment: ours {:?}, cwebp {:?}", ours.map(|b| b.len()), theirs.map(|b| b.len()))),
	}
	// Without -metadata the same file converts on both sides: nothing was asked of it.
	match both(&cwebp, &EncodeJob::default(), &broken, &dir, "broken-default") {
		(Ok(_), Ok(_)) => {}
		(ours, theirs) => failures.push(format!("a JPEG missing an ICC segment, metadata not kept: ours {:?}, cwebp {:?}", ours.map(|b| b.len()), theirs.map(|b| b.len()))),
	}

	let _ = fs::remove_dir_all(&dir);
	assert!(failures.is_empty(), "METADATA FAILED ({} of {compared}):\n{}", failures.len(), failures.join("\n"));
	eprintln!("METADATA OK: {compared} conversions ({} sources x 2 modes x {} -metadata choices) match cwebp byte for byte; malformed ICC refused as cwebp refuses it", sources.len(), kinds.len());
}

/// A real, parseable ICC v2 display profile: Display P3 primaries (adapted to the D50 PCS),
/// a D50 white point and a 2.2 gamma, with `desc` and `cprt`. libjxl parses and uses the
/// profile it is given, so the placeholder above will not do there.
fn display_p3_profile() -> Vec<u8> {
	// s15.16 fixed point, i.e. value * 65536.
	let xyz = |x: i32, y: i32, z: i32| [&b"XYZ \0\0\0\0"[..], &x.to_be_bytes(), &y.to_be_bytes(), &z.to_be_bytes()].concat();
	let mut desc = b"desc\0\0\0\0".to_vec();
	let name = b"Skidbladnir test Display P3\0";
	desc.extend_from_slice(&u32::try_from(name.len()).expect("short").to_be_bytes());
	desc.extend_from_slice(name);
	desc.extend_from_slice(&[0; 4 + 4 + 2 + 1 + 67]);
	// XYZ values in s15.16 (value x 65536): the D50 white 0.9642, 1.0, 0.8249, and the P3
	// primaries adapted to D50 — red 0.5151, 0.2412, -0.0011; green 0.2920, 0.6922, 0.0419;
	// blue 0.1571, 0.0666, 0.7841. The curves are gamma 2.2 (u8.8 0x0233).
	let curve = b"curv\0\0\0\0\0\0\0\x01\x02\x33".to_vec();
	let tags: Vec<(&[u8; 4], Vec<u8>)> = vec![(b"desc", desc), (b"cprt", [&b"text\0\0\0\0"[..], b"No copyright, test data\0"].concat()), (b"wtpt", xyz(63190, 65536, 54061)), (b"rXYZ", xyz(33758, 15807, -72)), (b"gXYZ", xyz(19137, 45364, 2746)), (b"bXYZ", xyz(10296, 4365, 51387)), (b"rTRC", curve.clone()), (b"gTRC", curve.clone()), (b"bTRC", curve)];
	let mut body = Vec::new();
	let mut table = u32::try_from(tags.len()).expect("few").to_be_bytes().to_vec();
	let data_start = 128 + 4 + 12 * tags.len();
	for (signature, data) in &tags {
		while body.len() % 4 != 0 {
			body.push(0);
		}
		table.extend_from_slice(*signature);
		table.extend_from_slice(&u32::try_from(data_start + body.len()).expect("small").to_be_bytes());
		table.extend_from_slice(&u32::try_from(data.len()).expect("small").to_be_bytes());
		body.extend_from_slice(data);
	}
	while body.len() % 4 != 0 {
		body.push(0);
	}
	let mut icc = icc_profile(128);
	icc.extend(table);
	icc.extend(body);
	let length = u32::try_from(icc.len()).expect("small").to_be_bytes();
	icc[0..4].copy_from_slice(&length);
	icc
}

/// An ICC profile's `rXYZ`, `gXYZ` and `bXYZ` values, if it has them.
fn primaries(icc: &[u8]) -> Option<Vec<f64>> {
	let count = u32::from_be_bytes(icc.get(128..132)?.try_into().ok()?) as usize;
	let mut out = Vec::new();
	for tag in [b"rXYZ", b"gXYZ", b"bXYZ"] {
		let entry = (0..count).map(|i| 132 + 12 * i).find(|&at| icc.get(at..at + 4) == Some(&tag[..]))?;
		let offset = u32::from_be_bytes(icc.get(entry + 4..entry + 8)?.try_into().ok()?) as usize;
		for component in 0..3 {
			let at = offset + 8 + 4 * component;
			out.push(f64::from(i32::from_be_bytes(icc.get(at..at + 4)?.try_into().ok()?)) / 65536.0);
		}
	}
	Some(out)
}

/// Whether two profiles have the same primaries, to within profile rounding.
fn same_primaries(a: &[u8], b: &[u8]) -> bool {
	matches!((primaries(a), primaries(b)), (Some(a), Some(b)) if a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 0.003))
}

/// The top-level boxes of an ISO-BMFF file (a JPEG XL container), as `(type, payload)`.
fn bmff_boxes(file: &[u8]) -> Vec<([u8; 4], &[u8])> {
	let mut out = Vec::new();
	// A bare codestream (`FF 0A`) has no boxes at all.
	if !file.starts_with(b"\0\0\0\x0cJXL \r\n\x87\n") {
		return out;
	}
	let mut at = 0;
	while let Some(header) = file.get(at..at + 8) {
		let size = u32::from_be_bytes(header[..4].try_into().expect("four")) as usize;
		let kind: [u8; 4] = header[4..8].try_into().expect("four");
		let (start, end) = match size {
			0 => (at + 8, file.len()),
			1 => (at + 16, at + usize::try_from(u64::from_be_bytes(file[at + 8..at + 16].try_into().expect("eight"))).expect("fits")),
			_ => (at + 8, at + size),
		};
		out.push((kind, &file[start..end]));
		at = end;
	}
	out
}

/// Keeping metadata in **JPEG XL** output, checked by libjxl's own decoder: `djxl` must
/// report the source's ICC profile as the image's (`--icc_out`) — verbatim for lossless,
/// with the same primaries for lossy — and decode a lossless file to the source's pixels — so the profile *labels* them rather than being carried
/// beside sRGB. EXIF and XMP must be the container's `Exif` (after its 4-byte TIFF-header
/// offset) and `xml ` boxes, byte for byte. With nothing kept, none of the three appears.
#[test]
fn keeps_metadata_in_jpeg_xl() {
	let require = env::var("SKIDBLADNIR_REQUIRE_PARITY").is_ok_and(|v| v == "1");
	let djxl = env::var_os("SKIDBLADNIR_REFERENCE_DJXL").map(PathBuf::from).filter(|path| path.is_file()).or_else(|| Command::new("djxl").arg("--version").output().ok().filter(|out| out.status.success()).map(|_| PathBuf::from("djxl")));
	let Some(djxl) = djxl else {
		let message = "JPEG XL METADATA NOT CHECKED: no djxl (set SKIDBLADNIR_REFERENCE_DJXL).";
		assert!(!require, "{message}");
		eprintln!("{message}");
		return;
	};
	let dir = env::temp_dir().join(format!("skidbladnir-jxl-metadata-{}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("create the scratch directory");

	let icc = display_p3_profile();
	let (exif, xmp) = (exif_block(), xmp_packet());
	let img = pixels(true);
	let source = dir.join("source.png");
	{
		let mut out = Vec::new();
		let mut info = png::Info::with_size(img.width(), img.height());
		info.color_type = png::ColorType::Rgba;
		info.bit_depth = png::BitDepth::Eight;
		info.icc_profile = Some(std::borrow::Cow::Owned(icc.clone()));
		info.exif_metadata = Some(std::borrow::Cow::Owned(exif.clone()));
		let mut encoder = png::Encoder::with_info(&mut out, info).expect("a valid header");
		encoder.add_itxt_chunk("XML:com.adobe.xmp".to_owned(), xmp.clone()).expect("add iTXt");
		let mut writer = encoder.write_header().expect("write the header");
		writer.write_image_data(img.as_raw()).expect("write the pixels");
		drop(writer);
		fs::write(&source, out).expect("write");
	}
	let found = metadata::extract(&fs::read(&source).expect("read")).expect("well formed");
	assert_eq!((found.icc.as_deref(), found.exif.as_deref(), found.xmp.as_deref()), (Some(&icc[..]), Some(&exif[..]), Some(xmp.as_bytes())), "the fixture must carry all three");

	let mut failures = Vec::new();
	for (mode, lossless) in [("lossless", true), ("lossy", false)] {
		for kind in ["all", "none"] {
			let case = format!("{mode}, keep {kind}");
			let mut job = EncodeJob { format: skidbladnir_encode::OutputFormat::Jxl, metadata: keep(kind), ..EncodeJob::default() };
			job.jxl.lossless = lossless;
			job.jxl.effort = 3;
			let output = dir.join(format!("{mode}-{kind}.jxl"));
			if let Err(error) = encode_file(&job, &source, &output) {
				failures.push(format!("{case}: {error}"));
				continue;
			}
			let written = fs::read(&output).expect("read");
			let (icc_out, png_out) = (dir.join(format!("{mode}-{kind}.icc")), dir.join(format!("{mode}-{kind}.png")));
			let run = Command::new(&djxl).arg(&output).arg(&png_out).arg(format!("--icc_out={}", icc_out.display())).output().expect("run djxl");
			if !run.status.success() {
				failures.push(format!("{case}: djxl refused it: {}", String::from_utf8_lossy(&run.stderr)));
				continue;
			}
			let reported = fs::read(&icc_out).unwrap_or_default();
			let boxes = bmff_boxes(&written);
			let find = |kind: &[u8; 4]| boxes.iter().find(|(k, _)| k == kind).map(|(_, body)| body.to_vec());
			if kind == "all" {
				// Lossless keeps the profile verbatim. Lossy (XYB) stores the colour space in
				// JPEG XL's compact form, as `cjxl` does, and the decoder regenerates a profile:
				// there the primaries must be the source's, which sRGB's are far from.
				if lossless && reported != icc {
					failures.push(format!("{case}: djxl reports a {}-byte profile, not the source's {}", reported.len(), icc.len()));
				}
				if !lossless && !same_primaries(&reported, &icc) {
					failures.push(format!("{case}: djxl's profile does not have the source's primaries"));
				}
				if find(b"Exif") != Some([&[0_u8; 4][..], &exif].concat()) {
					failures.push(format!("{case}: no Exif box with the source's EXIF"));
				}
				if find(b"xml ").as_deref() != Some(xmp.as_bytes()) {
					failures.push(format!("{case}: no xml box with the source's XMP"));
				}
				if lossless {
					let decoded = image::open(&png_out).expect("read djxl's PNG").into_rgba8();
					if decoded.as_raw() != img.as_raw() {
						failures.push(format!("{case}: djxl does not decode the source's pixels in the source's profile"));
					}
				}
			} else if reported == icc || find(b"Exif").is_some() || find(b"xml ").is_some() {
				failures.push(format!("{case}: metadata was kept when none was asked for"));
			}
		}
	}
	if env::var_os("SKIDBLADNIR_KEEP_SCRATCH").is_none() {
		let _ = fs::remove_dir_all(&dir);
	}
	assert!(failures.is_empty(), "JPEG XL METADATA FAILED:\n{}", failures.join("\n"));
	eprintln!("JPEG XL METADATA OK: djxl reads the kept ICC profile as the image's, lossless pixels decode exactly in it, EXIF and XMP are the container's boxes; nothing is kept when nothing is asked for");
}
