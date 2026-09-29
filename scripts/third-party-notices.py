#!/usr/bin/env python3
"""Regenerate THIRD-PARTY-NOTICES.md from the licence files of the native libraries the
app ships, read from the exact sources that are built: the libwebp-sys, mozjpeg-sys and
jpegxl-src crates in Cargo's registry, and the libheif, libde265 and Kvazaar submodules.

    scripts/third-party-notices.py

Run it after bumping any of them; CI's `notices` check fails if the file is stale.
"""
import pathlib, subprocess, sys, json

root = pathlib.Path(__file__).resolve().parent.parent
meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1"], cwd=root, capture_output=True, text=True, check=True).stdout)
crate = {p["name"]: pathlib.Path(p["manifest_path"]).parent for p in meta["packages"]}
webp = crate["libwebp-sys"] / "vendor"
jxl = crate["jpegxl-src"] / "libjxl"
mozjpeg = crate["mozjpeg-sys"]
# The pinned tag is recorded beside each submodule in .gitmodules (`version = ...`): a
# shallow CI checkout has no tags to describe.
submodule = lambda name: subprocess.run(["git", "config", "-f", str(root / ".gitmodules"), f"submodule.third_party/{name}.version"], capture_output=True, text=True, check=True).stdout.strip()

sections = [
	("libwebp", "BSD-3-Clause", "The WebP encoder and decoder, statically linked.", [webp / "COPYING", webp / "PATENTS"]),
	("MozJPEG (libjpeg-turbo)", "IJG AND BSD-3-Clause AND Zlib", "The JPEG decoder for JPEG input, statically linked. This software is based in part on the work of the Independent JPEG Group.", [mozjpeg / "LICENSE"]),
	("libjxl", "BSD-3-Clause", "The JPEG XL encoder, statically linked.", [jxl / "LICENSE", jxl / "PATENTS"]),
	("Highway (part of libjxl)", "BSD-3-Clause", "SIMD library used by libjxl.", [jxl / "third_party/highway/LICENSE-BSD3"]),
	("Brotli (part of libjxl)", "MIT", "Compression used by libjxl.", [jxl / "third_party/brotli/LICENSE"]),
	("skcms (part of libjxl)", "BSD-3-Clause", "Colour management used by libjxl.", [jxl / "third_party/skcms/LICENSE"]),
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
		out += ["```text", path.read_text(encoding="utf-8", errors="replace").rstrip(), "```", ""]
text = "\n".join(out)

target = root / "THIRD-PARTY-NOTICES.md"
if "--check" in sys.argv:
	if not target.exists() or target.read_text(encoding="utf-8") != text:
		sys.exit("THIRD-PARTY-NOTICES.md is stale: run scripts/third-party-notices.py")
	sys.exit(0)
target.write_text(text, encoding="utf-8")
print(f"wrote {target} ({len(text)} bytes)")
