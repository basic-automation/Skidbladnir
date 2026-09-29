//! The metadata gate: keeping ICC, EXIF and XMP must write what `cwebp -metadata` writes.
//!
//! Every fixture is built here, with its metadata put in by hand, so nothing depends on
//! the host's tools or colour profiles: JPEGs with `APP1`/`APP2` segments (an ICC profile
//! big enough to need three segments, stored out of order), PNGs with `iCCP` and `eXIf`
//! or with `ImageMagick`-style hex "raw profile" and XMP text chunks, a TIFF with an ICC
//! tag, and WebP files written by `cwebp -metadata all` itself.
//!
//! **Byte for byte** for PNG, TIFF and WebP sources, which both sides decode to the same
//! pixels. A JPEG is decoded by libjpeg in `cwebp` and not by us, so its image chunk may
//! legitimately differ by a rounding; there every *other* chunk — the `VP8X` header, the
//! order, and each metadata payload — must still be identical.
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

/// The top-level chunks of a WebP file.
fn chunks(webp: &[u8]) -> Vec<([u8; 4], &[u8])> {
	let mut out = Vec::new();
	let mut at = 12;
	while let Some(header) = webp.get(at..at + 8) {
		let fourcc: [u8; 4] = header[..4].try_into().expect("four bytes");
		let size = u32::from_le_bytes(header[4..8].try_into().expect("four bytes")) as usize;
		out.push((fourcc, &webp[at + 8..at + 8 + size]));
		at += 8 + size + (size & 1);
	}
	out
}

/// The chunks with the image data emptied, for comparing everything else.
fn without_image(webp: &[u8]) -> Vec<([u8; 4], &[u8])> {
	chunks(webp).into_iter().map(|(fourcc, body)| (fourcc, if matches!(&fourcc, b"VP8 " | b"VP8L" | b"ALPH") { &[][..] } else { body })).collect()
}

fn keep(names: &str) -> KeepMetadata {
	KeepMetadata { icc: names.contains("icc"), exif: names.contains("exif"), xmp: names.contains("xmp") }
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
	// (name, path, whether the image data must match too: [lossy, lossless])
	let mut sources: Vec<(&str, PathBuf, [bool; 2])> = Vec::new();
	let mut add = |name: &'static str, bytes: Vec<u8>, extension: &str, exact: [bool; 2]| {
		let path = dir.join(format!("{name}.{extension}"));
		fs::write(&path, bytes).expect("write a fixture");
		sources.push((name, path, exact));
	};
	add("jpeg, ICC in 3 segments out of order + EXIF + XMP", jpeg_with(&[icc_segments(&big_icc, 65_519, &[2, 3, 1]), vec![app1(b"Exif\0\0", &exif), app1(b"http://ns.adobe.com/xap/1.0/\0", xmp.as_bytes()), app1(b"Exif\0\0", b"a second EXIF, ignored")]].concat()), "jpg", [false; 2]);
	add("jpeg, EXIF only", jpeg_with(&[app1(b"Exif\0\0", &exif)]), "jpg", [false; 2]);
	add("png, iCCP + eXIf", png_with_icc_and_exif(false), "png", [true; 2]);
	add("png with alpha, iCCP + eXIf", png_with_icc_and_exif(true), "png", [true; 2]);
	add("png, raw-profile EXIF + iTXt XMP", png_with_text_chunks(), "png", [true; 2]);
	add("tiff, ICC", tiff_with_icc(), "tif", [true; 2]);
	// WebP input carrying all three, written by cwebp itself from a PNG.
	let seed = dir.join("seed.png");
	fs::write(&seed, png_with_icc_and_exif(true)).expect("write");
	let webp_source = dir.join("webp, ICC + EXIF (cwebp-written).webp");
	let status = Command::new(&cwebp).args(["-quiet", "-lossless", "-metadata", "all"]).arg(&seed).arg("-o").arg(&webp_source).status().expect("run cwebp");
	assert!(status.success(), "cwebp must write the WebP fixture");
	// Lossy output from a WebP source is not byte-identical even with no metadata: cwebp
	// decodes a WebP input straight to YUV for a lossy encode (ROADMAP.md), so only its
	// lossless output is compared whole.
	sources.push(("webp, ICC + EXIF (cwebp-written)", webp_source, [false, true]));

	let jobs = [("lossy", WebpSettings::default()), ("lossless", WebpSettings { mode: Mode::Lossless, ..WebpSettings::default() })];
	let mut whole = 0;
	let kinds = ["all", "icc", "exif", "xmp", "exif,xmp", "none"];
	let mut failures = Vec::new();
	let mut compared = 0;
	for (name, input, exact) in &sources {
		let source_bytes = fs::read(input).expect("read");
		let found = metadata::extract(&source_bytes).expect("the fixtures' metadata is well formed");
		assert!(!found.is_empty(), "{name}: the fixture must carry metadata, or this compares nothing");
		for ((mode, webp), exact) in jobs.iter().zip(exact) {
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
				if *exact {
					whole += 1;
					if ours != theirs {
						failures.push(format!("{case}: {} bytes, cwebp {} — not identical", ours.len(), theirs.len()));
					}
				} else {
					// Every chunk but the image data must match, in order.
					if without_image(&ours) != without_image(&theirs) {
						let names = |webp: &[u8]| chunks(webp).iter().map(|(fourcc, body)| format!("{}:{}", String::from_utf8_lossy(fourcc), body.len())).collect::<Vec<_>>().join(" ");
						failures.push(format!("{case}: chunks differ: ours [{}] cwebp [{}]", names(&ours), names(&theirs)));
					}
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
	eprintln!("METADATA OK: {compared} conversions ({} sources x 2 modes x {} -metadata choices) match cwebp — {whole} whole-file, the rest chunk for chunk apart from the image data; malformed ICC refused as cwebp refuses it", sources.len(), kinds.len());
}
