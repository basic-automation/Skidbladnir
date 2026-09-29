#!/usr/bin/env python3
"""Regenerate THIRD-PARTY-NOTICES.md from the licence files of the native libraries the
app ships, read from the exact sources that are built: the libwebp-sys and jpegxl-src
crates in Cargo's registry, and the libavif, libaom, libjpeg-turbo, libheif, libde265 and
Kvazaar submodules.

    scripts/third-party-notices.py

Run it after bumping any of them; CI's `notices` check fails if the file is stale.
"""
import pathlib, subprocess, sys, json

root = pathlib.Path(__file__).resolve().parent.parent
meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1"], cwd=root, capture_output=True, text=True, check=True).stdout)
crate = {p["name"]: pathlib.Path(p["manifest_path"]).parent for p in meta["packages"]}
webp = crate["libwebp-sys"] / "vendor"
jxl = crate["jpegxl-src"] / "libjxl"
# The pinned tag is recorded beside each submodule in .gitmodules (`version = ...`): a
# shallow CI checkout has no tags to describe.
submodule = lambda name: subprocess.run(["git", "config", "-f", str(root / ".gitmodules"), f"submodule.third_party/{name}.version"], capture_output=True, text=True, check=True).stdout.strip()

sections = [
	("libwebp", "BSD-3-Clause", "The WebP encoder and decoder, statically linked.", [webp / "COPYING", webp / "PATENTS"]),
	("libjxl", "BSD-3-Clause", "The JPEG XL encoder, statically linked.", [jxl / "LICENSE", jxl / "PATENTS"]),
	("Highway (part of libjxl)", "BSD-3-Clause", "SIMD library used by libjxl.", [jxl / "third_party/highway/LICENSE-BSD3"]),
	("Brotli (part of libjxl)", "MIT", "Compression used by libjxl.", [jxl / "third_party/brotli/LICENSE"]),
	("skcms (part of libjxl)", "BSD-3-Clause", "Colour management used by libjxl.", [jxl / "third_party/skcms/LICENSE"]),
	(f"libavif {submodule('libavif')}", "BSD-2-Clause", "AVIF writing, statically linked, with parts of its avifenc app adapted in native/avif_shim.c and its JPEG reader (apps/shared/avifjpeg.c, with third_party/iccjpeg) compiled into it.", [root / "third_party/libavif/LICENSE"]),
	(f"libaom {submodule('aom')}", "BSD-2-Clause", "The AV1 encoder for AVIF, statically linked.", [root / "third_party/aom/LICENSE", root / "third_party/aom/PATENTS"]),
	(f"libjpeg-turbo {submodule('libjpeg-turbo')}", "IJG and BSD-3-Clause (and Zlib for its SIMD code)", "JPEG decoding, statically linked. This software is based in part on the work of the Independent JPEG Group.", [root / "third_party/libjpeg-turbo/LICENSE.md", root / "third_party/libjpeg-turbo/README.ijg"]),
	(f"libheif {submodule('libheif')} image readers (heifio)", "MIT", "heif-enc's JPEG reader and Exif helpers (heifio/decoder_jpeg.cc, heifio/exif.cc), compiled into the app by native/heic_jpeg.cc. Unlike the rest of libheif they are MIT-licensed.", [(root / "third_party/libheif/heifio/decoder_jpeg.cc", "/*", "*/")]),
	(f"Kvazaar {submodule('kvazaar')}", "BSD-3-Clause", "The HEVC encoder for HEIC, built into the shipped libheif.", [root / "third_party/kvazaar/LICENSE"]),
	(f"libde265 {submodule('libde265')}", "LGPL-3.0", "The HEVC decoder for HEIC, built into the shipped libheif.", [root / "third_party/libde265/COPYING"]),
	(f"libheif {submodule('libheif')}", "LGPL-3.0", "HEIF/HEIC reading and writing, shipped as a separate shared library.", [root / "third_party/libheif/COPYING"]),
]

out = ["# Third-party notices", "",
	"Skidbladnir is ISC-licensed. It includes the native libraries below, whose licences",
	"require their notices to travel with the app. Rust crates are listed with their",
	"licences by `cargo deny`'s policy in `deny.toml`.", "",
	"**libheif and libde265 (LGPL-3.0).** They are shipped as one shared library",
	"(`libheif.so.1`, `heif.dll`, or `libheif.1.dylib`) beside the app, which loads it",
	"dynamically, so you may replace it with your own build of the same interface. Their",
	"source is the exact upstream tags named below, at",
	"<https://github.com/strukturag/libheif> and <https://github.com/strukturag/libde265>;",
	"`scripts/build-libheif.sh` in Skidbladnir's repository is how the shipped library is",
	"built from them.", ""]
for name, spdx, what, files in sections:
	out += [f"## {name}", "", f"{what} Licence: {spdx}.", ""]
	for path in files:
		if isinstance(path, tuple):
			# The licence comment at the top of a source file.
			path, start, end = path
			source = path.read_text(encoding="utf-8")
			text = source[source.index(start) + len(start):source.index(end)].strip("\n")
		else:
			text = path.read_text(encoding="utf-8", errors="replace").rstrip()
		out += ["```text", text, "```", ""]
text = "\n".join(out)

target = root / "THIRD-PARTY-NOTICES.md"
if "--check" in sys.argv:
	if not target.exists() or target.read_text(encoding="utf-8") != text:
		sys.exit("THIRD-PARTY-NOTICES.md is stale: run scripts/third-party-notices.py")
	sys.exit(0)
target.write_text(text, encoding="utf-8")
print(f"wrote {target} ({len(text)} bytes)")
