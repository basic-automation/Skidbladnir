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

Requires Node.js and npm.

```bash
git clone https://github.com/basic-automation/Skidbladnir.git
cd Skidbladnir
npm install
```

Skidbladnir shells out to the `cwebp` binary, which is not vendored in this
repository. Download the [WebP precompiled
binaries](https://developers.google.com/speed/webp/docs/precompiled) and place
`cwebp.exe` at `./resources/win/bin/cwebp.exe`.

Then:

```bash
npm start          # run the app
npm run dist       # build a distributable into ./dist
```

## Status

Skidbladnir is an Electron application. A rewrite onto **Rust + Tauri 2** with a
**Nuxt + Tailwind** frontend — targeting Windows, Linux and macOS — is in progress;
the work queue lives in [ROADMAP.md](ROADMAP.md). The Electron app remains the
shipping application until that port reaches feature parity.

Known gaps in the current release:

- Windows only; no Linux or macOS build.
- The `cwebp` binary is not bundled and must be downloaded manually.
- WebP is the only output format. JPEG 2000 has been listed as "coming soon" since
  2019 and has not been implemented.
- No automated tests and no CI.

## License

ISC. See [LICENSE](LICENSE) if present, or the `license` field in `package.json`.
