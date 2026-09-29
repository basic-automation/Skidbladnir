#!/usr/bin/env python3
"""Regenerate the third-party notices from the licence files of the native libraries the
app ships, read from the exact sources that are built: the libwebp-sys and jpegxl-src
crates in Cargo's registry, and the libheif, libde265, Kvazaar and x265 submodules.

    scripts/third-party-notices.py [--check]

There is one file per edition, because the editions ship different HEVC encoders under
different licences: THIRD-PARTY-NOTICES.md for the standard edition (Kvazaar, ISC as a
whole) and THIRD-PARTY-NOTICES-GPL.md for the GPL edition (x265, GPL-3.0-or-later as a
whole). Each installer carries its own edition's file as THIRD-PARTY-NOTICES.md.

Run it after bumping any of them; CI's `notices` check fails if either file is stale.
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

common = [
	("libwebp", "BSD-3-Clause", "The WebP encoder and decoder, statically linked.", [webp / "COPYING", webp / "PATENTS"]),
	("libjxl", "BSD-3-Clause", "The JPEG XL encoder, statically linked.", [jxl / "LICENSE", jxl / "PATENTS"]),
	("Highway (part of libjxl)", "BSD-3-Clause", "SIMD library used by libjxl.", [jxl / "third_party/highway/LICENSE-BSD3"]),
	("Brotli (part of libjxl)", "MIT", "Compression used by libjxl.", [jxl / "third_party/brotli/LICENSE"]),
	("skcms (part of libjxl)", "BSD-3-Clause", "Colour management used by libjxl.", [jxl / "third_party/skcms/LICENSE"]),
]
heif = [
	(f"libde265 {submodule('libde265')}", "LGPL-3.0", "The HEVC decoder for HEIC, built into the shipped libheif.", [root / "third_party/libde265/COPYING"]),
	(f"libheif {submodule('libheif')}", "LGPL-3.0", "HEIF/HEIC reading and writing, shipped as a separate shared library.", [root / "third_party/libheif/COPYING"]),
]
kvazaar = (f"Kvazaar {submodule('kvazaar')}", "BSD-3-Clause", "The HEVC encoder for HEIC, built into the shipped libheif.", [root / "third_party/kvazaar/LICENSE"])
x265 = (f"x265 {submodule('x265')}", "GPL-2.0-or-later", "The HEVC encoder for HEIC, built into the shipped libheif, with its 8-bit and 10-bit encoders.", [root / "third_party/x265/COPYING"])

lgpl = ["**libheif and libde265 (LGPL-3.0).** They are shipped as one shared library",
	"(`libheif.so.1`, `heif.dll`, or `libheif.1.dylib`) beside the app, which loads it",
	"dynamically, so you may replace it with your own build of the same interface. Their",
	"source is the exact upstream tags named below, at",
	"<https://github.com/strukturag/libheif> and <https://github.com/strukturag/libde265>;",
	"`scripts/build-libheif.sh` in Skidbladnir's repository is how the shipped library is",
	"built from them.", ""]
editions = {
	"THIRD-PARTY-NOTICES.md": (["# Third-party notices", "",
		"This is the **standard edition** of Skidbladnir. Skidbladnir is ISC-licensed (`LICENSE`),",
		"and so is this build as a whole: its HEIC encoder is Kvazaar. It includes the native",
		"libraries below, whose licences require their notices to travel with the app. Rust",
		"crates are listed with their licences by `cargo deny`'s policy in `deny.toml`.", ""] + lgpl, common + [kvazaar] + heif),
	"THIRD-PARTY-NOTICES-GPL.md": (["# Third-party notices", "",
		"This is the **GPL edition** of Skidbladnir. Skidbladnir's own source is ISC-licensed",
		"(`LICENSE`), but this build encodes HEIC with x265, which is licensed GPL-2.0-or-later,",
		"so this build as a whole is distributed under the GNU General Public License, version 3",
		"or (at your option) any later version (`COPYING`). You may copy, modify and",
		"redistribute it under those terms. It comes with NO WARRANTY, to the extent permitted",
		"by law.", "",
		"**Source.** The complete corresponding source of each release is attached to that",
		"release at <https://github.com/basic-automation/Skidbladnir/releases>, as",
		"`Skidbladnir-<version>-source.tar.gz`: Skidbladnir's own source with the exact x265,",
		"libheif, libde265 and other native sources it was built from, and the build scripts.",
		"The Rust crates and npm packages it also compiles in are pinned by `Cargo.lock` and",
		"`frontend/package-lock.json` in that archive, and are published unmodified on",
		"<https://crates.io> and <https://www.npmjs.com>.", "",
		"It includes the native libraries below, whose licences require their notices to travel",
		"with the app. Rust crates are listed with their licences by `cargo deny`'s policy in",
		"`deny.toml`.", ""] + lgpl, common + [x265] + heif),
}

stale = []
for name, (intro, sections) in editions.items():
	out = list(intro)
	for title, spdx, what, files in sections:
		out += [f"## {title}", "", f"{what} Licence: {spdx}.", ""]
		for path in files:
			out += ["```text", path.read_text(encoding="utf-8", errors="replace").rstrip(), "```", ""]
	text = "\n".join(out)
	target = root / name
	if "--check" in sys.argv:
		if not target.exists() or target.read_text(encoding="utf-8") != text:
			stale.append(name)
		continue
	target.write_text(text, encoding="utf-8")
	print(f"wrote {target} ({len(text)} bytes)")
if stale:
	sys.exit(f"{', '.join(stale)} stale: run scripts/third-party-notices.py")
