#!/usr/bin/env python3
"""Regenerate the third-party notices from the licence files of the native libraries the
app ships, read from the exact sources that are built: the libwebp-sys and jpegxl-src
crates in Cargo's registry, and the libavif, libaom, libjpeg-turbo, libheif, libde265,
Kvazaar and x265 submodules.

    scripts/third-party-notices.py [--check]

There is one file per edition, because the editions ship different HEVC encoders under
different licences: THIRD-PARTY-NOTICES.md for the standard edition (Kvazaar; ISC, with
libheif and libde265 as a separate, replaceable LGPL-3.0 library) and
THIRD-PARTY-NOTICES-GPL.md for the GPL edition (x265, GPL-3.0-or-later as a whole). Each
installer carries its own edition's file as THIRD-PARTY-NOTICES.md.

Both also list what the window's frontend bundle carries that needs a notice — its font
and its icon collections — and libpng, whose gamma arithmetic png_gamma.rs reproduces,
with licence texts from LICENSES/third-party/ (see the README there), and end with the Rust
crates compiled into the app: LICENSES/third-party/rust-crates.md, which cargo-about writes
from the crates' own licence files (about.toml says how; regenerate it first after any
Cargo.lock change), and the npm packages in the webview window's bundle:
LICENSES/third-party/javascript.md, which the frontend build writes when asked
(frontend/tools/javascript-notices.ts; regenerate it after any package-lock.json change).

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
	(f"libavif {submodule('libavif')}", "BSD-2-Clause", "AVIF writing, statically linked, with parts of its avifenc app adapted in native/avif_shim.c and its JPEG reader (apps/shared/avifjpeg.c, with third_party/iccjpeg) compiled into it.", [root / "third_party/libavif/LICENSE"]),
	(f"libaom {submodule('aom')}", "BSD-2-Clause", "The AV1 encoder for AVIF, statically linked.", [root / "third_party/aom/LICENSE", root / "third_party/aom/PATENTS"]),
	(f"libjpeg-turbo {submodule('libjpeg-turbo')}", "IJG and BSD-3-Clause (and Zlib for its SIMD code)", "JPEG decoding, statically linked. This software is based in part on the work of the Independent JPEG Group.", [root / "third_party/libjpeg-turbo/LICENSE.md", root / "third_party/libjpeg-turbo/README.ijg"]),
	(f"libheif {submodule('libheif')} image readers (heifio)", "MIT", "heif-enc's JPEG reader and Exif helpers (heifio/decoder_jpeg.cc, heifio/exif.cc), compiled into the app by native/heic_jpeg.cc. Unlike the rest of libheif they are MIT-licensed.", [(root / "third_party/libheif/heifio/decoder_jpeg.cc", "/*", "*/")]),
]
# The window's font and the Iconify icon collections it bundles (the webview window from npm,
# the gpui window the same SVGs and cuts of the same font), each as its Iconify package
# declares it. The versions come from the frontend's lockfile.
lock = json.loads((root / "frontend/package-lock.json").read_text(encoding="utf-8"))["packages"]
npm_version = lambda name: lock[f"node_modules/{name}"]["version"]
third = root / "LICENSES/third-party"
cc_by = lambda name, author, url: f"Icons from {name} by {author} (<{url}>), as Iconify distributes them (its SVG clean-up applied, otherwise unchanged; only the icons the window shows are bundled), under the Creative Commons Attribution 4.0 International licence, <https://creativecommons.org/licenses/by/4.0/>. The licensor offers them as-is and as-available, with no warranties."
icons = {
	"bi": ("Bootstrap Icons", "MIT", "By The Bootstrap Authors, <https://github.com/twbs/icons>.", [third / "bootstrap-icons.txt"]),
	"codicon": ("Codicons", "CC-BY-4.0", cc_by("Codicons", "Microsoft Corporation", "https://github.com/microsoft/vscode-codicons"), []),
	"el": ("Elusive Icons", "OFL-1.1", "By Team Redux, <https://github.com/dovy/elusive-icons>. The Elusive Icons font is licensed under the SIL Open Font License 1.1, whose text is in the Fira Code section above.", []),
	"ic": ("Google Material Icons", "Apache-2.0", "By the Material Design Authors, <https://github.com/material-icons/material-icons>.", [third / "apache-2.0.txt"]),
	"iconamoon": ("IconaMoon", "CC-BY-4.0", cc_by("IconaMoon", "Dariush Habibpour", "https://github.com/dariushhpg1/IconaMoon"), []),
	"iconoir": ("Iconoir", "MIT", "By Luca Burgio, <https://github.com/iconoir-icons/iconoir>.", [third / "iconoir.txt"]),
	"lucide": ("Lucide", "ISC", "By the Lucide contributors, <https://github.com/lucide-icons/lucide>.", [third / "lucide.txt"]),
	"material-symbols": ("Material Symbols", "Apache-2.0", "By Google, <https://github.com/google/material-design-icons>. The Apache License 2.0 text is in the Google Material Icons section above.", []),
	"mdi": ("Material Design Icons", "Apache-2.0", "By Pictogrammers, <https://github.com/Templarian/MaterialDesign>, under their free licence, which licenses the icons under Apache 2.0 (text in the Google Material Icons section above).", [third / "pictogrammers-free-license.txt"]),
	"subway": ("Subway Icon Set", "CC-BY-4.0", cc_by("the Subway Icon Set", "Mariusz Ostrowski", "https://github.com/mariuszostrowski/subway"), []),
	"vscode-icons": ("VSCode Icons", "MIT", "By Roberto Huertas, <https://github.com/vscode-icons/vscode-icons>.", [third / "vscode-icons.txt"]),
}
declared = sorted(name.removeprefix("@iconify-json/") for name in json.loads((root / "frontend/package.json").read_text(encoding="utf-8"))["devDependencies"] if name.startswith("@iconify-json/"))
if declared != sorted(icons):
	sys.exit(f"the frontend's icon collections {declared} are not the ones described here {sorted(icons)}: add or remove their notices")
frontend = [(f"Fira Code {npm_version('@fontsource-variable/fira-code')} (the window's font)", "OFL-1.1", "The window's typeface: bundled as web fonts from @fontsource-variable/fira-code in the webview window, and as two static cuts made from those (weights 450 and 600, the baseline moved 0.1em; crates/skidbladnir-gpui/tools/shift_baseline.py) in the gpui window.", [third / "fira-code.txt"])]
frontend += [(f"{title} (Iconify icons, @iconify-json/{prefix} {npm_version('@iconify-json/' + prefix)})", spdx, what, files) for prefix, (title, spdx, what, files) in sorted(icons.items(), key=lambda item: (item[0] not in ("ic",), item[0]))]
libpng = ("libpng 1.6.58 (arithmetic reproduced)", "libpng-2.0", "crates/skidbladnir-encode/src/png_gamma.rs reproduces libpng 1.6.58's gamma-correction arithmetic so that PNGs reach the WebP encoder exactly as cwebp hands them over; no libpng code is linked from it.", [third / "libpng.txt"])

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
		"and this build's HEIC encoder is Kvazaar (BSD-3-Clause), not x265. The build ships",
		"libheif and libde265 as a separate, replaceable LGPL-3.0 shared library, described",
		"next. It includes the native libraries below, whose licences require their notices to",
		"travel with the app, then libpng's licence, the font and icons the window bundles, the",
		"JavaScript packages in the webview window, and the Rust crates compiled into the app.",
		""] + lgpl, common + [kvazaar] + heif + [libpng] + frontend),
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
		"with the app, then libpng's licence, the font and icons the window bundles, the",
		"JavaScript packages in the webview window, and the Rust crates compiled into the app.", ""] + lgpl, common + [x265] + heif + [libpng] + frontend),
}

rust_crates = (third / "rust-crates.md").read_text(encoding="utf-8")
javascript = (third / "javascript.md").read_text(encoding="utf-8")

stale = []
for name, (intro, sections) in editions.items():
	out = list(intro)
	for title, spdx, what, files in sections:
		out += [f"## {title}", "", f"{what} Licence: {spdx}.", ""]
		for path in files:
			if isinstance(path, tuple):
				# The licence comment at the top of a source file.
				path, start, end = path
				source = path.read_text(encoding="utf-8")
				text = source[source.index(start) + len(start):source.index(end)].strip("\n")
			else:
				text = path.read_text(encoding="utf-8", errors="replace").rstrip()
			out += ["```text", text, "```", ""]
	out += ["## JavaScript packages", "",
		"The npm packages whose code the webview window's bundle carries (the window Skidbladnir",
		"falls back to when the native one cannot open), grouped by licence text, each text as",
		"the packages below it carry it.", ""]
	out += [javascript.strip("\n"), ""]
	out += ["## Rust crates", "",
		"The Rust crates compiled into the app on any of the platforms it is built for, grouped",
		"by licence text: each text is quoted as the crates below it carry it, under the licence",
		"Skidbladnir takes it under where a crate offers a choice. A crate that ships no licence",
		"file is shown with its licence's standard text.", ""]
	out += [rust_crates.strip("\n"), ""]
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
