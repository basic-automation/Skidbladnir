# Skidbladnir — Roadmap

## Phase 0 — Foundations (make the repo routine-ready)

The repo is routine-ready. It has this roadmap, a consumer-facing README, `/scratch` ignored and lockfiles committed. A CI workflow checks formatting, runs clippy `-D warnings`, and builds and tests on Linux, Windows and macOS. It also builds the Nuxt frontend, which `tauri::generate_context!` needs at compile time, and caches that build. `rustfmt.toml` and the workspace clippy lints match the owner's other repos. Only rustfmt runs on nightly, because its import options are nightly-gated; everything that compiles stays on stable, and `clippy::nursery` is left out on purpose.

Decided: the product is **Skidbladnir** (`com.basicautomation.skidbladnir`; binary and crate `skidbladnir`). `ba-nextgenimg`, `skidblad` and `ba | Next Generation Image` were rejected. `copilot/add-resize-control-option` turned out to be merged already, so there was nothing to land. The 2021 Vue 3 branch `origin/Skidbladnir` is dead and kept only for archaeology. Details: [Phase 0 notes](https://basicautomation.io/projects/skidbladnir/docs/phase-0-notes).

## Phase 1 — Tauri 2 + Nuxt + Tailwind scaffold

A Cargo workspace was scaffolded beside the Electron app: `crates/*` with no Tauri dependency, `src-tauri/` as a thin IPC shell, and `frontend/` for Nuxt 4 + Tailwind 4. The first `cargo tauri build` produced a runnable Linux binary, and the dev window rendered the Nuxt shell; both were checked over WebDriver. Two traps were recorded. The before-commands run from the repo root. Release builds must go through `cargo tauri build`, or the `custom-protocol` feature is off. tauri-cli would not build on the host's nightly, so it is installed on stable. Installing `patchelf` on the dev host became unnecessary once CI built the AppImage.

The CSP is `'self'` everywhere. `script-src 'unsafe-inline'` went without the planned build-time hashing, because Tauri covers its own inline scripts: a hash script was written, then deleted as redundant, and enforcement was verified in the running window. The smoke test runs in CI on Linux (xvfb, `tauri-driver`) and on Windows (msedgedriver, pinned to `windows-2022`, four traps recorded). tauri-driver has no macOS support. Details: [Phase 1 notes](https://basicautomation.io/projects/skidbladnir/docs/phase-1-notes).

## Phase 2 — Encode core in Rust (the real work)

The encode core is `libwebp-sys` binding `WebPConfig`, built from source and statically linked, with no vendored binaries. The `image` crate was rejected because it cannot reach the advanced knobs; the `cwebp` sidecar stays the documented fallback. Every control of the Electron UI was ported, from mode and preset through filter, target size/PSNR, sharp YUV, resize and multi-threading. 76 settings are byte-identical to `cwebp` 1.6.0 under a mutation-tested gate. CI enforces it against a version-matched `cwebp` on Linux and macOS, and only reports it on Windows. Input is identified by content, real sizes come back to the UI, and writes are staged so they never overwrite the source. That closes a data-loss bug the Electron app had.

Testing parity per input format found real divergences, each fixed and gated. 16-bit PNG is now truncated as libpng does. JPEG decodes through libjpeg-turbo, and 456 of 456 cases match, up from 17. WebP sources decode to YUV as `cwebp` does (304 of 304). 16-bit TIFF is rounded as libtiff does, and associated-alpha TIFF is un-multiplied as `cwebp` does. CMYK JPEG is not a parity question, because `cwebp` refuses it. Our decode is pinned to ImageMagick's colours, with or without an Adobe marker. Decided against copying `cwebp`: an unassociated-alpha TIFF keeps its colours. Details: [Phase 2 notes](https://basicautomation.io/projects/skidbladnir/docs/phase-2-notes).

## Phase 3 — Frontend parity (Nuxt + Tailwind)

Every Electron control is in the Nuxt window with the same ranges, and its defaults are fetched from the Rust core. Files are chosen through the dialog plugin, with no webview filesystem permission, or dropped through Tauri's native drag-drop event. The drop gesture itself cannot be verified over WebDriver. The window has the mode-dependent control groups, the advanced disclosure, validation by the Rust core and per-file progress. libwebp has no guaranteed 100% callback, so completion comes from the command returning. A before/after size table follows each run.

The help-text item's premise was wrong: the Electron `-info` elements were value readouts, so the help comes from `cwebp -longhelp`. Theming follows the owner's directive: the Electron layout in the Palenight palette, on Nuxt UI 4 (chosen over shadcn-vue), dark only, with the Lucide icons bundled so they work offline. Details: [Phase 3 notes](https://basicautomation.io/projects/skidbladnir/docs/phase-3-notes).

## Phase 4 — Beyond parity

Batch conversion has per-file status and cancel. Cancel works through the progress callback, and it needed `convert_image` to be `async`; that was measured, not assumed. The preview encodes in memory and returns `data:` URLs. It was ticked before any panel called it; the panel exists now. Settings persist through a loader that never fails. Named presets share one file, so a preset name can never become a path. Folder input is bounded (no symlinks, depth 32, 10,000 files) and mirrors the source tree. The window warns about duplicate outputs and has a Replace toggle, a `cwebp`-defaults reset and an About/licence view. Accessibility work brought all 32 controls a name and a clean axe-core audit, which CI runs. The audit found contrast failures in Palenight's own colours.

GIF input matches `gif2webp` byte for byte (49 cases). Metadata is kept in every format. WebP matches `cwebp -metadata` in 84 of 84 cases, and JPEG XL and HEIC are checked by their reference decoders. AVIF kept EXIF first, then ICC and XMP once Phase 8 moved it to libavif. Metadata from GIF and animated-WebP sources is held to `gif2webp` and `webpmux`. Two bugs were fixed on the way: opaque HEIC output had an alpha channel, and a test helper had made the WebP `-metadata all` cases vacuous.

`release.yml` builds the `.deb`, AppImage, NSIS installer and `.dmg` (Apple Silicon and Intel), and has a dry-run mode. v0.5.0 was the first tri-platform release. The installed Windows app passes the smoke test in CI, and releases carry only CI's artifacts. The Tauri updater was first ruled out for 0.x, then adopted at the owner's request on three conditions: owner-held minisign keys, nothing silent, and per-bundle manifests on an `updater` branch. Copies at 0.7.0 or earlier must be updated by hand once. Using the prebuilt Tauri CLI cut the window CI job from 10m42s to 1m41s. Details: [Phase 4 notes](https://basicautomation.io/projects/skidbladnir/docs/phase-4-notes).

- [ ] **The comparison view — the best image comparison there is**
- [ ] "Basic Automation", not "basicautomation", as the Windows publisher
- [ ] First launch of the **macOS** build

### Distribution, deferred past the 1.0.0 launch

Shipped: AppStream metainfo for each edition (validated; every version bump adds a `<release>`), and an `.rpm` for each edition, which CI installs with dnf.

- [ ] (owner-gated) Code signing
- [ ] winget (the standard edition only, or give the GPL edition its own identifier first) and Scoop (a bucket of our own now; Extras wants 100+ stars)
- [ ] AUR `skidbladnir-bin` and AppImageHub (the AppStream metainfo is now in place)
- [ ] Flathub
- [ ] A Homebrew cask (after notarization; the official tap also wants 225 stars), Snap (the owner registers the name), and the Microsoft Store (a signed, offline installer)
- [ ] (owner decision) A supported way for packagers to turn the update check off
- [ ] More platforms: a universal `.dmg`

## Phase 6 — Retire Electron

The Electron app was retired once the installed Windows Tauri build passed the smoke test in CI (2026-09-26) and the owner approved it. Its sources, the root `package.json` and the README's Electron build section were removed, and the icon master moved to `resources/icons/`. The last Electron binary stays v0.4.3, and the `electron-final` tag holds its last source. No new Electron build was made, because none could be verified. Details: [Phase 6 notes](https://basicautomation.io/projects/skidbladnir/docs/phase-6-notes).

## Phase 7 — Beyond WebP

AVIF was chosen as the next format, and the JPEG 2000 promise was struck from the README. JPEG XL stayed a watch item until the owner asked for it ahead of Chrome. AVIF output is shaped as `EncodeJob` with a per-format sub-struct, all kept live; an enum and a flat struct were rejected. The encoder went from `libavif-sys` (unmaintained) to `ravif` 0.13, gated by `avifdec` decoding our output. Once `nasm` was available, rav1e's `asm` came back on with byte-identical output. AVIF input decodes through `avif-decode`/rav1d and agrees with `avifdec` across 17 encodings. It honours `clap`/`irot`/`imir` and grid images, and the preview is decoded in Rust so it shows on every platform. Found on the way: `avif-parse` panics on malformed boxes (the panic is now caught), rav1d cannot link into a `cdylib`, and Ubuntu's libavif 1.0.4 ignores the transform properties.

Animated WebP is inspected without decoding. It used to be refused with a clear message (an earlier claim that it kept only the first frame was wrong), and it is now re-encoded byte-identical to `img2webp`. AVIF output of an animation is refused by name. HEIC shipped on libheif with Kvazaar, with plugin loading off. A separate GPL edition uses x265 with its full control set, and both editions are built and released. HEVC patent licensing is an accepted risk that has not been examined. JPEG XL output and input shipped on libjxl, bound directly because `jpegxl-rs` is GPL, with lossless JPEG recompression and decoding by `jxl-oxide`. libwebp 1.6.0 was confirmed current. Two fixes on the way: concurrent HEIC encodes broke libheif's memory accounting, so the reader's context now lives until its image is released. Windows also linked two C runtimes; everything is now on the static runtime, a problem CI's `RUSTFLAGS` had hidden. Details: [Phase 7 notes](https://basicautomation.io/projects/skidbladnir/docs/phase-7-notes).

- [ ] **Chrome turns JPEG XL on by default — update the README when it ships.**
- [ ] Watch **Tauri 3**, do not adopt it
- [ ] **Drop `macos-private-api`.**
- [ ] Animated **AVIF** output (AVIF image sequences)
- [ ] **Intermittent stack overflow in the animation tests on Windows CI**

## Phase 8 — Reference-CLI parity

The claim holds for still images: every `cwebp`, `cjxl`, `avifenc` and `heif-enc -e kvazaar` option is a control. AVIF moved to libavif + libaom, and the `avifenc` and `heif-enc` still paths were ported as C shims. The parity cases are 246 for WebP, 202 for JPEG XL, 191 for AVIF and 147 for HEIC. CI's `parity` job requires all 2,861 of them, against references built from the app's own sources, with JPEG decoded the way each tool decodes it. This work was merged with 0.12 and 0.13 and shipped as 0.14.0. Parity also runs on macOS, and on Windows with MSVC builds of every reference. The Windows run found libheif built without sharp YUV, and debug-quality builds of libaom, libavif, libjpeg-turbo and libjxl. All are fixed.

New inputs are each held to the tool that reads them: PNM/PAM, PFM, PGX and raw YUV; EXR with extra channels; JPEG XL sources with their IPTC and `jhgm` boxes; Y4M into AVIF and into HEIC; a still GIF into `cjxl`; and TIFF, WebP and HEIC into HEIC. Animated WebP's own options and `img2webp`'s per-frame options became controls, and the GPL edition takes free-form x265 parameters.

Decided: TIFF colours are correct by default, with an opt-in to reproduce the tools. `heif-enc --raw` has nothing to reproduce. Where a tool misreads a file (12-bit PGX, non-4:2:0 Y4M into HEIC), the file is read as meant, with no reference to match. The Windows EXR mismatch was traced to uninitialised memory in libjxl's `exr.cc`, and the EXR gate now runs under `MALLOC_PERTURB_`. A lossy JPEG XL made lossless matches only in optimised builds. Details: [Phase 8 notes](https://basicautomation.io/projects/skidbladnir/docs/phase-8-notes).

- [ ] **libjxl 0.12.0 fails a lossless float encode at effort 3**
- [ ] JPEG XL to JPEG XL, not matched yet
- [ ] HTJ2K-compressed EXR
- [ ] Animated GIF and APNG into `cjxl` (with the multi-image work below)
- [ ] Report libjxl's uninitialised EXR border upstream (an issue on libjxl/libjxl; none found by searching its tracker on 2026-10-04)
- [ ] Build libjxl from a pinned source with our own CMake call instead of `jpegxl-src`'s

### Multi-image features the tools have, deferred by the owner

- [ ] Animation and image sequences
- [ ] Several inputs into one file
- [ ] `heif-enc` metadata tracks, MIME items, and the other experimental item types

### Formats

SVG and `.svgz` input shipped through `resvg`. It draws at the file's own size, or at the resize or crop size. It never opens a file that the SVG names, and it is capped at 100 megapixels.

- [ ] **SVG output**

## Cross-cutting

The gpui window has been the default since 1.3.0, with the Nuxt webview kept as a fallback, and every UI change goes into both. CI opens it on Linux (lavapipe on Xvfb), on Windows (WARP) and on macOS. Automated audits read it the way a screen reader does, through AT-SPI and UI Automation. They found that no text field reached a screen reader and that a select's value did not reach AT-SPI; both are fixed. CI also tests keyboard reach, drops over X11, and real Explorer drops on Windows.

The saving percentage is computed only in Rust. Dependencies live in `[workspace.dependencies]` and are kept current; `vue-router` 5 was deliberately not taken. `cargo deny` runs in CI and fails only on unmaintained crates the workspace chose itself. Dependabot is configured, and screenshots are captured by a script in CI. Third-party notices cover the font and icons, the Rust crates (through cargo-about), the JavaScript packages that ship and the Ubuntu libraries inside the AppImage. The AppImage check found 10 packages whose licences had never shipped. The README gained acknowledgements and a dated comparison table. Details: [Cross-cutting notes](https://basicautomation.io/projects/skidbladnir/docs/cross-cutting-notes).

- [ ] A person using the gpui window with a screen reader
- [ ] A drop onto the gpui window on Wayland (a native Wayland drag source) is still untested
- [ ] AccessKit's AT-SPI mapping reports every button enabled
- [ ] The gpui window on macOS and Windows has not been opened by a person (CI builds and tests it; the Windows installer smoke runs the webview window)
- [ ] The gpui window's macOS accessibility tree, read by the audit (`AxReader`, 2026-10-04; a reported-only CI step)
- [ ] **On CI's Xvfb, some private AT-SPI sessions never get the window's controls**
- [ ] Re-capture whenever the UI changes materially
- [ ] Dependabot's cargo and npm PRs now fail CI's notices checks until someone regenerates `LICENSES/third-party/rust-crates.md` or `javascript.md`
- [ ] Two Dependabot alerts with no clean fix (2026-10-01)
- [ ] README: a short demo clip
