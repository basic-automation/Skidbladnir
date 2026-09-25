# Skidbladnir

A desktop GUI for converting images to next-generation open-source formats. Today it
drives Google's `cwebp` encoder and exposes its full control surface — not just a
quality slider — so you can tune an encode the way the command-line tool allows,
without memorising the command line.

![The Skidbladnir window: a dot-textured header, the mode selector, and the quality and advanced encoder controls](resources/images/screenshot.webp)

*The Tauri app. The Electron app it replaces looks different; see Status below.*

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

Two applications are released from this repository while the migration is under way:

- **The Electron app** — the one to use for production work. Windows only; the last
  release of it is v0.4.3.
- **The Tauri app** — the rewrite, released as a **prerelease**. Linux `.deb` today.
  Windows and macOS installers are not produced yet: those platforms compile and pass
  their tests in CI, but nothing has bundled an installer for them.

Each release says which of the two it contains.

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
- Convert a whole folder, including subfolders, recreating its structure in the
  destination.
- Preview the result beside the original before anything is written to disk.
- Tell you what an existing WebP already is — size, lossy or lossless, alpha — and say
  plainly when a file is an animation it cannot re-encode, rather than failing obscurely.
- Remember your settings and destination between launches, and save named presets of
  your own.
- Be driven entirely from the keyboard, with every control labelled for a screen reader.
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
- The Electron app looks different from the screenshot above, which is the Tauri app.

## License

ISC. See [LICENSE](LICENSE) if present, or the `license` field in `package.json`.
