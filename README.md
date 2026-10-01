# Skidbladnir

[![CI](https://github.com/basic-automation/Skidbladnir/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/basic-automation/Skidbladnir/actions/workflows/ci.yml?query=branch%3Amaster)
[![Latest release](https://img.shields.io/github/v/release/basic-automation/Skidbladnir)](https://github.com/basic-automation/Skidbladnir/releases/latest)
[![Platforms: Windows, macOS (experimental), Linux](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%28experimental%29%20%7C%20Linux-blue)](#install)

Skidbladnir makes image files smaller by converting them to the modern formats:
**WebP**, **AVIF**, **JPEG XL** and **HEIC**. It is for anyone who wants smaller images for
a website, an archive or a photo library without learning four command-line tools. Pick
your files, see the result beside the original, then convert one image, a batch or a
whole folder.

Free and open source · Windows, macOS (experimental) and Linux · converts on your
computer; the only network request is the in-app update check

**[Download](https://github.com/basic-automation/Skidbladnir/releases/latest)** ·
[Which file to pick](#install) ·
[Project page](https://basicautomation.io/projects/skidbladnir/about)

Each of these formats has an official encoder, and each one is a command-line tool with
dozens of options that are easy to forget and easy to get wrong. Skidbladnir puts those
options in a window, each labelled with the flag it sets, instead of a single quality
slider.

For a still image, every option of `cwebp`, `avifenc` (with libaom), `cjxl` and
`heif-enc` (with Kvazaar) is a control in the window, and Skidbladnir writes the same bytes
as those tools built from the same library versions, checked in CI against the real tools,
with the exceptions listed under [What "every option" means](#what-every-option-means).

![The Skidbladnir window with WebP selected: the format rail, the queue, destination and presets sidebar, and every cwebp option as a control labelled with its flag — compression, targets, presets and lossless levels, transparency, lossy tuning, metadata, performance, crop and resize — with a preview of the result beside the original](resources/images/screenshot.webp)

*WebP selected, with a file previewed: every `cwebp` option, each labelled with its flag.*

<details>
<summary>With AVIF selected</summary>

![The Skidbladnir window with AVIF selected: avifenc's options as controls labelled with their flags — colour and alpha quality, speed, lossless and target size, then bit depth, YUV format and range, premultiplied alpha and sharp YUV, then colour signalling, content light level, and the ICC profile, Exif and XMP taken from the input, left out or read from a file](resources/images/screenshot-avif.webp)

</details>

## Features

**Input formats**

- PNG, JPEG (including Photoshop's CMYK), TIFF, **AVIF**, **JPEG XL** and **HEIC**, as
  still images
- WebP and **GIF**, still or animated
- **PNM**: binary PGM, PPM and PAM (8 or 16 bits, with or without alpha), and **PFM**
  floating-point images
- Each file's format is recognised from its contents, not its extension. An AVIF's crop,
  rotation and mirror are applied, so a sideways-stored portrait converts upright, and
  tiled (grid) AVIFs, as some cameras write large captures, are decoded whole.

**Output formats, each with its reference encoder's whole command line for a still image**

| Format | Reference tool | Encoder in the app | Byte-for-byte parity, tested |
|---|---|---|---|
| WebP | `cwebp` 1.6.0 | libwebp 1.6.0 | 2,673 cases: every option, through PNG (every colour type, gamma), JPEG, TIFF (including premultiplied alpha), WebP and PNM files, and `-metadata` |
| Animated WebP | `img2webp`, `gif2webp` and `webpmux` 1.6.0 | libwebp 1.6.0 | 97 cases: animated WebP and GIF input, every Skidbladnir setting those tools can express, and metadata (`gif2webp -metadata`, `webpmux -set`). Their `-mixed`, `-min_size`, `-kmin`/`-kmax`, `-loop`, `-loop_compatibility` and per-frame options are not offered yet |
| AVIF | `avifenc` 1.4.2 | libavif 1.4.2 + libaom 3.15.1 | 191 cases: every option, PNG inputs, JPEG input |
| JPEG XL | `cjxl` 0.12.0 | libjxl 0.12.0 | 254 cases: every option, PNG, PNM, PFM and still-GIF inputs, JPEG recompression and decoding |
| HEIC | `heif-enc -e kvazaar` 1.23.5 | libheif 1.23.5 + Kvazaar 2.3.2 | 172 cases: every option, PNG, JPEG, TIFF (RGB and straight alpha), WebP and HEIC inputs |

That is 3,387 cases in all. CI runs every one of them on Linux x86-64, for every pull
request and every push to `master`.

The [GPL edition](#editions) writes HEIC with x265 instead, adding `heif-enc`'s `-L`
lossless, 4:4:4 and 4:2:2 chroma, 10-bit output and a fixed set of x265's tuning controls;
its HEIC is checked by decoding, not byte for byte against a reference.

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
  recompression** — a JPEG repacked typically about 20% smaller (libjxl's figure) and
  rebuildable bit for bit, and the default for JPEG input — and every one of `cjxl`'s
  modular, progressive, filter, colour-space, metadata and container options.
- **HEIC**, the format iPhones use: quality, Kvazaar's lossless coding, chroma
  downsampling, the colour-profile presets and custom code points, thumbnails, alpha and
  premultiplied alpha, rotation and mirroring, tiles, 360° projection, a description,
  compatible brands and the compact `mini` format.
- **Metadata**: a photo's ICC colour profile, Exif and XMP are kept as each reference
  tool keeps them: in WebP when you ask, as `cwebp -metadata` does (off by default, as in
  `cwebp`); in AVIF, JPEG XL and HEIC by default, as `avifenc`, `cjxl` and `heif-enc` do,
  with each kind switchable off.
- Crop and resize (with `-resize_mode`), shared by every format.

**Workflow**

- Preview the result beside the original, in any format, before anything is written to
  disk. AVIF and JPEG XL previews are decoded in Rust, so they show on every platform.
- Animated WebP or GIF in, animated WebP out: every frame, its timing and its loop count
  kept, every WebP setting applied to each frame, and a preview of the animation first.
  A GIF converts as `gif2webp` converts it, across disposal methods, transparency, timing
  and loop counts, keeping its ICC profile and XMP when you ask, as
  `gif2webp -metadata` does.
- Drop files onto the window (any it cannot read are set aside), or convert a whole
  folder, optionally with its subfolders, recreating their structure in the destination.
- Batch conversion with progress per file, and a Cancel button that never leaves a
  half-written image behind.
- The original and converted sizes, and the dimensions produced, reported after each
  conversion; and what an existing WebP already is: size, lossy or lossless, alpha,
  animated.
- Each format keeps its own settings while you try another. Settings and the destination
  are remembered between launches, and you can save named presets of your own.
- Every control shows the flag it sets, with the expert ones in folding sections. The
  whole window works from the keyboard, and every control is labelled for a screen
  reader.
- Your source image is never overwritten, and every write goes through a temporary file,
  so a failed conversion cannot damage a file that was already there. A file already in
  the destination with the output's name (for example `photo.webp` when converting
  `photo.png`, or when `photo.png` and `photo.jpg` are converted together) is replaced, as
  `cwebp -o` would replace it.

## Install

Download the file for your computer from the
[latest release](https://github.com/basic-automation/Skidbladnir/releases/latest)
(`<version>` is the release's version number):

| Your computer | File to download |
|---|---|
| Windows 10 or 11, 64-bit | `Skidbladnir_<version>_x64-setup.exe` |
| Mac with Apple silicon (M1 or later), experimental | `Skidbladnir_<version>_aarch64.dmg` |
| Mac with an Intel processor, experimental | `Skidbladnir_<version>_x64.dmg` |
| Ubuntu 22.04 or later, Debian 12 or later, Linux Mint 21 or later, and distributions based on them | `Skidbladnir_<version>_amd64.deb` |
| Any other 64-bit Linux with glibc 2.35 or later | `Skidbladnir_<version>_amd64.AppImage` |

**Not sure which edition?** Download the standard edition: the files above, named
`Skidbladnir_…` with an underscore. Choose the GPL edition, the same files named
`Skidbladnir-GPL_…`, only if you write HEIC and want the x265 encoder: `heif-enc -L`
lossless, 4:4:4 and 4:2:2 chroma, 10-bit, and x265's preset, tune and a fixed set of its
other tuning controls. Both are free. The licence matters only if you redistribute the
app; [Editions](#editions) has the details.

The release page lists other files too; they are not installers. The `.app.tar.gz` files
are for the in-app updater on a Mac. `Skidbladnir-<version>-source.tar.gz` is the complete
source with every submodule; GitHub's own "Source code" archives leave the submodules out.

There are no builds for 32-bit systems, Linux on ARM or Windows on ARM. Windows on ARM
can run x64 programs through emulation, but nobody has tried Skidbladnir there.
[Status](#status) says which builds have been run, and how.

### Opening it the first time

The installers are not code-signed yet, so Windows and macOS warn before they open an app
from a developer they cannot identify. This is what to expect, and how to get past it.

**Windows.** Your browser may warn that the file is not commonly downloaded. Keep it
anyway. In Edge, open the **...** (More actions) menu beside the download, choose
**Keep**, then **Show more** > **Keep anyway**. In Chrome, choose **Keep** or **Download
suspicious file**. When you run the installer, SmartScreen shows *Windows protected your
PC*: click **More info**, then **Run anyway**. Skidbladnir installs for your account
only, in `%LOCALAPPDATA%\Skidbladnir`, without administrator rights. If Microsoft's
WebView2 runtime is missing (Windows 10 and 11 normally have it), the installer downloads
it. If **Smart App Control** is turned on (Windows Security > App & browser control),
Windows blocks unsigned apps and offers no exception, so Skidbladnir cannot be installed
on that PC until it is signed.

**macOS (experimental).** The macOS builds are new: CI builds them and runs the test suite
on macOS, but nobody has opened the app on a real Mac yet, so please
[report problems](https://github.com/basic-automation/Skidbladnir/issues). Open the `.dmg`,
drag Skidbladnir into Applications, and open it from there.

- On macOS 15 (Sequoia) or later, the first time you open it, macOS says it cannot
  verify the app. Click **Done**, open **System Settings > Privacy & Security**, scroll
  down, click **Open Anyway** beside the message about Skidbladnir, and confirm.
- On macOS 14 or earlier, right-click (or Control-click) Skidbladnir in Applications,
  choose **Open**, then choose **Open** again.
- If macOS says that Skidbladnir *is damaged and can't be opened*, there is no Open Anyway
  button. Remove the quarantine flag your browser set, in Terminal, then open it again:

  ```bash
  xattr -dr com.apple.quarantine /Applications/Skidbladnir.app
  ```

**Linux.** Both packages need glibc 2.35 or later: Ubuntu 22.04, Debian 12 or newer.

- The `.deb` (Ubuntu, Debian, Linux Mint): in the folder you downloaded it to, run
  `sudo apt install ./Skidbladnir_<version>_amd64.deb`. apt installs what it needs
  (WebKitGTK and GTK) and adds Skidbladnir to your applications menu.
- The AppImage (any distribution): run `chmod +x Skidbladnir_<version>_amd64.AppImage`,
  then `./Skidbladnir_<version>_amd64.AppImage`. It adds no menu entry. It needs FUSE: a
  `fusermount3` or `fusermount` program (the `fuse3` or `fuse` package) and `/dev/fuse`;
  libfuse2 is not needed. Where FUSE is not available, run it with
  `--appimage-extract-and-run`.

### Checking a download

The release page lists a SHA-256 checksum beside each file. Work out the checksum of the
file you downloaded and compare the two:

- Linux: `sha256sum Skidbladnir_<version>_amd64.deb`
- macOS: `shasum -a 256 Skidbladnir_<version>_aarch64.dmg`
- Windows, in PowerShell: `Get-FileHash .\Skidbladnir_<version>_x64-setup.exe` (it uses
  SHA-256 unless told otherwise)

If they differ, the file is not the one the release published: delete it and download it
again.

### Updates

Once installed, Skidbladnir checks for a newer release each time it starts and offers it
in a banner; nothing is downloaded until you click **Install and restart**, and every
update is verified against the project's signing key before it is installed. A `.deb`
install updates with a `.deb` (and asks for your password to do it); an AppImage, the
Windows installer and the macOS app update themselves in place. Each edition updates only
to the same edition; to switch, install the other one over it. Releases before 0.8.0
have no updater, so moving off them is a manual download, once.

### Privacy

Skidbladnir has no telemetry and no analytics, and your images never leave your
computer. The app makes one network request of its own: when it starts, it fetches a
small update manifest (`latest.json`, or `latest-gpl.json` for the GPL edition) from
`raw.githubusercontent.com`. Like any web request, that shows GitHub your IP address;
nothing about you or your files is sent. An update is downloaded from GitHub, and
installed, only when you click **Install and restart**.

### Editions

The two editions are the same app except for how they write HEIC, and so for their
licence:

| | Standard (`Skidbladnir_*`) | GPL (`Skidbladnir-GPL_*`) |
|---|---|---|
| HEIC encoder | Kvazaar (BSD-3-Clause) | x265 (GPL-2.0-or-later) |
| HEIC controls | every `heif-enc` option Kvazaar honours | those, plus `-L` lossless, 4:4:4 and 4:2:2 chroma, 10-bit, and x265's preset, tune, TU depth, AQ, psy-rd/psy-rdoq, deblock and SAO (a fixed set, not every `-p x265:` parameter) |
| HEIC checked | byte for byte against `heif-enc -e kvazaar` | by decoding |
| Licence of the build | ISC; ships libheif/libde265 as a separate, replaceable LGPL-3.0 library | GPL-3.0-or-later, as a whole |

The GPL edition exists for people who want x265, the HEVC encoder a default `heif-enc`
build uses, and x265 is GPL: an app that ships it is distributed under the GPL.
Skidbladnir's own source is ISC either way. Everything but HEIC output — WebP, AVIF,
JPEG XL, and HEIC input — is identical in both. In both, libheif ships as a separate
shared library beside the app, with its codec plugins switched off, so the standard
edition can never pick up x265 from your system.

Each release attaches `Skidbladnir-<version>-source.tar.gz`, the complete source with
every submodule, which is what the GPL edition's licence requires to be offered with it.
The Rust crates and npm packages it compiles in are pinned by `Cargo.lock` and
`frontend/package-lock.json` in that archive, and published unmodified on crates.io and
npmjs.com.

## What "every option" means

The claim is tested, not assumed: for each format, a test runs the reference tool and
Skidbladnir on the same input with the same options, across the whole option surface and
across the inputs the tool reads differently (PNG colour types and chunks, JPEG chroma
samplings, CMYK, metadata), and requires identical bytes. CI builds each reference from the
same source as the library the app links (`scripts/build-reference-tools.sh`). Precisely:

- **Still images**, and animated WebP out of an animated WebP or a GIF, as `img2webp` and
  `gif2webp` write it at their default keyframe, mixing and size settings and with the
  source's loop count. Their `-mixed`, `-min_size`, `-kmin`/`-kmax`, `-loop`,
  `-loop_compatibility` and per-frame options are not offered yet. Neither are other
  options about more than one image: animated AVIF, HEIC or JPEG XL, image sequences,
  `avifenc --layered` and grids assembled from several files, `heif-enc` with several
  inputs or `-T` tiled input, and `cjxl` from an animated GIF or APNG. They are queued in
  [ROADMAP.md](ROADMAP.md), Phase 8.
- **The input formats Skidbladnir reads**: PNG, JPEG, TIFF, WebP, AVIF, JPEG XL, HEIC,
  GIF, PNM/PAM and PFM. Some of the tools also read PGX, Y4M, EXR or raw pixels, which
  Skidbladnir does not yet. PNM is read the way `cwebp` reads it for WebP and the way
  `cjxl` reads it for JPEG XL (at its own bit depth); PFM is read as `cjxl` reads it, and
  only `cjxl` reads PFM, so PFM to another format has no reference to match. JPEG is read
  through libjpeg-turbo 3.2.0, the way each tool reads it — including `avifenc`'s and
  `heif-enc`'s copying of a JPEG's own YCbCr planes — and PNG by libpng 1.6's rules,
  including `cwebp`'s gamma correction. For WebP, 16-bit files are reduced to 8 bits the
  way `cwebp`'s libpng or libtiff reduces them, a WebP source takes `cwebp`'s route
  straight to YUV, and a TIFF with premultiplied (associated) alpha is un-multiplied as
  `cwebp` un-multiplies it. A tool linked against a different libjpeg or libpng can
  decode the same file differently.
- **The same encoders, built the same way**: libwebp 1.6.0, libavif 1.4.2 with libaom
  3.15.1, libjxl 0.12.0, libheif 1.23.5 with Kvazaar 2.3.2. Another version of a tool
  writes other bytes, as it would against itself. The app's libavif and the reference
  `avifenc` are both built without libyuv. A packaged `avifenc` (Homebrew, most Linux
  distributions) usually links libyuv, which converts RGB to YUV differently, so it
  writes other bytes even at libavif 1.4.2.
- **Tested on Linux x86-64.** CI runs the parity tests there. The Windows and macOS builds
  link the same sources, but no reference tool runs on those platforms, so identical
  bytes there are expected, not tested.
- **AVIF with libaom**, `avifenc`'s default codec: `-c rav1e` and `-c svt` are not built
  in. JPEG gain-map conversion needs `avifenc` built with libxml2, which it is not by
  default, and is not offered.
- **HEIC with Kvazaar** in the standard edition, the BSD-licensed HEVC encoder; the
  default `heif-enc` build uses x265, which is GPL, and is the [GPL edition's](#editions)
  encoder. `heif-enc -L` fails with Kvazaar, so lossless is Kvazaar's own
  (`-p lossless=true`), and 16-bit PNGs, which `heif-enc` hands Kvazaar at 10 bits and
  Kvazaar refuses, are encoded from their high bytes instead. The GPL edition's x265
  output is checked by decoding it, not byte for byte.
- **WebP's starting settings.** Until you change them, the WebP controls start from
  Skidbladnir's own long-standing defaults (`-af -alpha_filter best -pass 6 -mt`), not
  from what `cwebp` does with no flags. To start where `cwebp` starts, choose `default`
  under **libwebp preset** and click **Apply** (it keeps your quality), then set the flags
  you use.
- Options that change how a tool runs but not the file — verbosity, timing, benchmarks,
  printing statistics — have no control, and neither do `cwebp`'s dump and map outputs.

Where a tool refuses an input — `cwebp` a CMYK JPEG, `heif-enc` an RGB-coded one —
Skidbladnir converts it anyway. Two results differ from the reference tool, both for a
TIFF with transparency:

- **Straight (unassociated) alpha, to WebP**, deliberately: `cwebp` reads such a TIFF
  premultiplied, through libtiff, and darkens its semi-transparent pixels; Skidbladnir
  keeps the colours the file holds.
- **Premultiplied (associated) alpha, to HEIC**, not yet decided: Skidbladnir
  un-multiplies it, as `cwebp` does, and keeps the image's true colours. `heif-enc` takes
  that alpha as straight, so its HEIC is darker where the image is semi-transparent.
  Whether HEIC should keep the true colours or follow `heif-enc` is still open
  ([ROADMAP.md](ROADMAP.md), Phase 8).

Besides the byte comparisons, the output is decoded with each format's own tools. AVIF is
decoded with libavif's `avifdec`, and the tests measure how close the pixels come back and
check that transparency survives. JPEG XL is decoded with libjxl's `djxl`, which must
decode it at the right size and alpha, a lossless file exactly, and rebuild a recompressed
JPEG byte for byte. HEIC is decoded with libheif's `heif-dec` and `heif-info`, from a
separate build; that is how the GPL edition's x265 output is checked. Metadata is checked
against `cwebp` itself and with `avifdec`, `djxl` and `heif-info`. AVIF input is checked
against `avifdec` across 17 colour encodings (bit depths, chroma subsampling, range,
colour matrices, alpha), and JPEG XL input against `djxl`.

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

```bash
npm --prefix frontend install        # also installs the Tauri CLI
cd src-tauri && ../frontend/node_modules/.bin/tauri build --no-bundle
```

Run it with `../frontend/node_modules/.bin/tauri dev` from `src-tauri/`, which starts the
Nuxt dev server and the window together. Release builds must go through `tauri build`
rather than `cargo build --release`: the `custom-protocol` feature that embeds the
frontend is only enabled by the former.

Without CMake, the build links the system libjxl, libavif and libjpeg through
`pkg-config` instead, and says so. Set `SKIDBLADNIR_LIBJXL`, `SKIDBLADNIR_LIBAVIF` or
`SKIDBLADNIR_LIBJPEG` to `vendored` to insist on the static build, or to `system` to insist
on the system library.

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
Linux and macOS, and it builds and passes its tests on all three in CI. Releases are
stable from 1.0.0; the ones before it were prereleases. The work queue lives in
[ROADMAP.md](ROADMAP.md).

Skidbladnir used to be an Electron app for Windows. That app has been retired: its last
binary release is [v0.4.3](https://github.com/basic-automation/Skidbladnir/releases/tag/v0.4.3)
(2019), and its last source is the
[`electron-final`](https://github.com/basic-automation/Skidbladnir/tree/electron-final) tag.

Known gaps:

- **macOS is experimental.** The `.dmg`s (Apple silicon and Intel) are built by CI, and
  the test suite runs on macOS, but nobody has yet opened the app on a real Mac. The
  standard edition's Windows installer is installed and exercised by CI on every change,
  and the Linux app (both editions) is launched and driven by CI on every change, though
  not from its `.deb` or AppImage. The published 0.5.0 `.deb` and AppImage were run
  through that release's smoke test by hand.
- The app is not code-signed on any platform;
  [Opening it the first time](#opening-it-the-first-time) shows how to get past each
  system's warning.
- An AVIF, JPEG XL or HEIC encode cannot be cancelled mid-file: none of those encoders
  reports progress, so Cancel takes effect when the current file finishes (which is then
  discarded, not written).
- An animated AVIF, JPEG XL or HEIC is read as one still image: its primary image or
  first frame.
- Animated AVIF, JPEG XL and HEIC are not written (see below), so an animation's metadata
  is kept in animated WebP only: a GIF's ICC profile and XMP as `gif2webp -metadata` keeps
  them, an animated WebP's ICC profile, Exif and XMP as `webpmux -set` would set them.
- An animation (animated WebP or GIF) converts to WebP only. AVIF, JPEG XL and HEIC
  output refuse it by name rather than keeping just the first frame, because those
  encoders write still images here. A target size or PSNR applies to each frame of an
  animation, not to the whole file.
- JPEG XL files open in Safari, and in Firefox and Chrome as each enables it by default;
  until then, most web pages cannot show them. The long-promised JPEG 2000 was dropped
  as a goal; it has no momentum outside medical and archival imaging.
- The standard edition writes HEIC 8-bit 4:2:0 only (Kvazaar), so its lossless coding is
  lossless after the conversion to 4:2:0, as with `heif-enc`; `-L` lossless, 4:4:4 and
  10-bit take the GPL edition. Neither writes 12-bit HEIC.
- **HEVC patents.** HEIC is HEVC (H.265) inside HEIF, and HEVC is covered by patents
  licensed through patent pools such as Access Advance and Via LA. Skidbladnir includes
  open-source HEVC software: Kvazaar (x265 in the GPL edition) to write HEIC, and libde265
  to read it. The project holds no HEVC patent licence and passes none on. Whether you
  need one depends on where you are and what you do with the files; this is not legal
  advice.

## License

Skidbladnir's own code is ISC ([LICENSE](LICENSE)), as declared in the workspace
`Cargo.toml`, apart from code adapted from the encoders, which keeps its licence:
`crates/skidbladnir-encode/native/avif_shim.c` carries portions of libavif's `avifenc`
under BSD-2-Clause.

The standard edition's own code is ISC; the libraries it ships keep their licences,
including LGPL-3.0 for libheif and libde265, which ship as a separate shared library you
may replace with your own build. The GPL edition's builds include x265, so each of them
is distributed as a whole under the GNU GPL, version 3 or later
([LICENSES/GPL-3.0.txt](LICENSES/GPL-3.0.txt)); see [Editions](#editions).

The native libraries each edition ships, and their licences, are listed in
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) and
[THIRD-PARTY-NOTICES-GPL.md](THIRD-PARTY-NOTICES-GPL.md), and every installer carries its
own edition's copy with its licence texts.
