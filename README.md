# Skidbladnir

A desktop GUI for converting images to next-generation open-source formats. Today it
drives Google's `cwebp` encoder and exposes its full control surface — not just a
quality slider — so you can tune an encode the way the command-line tool allows,
without memorising the command line.

![screenshot](https://basicautomation.io/ba-nextGenIMG/images/screenshots/nextgenimg-v0-1-1.webp)

## Features

**Encoding modes**

- Lossy, lossless, near-lossless and JPEG-like modes
- Built-in presets: default, photo, picture, drawing, icon, text
- Quality and alpha-quality control
- Target file size or target PSNR instead of a fixed quality

**Expert controls**

- Compression method and segment count
- Spatial noise shaping (SNS)
- Filter strength, filter sharpness, strong/simple filtering and auto-filter
- Multi-pass encoding
- Partition limit
- Sharp YUV (higher-quality RGB→YUV conversion)
- Low-memory mode
- Resize on the way out (width and height)
- Multi-threading

**Workflow**

- Batch conversion across multiple input files
- Original and converted file sizes reported after each conversion
- Advanced options hidden behind a disclosure so the common path stays simple

## Install

Grab a build from the [releases page](https://github.com/basic-automation/Skidbladnir/releases).

Builds are currently **Windows only**.

## Build from source

The repository currently holds two applications: the Electron app that ships today,
and the Rust + Tauri 2 app replacing it. Both build from a clean checkout.

### The Tauri app (in progress)

Requires a stable Rust toolchain, Node.js and npm. On Linux you also need
`webkit2gtk-4.1` and its development headers.

```bash
cargo install tauri-cli --locked --version '^2'
npm --prefix frontend install
cargo tauri build --no-bundle
```

Run it from the workspace root with `cargo tauri dev`, which starts the Nuxt dev
server and the window together. Release builds must go through `cargo tauri build`
rather than `cargo build --release`: the `custom-protocol` feature that embeds the
frontend is only enabled by the former.

The encoder is libwebp itself, linked into the binary — there is no `cwebp` to
download and no subprocess.

### The Electron app (shipping today)

Requires Node.js and npm.

```bash
npm install
```

It shells out to the `cwebp` binary, which is not vendored in this repository.
Download the [WebP precompiled
binaries](https://developers.google.com/speed/webp/docs/precompiled) and place
`cwebp.exe` at `./resources/win/bin/cwebp.exe`.

Then:

```bash
npm start          # run the app
npm run dist       # build a distributable into ./dist
```

## Status

The Electron app remains the shipping application. The **Rust + Tauri 2** rewrite,
with a **Nuxt + Tailwind** frontend and targeting Windows, Linux and macOS, now has a
working window and a complete encode core; the work queue lives in
[ROADMAP.md](ROADMAP.md).

What the new app can already do:

- Encode with libwebp linked directly into the binary, exposing every control listed
  above — and its output is **byte-for-byte identical to `cwebp`** across 76 settings
  spanning the whole control surface, verified by a test that runs both encoders on the
  same pixels and compares the result.
- Convert images from a window with **every encoder control above** exposed, grouped
  the way the Electron app grouped them, with the advanced controls behind a
  disclosure and shown only for the mode that actually uses them.
- Accept files dropped onto the window, sorting out the ones it cannot read by looking
  at their contents rather than their file extension.
- Convert a batch of files, showing progress per file, with a Cancel button that stops
  without leaving a half-converted image behind.
- Preview the result beside the original before anything is written to disk.
- Tell you what an existing WebP already is — size, lossy or lossless, alpha — and warn
  when a file is an animation it cannot re-encode.
- Remember your settings and destination between launches, and save named presets of
  your own.
- Look like Skidbladnir: the original layout — dot-textured header and footer bands,
  dotted option groups, two-column controls and the circular action button — restyled
  in the Palenight palette.
- Read PNG, JPEG, TIFF and WebP input, identifying the format by its contents rather
  than by its file extension.
- Refuse to overwrite your source image, and stage every write through a temporary
  file so a failed conversion cannot damage a file that was already there.
- Report the before and after sizes, and the dimensions actually produced.
- Build and pass its tests on Windows, Linux and macOS in CI.

Known gaps:

- The released build is Windows only; no Linux or macOS release has been cut yet.
- No installer or AppImage is produced on Linux yet; bundling needs `patchelf`.
- The Electron app still requires a manually downloaded `cwebp.exe`, and it will
  silently overwrite your original if you convert a WebP into the folder it already
  lives in. The Tauri app refuses that conversion instead.
- WebP is the only output format. JPEG 2000 has been listed as "coming soon" since
  2019 and has not been implemented.
- The screenshot above is the Electron app.

## License

ISC. See [LICENSE](LICENSE) if present, or the `license` field in `package.json`.
