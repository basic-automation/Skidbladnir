# Skidbladnir

A desktop GUI for converting images to next-generation formats — **WebP**, **AVIF**,
**JPEG XL** and **HEIC** — that gives you every option of each format's official
command-line encoder, not just a quality slider.

**For a still image, any file the official encoder can write, Skidbladnir can write too,
byte for byte:** `cwebp` for WebP, `avifenc` (with libaom) for AVIF, `cjxl` for JPEG XL and
`heif-enc` with the Kvazaar encoder for HEIC. Every option they have for one image is a
control in the window, labelled with the flag it sets, and tests run the real tools and
compare the bytes. What that covers and where it stops is spelled out under
[What "every option" means](#what-every-option-means).

![The Skidbladnir window with WebP selected: the format rail, the queue, destination and presets sidebar, and every cwebp option as a control labelled with its flag — compression, targets, presets and lossless levels, transparency, lossy tuning, metadata, performance, crop and resize — with a preview of the result beside the original](resources/images/screenshot.webp)

*WebP selected, with a file previewed: every `cwebp` option, each labelled with its flag.*

<details>
<summary>With AVIF selected</summary>

![The Skidbladnir window with AVIF selected: avifenc's options as controls labelled with their flags — colour and alpha quality, speed, lossless and target size, then bit depth, YUV format and range, premultiplied alpha and sharp YUV, then colour signalling, content light level, and the ICC profile, Exif and XMP taken from the input, left out or read from a file](resources/images/screenshot-avif.webp)

</details>

## Features

**Input formats**

- PNG, JPEG (including Photoshop's CMYK), TIFF and WebP (including animated WebP)
- **AVIF**, **JPEG XL**, **HEIC**, and **GIF**, still or animated
- **PNM**: binary PGM, PPM and PAM (8 or 16 bits, with or without alpha), and **PFM**
  floating-point images

**Output formats, each with its reference encoder's whole command line**

| Format | Reference tool | Encoder in the app | Byte-for-byte parity, tested |
|---|---|---|---|
| WebP | `cwebp` 1.6.0 | libwebp 1.6.0 | 2,673 cases: every option, through PNG (every colour type, gamma), JPEG, TIFF (including premultiplied alpha), WebP and PNM files, and `-metadata` |
| Animated WebP | `img2webp` and `gif2webp` 1.6.0 | libwebp 1.6.0 | 87 cases: animated WebP and GIF input, every setting those tools can express, and a GIF's ICC profile and XMP (`gif2webp -metadata`) |
| AVIF | `avifenc` 1.4.2 | libavif 1.4.2 + libaom 3.15.1 | 191 cases: every option, PNG inputs, JPEG input |
| JPEG XL | `cjxl` 0.12.0 | libjxl 0.12.0 | 234 cases: every option, PNG, PNM and PFM inputs, JPEG recompression and decoding |
| HEIC | `heif-enc -e kvazaar` 1.23.5 | libheif 1.23.5 + Kvazaar 2.3.2 | 147 cases: every option, PNG inputs, JPEG input |

The [GPL edition](#editions) writes HEIC with x265 instead, adding `heif-enc`'s `-L`
lossless, 4:4:4 and 4:2:2 chroma, 10-bit output and x265's own tuning; its HEIC is checked
by decoding, not byte for byte against a reference.

- **WebP**: lossless and near-lossless, `-exact`, the libwebp presets and `-z` lossless
  levels, targets by size or PSNR, every lossy tuning control (SNS, segments, filter
  type, strength, sharpness and auto-filter, passes, quality range, pre-processing,
  partition limit, JPEG-like), alpha quality, method and filtering, `-noalpha`,
  `-blend_alpha`, and `-metadata`.
- **AVIF**: colour and alpha quality (or leave them to `avifenc`), speed, lossless, 8,
  10 or 12 bits and the 16-bit depth extension, 4:4:4, 4:2:2, 4:2:0 or 4:0:0, sharp YUV,
  premultiplied alpha, CICP and range, target size, progressive layers, grids, tiling,
  scaling, quantizers, libaom's own options, pixel aspect, clean aperture, rotation,
  mirroring, content light level, and ICC, Exif and XMP from the input, left out or from a
  file.
- **JPEG XL**: distance or quality, alpha distance, effort (up to 11), **lossless JPEG
  recompression** — a JPEG repacked about 20% smaller and rebuildable bit for bit — and
  every one of `cjxl`'s modular, progressive, filter, colour-space, metadata and container
  options.
- **HEIC**, the format iPhones use: quality, Kvazaar's lossless coding, chroma
  downsampling, the colour-profile presets and custom code points, thumbnails, alpha and
  premultiplied alpha, rotation and mirroring, tiles, 360° projection, a description,
  compatible brands and the compact `mini` format.
- Crop and resize (with `-resize_mode`), shared by every format.

**Workflow**

- Preview the result beside the original, in any format, before anything is written
- Animated WebP or GIF in, animated WebP out, every frame kept
- Batch conversion across multiple files or whole folders
- Original and converted file sizes reported after each conversion
- Saved presets of your own, and settings remembered between launches

## What "every option" means

The claim is tested, not assumed: for each format, a test runs the reference tool and
Skidbladnir on the same input with the same options, across the whole option surface and
across the inputs the tool reads differently (PNG colour types and chunks, JPEG chroma
samplings, CMYK, metadata), and requires identical bytes. CI builds each reference from the
same source as the library the app links (`scripts/build-reference-tools.sh`). Precisely:

- **Still images**, and animated WebP out of an animated WebP or a GIF, as `img2webp` and
  `gif2webp` write it. Other options about more than one image are not offered yet:
  animated AVIF, HEIC or JPEG XL, image sequences, `avifenc --layered` and grids assembled
  from several files, `heif-enc` with several inputs or `-T` tiled input, and `cjxl` from
  GIF or APNG. They are queued in [ROADMAP.md](ROADMAP.md), Phase 8.
- **The input formats Skidbladnir reads**: PNG, JPEG, TIFF, WebP, AVIF, JPEG XL, HEIC,
  GIF, PNM/PAM and PFM. Some of the tools also read PGX, Y4M, EXR or raw pixels, which
  Skidbladnir does not yet. PNM is read the way `cwebp` reads it for WebP and the way
  `cjxl` reads it for JPEG XL (at its own bit depth); PFM is read as `cjxl` reads it, and
  only `cjxl` reads PFM, so PFM to another format has no reference to match. JPEG is read through libjpeg-turbo 3.2.0, the way each
  tool reads it — including `avifenc`'s and `heif-enc`'s copying of a JPEG's own YCbCr
  planes — and PNG by libpng 1.6's rules, including `cwebp`'s gamma correction. A tool
  linked against a different libjpeg or libpng can decode the same file differently.
- **The same encoder versions**: libwebp 1.6.0, libavif 1.4.2 with libaom 3.15.1, libjxl
  0.12.0, libheif 1.23.5 with Kvazaar 2.3.2. Another version of a tool writes other bytes,
  as it would against itself.
- **AVIF with libaom**, `avifenc`'s default codec: `-c rav1e` and `-c svt` are not built
  in. JPEG gain-map conversion needs `avifenc` built with libxml2, which it is not by
  default, and is not offered.
- **HEIC with Kvazaar** in the standard edition, the BSD-licensed HEVC encoder; the
  default `heif-enc` build uses x265, which is GPL, and is the [GPL edition's](#editions)
  encoder. `heif-enc -L` fails with Kvazaar, so lossless is Kvazaar's own
  (`-p lossless=true`), and 16-bit PNGs, which `heif-enc` hands Kvazaar at 10 bits and
  Kvazaar refuses, are encoded from their high bytes instead. The GPL edition's x265
  output is checked by decoding it, not byte for byte.
- Options that change how a tool runs but not the file — verbosity, timing, benchmarks,
  printing statistics — have no control, and neither do `cwebp`'s dump and map outputs.

Where a tool refuses an input — `cwebp` a CMYK JPEG, `heif-enc` an RGB-coded one —
Skidbladnir converts it anyway. One result is deliberately not reproduced: `cwebp` reads a
TIFF with straight (unassociated) alpha premultiplied, through libtiff, and darkens its
semi-transparent pixels; Skidbladnir keeps the colours the file holds.

## Install

Grab a build from the [releases page](https://github.com/basic-automation/Skidbladnir/releases).

Each release carries a Linux `.deb` and AppImage, a Windows installer, and macOS `.dmg`s
for Apple Silicon and Intel, in two [editions](#editions): `Skidbladnir_*` is the
standard edition and `Skidbladnir-GPL_*` the GPL edition. Releases are still marked
**prerelease**; see Status for which builds have actually been run.

Once installed, Skidbladnir checks for a newer release each time it starts and offers it
in a banner; nothing is downloaded until you click **Install and restart**, and every
update is verified against the project's signing key before it is installed. A `.deb`
install updates with a `.deb` (and asks for your password to do it); an AppImage, the
Windows installer and the macOS app update themselves in place. Each edition updates only
to the same edition; to switch, install the other one over it. Releases before 0.8.0
have no updater, so moving off them is a manual download, once.

### Editions

The two editions are the same app except for how they write HEIC, and so for their
licence:

| | Standard (`Skidbladnir_*`) | GPL (`Skidbladnir-GPL_*`) |
|---|---|---|
| HEIC encoder | Kvazaar (BSD-3-Clause) | x265 (GPL-2.0-or-later) |
| HEIC controls | every `heif-enc` option Kvazaar honours | those, plus `-L` lossless, chroma, 10-bit, and x265's preset, tune and rate controls |
| Licence of the build | ISC | GPL-3.0-or-later, as a whole |

The GPL edition exists because x265 is the better encoder, and x265 is GPL: an app that
ships it is distributed under the GPL. Skidbladnir's own source is ISC either way. Each
release attaches `Skidbladnir-<version>-source.tar.gz`, the complete source with every
submodule, which is what the GPL edition's licence requires to be offered with it.
Everything but HEIC — WebP, AVIF, JPEG XL, and HEIC input — is identical in both.

Skidbladnir used to be an Electron app for Windows. That app has been retired: its last
binary release is [v0.4.3](https://github.com/basic-automation/Skidbladnir/releases/tag/v0.4.3)
(2019), and its last source is the
[`electron-final`](https://github.com/basic-automation/Skidbladnir/tree/electron-final) tag.

## Build from source

Requires a stable Rust toolchain, Node.js and npm, **`nasm`** — libaom, libjpeg-turbo and
the AV1 decoder assemble their SIMD code with it, and x265 in the GPL edition (Arch:
`nasm`; Debian/Ubuntu: `nasm`; macOS: `brew install nasm`; Windows: `choco install nasm`)
— and **CMake** with a C++ compiler, which build libjxl, libavif with libaom, and
libjpeg-turbo from source so they are linked statically. Clone with
`--recurse-submodules`: those sources are pinned under `third_party/`. HEIC needs libheif,
built with `scripts/build-libheif.sh` from the same pinned sources into `build/libheif`;
without that build, the system libheif is linked instead. On Linux you also need
`webkit2gtk-4.1` and its development headers.

The GPL edition is the `gpl` feature (or `full`, every optional feature), with a libheif
built around x265 into `build/libheif-gpl`. The build refuses to link either edition
against the other's libheif.

```bash
scripts/build-libheif.sh --edition gpl
cd src-tauri && ../frontend/node_modules/.bin/tauri build --features gpl --config tauri.gpl.conf.json
```

`tauri.gpl.conf.json` (and on Windows `tauri.gpl.windows.conf.json` too) bundles x265's
libheif, the GPL edition's notices and the GPL text, and points the updater at the GPL
edition's releases.

Without CMake, the build links the system libjxl, libavif and libjpeg through
`pkg-config` instead, and says so. Set `SKIDBLADNIR_LIBJXL`, `SKIDBLADNIR_LIBAVIF` or
`SKIDBLADNIR_LIBJPEG` to `vendored` to insist on the static build, or to `system` to insist
on the system library.

```bash
npm --prefix frontend install        # also installs the Tauri CLI
cd src-tauri && ../frontend/node_modules/.bin/tauri build --no-bundle
```

Run it with `../frontend/node_modules/.bin/tauri dev` from `src-tauri/`, which starts the
Nuxt dev server and the window together. Release builds must go through `tauri build`
rather than `cargo build --release`: the `custom-protocol` feature that embeds the
frontend is only enabled by the former.

The encoders are the libraries themselves, linked into the binary — there is no command-line
tool to download and no subprocess.

### Checking a build

```bash
cargo test --workspace     # includes the encoder-parity tests
scripts/smoke-test.sh      # drives the built window: renders, IPC, a real conversion
scripts/a11y-audit.sh      # axe-core against the live window
```

The parity tests compare each encoder with its reference tool and need them built from
the same sources as the libraries the app links, reading JPEG through the same
libjpeg-turbo: `scripts/build-reference-tools.sh` builds `cwebp`, `img2webp`, `gif2webp`,
`avifenc`, `cjxl` and `heif-enc` and prints the `SKIDBLADNIR_REFERENCE_*` variables that
point the tests at them. The decoding checks use libavif's `avifdec` and `avifenc`,
libjxl's `djxl` and libheif's `heif-dec` and `heif-info` from `PATH`. Missing tools make
the tests say so and skip rather than pass quietly; set `SKIDBLADNIR_REQUIRE_PARITY=1` to turn a skip
into a failure.

The two scripts need `tauri-driver` (`cargo install tauri-driver`) and `WebKitWebDriver`
(`webkit2gtk-driver` on Debian/Ubuntu, `webkitgtk-6.0` on Arch). They skip, loudly, when
those are missing.

## Status

Skidbladnir is a **Rust + Tauri 2** app with a **Nuxt + Tailwind** frontend, for Windows,
Linux and macOS. It replaced an Electron app, now retired. The work queue lives in
[ROADMAP.md](ROADMAP.md).

What it can do:

- Encode each format with its reference encoder's library linked into the binary, with
  every option of the reference command line for a still image, and prove the output
  **byte-for-byte identical** to `cwebp`, `img2webp`, `gif2webp`, `avifenc`, `cjxl` and
  `heif-enc` across 3,332 cases (see above).
- Convert images from a window with every one of those options, labelled with the flag
  it sets, the expert ones in folding sections.
- Accept files dropped onto the window, sorting out the ones it cannot read by looking
  at their contents rather than their file extension.
- Convert a batch of files, showing progress per file, with a Cancel button that stops
  without leaving a half-converted image behind.
- Convert a whole folder, optionally including its subfolders and recreating their
  structure in the destination.
- Write **AVIF** as well as WebP, with its own panel of controls. Each format keeps its
  own settings while you try another. AVIF output is also checked by decoding it with
  the runner's `avifdec`, measuring how close the pixels come back, and confirming
  transparency survives.
- Write **JPEG XL**, ahead of browsers enabling it by default, so a library can be
  converted in anticipation. JPEGs are recompressed losslessly by default. JPEG XL output
  is checked by libjxl's own `djxl`: it must decode our files at the right size and
  alpha, lossless files exactly, and rebuild a recompressed JPEG byte for byte.
- Preview the result beside the original, in any format, before anything is written to
  disk. AVIF and JPEG XL previews are decoded in Rust, so they show on every web engine.
- Tell you what an existing WebP already is — size, lossy or lossless, alpha, animated.
- Re-encode an **animated WebP** as an animated WebP, keeping every frame, its timing and
  its loop count, with every WebP setting applied to each frame, and preview it as an
  animation before converting. Its output is byte-for-byte identical to libwebp's own
  `img2webp` for every setting `img2webp` can express.
- Convert **GIFs**, still or animated, to WebP exactly as libwebp's own `gif2webp` does —
  byte for byte, across disposal methods, transparency, timing and loop counts, and
  keeping a GIF's ICC profile and XMP when you ask, as `gif2webp -metadata` does.
- Remember your settings and destination between launches, and save named presets of
  your own.
- Be driven entirely from the keyboard, with every control labelled for a screen reader.
- A frameless window with a format rail, a sidebar for the queue, destination and
  presets, and one column of settings with Convert at its head.
- Write **HEIC** through libheif with Kvazaar, a BSD-licensed HEVC encoder, and in the
  GPL edition with x265 — lossless, 4:4:4 and 4:2:2, 10-bit, and x265's tuning — and read
  HEIC input with libde265. libheif ships as a separate shared library beside the app,
  with its codec plugins switched off, so the standard edition can never pick up x265
  from your system. Checked by the runner's own libheif (`heif-dec`), a separate build.
- Read PNG, JPEG, TIFF, WebP, **AVIF**, **JPEG XL**, **HEIC**, **GIF**, **PNM/PAM** and
  **PFM** input,
  identifying the format by its contents rather than by its file extension. JPEGs are
  decoded by libjpeg-turbo, as the reference tools decode them. That includes **CMYK
  JPEGs** as Photoshop writes them, which `cwebp` itself refuses to read. AVIF input is
  checked against libavif's own `avifdec` across 17 colour encodings (bit depths, chroma
  subsampling, range, colour matrices, alpha), and an AVIF's crop, rotation and mirror are
  applied, so a sideways-stored portrait converts upright. Tiled (grid) AVIFs, as some
  cameras write large captures, are decoded whole. JPEG XL input is checked against
  libjxl's `djxl`.
- Keep a photo's **ICC colour profile, EXIF and XMP** as each reference tool does: in WebP
  when you ask, exactly as `cwebp -metadata` does (off by default, as in `cwebp`); in AVIF,
  JPEG XL and HEIC by default, as `avifenc`, `cjxl` and `heif-enc` do, with each kind
  switchable off. Checked against `cwebp` itself and by libavif's `avifdec`, libjxl's
  `djxl` and libheif's `heif-info`.
- Convert **JPEG, PNG, TIFF and WebP files to WebP byte for byte as `cwebp` itself
  would**, across every setting: JPEGs are decoded with libjpeg-turbo, 16-bit files are
  reduced to 8 bits the way `cwebp`'s libpng or libtiff reduces them, PNG gamma is
  corrected as libpng corrects it for `cwebp`, and WebP sources take `cwebp`'s route
  straight to YUV; a TIFF with premultiplied (associated) alpha is un-multiplied exactly as
  `cwebp` un-multiplies it. One deliberate exception: a TIFF with straight (unassociated) alpha
  keeps its colours, where `cwebp` darkens its semi-transparent pixels.
- Refuse to overwrite your source image, and stage every write through a temporary
  file so a failed conversion cannot damage a file that was already there.
- Report the before and after sizes, and the dimensions actually produced.
- Build and pass its tests on Windows, Linux and macOS in CI.

Known gaps:

- The macOS `.dmg`s (Apple Silicon and Intel) are built by CI but **have never been
  launched** — treat them as untested. The Windows installer is installed and exercised
  by CI on every change, and the Linux `.deb` and AppImage have been run.
- No AppImage is produced on the maintainer's machine, because bundling one needs
  `patchelf`, which is not installed there. CI has it.
- The app is not code-signed on any platform.
- An AVIF, JPEG XL or HEIC encode cannot be cancelled mid-file: none of those encoders reports progress, so
  Cancel takes effect when the current file finishes (which is then discarded, not
  written). Animated AVIF is not read.
- Metadata is kept in still images, and a GIF's ICC profile and XMP in its animated WebP;
  an animated WebP re-encoded keeps none (`img2webp`, its reference, copies none).
- An animation (animated WebP or GIF) converts to WebP only. AVIF, JPEG XL and HEIC
  output refuse it by name rather than keeping just the first frame, because those
  encoders write still images here. A target size or PSNR applies to each frame of an
  animation, not to the whole file.
- JPEG XL files open in Safari, and in Firefox and Chrome as each enables it by default;
  until then, most web pages cannot show them. The long-promised JPEG 2000 was dropped
  as a goal; it has no momentum outside medical and archival imaging.
- HEIC is HEVC, which is covered by patent pools; Skidbladnir ships an open-source HEVC
  encoder and decoder as GIMP, ImageMagick and ffmpeg do. The standard edition writes
  HEIC 8-bit 4:2:0 only (Kvazaar), so its lossless coding is lossless after the
  conversion to 4:2:0, as with `heif-enc`; `-L` lossless, 4:4:4 and 10-bit take the GPL
  edition. Neither writes 12-bit HEIC.

## License

Skidbladnir's source is ISC ([LICENSE](LICENSE)), as declared in the workspace
`Cargo.toml`. The standard edition's builds are ISC too. The GPL edition's builds
include x265, so each of them is distributed as a whole under the GNU GPL, version 3 or
later ([LICENSES/GPL-3.0.txt](LICENSES/GPL-3.0.txt)); see [Editions](#editions).

The native libraries each edition ships, and their licences, are listed in
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) and
[THIRD-PARTY-NOTICES-GPL.md](THIRD-PARTY-NOTICES-GPL.md), and every installer carries its
own edition's copy with its licence texts.
