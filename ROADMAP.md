# Skidbladnir — Roadmap

Single source of truth for the work queue. Every item is a `[ ]`/`[x]` checkbox in
phase order. Tick `[x]` only when the item genuinely shipped and was verified.
Discovered work becomes a new `[ ]` in the right phase. No status tables, no run
logs, no prose essays — git history and the PRs are the record.

**The migration in one line:** Skidbladnir was an Electron 9 GUI wrapping `cwebp`.
It moved to **Rust + Tauri 2** with a **Nuxt + Tailwind** frontend, shipping on
**Windows, Linux and macOS**, without losing a single one of the encoder controls
the Electron app exposed; the Electron app was retired in 0.7.0 (Phase 6).

---

## Phase 0 — Foundations (make the repo routine-ready)

- [x] Add `ROADMAP.md` as the phase-ordered checkbox work queue (this file).
- [x] Rewrite `README.md` as the consumer-facing home (about + shipped features only).
- [x] Add `/scratch` to `.gitignore` for routine transient output.
- [x] Remove `yarn.lock` / `package-lock.json` from `.gitignore` — a lockfile-less
      JS project cannot have reproducible builds, and the Tauri frontend will need one
      committed.
- [x] Add a CI workflow (`.github/workflows/ci.yml`): fmt, clippy `-D warnings`, build
      and test the Rust workspace on `ubuntu-latest`, `windows-latest`, `macos-latest`,
      on stable Rust, with `fail-fast: false` so a Windows- or macOS-only break is not
      hidden. It installs the Linux Tauri system dependencies and a reference `cwebp`
      (apt on Linux, brew on macOS) ahead of the phases that need them.
- [x] Build the frontend in `ci.yml`. It turned out not to be an optional extra job:
      `tauri::generate_context!` embeds `frontendDist` at **compile** time, so without
      `npm ci && npm run generate` ahead of it the Rust build fails outright with
      "the `frontendDist` configuration is set to ... but this path doesn't exist".
- [x] Cache the Nuxt build in CI as well as npm's download cache. `node_modules/.cache`
      (where Nuxt 4 keeps its build cache) is keyed on the lockfile plus the frontend
      sources, so an unchanged frontend is not regenerated on all three runners.
- [x] Add `rustfmt.toml` and a clippy configuration matching the owner's other Rust
      repos (hard tabs, `-D warnings` in CI). `rustfmt.toml` copies `DSP/rustfmt.toml`
      verbatim (hard tabs, `tab_spaces = 8`, `max_width = 10000`, horizontal imports,
      `StdExternalCrate` grouping). Clippy is configured as `[workspace.lints.clippy]`
      with `all` and `pedantic` at warn, consumed by members via `lints.workspace = true`.
      `clippy::nursery` is deliberately excluded — its lints move between toolchains and
      the dev host (nightly) and CI (stable) would disagree.
      **Formatting is checked by a separate CI job on nightly rustfmt.**
      `imports_layout`, `imports_granularity` and `group_imports` are still nightly-gated;
      stable rustfmt does not reject them, it silently ignores them and formats
      differently, so a stable `cargo fmt --check` fails against a nightly-formatted tree.
      Do not "fix" that by deleting the options — everything that *compiles* stays on
      stable, and only rustfmt runs on nightly.
- [x] Decide and record the app's product identity. **The answer:** the product is
      **Skidbladnir** — the repo name, the name every released tag carries, and the
      only one of the four candidates a user has ever seen. Concretely:
      - Product / window title / release name: **Skidbladnir**
      - `appId` (Tauri + electron-builder): **`com.basicautomation.skidbladnir`**
      - Binary, crate and Nuxt package name: **`skidbladnir`**
      - Rejected: `ba-nextgenimg` (an internal slug), `skidblad` (a truncation from the
        dead 2021 branch), `ba | Next Generation Image` (a description, not a name).
      The Electron `package.json` keeps `name: ba-nextgenimg` and `appId: ba.nextgenimg`
      until Phase 6 retires it — renaming it now would change the electron-builder output
      path that the shipped v0.4.3 artifact uses, for an app that is being deleted anyway.
- [x] Triage `origin/copilot/add-resize-control-option`. **The answer: there was
      nothing to land.** PR #31 merged it on 2026-06-04 and
      `git merge-base --is-ancestor origin/copilot/add-resize-control-option master`
      confirms the tip is an ancestor of `master` with an empty diff — it is a stale
      post-merge branch, not unmerged work. Deleted from the remote.
- [x] Declare `origin/Skidbladnir` (the cold 2021 Vue 3 POC) dead. **It is dead.**
      `master` is the only codeline. Do not revive that branch, do not port from it, and
      do not mistake it for the migration target: it is a Vue 3 + Vite proof of concept
      abandoned in 2021, predating this Tauri 2 + Nuxt decision entirely. It is kept on
      the remote for archaeology only — the one thing it is good for is prior art on the
      settings-store shape, and even that must be re-derived against the Electron app's
      real control surface rather than trusted.

## Phase 1 — Tauri 2 + Nuxt + Tailwind scaffold

The new app lands **alongside** the Electron app, not on top of it. `master` keeps
building and running the Electron app until Phase 6 retires it.

- [x] Choose the layout. **The answer: exactly that.** A Cargo workspace at the repo
      root, with:
      - `crates/*` — pure-Rust logic with **no Tauri dependency**, so the encode core is
        testable from plain `cargo test` with no window, no IPC and no frontend. First
        member: `crates/skidbladnir-encode`.
      - `src-tauri/` — the Tauri 2 application shell, a thin IPC layer over `crates/*`.
      - `frontend/` — the Nuxt + Tailwind frontend.
      The Electron files stay at the repo root until Phase 6. Nothing in the workspace
      may reference them, so deleting them cannot break `cargo build`.
- [x] `cargo install tauri-cli` on the dev host — **installed, version 2.11.5**.
      It does **not** build on the dev host's default nightly: every tauri-cli 2.x
      dependency tree resolves a `rustix` below 1.0 (`0.37.28` under `--locked`,
      `0.38.43` unlocked), and those use `rustc_attrs`, which rustc 1.100.0-nightly
      rejects with "attributes starting with `rustc` are reserved". Install it on
      **stable** with the global nightly-only rustflags cleared:
      `RUSTFLAGS= cargo +stable install tauri-cli --locked --version '^2'`.
      This is a host/toolchain quirk, not an in-tree nightly dependency — the workspace
      itself must keep building on stable.
- [x] Scaffold the Tauri 2 app: `src-tauri/` with `tauri.conf.json`, the appId
      `com.basicautomation.skidbladnir`, a 1000x780 window, a minimal capability set, and
      a generated icon set. The shipped `build/icon.png` is 2363x2364, one pixel off
      square, which `cargo tauri icon` rejects; it was cropped to 2363x2363 with
      `cwebp -crop` + `dwebp` in the run's scratch dir. The mobile (android/ios) icon sets
      `cargo tauri icon` also emits were deleted — this is a desktop-only product.
      `tauri.conf.json` deliberately omits `version` so it inherits from
      `src-tauri/Cargo.toml` and there is no third copy of the version to drift.
- [x] Scaffold the Nuxt frontend in `frontend/` — Nuxt 4 + Tailwind 4 via
      `@tailwindcss/vite`, `ssr: false`, the static Nitro preset, output in
      `.output/public`. `package-lock.json` is committed.
- [x] Wire `beforeDevCommand` / `beforeBuildCommand` / `frontendDist`. Two traps worth
      recording, both found by running the app rather than reading the config:
      - The before-commands run from the **repo root**, not from `src-tauri/`, so they are
        `npm --prefix frontend run ...`. With `../frontend` they resolve outside the repo.
      - The `custom-protocol` Cargo feature is what makes a binary serve the embedded
        frontend. Without it — and a bare `cargo build --release` does not set it — the
        release binary still points at `devUrl` and opens showing
        "Could not connect to 127.0.0.1: Connection refused". **Release builds must go
        through `cargo tauri build`**, which enables the feature.
- [x] First green `cargo tauri build` on Linux producing a runnable binary.
      `cargo tauri build --no-bundle` produces a 12 MB `skidbladnir`. Driven over
      WebDriver with no dev server running, it loads `tauri://localhost/`, renders the
      Nuxt shell, and its IPC commands return live data from the Rust core. `--no-bundle`
      because the AppImage target needs `patchelf`, which the dev host lacked. Installers
      and the AppImage have since been built by `release.yml` in CI, where `patchelf` is an
      apt package (Phase 4).
- [x] First green dev window that renders the Nuxt shell — verified over WebDriver
      against the Nuxt dev server on 127.0.0.1:1420: `document.title` is "Skidbladnir",
      the shell renders, and `encoder_version` / `default_settings` return real values.
      The dev host's shared WebDriver harness drives this app correctly; its first green
      run against Skidbladnir is this one.
- [x] (owner-gated) `patchelf` on the dev host for AppImage bundling. *Superseded:* CI
      builds the AppImage (`release.yml` installs `patchelf` from apt), and the published
      AppImage is what gets checked, so the dev host does not need it.

- [x] Tighten the CSP. Everything except inline script is now locked down:
      `default-src 'self'`, `img-src 'self' data:` (the preview's two images and nothing
      else), `font-src 'self'` (the Inter webfont is bundled, not fetched),
      `connect-src 'self' ipc: http://ipc.localhost` — the app talks to Rust over the IPC
      bridge and should never open a socket — plus `object-src`, `frame-src`,
      `frame-ancestors` and `form-action` all `'none'`, and `base-uri 'self'`.
- [x] Remove `script-src 'unsafe-inline'`. **It turned out not to need the build-time
      hashing this item described.** Tauri rewrites the CSP it delivers to cover the inline
      scripts it serves from the bundle, so `script-src 'self'` is enough on its own and
      naming Nuxt's three inline scripts by hash is unnecessary machinery. A hash-injecting
      build script was written, and then deleted once measurement showed it was redundant.
      **Enforcement was verified, not assumed** — in the running window an injected inline
      `<script>` does not execute and `fetch("https://example.com/")` is blocked.
- [x] Run `scripts/smoke-test.sh` in CI — the `window smoke + accessibility` job, under
      `xvfb-run` with the `webkit2gtk-driver` package and `tauri-driver`. Note
      `execute/sync`, not `execute/async`: the async endpoint waits for a completion
      callback, so a script that simply returns hangs until the driver times out (this
      cost a debugging round already).
- [x] Run the smoke test on the **Windows** runner too — the `window smoke (windows)` job.
      `tauri-driver` drives Windows through Microsoft's `msedgedriver`, fetched by the
      pinned `msedgedriver-tool` to match the installed WebView2. Four traps, each found
      by a failing run rather than by reading:
      - Git Bash paths (`/d/a/...`) reach native Windows programs unreadable; the harness
        converts with `cygpath`.
      - On the `windows-2025` image WebView2 never opens its DevTools port ("DevToolsActivePort
        file doesn't exist"); the job is pinned to `windows-2022`, per
        <https://github.com/actions/runner-images/issues/14738>.
      - Git Bash rewrites the installer's `/S` into a drive path, turning a silent install
        into one waiting for a click; `MSYS_NO_PATHCONV=1`, plus a 45-minute job ceiling.
      - Windows serves the bundled frontend from `http://tauri.localhost`, not
        `tauri://localhost`.
      tauri-driver has no macOS support, so the macOS build still cannot be launched this way.

## Phase 2 — Encode core in Rust (the real work)

The Electron app shells out to `cwebp.exe`. The Rust port should not.

- [x] **Decide the encode backend. The answer: `libwebp-sys`, binding libwebp's
      `WebPConfig`.** It carries a 1:1 field for every control the Electron UI exposes, and
      byte-for-byte parity with `cwebp` is now demonstrated rather than assumed (see the
      parity item below). It vendors and statically links libwebp, so there is no system
      library to locate on Windows or macOS — CI builds it green on all three platforms.
      The `image` crate was rejected: it reaches none of the advanced knobs, so it cannot
      reach parity. The **Tauri sidecar remains the documented fallback**, and
      `cwebp_args()` keeps it a working one rather than a paper plan.
      **No binary is vendored into this repository, and none will be without writing its
      provenance and update path down first.** libwebp-sys compiles libwebp *from source*
      as part of the build, so what ships is built here from a pinned crate version, not a
      downloaded executable of unknown origin. If the sidecar fallback is ever taken, the
      three `cwebp` binaries, where each came from, how their integrity is checked and who
      updates them go into this file and the README **before** they are committed.
- [x] Define the `EncodeSettings` type — `crates/skidbladnir-encode/src/settings.rs`.
      (Since split into `EncodeJob` + `WebpSettings` for Phase 7; see the AVIF item.)
      Serializable, camelCase over the wire, `#[serde(default)]` so a partial payload from
      the frontend fills in rather than failing, with every range and default taken from
      the Electron UI's own `<input>` attributes and pinned by test.
- [x] Implement the encoder and prove **parity against `cwebp` itself**.
      `crates/skidbladnir-encode/tests/parity.rs` encodes the same pixels through
      `encode_rgba` and through a real `cwebp` and compares the output byte for byte.
      **76 settings covering the whole control surface match `cwebp` 1.6.0 exactly.**
      The fixture is a PAM (`P7`, `RGB_ALPHA`) file, which `cwebp` reads with its own
      built-in PNM reader, so no image decoder sits between the two encoders and a
      mismatch can only be the encoder configuration.
      The gate was itself mutation-tested: deleting the `near_lossless => lossless = 1`
      line makes 5 cases diverge, and forcing `use_argb = 0` aborts the run, so it
      genuinely catches regressions rather than passing vacuously.
- [x] Build a **version-matched** reference `cwebp` in CI, so parity is **enforced**
      there rather than reported. CI asks the linked library its own version
      (`cargo run --example libwebp-version`), clones libwebp at that exact tag, builds
      `cwebp` from `makefile.unix` and runs the parity test with
      `SKIDBLADNIR_REQUIRE_PARITY=1` — so a missing or mismatched reference now fails the
      build. A `libwebp-sys` bump is followed automatically instead of silently degrading
      the gate to a skip.
      Unix only: `makefile.unix` does not build on the Windows runner, so parity is
      enforced on Linux and macOS and reported on Windows.
- [x] Port the **mode** surface: lossy · lossless · near-lossless · JPEG-like · preset — all five, parity-tested.
- [x] Port the **preset** surface: `default`, `photo`, `picture`, `drawing`, `icon`, `text` — all six, and libwebp's own preset table is pinned by test.
- [x] Port **quality** and **alpha quality** — parity-tested at quality 0/1/50/99/100 and alpha quality 0/50/100.
- [x] Port **compression method** (`-m`) and **segments** (`-segments`) — parity-tested across every value, 0..=6 and 1..=4.
- [x] Port **spatial noise shaping** (`-sns`) — parity-tested at 0/25/50/100.
- [x] Port the **filter** surface: strength, sharpness, strong/simple, auto-filter — parity-tested: auto, plus strong and simple at 0/0, 20/3 and 100/7.
- [x] Port **target size** and **target PSNR** (mutually exclusive with quality) — parity-tested; `-print_psnr` sets `show_compressed`, as cwebp does.
- [x] Port **multi-pass** (`-pass`) — parity-tested at 1/6/10.
- [x] Port **partition limit** (`-partition_limit`) — parity-tested at 0/50/100.
- [x] Port **sharp YUV** (`-sharp_yuv`) — parity-tested, including its effect on the ARGB colour path.
- [x] Port **low memory** (`-low_memory`) — parity-tested.
- [x] Port **resize** (width/height) — parity-tested at four sizes including width-only and height-only, and the `-exact` path lossless takes.
- [x] Port **multi-threading** (`-mt`) — parity-tested on and off.
- [x] Report real before/after file sizes and the original size back to the UI.
      `Conversion { source_bytes, output_bytes, width, height }` plus `saving_percent()`.
      The dimensions are read back out of the encoded file rather than echoed from the
      request, because a resize with one dimension given as `0` is only resolved by the
      encoder.
- [x] Read the user's input files. `crates/skidbladnir-encode/src/source.rs` decodes PNG,
      JPEG and TIFF with the `image` crate and WebP with libwebp itself, identifying the
      format by **content sniffing** rather than by extension as the Electron app does.
      The full file pipeline is parity-tested too: our PNG decode plus libwebp encode
      matches `cwebp`'s libpng decode plus libwebp encode byte for byte.
- [x] Write output files safely. `encode_file` refuses to overwrite the source image and
      stages the write through a temporary file in the destination directory, renamed into
      place only on success, so a failed encode cannot truncate an existing file. It also
      refuses to create an output directory the user did not choose.
      **This closes a real data-loss bug in the Electron app**: it accepts WebP input and
      derives the output name as `<stem>.webp` in the chosen directory, so converting a
      WebP into its own folder silently overwrites the original.
- [x] A `convert_image` IPC command over the above, returning the input path, the output
      path and the sizes — enough for a batch UI to attach each result to its row.
- [x] The Electron file filter's JPEG aliases (`.jpe`, `.jif`, `.jfif`, `.jfi`) need no
      special handling: input is identified by **content**, not extension, so they are all
      simply JPEG. The picker still lists them so the dialog does not hide the user's
      files.
- [x] 16-bit PNG input. **This was a real divergence, found by testing it.** `cwebp`
      reads PNGs through libpng with `png_set_strip_16`, which **discards the low byte** of
      a 16-bit sample, while the `image` crate *scales* the value — so 16-bit input encoded
      differently on the two paths (216 bytes vs `cwebp`'s 212 for RGBA, 168 vs 166 for
      RGB) while every 8-bit colour type already matched exactly. The reduction now
      truncates to match, and `matches_reference_cwebp_across_png_colour_types` pins
      16-bit RGBA, 16-bit RGB, 8-bit grayscale and 8-bit grayscale+alpha so it cannot drift
      back. 8-bit palette was verified identical by hand (the `image` crate cannot write
      one, so it is not in the test).
- [x] CMYK JPEG input. **Tested, and it is not a parity question after all: `cwebp`
      refuses CMYK JPEG outright.** It asks libjpeg for `JCS_RGB`, which libjpeg cannot
      produce from four channels, so `cwebp` 1.6.0 exits with "libjpeg error: Unsupported
      color conversion request". There is no reference output to match. What
      `tests/cmyk_jpeg.rs` pins instead, against a 439-byte Adobe/YCCK fixture made with
      `magick` (provenance and SHA-256 in the test):
      - our decoder produces the **right colours**, not their inverse — within 3 levels of
        ImageMagick's own CMYK→sRGB conversion at both ends and the middle of a ramp;
      - the file converts end to end;
      - the reference `cwebp` **still refuses** it — after first proving that `cwebp` reads
        RGB JPEG at all, so a build without libjpeg cannot pass it vacuously. If a future
        libwebp starts converting CMYK, this fails, and agreement becomes a parity test.
      Runs in CI's enforced parity step alongside `parity.rs`.
- [x] CMYK JPEG **without** an Adobe marker. Tested with a hand-made fixture
      (`tests/fixtures/cmyk-plain.jpg`: written by the `jpeg-encoder` crate, APP14 cut out).
      **Finding:** strictly by libjpeg's convention such a file is not inverted, but our
      decoder reads it as Photoshop-inverted — and so do `magick` 7.1.2 and Chrome's Skia,
      which inverts CMYK with no marker check
      (<https://github.com/google/skia/blob/main/src/codec/SkSwizzler.cpp>). Matching what
      the user's browser shows is the right call, so the test pins our decode to `magick`'s
      reading of the file, and fails if the decoder ever starts honouring the missing
      marker. The reference `cwebp` refuses this file too, now checked alongside the Adobe one.

- [x] **A WebP source converted to lossy WebP matched `cwebp` in only 84 of 304 cases**
      (found 2026-09-28 by the metadata gate as a 4-byte difference, fixed the same day).
      For a lossy encode with no resize and no sharp YUV, `cwebp` decodes a WebP input
      straight into a YUV 4:2:0 picture with libwebp's decoder (`imageio/webpdec.c`,
      `use_argb` off); Skidbladnir decoded to RGBA and let the encoder convert back.
      `encoder::encode_webp_source_with_progress` now takes `cwebp`'s route, for conversion
      and preview alike (`source::encode_decoded`). PAM, PNG, JPEG and TIFF sources were
      never affected: `cwebp` imports their RGB the way we do.
      **Gate:** `matches_reference_cwebp_through_a_webp_file` in `tests/parity.rs` — the
      whole 76-setting surface from four `cwebp`-written sources (lossless/lossy × alpha/
      opaque): 304 of 304 byte-identical; with the old route, 220 diverge. The metadata
      gate now compares WebP sources whole in both modes.

- [x] **JPEG to WebP matched `cwebp` in 17 of 456 cases** (found and fixed 2026-09-28) —
      the most common conversion. The encoder was never the problem: `cwebp` reads JPEGs
      with libjpeg(-turbo) as `JCS_RGB` with fancy upsampling (`imageio/jpegdec.c`), and
      the `image` crate's decoder (zune-jpeg) rounds its IDCT and chroma upsampling
      differently, so the pixels handed to the encoder differed (2114 vs 2130 bytes at the
      defaults). JPEG input now decodes through libjpeg-turbo's decoder, as MozJPEG
      carries it (`mozjpeg` crate, built from source with `cc` and `nasm`, no CMake),
      configured as `cwebp` configures libjpeg; CMYK/YCCK JPEGs, which `cwebp` refuses,
      keep the `image` decoder and its Photoshop-inversion handling. Licence: IJG AND
      BSD-3-Clause AND Zlib — `IJG` added to `deny.toml`, its notice (with the credit the
      IJG licence asks for) in `THIRD-PARTY-NOTICES.md`.
      **Gate:** `matches_reference_cwebp_through_a_jpeg_file` in `tests/parity.rs` — six
      committed `magick`-written JPEGs (4:4:4, 4:2:2, 4:2:0, progressive, restart markers,
      greyscale; 97x63 so the upsampler's edges count) × the 76-setting surface: 456 of 456
      byte-identical; with the old decoder, 439 diverge. The metadata gate's JPEG sources
      are now compared whole too (84 of 84). **CI's cwebp links the runner's libjpeg-turbo**,
      so the gate also checks that libjpeg-turbo versions agree: it passed on the Arch host
      and in CI on `ubuntu-latest` and `macos-latest` (Homebrew) — run 36518337021.

- [x] **TIFF to WebP, across the whole surface** (2026-09-28). 8-bit RGB TIFFs (plain and
      LZW) already matched `cwebp`; **16-bit TIFFs matched in 0 of 76 settings**, because
      libtiff's `TIFFReadRGBAImage` reduces 16-bit samples with rounding,
      `(v * 255 + 32767) / 65535` (its `Bitdepth16To8`), where we truncated as libpng does
      for PNG. The reduction is now per format. **Gate:**
      `matches_reference_cwebp_through_a_tiff_file` — four committed `magick`-written TIFFs
      minus the one below × 76 settings: 228 of 228 byte-identical; truncating again fails 76.
- [x] **Decided: an unassociated-alpha TIFF keeps its colours, unlike `cwebp`.** libtiff
      returns such a file premultiplied, and `cwebp` un-multiplies only *associated* alpha,
      so it encodes premultiplied colour as straight and darkens every semi-transparent
      pixel — measured exactly, `(c * a + 127) / 255` on all 24,444 samples. A reading bug,
      not a setting, so not copied (as the Electron app's `-size 1` default was not).
      Pinned both ways by `unassociated_alpha_tiff_keeps_its_colours`: ours lossless equals
      the source; `cwebp`'s equals it premultiplied, and the test says what to do if a
      libtiff or libwebp release changes that.
- [x] **Associated-alpha (premultiplied) TIFF input** (fixed 2026-10-01). **It diverged:**
      the `image` crate hands the stored, premultiplied samples back, and Skidbladnir encoded
      them as straight colour, darkening every semi-transparent pixel — 272 of 276
      conversions differed from `cwebp`. `cwebp` gets them from libtiff as stored and
      un-multiplies them itself (`imageio/tiffdec.c`, `MultARGBRow`: 24-bit fixed point,
      black under alpha 0) when `ExtraSamples` is exactly `[1]`; `source.rs` now does the
      same, and un-multiplies the 16-bit samples exactly for the other formats. The fixture
      problem was solved by writing the TIFFs byte by byte in the test rather than with
      `magick`. **Gate:** `matches_reference_cwebp_through_an_associated_alpha_tiff` — 8- and
      16-bit, every alpha value 0-255 present, x the 138-setting surface: 276 of 276
      byte-identical. Mutation: plain rounded division instead of the fixed-point form fails
      79; no un-multiply fails 272.

## Phase 3 — Frontend parity (Nuxt + Tailwind)

Parity means a user of the Electron app finds every control they had, not a
prettier subset.

- [x] File selection: input paths and output path, via `tauri-plugin-dialog`. The window
      holds only `dialog:allow-open` and no filesystem permission at all — every read and
      write happens in Rust against a path the user picked.
- [x] Drag-and-drop of input files onto the window, using Tauri's **native** drag-drop
      event rather than HTML5 dragover/drop — with the native handler enabled the
      webview's own events never fire, so the HTML5 approach looks right and silently does
      nothing. Which dropped files are usable is decided by the Rust core reading each
      file's leading bytes, so the UI cannot accept something the loader then refuses.
      **Caveat: the drop gesture itself is unverified.** A native drag cannot be
      synthesised over WebDriver; what was verified in the running app is that the
      listener registers without error and that `inspect_dropped_paths` correctly sorts a
      real PNG, a text file and a missing path.
- [x] The mode selector and its conditional control groups. Verified in the running
      window over WebDriver: lossy shows 9 sliders plus the advanced disclosure, and
      switching to lossless collapses to 3 sliders with no advanced group.
- [x] Every Phase 2 control bound to the UI, with the same ranges and defaults. The
      defaults are **fetched from the Rust core** on mount rather than written again in
      TypeScript, which is how the two would otherwise drift.
- [x] The advanced-options disclosure. It is present for lossy mode, where the expert
      controls actually reach the encoder.
- [x] Per-control help text. **The premise of this item was wrong and is corrected
      here:** the Electron UI's `-info` elements are live numeric value readouts, not help
      text, and the whole app contains exactly one tooltip (on the file picker). There was
      no copy to carry over, so the help text is taken from `cwebp -longhelp` — libwebp's
      own wording, which is the authoritative description of what each flag does.
- [x] Input/output validation and error states. Convert is disabled until inputs, a
      destination and (in preset mode) a preset are chosen, and settings are validated by
      the **Rust** `validate_settings` command rather than a second copy of the rules in
      TypeScript. Per-file failures are listed without stopping the run.
- [x] Conversion progress and the completed state. Real per-file progress via libwebp's
      `WebPPicture::progress_hook`, emitted as a `conversion-progress` event and shown as a
      bar with a file counter.
      **libwebp does not guarantee a final call at 100** — a default-quality encode of a
      96x64 fixture stops reporting at 68 — so completion is signalled by the command
      returning, not by the bar filling. The UI says so rather than appearing stuck.
- [x] Original vs converted size readout — a results table with before, after, the
      percentage change and the encoded dimensions, per file.
- [x] Theming. **Owner directive 2026-09-24: keep the Electron app's design, restyled in
      the Tailwind Palenight palette, using a skinned component library.** Delivered as:
      - **Nuxt UI 4** for the components (chosen over shadcn-vue: it is a Nuxt module, so
        it owns the Tailwind 4 integration and needs no per-component vendoring).
      - The **Material Theme Palenight** palette written out as Tailwind `@theme` colours
        in `frontend/assets/css/main.css`, with Nuxt UI's semantic tokens (`--ui-bg`,
        `--ui-primary`, …) re-pointed at it, so every component it renders is skinned by
        default rather than restyled one at a time.
      - The Electron app's **layout kept**: the full-width dot-textured header band with a
        large centred title, the 2px **dotted** borders around option groups, the
        two-column control rows, the fixed dot-textured footer, and the circular accent
        FAB bottom-right. Only the palette moved.
      The app is dark-only, which is what the Palenight theme is; there is no light variant
      to follow the system preference into.
- [x] Bundle the icon set (`@iconify-json/lucide`) rather than letting Nuxt UI fetch icons
      from the Iconify API at runtime — a desktop app cannot assume network access, and an
      icon that silently fails to load offline is a broken window. 43 icons, 10.4 KB.

## Phase 4 — Beyond parity

- [x] Batch conversion with a real queue, per-file status, and cancel. Multi-select,
      sequential conversion with per-file success/failure rows, live progress, and a
      Cancel button.
      Cancellation works by the encode's own progress callback refusing to continue —
      there is no way to interrupt `WebPEncode` from outside — and a cancelled conversion
      writes nothing at all, so stopping a batch cannot leave a half-converted image.
      **This is why `convert_image` is `async` with the encode on a blocking thread:** as a
      synchronous command it held the event loop, and a cancel issued from the frontend
      could never be delivered. Measured, not assumed — a 1800x1400 encode ran to
      completion every time before the change, and cancels after 3 progress events with an
      empty output directory after it.
- [x] Preview: original vs encoded, side by side, before committing the write. Encodes
      into memory and writes nothing.
      Both sides come back as `data:` URLs rather than file paths, which means the preview
      needs **no filesystem permission in the webview at all** and cannot show a stale file
      from an earlier run. The left-hand side is the source pixels encoded *losslessly*
      rather than the source file re-served, so both sides are WebP the webview can display
      while the reference image stays pixel-exact — and any resize is applied to both, so
      the comparison is like for like instead of a big image beside a small one.
      Refused above 24 megapixels: two base64 copies in the webview is real memory.
      **Correction (2026-09-26): this was ticked when only the backend existed.** The
      `preview_encode` command and its tests landed in 343f7c0, but nothing in the window
      ever called it, so the README's "preview the result" was untrue until the Preview
      panel was built. It now exists: pick one of the selected files, see both sides with
      the size, dimensions and the core's saving figure, and a notice when the settings
      have changed since. `preview_encode` is now `async` too — as a synchronous command
      an AVIF preview froze the window while it encoded. Verified in the running window,
      for both formats, by dropping a file through Tauri's own drop event.
- [x] Persist settings between launches. `src-tauri/src/preferences.rs` stores the last
      used settings and destination as JSON in the OS's per-app config directory, saved
      after a conversion run rather than on every slider drag.
      The loader **never fails** — a preferences file is read at startup and is the one
      file a user can hand-edit or a crash can half-write, so anything unreadable,
      unparseable or *invalid for the encoder* falls back to the defaults and reports
      which, letting the UI say the saved settings were discarded instead of silently
      losing them. A destination that has since been deleted or unmounted is dropped.
      Writes are staged and renamed, as the image writer is.
- [x] Named user presets (save/load an `EncodeSettings`), distinct from libwebp's own
      `-preset` values. Stored in **one** JSON file keyed by name rather than a file per
      preset, deliberately: a name typed by the user must never become a path component.
      There is a test that saves a preset called `../../escaped` and asserts the config
      directory still contains exactly one file and nothing was written outside it.
      Names are trimmed, must be non-empty and at most 80 characters; settings the encoder
      would reject are refused on save and hidden on load; a damaged file reads as empty
      and is left on disk rather than destroyed.
- [x] Recursive directory input with an output-structure mirror. "…or a whole folder"
      scans a directory for images **by content**, and conversions reproduce the source
      tree in the destination (toggleable — off writes everything side by side).
      Three bounds, because a folder picker is an invitation to point at a home directory:
      **symlinks are not followed** (a symlinked directory can point at its own ancestor,
      which turns a scan into an endless walk, and a symlinked file can point outside the
      tree the user thought they chose), the walk is depth-bounded at 32, and it stops at
      10,000 files.
      `mirrored_output_path` refuses any relative path containing `..`, an absolute path or
      a path prefix, so a mirrored write cannot land outside the chosen folder.
- [x] Keyboard-navigable and screen-reader-labelled controls. Audited in the running
      window rather than assumed: **all 32 focusable controls now have an accessible name**
      (3 did not — the preset-name and resize/target number fields, whose labels were
      sibling spans rather than associated labels). Sliders carry an explicit `aria-label`
      instead of relying on proximity; the mode, filter and target groups are real
      `radiogroup`s with `aria-checked`; conversion status and results are `aria-live`
      regions so progress is announced rather than only drawn; validation failures are
      `role="alert"`. A `:focus-visible` ring was added because a keyboard-navigable app
      whose focused control is invisible against a dark palette is not usable.
- [x] Run a real accessibility audit. `scripts/a11y-audit.sh` injects **axe-core** into the
      live window and reports violations; the window is now **clean at the `minor`
      threshold**. It found two things the hand-written name check could not:
      - **Colour contrast on 25 nodes.** Palenight's own comment colour `#676e95` is
        2.76:1 against the background, far under WCAG AA's 4.5:1. It is kept for borders
        and the dot texture, and a new `--color-palenight-muted` (`#9099bd`, 4.86:1) now
        carries the same muted role for anything a person reads. The error red was
        4.37:1 — just under — and was lightened to `#ff6b85` (5.00:1).
      - **A missing `lang` attribute** on `<html>`.
      Six failures survived the first fix and were worth understanding rather than
      suppressing: five were the *disabled* filter sliders, where `opacity-40` on the whole
      block took their labels to 2.6:1 and their help text to 2.0:1. Dimming now applies to
      the slider track alone and the label reads "· not in use", which is both accessible
      and clearer. The sixth was the selected mode card, whose tinted background lifts to
      `#373d42` where the muted token is 3.92:1; its description uses the brighter token.
- [x] Speed up the `window` CI job. It now uses the **prebuilt** `@tauri-apps/cli` from the
      frontend's lockfile instead of `cargo install tauri-cli`: **10m42s → 1m41s** for the
      whole job on its first run with the change.
- [x] Use the prebuilt CLI in `release.yml` as well — done, and **tested before a tag**:
      the workflow gained a `dry_run` dispatch input that builds the dispatching branch and
      attaches nothing. The dry run built all four installers in 7m44s (the 0.5.0 release
      took 13m36s).
- [x] Run `scripts/a11y-audit.sh` in CI alongside the smoke test — same job.
- [x] **GIF input**, still and animated — GIF to animated WebP is WebP's classic job, and
      libwebp ships the reference converter, `gif2webp`. `crates/skidbladnir-encode/src/gif_input.rs`
      reads a GIF the way `gif2webp` does, rule for rule (transparent canvas, blending,
      the three disposals, the 10 ms → 100 ms floor, GIF repeats → WebP plays, the
      background hint), from the `gif` crate's raw indexed frames; the animation encoder
      takes `gif2webp`'s keyframe spacing (9/17 lossless, 3/5 lossy). Any GIF converts
      through the animation encoder, as in `gif2webp`; a still GIF bound for AVIF is a
      still, an animated one is refused by name.
      **Gate:** `tests/gif.rs` — **49 conversions byte-identical to libwebp 1.6.0's
      `gif2webp`**: seven fixtures (sub-rectangles, transparency, every disposal, local
      palettes, interlacing, short delays, each loop form, a still, a 24-frame clip, and a
      GIF written by ImageMagick) × seven settings. Mutation-tested: skipping
      dispose-to-previous fails 7, dropping either keyframe spacing fails 3 and 8 — the
      last needed the 24-frame fixture, since no shorter clip exercised it. Found on the
      way: the app's default alpha filtering ("best") moved mostly-transparent lossy
      frames 2 bytes from libwebp's default ("fast"). CI builds `gif2webp` (giflib).
      Verified in the running window by the smoke test.
- [x] **Keep metadata** — ICC profile, EXIF and XMP — in WebP output: `cwebp -metadata`,
      which the Electron app never exposed, so every conversion silently dropped a photo's
      colour profile (a Display P3 or Adobe RGB image then shows as sRGB). A job-level
      `metadata` choice, all off by default so a default job still matches `cwebp`
      exactly; a "Metadata" panel in the WebP settings. `crates/skidbladnir-encode/src/metadata.rs`
      reads the metadata the way libwebp 1.6.0's `imageio` does (JPEG `APP1`/`APP2`, the ICC
      profile reassembled by segment number and refused when inconsistent, as `cwebp`
      refuses it; PNG `iCCP`, `eXIf`, XMP and `ImageMagick` raw-profile text; TIFF ICC and XMP;
      WebP `ICCP`/`EXIF`/`XMP `) and writes the container the way `WriteWebPWithMetadata`
      does. The preview keeps it too, so its size is the size written.
      **Gate:** `tests/metadata.rs` — 84 conversions (7 fixtures with hand-built metadata,
      incl. a 140 KB ICC profile in three out-of-order JPEG segments, × lossy/lossless × six
      `-metadata` choices) against `cwebp -metadata`: all 84 byte-identical whole files
      (once JPEG and WebP sources decoded as `cwebp` decodes them; below). A JPEG with a
      missing ICC segment is refused as `cwebp` refuses it. Mutation-tested: unsorted ICC
      segments, a missing `VP8X` alpha flag, no RIFF padding, `ICCP` after the image and
      last-EXIF-wins each fail it.
- [ ] **The comparison view — the best image comparison there is** (owner request,
      2026-09-28; the model is Upscayl's before/after slider with zoom, taken further).
      *Waits on its design (slice 13: designed in Figma before it is built); the Figma
      connector needed re-authorising on 2026-10-01, so no slice was started.* The
      preview today is two static images side by side at fit-to-width, which cannot show
      the artefacts the advanced controls exist to trade off. Slices, each landable alone:
      1. **A real viewer.** One full-size viewport instead of two thumbnails; 100% means
         one image pixel per *device* pixel (`devicePixelRatio`-aware), and above 100% pixels
         are drawn nearest-neighbour (`image-rendering: pixelated`), never smoothed — a
         smoothed zoom hides exactly the ringing and blocking being judged.
      2. **Wipe slider**, Upscayl-style: both images registered pixel for pixel in one
         viewport, a draggable divider between them, horizontal or vertical, keyboard-
         operable (arrow keys step, Shift steps further) and labelled for screen readers.
      3. **Synchronised zoom and pan** in every mode: wheel/pinch zooms about the cursor,
         drag pans, presets for Fit, 1:1, 2x, 4x, 8x, 16x, 32x, and the two sides can never
         drift apart. Side by side stays as a mode, with its viewports locked together.
      4. **Loupe magnifier**: a lens that follows the cursor at its own magnification
         (independent of the page zoom), split down the middle or showing A and B as twin
         lenses, with an optional pixel grid from 8x and a readout of both pixels' RGBA and
         their difference under the cursor.
      5. **Flicker (A/B blink)**: hold a key to show the original, release for the encode,
         or blink on a timer — the fastest way to see a subtle shift in colour or detail.
      6. **Difference view**: an amplified absolute-difference heatmap (x1 to x32), with the
         error computed in the Rust core from the decoded pixels, not by the webview.
      7. **Numbers beside the pictures**: PSNR, SSIM and a perceptual metric (SSIMULACRA2 or
         butteraugli; choose on licence and on agreement with the reference tools), computed
         in Rust. PSNR is checkable against `cwebp -print_psnr`, so it gets a parity test;
         the others against their reference implementations.
      8. **What you see is what the format decodes to.** AVIF and JPEG XL already decode in
         Rust and reach the webview as lossless WebP; do the same for every format so no
         side depends on the web engine's decoder or colour management, and state how a
         kept ICC profile is shown.
      9. **Compare settings within a format — variants.** Snapshot the current settings as a
         named variant; keep a tray of them for the same source; pick any two (or the
         original) as A and B; a table of size, saving, metrics and encode time for all of
         them; one click applies a variant's settings to the panel or saves it as a preset.
     10. **Compare across formats.** The same source through WebP, AVIF, JPEG XL and HEIC
         with each format's current settings, any two in the viewer and all four in the
         table. Two fair-fight helpers: **match size** (search each format's quality for the
         same byte size, then compare the pictures) and **match quality** (search for the
         same perceptual score, then compare the sizes).
     11. **Rate–distortion curve**: sweep quality for one or more formats and plot size
         against the metric; clicking a point loads that encode as a variant.
     12. **Scale.** Base64 `data:` URLs and the 24-megapixel refusal do not fit a viewer
         meant for zooming into large photos: serve the decoded images through a Tauri
         custom protocol, render tiles, and encode previews off the UI thread with
         cancellation when settings change mid-encode. A "preview a region" option can make
         huge images fast, but it changes the encoder's decisions near the region's edges,
         so it must say so and never be the default.
     13. **Verified like everything else**: the zoom/pan/registration maths unit-tested,
         the modes driven over WebDriver (including that A and B stay registered at 32x),
         axe-clean, and screenshots re-captured. Designed in the Nanna design (Figma and
         Nuxt UI components) before it is built.
- [x] **Keep metadata in JPEG XL output** (2026-09-28). The same `metadata` choice: an ICC
      profile *labels* the pixels (`JxlEncoderSetICCProfile` in place of the sRGB colour
      encoding — the decoded pixels are in the source's space, so tagging them sRGB was
      wrong whenever a profile existed), EXIF becomes an `Exif` box (after its 4-byte
      TIFF-header offset) and XMP an `xml ` box, as `cjxl` writes them
      (`JxlEncoderSetICCProfile`, `JxlEncoderUseBoxes`, `JxlEncoderAddBox` in libjxl's
      `encode.h`: <https://raw.githubusercontent.com/libjxl/libjxl/main/lib/include/jxl/encode.h>). A JPEG recompressed
      losslessly already keeps its own metadata (libjxl stores it for the bit-exact rebuild),
      whatever the setting. **Gate:** `keeps_metadata_in_jpeg_xl` in `tests/metadata.rs`,
      run by libjxl's `djxl`: lossless keeps a Display P3 profile verbatim and decodes to the
      source's pixels in it; lossy (XYB) keeps its primaries (libjxl regenerates the profile
      from its compact colour encoding, as `cjxl` output does); both boxes byte for byte;
      nothing when nothing is kept. Mutation-tested: ignoring the profile, dropping the
      boxes and omitting the `Exif` offset each fail it.
      Found on the way: the WebP gate's `-metadata all` cases had kept nothing on our side
      *or* `cwebp`'s (a test helper misread "all"); fixed, and all 84 still match.
- [x] **Keep metadata in HEIC output** (2026-09-28): the ICC profile as the primary image's
      `colr` box of type `prof` (`heif_image_set_raw_color_profile`), EXIF and XMP as its
      metadata items (`heif_context_add_exif_metadata` / `_XMP_metadata`). **Gate:**
      `keeps_metadata_in_heic` — the `colr prof` box is the source's profile byte for byte,
      and libheif's own `heif-info` (the runner's separate build in CI) lists the profile,
      `Exif` and `XMP`; nothing when nothing is kept. Mutation-tested: an unset profile and
      a missing EXIF item each fail it.
- [x] **An opaque image was written to HEIC with an alpha channel** (found and fixed
      2026-09-28: `heif-info` reported "alpha channel: yes" for a JPEG). `heic.rs` handed
      libheif interleaved RGBA for everything; an opaque image now goes as RGB, as `jxl.rs`
      already did, so no alpha plane is stored or advertised. **Gate:**
      `only_transparent_images_carry_alpha` (libheif's `has_alpha_channel` on our output, for
      an opaque and a transparent fixture); `heif-info` confirms "alpha channel: no".
      Mutation-tested: always writing RGBA fails it.
- [x] **Keep EXIF in AVIF output** (2026-09-28), through `ravif`'s `with_exif`. **Gate:**
      `keeps_exif_in_avif` — libavif's `avifdec --info` reports the source's EXIF at its exact
      size, and no ICC or XMP; none with nothing kept. Mutation-tested: dropping
      `with_exif` fails it. The window offers only the EXIF toggle for AVIF.
- [x] **ICC profile and XMP in AVIF output** — closed by Phase 8's move from `ravif` to
      libavif + libaom (0.14.0): the ICC profile, Exif and XMP go in as `avifenc` puts them,
      held to `avifenc` by `tests/avif_parity.rs` and checked by `keeps_metadata_in_avif`.
      (Left unticked when 0.14.0 shipped; ticked 2026-10-01.)
- [x] **Keep a GIF's ICC profile and XMP in animated WebP output**, as `gif2webp -metadata`
      (2026-10-01). `gif_input::metadata` reads the first `ICCRGBG1012` and `XMP DataXMP`
      application extensions as `gifdec.c`'s `GIFReadMetadata` does (an XMP packet keeps its
      sub-blocks' length bytes and loses the 257-byte magic trailer), and
      `animation::with_metadata` re-muxes ICCP then XMP with libwebp's mux. The WebP panel's
      ICC and XMP toggles are `gif2webp`'s `-metadata icc,xmp` for a GIF. **Gate:**
      `matches_reference_gif2webp_keeping_metadata` in `tests/gif.rs` — 20 of 20 whole
      conversions byte-identical to `gif2webp -metadata`; mutation: keeping the trailer
      fails 6, last-wins fails 2.
- [x] Metadata from an **animated WebP** source into animated WebP output (2026-10-01).
      `img2webp` has no `-metadata`, so the reference is the command-line route: re-encode,
      then `webpmux -set icc|exif|xmp`. The source's `ICCP`, `EXIF` and `XMP ` chunks are
      read as `cwebp` reads a still WebP's (`metadata::webp_chunks`) and set back with
      libwebp's mux under the WebP panel's three toggles. **Gate:**
      `keeps_an_animations_metadata_as_webpmux_sets_it` in `tests/animation.rs` — a source
      made with `webpmux -set`, five choices x lossless/lossy: 10 of 10 byte-identical.
      Mutation: dropping Exif fails 4. `webpmux` joins the reference tools
      (`scripts/build-reference-tools.sh`, CI's parity job).
- [x] Warn before two inputs in one run would write the same output (`photo.png` and
      `photo.jpg` both become `photo.webp`), and offer a "Replace existing files" toggle,
      on by default (2026-10-01). `plan_outputs` names each queued input's output with the
      conversion commands' own naming (flat or mirrored; case-insensitive on Windows and
      macOS) and the window lists shared names and files already in the destination. Off,
      `encode_file_with_options` refuses an existing output before encoding and claims the
      name with a `create_new` placeholder before the atomic rename, so nothing appearing
      meanwhile is replaced either. Remembered in the preferences (absent = on).
- [x] A "Start from `cwebp`'s defaults" button in the WebP panel (2026-10-01).
      **Gate:** `cwebps_defaults_are_a_bare_cwebp` runs `cwebp` with no flag at all and
      requires the reset to match it from four starting settings; mutation (resetting to the
      app's defaults) fails it.
- [x] An About and licence view in the window (2026-10-01): version, edition, licence, and
      the bundled `LICENSE`, notices and (GPL edition) `COPYING`, read by an enum, never a
      path (`src-tauri/src/legal.rs`, whose test holds the names to both editions'
      `bundle.resources`). Smoke-tested and axe-audited in the running window.
- [x] `cargo tauri build` green on **Linux** for `.deb` — **this was not actually blocked
      on `patchelf`.** Only the AppImage target needs it; `cargo tauri build --bundles deb`
      produces a valid 3.8 MB `Skidbladnir_<version>_amd64.deb` on the dev host today,
      containing `usr/bin/skidbladnir` and the hicolor icon set.
- [x] AppImage on Linux — produced by `release.yml` in CI (80 MB for 0.5.0). It is not
      *built* on the dev host, which has no `patchelf` (Phase 1), but it can be *checked*:
      the published `Skidbladnir_0.5.0_amd64.AppImage` was downloaded, unpacked with
      `--appimage-extract`, and passes the 9-check smoke test launched through its own
      `AppRun`.
- [x] The published 0.5.0 `.deb` passes the same smoke test. This closes the gap the
      0.5.0 run left: CI's upload replaced the locally-tested `.deb`, so the file users
      download had never been run. It now has.
- [x] `cargo tauri build` green on **Windows** (NSIS `x64-setup.exe`) — built by
      `release.yml` for 0.5.0. **Compiled and bundled, never launched** — see below.
- [x] `cargo tauri build` green on **macOS** (`.dmg`) — built by `release.yml` for 0.5.0,
      **Apple Silicon only** and never launched.
- [x] First launch of the **Windows** build — and of the *installed* app, not just the bare
      executable. CI builds the NSIS installer, installs it silently to
      `%LOCALAPPDATA%\Skidbladnir`, and runs the full smoke test (13 checks) against the
      installed `skidbladnir.exe`. WebView2 displays AVIF there.
- [ ] "Basic Automation", not "basicautomation", as the Windows publisher. Not a label
      only: Tauri's NSIS installer keeps the install folder under
      `HKCU\Software\<publisher>\Skidbladnir`, and with no `bundle.publisher` the
      publisher is `basicautomation`, from the identifier. Renamed, an in-app update of a
      copy in a custom folder installs to `%LOCALAPPDATA%\Skidbladnir` instead, its
      shortcuts keep opening the old copy, and that copy offers the update again on every
      start. Running the new setup.exe by hand, its "uninstall first" reads the same key
      before any installer hook can run. Needs a migration, tested on Windows over a
      custom-folder install, before the key is set.
- [ ] First launch of the **macOS** build. `tauri-driver` does not support macOS, so the
      `.dmg` still needs a human to open it once.
- [x] An Intel macOS build. `release.yml` cross-builds `x86_64-apple-darwin` on the Apple
      Silicon runner as a fourth matrix entry; the dry run produced
      `Skidbladnir_<version>_x64.dmg` beside the `aarch64` one. Never launched, like the
      Apple Silicon build.
- [x] Stop attaching a locally-built `.deb` to a release that CI then overwrites with
      `--clobber`. Since 1.0.0 releases carry CI's artifacts only. For 1.2.0 the CI-built
      AppImages of both editions (release dry run 36960672813) passed the smoke test on the
      dev host before the tag (42 and 43 checks), and the GPL `.deb` was checked to carry
      the GPL metainfo, `COPYING` and its libheif.
- [x] A `release.yml` workflow that builds all three targets and attaches them to the
      GitHub release for a tag. Triggers on `v*` tags, or manually with a tag input so a
      release whose build failed can be retried without moving the tag. `fail-fast: false`,
      so one platform failing does not deny the release the artifacts the others produced,
      and the collect step **fails loudly** if a platform produced no installer rather
      than uploading nothing.
      **Unverified until a tag exists** — a tag-triggered workflow cannot be exercised
      before the tag it reacts to.
- [x] Decide whether the Tauri updater is in scope. First decided **not for the 0.x
      line** (key custody; an auto-update channel for a pre-parity app; no telemetry to see
      a bad update), to be revisited once the Tauri app was the shipping app and a
      tri-platform release had gone out. Both held by 0.7.0, and the owner asked for it
      (2026-09-28), so it is in — shaped by those three objections:
      1. **Key custody.** The minisign keypair was generated in an interactive session
       with the owner, not by the routine. The private key and its password live with the
       owner and in the repository's Actions secrets
       `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, nowhere else;
       the public half is in `tauri.conf.json`. The routine never touches either.
      2. **Nothing is silent.** The app checks once per launch (release builds only) and
       shows a banner; nothing is downloaded or installed until the user clicks. Every
       download is verified against the public key before it is installed.
      3. **Each install updates in its own format**: the manifest is keyed per bundle
       (`linux-x86_64-appimage`, `linux-x86_64-deb`, `windows-x86_64-nsis`,
       `darwin-{aarch64,x86_64}-app`), so a `.deb` install is never replaced by an
       AppImage. A `.deb` update asks for the admin password through polkit.
- [x] Updater plumbing. `release.yml` builds with `createUpdaterArtifacts` only when the
      signing secret is present (so local and CI builds need no key), and a final
      `manifest` job writes `latest.json` from the signed installers
      (`scripts/updater-manifest.py`) and commits it to the orphan **`updater` branch**,
      which is what installed copies read. Since 1.0.0 neither the manifests nor the
      `.sig` files are attached to the release; they were clutter there. Not
      `releases/latest`: that skips prereleases, and every release before 1.0.0 was one.
      From 1.0.0 `release.yml` holds a tag's artifacts in a draft full release, not a
      draft prerelease. The manifest job never offers a draft, whose files are not public
      and would fail to download: publish the release, then re-run that one job, which
      reuses the run's signed installers. A re-run of an older tag never rolls the branch
      back. Dry runs sign with a throwaway key so the whole path is exercised without
      publishing.
      Verified locally: a signed `.deb` built with a throwaway key, served with a
      generated manifest, raised the banner in the real window; a tampered download was
      refused with "signature verification failed". A `release.yml` dry run signed all
      five updatable installers and wrote a manifest listing every target; its first run
      found that a `dmg`-only target list yields no `.app.tar.gz`, so `app` is now a bundle
      target too. **The first release that can be
      offered as an update is the first one built with the secret set** — 0.7.0 and
      earlier have no updater and must be updated by hand once.
- [x] First tri-platform release of the Tauri app — **v0.5.0**, a prerelease carrying a
      Windows installer, a macOS `.dmg`, a `.deb` and an AppImage.

**Distribution, deferred past the 1.0.0 launch (2026-10-01):**

- [ ] (owner-gated) Code signing: an Apple Developer ID with notarization, and Windows
      Authenticode (Azure Artifact Signing or SignPath Foundation) run through
      `bundle.windows.signCommand` inside the bundle step, so the updater's signatures
      cover the signed files.
- [ ] winget (the standard edition only, or give the GPL edition its own identifier
      first) and Scoop (a bucket of our own now; Extras wants 100+ stars).
- [ ] AUR `skidbladnir-bin` and AppImageHub (the AppStream metainfo is now in place).
      *AUR waits on the update-check switch below:* a `-bin` package repackaging the
      `.deb` carries a binary that identifies itself as a `.deb` install, so its updater
      would try to install `.deb` updates on Arch (found 2026-10-01).
- [ ] Flathub: metainfo, an app ID under `io.basicautomation.*` or
      `io.github.basic_automation.*`, an offline from-source build of every crate, npm
      package and submodule, and an owner-written manifest under Flathub's AI policy.
- [ ] A Homebrew cask (after notarization; the official tap also wants 225 stars), Snap
      (the owner registers the name), and the Microsoft Store (a signed, offline installer).
- [x] AppStream metainfo with PNG screenshots, shipped in the `.deb`, the `.rpm` and the
      AppImage (2026-10-01): `resources/linux/*.metainfo.xml`, one per edition (their
      licences differ), `appstreamcli validate --pedantic` clean, matched to the desktop
      entry and icon by `appstreamcli compose` over the built `.deb`.
      `src-tauri/tests/metainfo.rs` fails a build whose newest `<release>` is not its
      version, so **every version bump adds a `<release>` to both files**. The AppImage's copy
      is built only in CI.
- [ ] (owner decision) A supported way for packagers to turn the update check off: an
      `updater` cargo feature that keeps the two commands registered, and `FLATPAK_ID`
      detection. The smoke tests' environment switch is a test hook, not a documented one.
- [x] An `.rpm` per edition (2026-10-01): bundled libheif and metainfo, renamed to
      `Skidbladnir_<v>_x86_64.rpm` so the editions do not collide, glibc-floor checked
      (`glibc-floor.py` reads rpm payloads itself), offered by the updater
      (`linux-x86_64-rpm`), and installed with dnf on `fedora:latest` by `release.yml`'s
      `rpm-install` job.
- [ ] More platforms: a universal `.dmg`. (The Linux ARM64 leg, `ubuntu-22.04-arm`, shipped
      in 1.2.0: `.deb`, `.rpm` and AppImage for both editions.)

## Phase 6 — Retire Electron

Only once Phase 3 parity is `[x]` and a Tauri release has shipped.

**The Electron app still resolves its dependencies** on Node 26 / npm 11: a dry-run install
adds 267 packages including electron 9.4.4 and electron-builder 20.44.4, with no resolution
failure. Its sources parse (`node --check main.js`, and the single inline `<script>` block
in `index.html`). That is as far as this host can verify it — actually *running* it needs
Windows and a downloaded `cwebp.exe`. So "master still has a working app" holds to the level
that can be checked here, and no further claim is made.

- [x] **Gate added: do not retire Electron until the Windows build has been launched.**
      **Cleared 2026-09-26** — the installed Windows app passes the smoke test in CI.
      Retiring Electron is now unblocked. **The owner gave the go-ahead on 2026-09-27.**
      Both stated preconditions now hold (Phase 3 parity is ticked and a Tauri release
      has shipped), but the Electron app is **Windows-only**, and the Tauri Windows build
      has been compiled and never opened. Retiring the one Windows app that is known to
      have run, in favour of one that never has, would risk leaving Windows users with
      nothing. Clears when the Windows first-launch item in Phase 4 is ticked.
- [x] Remove `main.js`, `index.html`, `index.css` and the Electron `package.json` (its
      dependencies went with it — there was no committed lockfile), the three SVGs only
      `index.html` used, and Dependabot's root npm entry. `build/icon.png`, the 2363 px
      master the Tauri icon set was generated from, moved to `resources/icons/icon.png`
      rather than being deleted. Nothing in the Cargo workspace referenced any of it, as
      Phase 1 required, so `cargo build` was unaffected.
      The product version now lives in `src-tauri/Cargo.toml` alone (`tauri.conf.json`
      inherits it); the root `package.json` was the second copy.
- [x] Remove the `resources/win/bin` cwebp-download step from the README — the whole
      Electron build section went, and the README now describes one app.
- [x] Pin the Electron line for anyone still on it. **The last Electron *binary* stays
      v0.4.3 (2019)**; no new Electron build was made, because it could not be verified —
      running it needs Windows plus a hand-downloaded `cwebp.exe`, and nothing in CI
      exercises it. Its **last source** is the `electron-final` tag, on the final commit
      before removal (477dade), which carries the years of unreleased Electron work.

## Phase 7 — Beyond WebP

The README has promised JPEG 2000 "coming soon" since 2019. Decide it honestly.

- [x] Research and decide the next format. **The answer: AVIF. JPEG XL is a watch item,
      not a decision.** Evidence, gathered 2026-09-24:
      - **AVIF is at roughly 90–93% global browser support** as of early 2026, and is the
        recommended default with a JPEG fallback.
        <https://orquitool.com/en/blog/avif-browser-support-2026-compatibility-webp-switch>
      - **JPEG XL is still flag-gated almost everywhere.** Chrome 145 (February 2026) and
        Firefox 152 (June 2026) restored JXL decoding, but both ship it **disabled behind a
        flag**; only Safari enables it by default, which is about 16% of users. Chrome is
        expected to enable it by default in H2 2026, which would take support to ~85–90%.
        <https://theimagecdn.com/docs/jpeg-xl-explained>
      - JPEG 2000 has no momentum outside medical and archival imaging. The README's
        "coming soon" claim has already been struck.
      Revisit JPEG XL once Chrome ships it unflagged — that single event moves it from
      16% to usable, and it is the trigger to re-open this item.
- [ ] **Chrome turns JPEG XL on by default — update the README when it ships.** The JPEG XL
      project's news page (25 September 2026) says JPEG XL is set to be enabled by default
      in Chrome 155 stable, due early October. When it is out, re-word the README's "JPEG XL
      files open in Safari, and in Firefox and Chrome as each enables it" gap to what is
      actually shipped, and re-check Firefox's status the same way.
      <https://jpegxl.com/news/>
      Re-checked 2026-10-02: jpegxl.com's newest news is still that 25 September post;
      Chrome's stable channel is 154, and Chromium's schedule puts 155 stable on 6 October
      2026. <https://chromiumdash.appspot.com/fetch_milestone_schedule?mstone=155>
      Re-checked 2026-10-03: stable is still 154 (154.0.8037.98), and the schedule still
      says 6 October. <https://chromiumdash.appspot.com/fetch_releases?channel=Stable&platform=Windows&num=1>
      Re-checked 2026-10-04: still 154.0.8037.98. <https://chromiumdash.appspot.com/fetch_releases?channel=Stable&platform=Windows&num=1>
- [x] **JPEG XL is close to its trigger — prepare, do not build yet.** *Superseded:* the
      owner asked for JPEG XL ahead of Chrome, and it shipped in 0.10.0 on libjxl (see
      "JPEG XL output and input" below), with `cjxl`'s whole surface since 0.14.0. Mozilla announced
      (August 2026) that Firefox is shipping JPEG XL, decoded by Google Research's Rust
      `jxl-rs`, and that "Chrome are also intending to ship", expecting cross-browser
      support "before the end of the year". Chrome has not unflagged it yet, so the trigger
      above still stands; but when it fires, the first question is the **encoder** — the
      Rust side has decoders (`jxl-rs`, `jxl-oxide`), and encoding the full control surface
      likely means libjxl again. Research the encoder options (control surface, licence,
      build on all three runners) so the decision is ready.
      <https://hacks.mozilla.org/2026/08/intent-to-ship-jpeg-xl/>
- [x] Add AVIF output. **Design decided here so the work can start; no code yet.**

      *Which encoder.* ~~`libavif-sys`~~ — **superseded 2026-09-26 by `ravif`; see the
      re-check item below.** The original reasoning, kept for the record:
      **`libavif-sys`** (BSD-2-Clause, libavif 1.0.4), not `ravif`.
      Parity is not the constraint — there is no existing AVIF behaviour to preserve — so
      the question is control surface versus build complexity, and Skidbladnir exists to
      expose the control surface. `ravif` wraps `rav1e` and offers quality, speed and
      alpha-quality; libavif reaches the encoder's tuning, chroma subsampling, bit depth
      and range. The C dependency is the same trade already accepted for libwebp, and
      `libwebp-sys` has shown it builds green on all three platforms.

      *How settings are shaped.* The awkward part, and the reason this is written down
      before any code: `EncodeSettings` is WebP-specific — `sns`, `partition_limit`,
      `segments` and the filter surface mean nothing to AV1. Three options were weighed:
      - An **enum** `EncodeSettings::{Webp(..), Avif(..)}`. Cleanest types, but switching
        format in the UI throws away the other format's tuning, and every call site gains
        a match.
      - A **flat struct with unused fields** per format. No, for the obvious reason: a UI
        that shows `sns` for AVIF is lying about what the encoder will do, and this project
        has already fixed that exact class of bug in the mode show/hide logic.
      - **A job with shared settings plus one sub-struct per format, all kept live.**
        Chosen:
        ```
        struct EncodeJob { format: OutputFormat, resize: Resize, webp: WebpSettings, avif: AvifSettings }
        ```
        `resize` is genuinely shared. Each format's own settings persist while the user
        tries the other, which matches how the app already keeps every mode's settings live
        rather than resetting them, and it is what presets and the preferences file should
        store.

      *Migration path, in landable slices:*
      1. **Done.** Rename `EncodeSettings` → `WebpSettings`, lift `resize` out into
         `EncodeJob`, add `OutputFormat` with a single variant. No behaviour change: the 76
         parity settings, the PNG-file and colour-type parity and the CMYK check all stayed
         byte-identical against `cwebp` 1.6.0 through it. `EncodeJob` is the wire and
         on-disk type; it **reads the old flat shape too**, so preset and preferences files
         saved by 0.5.0 load with every value intact — verified in the running window with
         a hand-written 0.5.0-shape preferences file, not only by unit test.
      2. **Done (core only).** `AvifSettings` (quality, alpha quality, speed, bit depth,
         colour model, alpha mode, multi-threading — `ravif`'s defaults), `OutputFormat::Avif`,
         and `crates/skidbladnir-encode/src/avif.rs`. The shared resize runs through
         **libwebp's rescaler**, so a resize gives identical dimensions in either format.
         Output is named `.avif`, dimensions are read back from the file's `ispe` box, and
         the preview labels its encoded side `image/avif`.
         **The gate is a reference decoder, not a reference encoder:**
         `tests/avif_reference.rs` has libavif's own `avifdec` decode seven of our files
         and checks size, transparency and PSNR (47.5 dB at the defaults; quality 20 → 40.8
         dB, 95 → 50.6 dB, so quality demonstrably means something). It was
         mutation-tested — swapping R and B in the encode path drops it to 16.0 dB and
         fails it. Enforced in CI's parity step (`libavif-bin` / brew `libavif`).
         WebP parity was unchanged through it (76/76).
         **An AVIF encode cannot be cancelled mid-way:** `ravif` has no progress hook, so
         cancel is honoured before and after the encode only.
      3. **Done.** A Format selector (WebP / AVIF) above Mode; WebP's Mode, Quality and
         Advanced panels show only for WebP, and an AVIF panel (quality, alpha quality,
         speed, multi-threading, bit depth, colour model, colour under transparency) only
         for AVIF. Resize moved into its own panel because both formats share it. The file
         extension and the progress note follow the format. Verified in the running window:
         switching shows 3 AVIF sliders and hides Mode; **WebKitGTK displays the AVIF
         preview**; an AVIF conversion writes a file `avifdec` reads as 10-bit 4:4:4 with
         alpha. The axe audit now covers the AVIF panel and an on-screen preview too, and
         caught one contrast failure (a 3.91:1 description on the Format cards), fixed.
      4. Output naming and the preview — **done in slices 2 and 3**. What is left:
- [x] Read AVIF **input** (done 2026-09-27, once `nasm` was installed). `SourceFormat::Avif`
      is sniffed from the `ftyp` box — major *or* compatible brand `avif`/`avis`, and a
      HEIC with the same container is refused. `decode_avif` uses `avif-decode` 3.0 over
      rav1d; 16-bit output (10/12-bit AVIF) drops its low byte, as 16-bit PNG does.
      **Gate:** `our_avif_decoder_agrees_with_avifdec` decodes the same 10- and 8-bit files
      with ours and with libavif's `avifdec` — 49.5 and 50.5 dB agreement, alpha identical.
      Mutation-tested: a wrong bit-depth reduction drops it to 4.7 dB and fails.
      **Two findings on the way:**
      - `avif-parse` **panics** on a malformed box (a `debug_assert` — found by the
        corrupt-file test). `decode_avif` catches the panic and returns a decode error, so
        a user's damaged file can never take a conversion down.
      - rav1d's x86 assembly cannot be linked into a `cdylib` (non-PIC relocation against
        `dav1d_dr_intra_derivative`). The Tauri library's `staticlib`/`cdylib` crate types
        existed only for mobile targets and were dropped; it is `rlib` only now.
      Animated AVIF (`avis`) is sniffed but only its first frame would decode — not tested.
- [x] **Widen the AVIF-input gate** beyond two files. `tests/avif_input.rs` has `avifenc`
      write 17 encodings — 8/10/12-bit; 4:4:4, 4:2:2, 4:2:0, monochrome; full and limited
      range; BT.601/709/2020 and identity; straight and premultiplied alpha; lossless — and
      `decode_avif` must agree with `avifdec` (mean ≤ 2, worst ≤ 20) **and land no further
      from the original than libavif does**, within half a level of mean error (macOS
      CI's Homebrew libavif is 0.27 closer on 8-bit 4:4:4, a rounding difference). All 17
      pass on Arch (libavif 1.4.2), Ubuntu (1.0.4) and macOS. Mutation-tested against
      `decode_avif` itself: an off-by-12 narrowing fails 8 of 17. What it established:
      avif-decode upsamples chroma nearest-neighbour (bilinear decoders disagree on siting
      by up to 50 levels at a hard edge); at saturated limited-range pixels libavif is the
      one further from the original (pure blue: libavif 237, ours 250); and **libavif's and
      libheif's greyscale output leaves limited-range monochrome unexpanded** (black comes
      out as 16) while ours expands it, so that case is held to libavif's raw Y plane.
      Enforced in CI's parity step.
- [x] AVIF input honours the `clap`/`irot`/`imir` (crop, rotate, mirror) properties.
      `avif-parse` does not expose them, so `crates/skidbladnir-encode/src/avif_transform.rs`
      reads `pitm`/`iprp`/`ipco`/`ipma` itself and applies the primary item's properties in
      association order (MIAF: crop, rotate, mirror). A `clap` that is not whole pixels
      inside the image is ignored, as libavif does. Never panics on malformed boxes.
      **Gate:** `honours_crop_rotation_and_mirror` in `tests/avif_input.rs` — lossless
      `avifenc` fixtures for each `irot`, each `imir` axis, a crop and all three combined,
      checked **exactly** against the `image` crate's own rotate/flip/crop of the source (an
      independent implementation) *and* against `avifdec` where it applies them — found by
      CI: Ubuntu 24.04's libavif 1.0.4 `avifdec` ignores all three and shows the stored
      image, so there it is reported, not trusted (1.4.2 on Arch applies them and agrees
      exactly). Mutation-tested: clockwise
      rotation fails `irot` 1 and 3; swapped mirror axes fail both `imir`.
- [x] AVIF input decodes grid (tiled) images (done 2026-09-28). `avif-parse` refuses
      `grid` items, so `src/avif_grid.rs` reads the container itself, rewraps each tile (and
      its alpha tile, when the alpha plane is a grid too) as a minimal single-image AVIF for
      the ordinary decoder, and stitches the tiles, cropped to the grid's output size. The
      canvas is allocated only once the first tile proves the layout fits (MIAF: covering the
      output, overhanging by less than a tile), so a hostile descriptor cannot demand a huge
      buffer. The grid's own crop/rotation/mirror still apply. **Gate:** `decodes_grid_avifs`
      in `tests/avif_input.rs` — `avifenc --grid` 2x2 and 4x3 (with alpha) lossless files
      come back exactly as their source, a rotated grid exactly as the rotated source, and a
      lossy 4:2:0 grid within tolerance of `avifdec`; each fixture is checked to really be a
      grid. Mutation-tested: swapped row/column order, dropped alpha tiles and a lost edge
      column each fail it.
- [x] **Show the AVIF preview on every platform** (done 2026-09-27). The encoded AVIF is
      decoded in Rust and sent to the webview as lossless WebP, so no web engine has to
      decode AVIF; the smoke test's "displays the AVIF preview" is a hard check again. The
      "cannot display" notice remains in the window as a fallback that should now never
      show. History: Found by CI, not by reading: Ubuntu
      24.04's WebKitGTK cannot decode AVIF (the smoke test's image load fails there),
      while Arch's can, and the AppImage bundles the Ubuntu build. The window now says so
      instead of showing a broken image, but the comparison is lost. The fix is the same
      AV1 decoder the AVIF-input item needs: decode the preview in Rust and send it to the
      webview as lossless WebP, as the original side already is. The two items should be
      solved together.
      **Blocked on `nasm` (owner-gated), found 2026-09-26.** The right decoder exists:
      `avif-decode` 3.0.0 (20 September 2026, by `ravif`'s author) is pure Rust over
      `rav1d`, the Rust port of dav1d — no C library. But on x86 it depends on `rav1d` with
      default features, which include `asm`, and rav1d's build script then panics with
      "NASM build failed. Make sure you have nasm installed". Cargo feature unification
      means Skidbladnir cannot switch that off from its side. `sudo pacman -S nasm` on the
      dev host (and `nasm` on the three CI runners: apt, brew, choco) unblocks this **and**
      the rav1e `asm` speed-up at once.
      <https://crates.io/crates/avif-decode> · <https://crates.io/crates/rav1d>

      Do **not** start at step 2. Step 1 is the one that can silently change WebP output,
      and it is the one the existing parity tests can prove innocent.
- [x] **Re-check the AVIF encoder choice before slice 2. Decided: `ravif` 0.13**, with
      `default-features = false, features = ["threading"]`.
      - `libavif-sys` was last published July 2024 as `0.17.0+libavif.1.0.4`; upstream
        libavif is at **1.4.2** (26 May 2026). No maintained binding tracks it.
        <https://crates.io/crates/libavif-sys> · <https://github.com/AOMediaCodec/libavif/releases>
      - `ravif` 0.13.0 was published January 2026 and is the AVIF encoder under the
        `image` crate (~15M downloads in 90 days). It is pure Rust over `rav1e`, so
        there is no C toolchain to stand up on the Windows runner.
      - What it exposes: quality, alpha quality, speed, bit depth, internal colour model,
        alpha colour mode and thread count. What it does **not**: chroma subsampling
        (always 4:4:4 — its docs call subsampling "a bad idea for AVIF anyway"), AV1 tune,
        or a choice of codec. That is a smaller surface than libavif's, accepted in
        exchange for a maintained, current encoder.
        <https://docs.rs/ravif/latest/ravif/struct.Encoder.html>
      - Default features are off because `asm` needs `nasm` at build time, which the dev
        host lacks (owner-gated) and would be one more thing for each CI runner.
- [x] `rav1e`'s `asm` is back on (`ravif` default features), now that `nasm` is on the dev
      host (installed 2026-09-27 at the owner's request) and on every CI runner via
      `ilammy/setup-nasm`. Measured on the dev host: a 1600x1200 AVIF at the defaults went
      from 818 ms (0.6.0, pure Rust) to 595 ms, **byte-identical output** (180,742 bytes).
      `nasm` is now a build requirement, stated in the README.
- [ ] Watch **Tauri 3**, do not adopt it. `3.0.0-alpha` releases began appearing in
      September 2026, bringing a CEF runtime option, plugin-API changes
      (`js_init_script` → `initialization_script`) and removed deprecated APIs. Adopting an
      alpha mid-migration would be trading a known platform for an unknown one; the app
      stays on Tauri 2.x until 3 is stable and the migration has shipped.
      <https://github.com/tauri-apps/tauri/releases>
      Re-checked 2026-09-27: `3.0.0-alpha.3` shipped beside `2.12.0` (26 September). Its
      breaking changes that would touch this app: `Plugin` must be `Sync` and its hooks take
      `&self`; plugin closures must be `Fn + Send + Sync`; `run_on_main_thread` is removed in
      favour of a trait. Still an alpha — no change to the decision.
      <https://github.com/tauri-apps/tauri/releases>
      Re-checked 2026-09-28: `2.12.0` and `3.0.0-alpha.3` are still the newest; no change.
      Re-checked 2026-10-01: `2.12.1` (30 September; taken this run) and `3.0.0-alpha.4`
      (1 October). Still an alpha; no change.
      Re-checked 2026-10-03: still `2.12.1` and `3.0.0-alpha.4`.
- [ ] **Drop `macos-private-api`.** Tauri 2.12.1 no longer needs the `macos-private-api`
      feature (or `macOSPrivateAPI` in `tauri.conf.json`) for transparency or fullscreen on
      macOS (tauri-apps/tauri#16166). Skidbladnir enables the feature only for its frameless
      `transparent` window, and the private API is what keeps an app out of the Mac App
      Store. Remove the feature from the workspace `tauri` dependency once a macOS build
      confirms the window still draws transparent — which, since no macOS build has been
      launched, waits on the macOS first-launch item. Tauri `3.0.0-alpha.4` removes the
      feature and `macOSPrivateApi` outright, so it has to go before any move to 3.
      <https://github.com/tauri-apps/tauri/releases>
      <https://github.com/tauri-apps/tauri/releases/tag/tauri-v2.12.1>
- [x] Confirm the encoder is current. **No libwebp upgrade is pending:** 1.6.0
      (9 July 2025) is still the newest release, and it is exactly what `libwebp-sys`
      vendors and what the parity test compares against, so the parity claim is against
      current upstream. Re-check each run. Re-checked 2026-09-25, 2026-09-27, 2026-09-28, 2026-10-01 and 2026-10-03: still 1.6.0.
      <https://github.com/webmproject/libwebp/tags>
- [x] Strike the JPEG 2000 claim from the README — done; the Status section now lists
      WebP as the only output format rather than promising JPEG 2000.
- [x] Decoding/inspection of existing WebP files. `crates/skidbladnir-encode/src/inspect.rs`
      reads dimensions, lossy/lossless/mixed, alpha and animation from the bitstream header
      via libwebp's `WebPGetFeatures`, which does **not** decode the image, so inspecting a
      large file is cheap. Surfaced through path inspection, so selecting a WebP tells the
      user what it already is.
      The window also warns when a selected file is an **animated** WebP, which this
      still-image encoder cannot re-encode.
- [x] Handle animated WebP properly. **Correcting an earlier claim in this file:** it was
      recorded that converting an animation "would keep only the first frame". That was
      asserted without testing and is **wrong** — libwebp's still decoder refuses an
      animated WebP outright, so no frames were ever silently dropped. What was wrong was
      the *message*: the user got "libwebp rejected the file", which explains nothing.
      An animation is now detected before decoding and refused as
      `SourceError::Animated`, whose message says what the file is and why it cannot be
      converted. The UI warning was corrected to match.
      The test **builds its own two-frame animation** with libwebp's `WebPAnimEncoder`
      rather than reading one from an environment variable, so it always runs — a test
      that skips unless someone remembered to set a variable is a test that never runs in
      CI, and this one guards a refusal users depend on.
- [x] Re-encode animated WebP rather than refusing it, using libwebp's `WebPAnimEncoder`.
      This needs a per-frame settings story (do the advanced controls apply to every
      frame?) and a demuxer for the input, so it is a real piece of work, not a flag.
      *Slices:*
      1. **Done (core only).** `crates/skidbladnir-encode/src/animation.rs`: decode every
         frame with `WebPAnimDecoder` (full-canvas RGBA plus durations, loop count and
         background), re-encode with `WebPAnimEncoder`. **The settings story: every WebP
         control applies to every frame** — the per-frame `WebPConfig` comes from the same
         `build_config` the still path uses; target size and PSNR become per-frame. Resize
         uses the still path's `cwebp`-matching rescale on each frame. **Gate:**
         `tests/animation.rs` is byte-identical to libwebp 1.6.0's `img2webp` in 18 cases
         (lossless, lossy, quality, method, sharp YUV, near-lossless, loop count),
         mutation-tested; CI builds a version-matched `img2webp` beside `cwebp`. Found on the
         way: the app's default `-pass 6` changes lossy output even with no target set.
         Controls `img2webp` has no flag for are only proven to *reach* the frames.
      2. **Done.** `encode_file` sends an animated WebP through the animation encoder.
         `load` still refuses one (it returns a single picture, and handing back frame 1 as
         if it were the image is the silent truncation this project refuses), and so does
         **AVIF output — decided: refuse by name, never keep the first frame**, since
         `ravif` writes stills only. The preview shows **both sides as animations**, every
         frame, with a frame count; the size guard counts every decoded frame.
      3. **Done.** The window's warning now says what will happen (WebP: every frame kept;
         AVIF: refused, choose WebP) and the target controls say a target is per frame.
         Verified in the running release binary: the smoke test converts an
         `img2webp`-written 3-frame fixture, finds 3 `ANMF` frames on disk, gets the AVIF
         refusal, and the webview displays the animated preview; axe clean.
- [ ] Animated **AVIF** output (AVIF image sequences). `ravif` writes stills only; this
      needs a different encoder path, and belongs with the AVIF-input decoder work.
- [x] **HEIC output and input** (done 2026-09-28; the owner chose Kvazaar + libheif
      over x265, which would have made the binaries GPL, and over macOS-only ImageIO).
      libheif 1.23.5 (LGPL-3.0) is built by `scripts/build-libheif.sh` from pinned
      submodules as one **shared** library with Kvazaar 2.3.2 (BSD-3, the encoder) and
      libde265 1.1.3 (LGPL-3.0, the decoder) static inside it and **plugin loading off**,
      so no system x265 can ever be loaded. It ships beside the app (`tauri.<platform>
      .conf.json`), found through rpaths set in `src-tauri/build.rs`; its C API is bound in
      `src/heic.rs`. The encoder is requested by name, `kvazaar`. Kvazaar is 4:2:0 only, so
      quality is the only control and there is no lossless HEIC. **Gate:**
      `tests/heic_reference.rs` has the runner's own libheif (`heif-dec`, a separate
      build, with our library path removed) decode our files at the right size and
      alpha with PSNR rising with quality, and agree with our decode. Licence notices for
      every shipped native library are generated into `THIRD-PARTY-NOTICES.md` by
      `scripts/third-party-notices.py`, bundled into every installer, and checked in CI.
      **Not yet:** HEVC patent-pool licensing is unexamined (the owner accepted the risk,
      as GIMP, ImageMagick and ffmpeg do); iPhone grid images and HDR gain maps decode as
      libheif decodes them but are untested with real phone files.
      *Original plan, kept for the record:* **HEIF/HEIC input.** The common case is iPhone and macOS photos, which are HEIC
      by default. Converting those to WebP/AVIF is something users do, so this is a
      real input format, not a curiosity. Added to the queue 2026-09-28.
      - *Sniffing already half-exists:* `SourceFormat` reads the `ftyp` box for AVIF and
        explicitly **refuses** a HEIC with the same container. This item turns that refusal
        into a decode for the `heic`/`heix`/`mif1` brands.
      - *Decoder choice is the whole decision, and it is a licensing one.* `libheif-rs`
        wraps libheif (LGPL-3.0) and needs libde265 for HEVC. That is a C toolchain on
        three runners, and dynamic-linking obligations that `cargo deny` will have to be
        told about. `heic-rs` claims pure Rust (MIT OR Apache-2.0, no C toolchain) and
        covers iPhone grid images, 8/10-bit and 4:2:0/4:2:2/4:4:4. It is young, so it has
        to earn trust the way `avif-decode` did: an agreement gate against a reference
        decoder (libheif's `heif-dec`) in CI, before it ships.
        <https://github.com/tbraun96/heic-rs> · <https://crates.io/crates/libheif-rs>
      - *HEVC is patent-encumbered.* Decoding in a free desktop app is the common practice
        (libheif and GIMP ship it), but check this, and write the answer down, before
        release.
      - *Out of scope: HEIC output.* Encoding HEVC means x265 (GPL) and the patent pool.
        AVIF already is "HEIF with a royalty-free codec", and it is the output format to
        offer anyone who wants HEIF.
      - *Carry over from HEIC:* EXIF orientation (iPhone photos rely on it) and the
        embedded colour profile. Check whether the AVIF path already honours either.
- [x] **The GPL edition: HEIC through x265** (done 2026-09-28; the owner changed their
      mind about x265 and asked for two builds rather than one). The `gpl` cargo feature
      (and `full`, every optional feature) builds the app against a libheif that
      `scripts/build-libheif.sh --edition gpl` builds around **x265 4.2** (GPL-2.0-or-later,
      a Bitbucket submodule) with its 8- and 10-bit encoders linked together, in place of
      Kvazaar. That build is distributed under GPL-3.0-or-later as a whole; the source
      stays ISC, and the standard edition stays ISC and unchanged. **Guard:** each libheif
      build records its edition in `EDITION` beside it, `build.rs` refuses to link the
      other edition's, and a test asserts the linked libheif has this edition's encoder and
      not the other's. **Controls:** lossless (RGB kept as RGB at 4:4:4), 4:2:0/4:2:2/4:4:4,
      8- or 10-bit (Main 10, fed 10-bit RGB so the colour conversion runs at 10 bits),
      x265's ten presets and four tunings, TU intra depth, and through libheif's `x265:`
      pass-through: adaptive quantisation mode and strength, psy-RD, psy-RDOQ, deblocking
      on/off with its tC and beta offsets, and SAO. Settings a standard edition cannot
      write are refused there and reset to Kvazaar's when a preset or preferences file
      from the GPL edition is read. **Found on the way:** libheif encodes transparency with
      a *fresh* encoder that gets only its listed parameters, not `x265:` ones, so those
      shape the colour image only; and x265 refuses placebo's TU inter depth in the 16-px
      CTUs libheif uses under 32 px, which is refused up front for transparent images.
      Opaque images now carry no alpha image in either edition. **Gate:** unit tests for
      every chroma, bit depth, preset, tune and AQ mode at every CTU size; lossless exact;
      and `heif-dec` reads lossless exactly and 4:4:4, 4:2:2 and 10-bit. **Distribution:**
      each release builds both editions (`Skidbladnir-GPL_*` for the GPL one), attaches
      `Skidbladnir-<version>-source.tar.gz` with every submodule, and gives each edition its
      own updater manifest (`latest.json`, `latest-gpl.json`). Notices are per edition,
      and the GPL edition bundles the GPL text as `COPYING`. **Not yet:** 12-bit (Main 12),
      and HEVC's patent pools, exactly as for Kvazaar.
- [x] **JPEG XL output and input** (done 2026-09-28, ahead of Chrome at the owner's
      request, so libraries can be converted in anticipation). libjxl 0.12 is bound
      directly in `src/jxl.rs` — `jpegxl-rs`/`jpegxl-sys` are GPL-3.0 and cannot be linked
      into an ISC app — and built statically from `jpegxl-src`'s source via CMake
      (`SKIDBLADNIR_LIBJXL=vendored` in CI and releases; a CMake-less dev host links the
      system libjxl through `pkg-config`). Controls: quality (libjxl's own quality→distance
      map), effort, lossless, **lossless JPEG recompression** (on by default, as in `cjxl`;
      skipped when a resize takes effect) and multi-threading. `jxl-oxide` decodes for the
      preview, the reported dimensions and JPEG XL input. **Gate:** `tests/jxl_reference.rs`
      has `djxl` decode our files (size, alpha, PSNR rising with quality, lossless exact),
      rebuild a recompressed JPEG byte for byte, and agree with `jxl-oxide` (54 dB lossy,
      exact lossless). Note: `jpegxl-src`'s crates.io metadata says BSD-3-Clause but its
      `lib.rs` carries a GPL header; it only runs at build time and nothing of it is
      linked, but replacing it with our own CMake invocation over a pinned libjxl source
      would remove the question entirely.
      *Original plan, kept for the record:* **JPEG XL input, then output.** Supersedes the "watch item" in the format decision
      above: the trigger it named is close. Firefox 157 enables JXL by default (due the
      end of September 2026), and Chrome/Edge have formalised their intent to enable it by
      default after shipping it behind a flag in Chrome 145.
      <https://mintec.co/blog/jpeg-xl-firefox-pipeline-imagenes/> ·
      <https://www.phoronix.com/news/Chrome-145-Released>
      1. **Input first. It is cheap, and useful today** for anyone who already has .jxl
         files. `jxl-oxide` (0.12, pure Rust, MIT/Apache) is a complete decoder with no C
         toolchain. Sniff the bare codestream (`FF 0A`) and the ISOBMFF container
         (`JXL ` box). Gate it the way AVIF input was gated: our decode against libjxl's
         `djxl` on the same files, in CI.
         <https://crates.io/crates/jxl-oxide>
      2. **Output once Chrome enables it by default** (re-check before starting). The
         encoder is libjxl, via `jpegxl-rs`/`jpegxl-sys` (BSD-3). That is a C++ build on
         three runners, so price it the way the AVIF encoder choice was priced. Its
         controls map naturally onto the existing panel: distance/quality, effort
         (1–10, the analogue of `method`/`speed`), and lossless. One control is worth
         adding: **lossless JPEG recompression**, which re-packs a JPEG into JXL about 20%
         smaller and can reconstruct the original JPEG bit for bit. No other output format
         here can offer that.
      3. The preview already decodes AVIF in Rust and shows lossless WebP, so JXL gets a
         preview on every web engine the same way, whatever the browsers do.

- [ ] **Intermittent stack overflow in the animation tests on Windows CI** (seen once,
      2026-09-28, run 36521408006: `a_still_webp_decodes_as_one_frame`,
      `a_reference_animation_decodes_as_built` and `lossless_round_trip_is_exact` overflowed
      together; the next run on the same code passed). Each builds its fixture with
      `WebPAnimEncoder` first, whose candidate encodes are stack-heavy, and in a debug build
      the `cc`-compiled libwebp is unoptimised. Not reproducible on the Linux host. Next:
      measure the stack high-water mark of an animation encode on Windows, then either
      optimise the C dependencies in the dev profile (`[profile.dev.package.libwebp-sys]
      opt-level`, re-running the parity gates to prove the output is unchanged) or run the
      app's animation encodes on a thread with a stated stack size — not raise
      `RUST_MIN_STACK` in CI to hide it.
      **Measured 2026-10-01, and the stack-depth hypothesis does not hold on Linux:** each of
      the three test bodies (and a lossy animation encode) runs on a main thread limited by
      `ulimit -s` to **under 50 KiB**, with libwebp's C at `-O0` (debug) and at `-O3` alike —
      versus the 1 MiB / 2 MiB Windows gives a main / test thread. Two things the failing
      run's log does show: the three tests are exactly those that decode *lossless* (VP8L)
      data through `WebPAnimDecoder` (`a_still_webp_decodes_as_one_frame` never touches
      `WebPAnimEncoder`), and those test binaries link with
      `LNK4098: defaultlib 'libcmt.lib' conflicts with use of other libs` — a static and a
      dynamic C runtime in one process (`native/heic_jpeg.cc` is built `static_crt(true)` to
      match libjxl's; see the next item). Revised next step: make the CRT consistent, then
      reproduce on the Windows runner (a dispatch-only job running `tests/animation.rs` in a
      loop); only if it still overflows, measure there. **The CRT is now consistent**
      (0.15.0, below); the overflow did not recur in that PR's Windows runs, which proves
      little for an intermittent failure — keep watching before ticking.
      **Measured again 2026-10-04, for every encoder:** `scripts/stack-probe.sh` (with
      `examples/stack-probe.rs`) bisects the smallest stack each encoder's heaviest settings
      survive with *every* thread limited, the encoders' own worker threads included (glibc
      takes their default from `ulimit -s`; on Windows a thread started without a size gets
      the executable's 1 MiB). On the dev host, release build: AVIF at speed 0 needs 124 KiB,
      everything else (WebP `-m 6` lossy and lossless, animated WebP from GIF, JPEG XL
      `-e 9` lossy and lossless, HEIC) at most 44 KiB; debug build: at most 92 KiB. This
      mattered beyond the tests: gpui runs the window's background tasks, conversions and
      previews included, on the Windows thread pool (`TrySubmitThreadpoolCallback`,
      `gpui-windows` 1.17.2 `dispatcher.rs`), whose threads have that 1 MiB, so the gpui
      window on Windows has about 8x headroom (MSVC's frames are not measured, and can be
      larger). Nothing on Linux comes near the 2 MiB of a test thread, so the overflow is
      still unexplained there.
- [x] **Concurrent HEIC encodes tripped libheif's memory limit** (found and fixed
      2026-10-01/02 by CI: two of PR #68's macOS parity runs failed one HEIC case each with
      "Memory usage of 18446744073709539283 bytes ... exceeds the security limit", while the
      same commits passed in the other runs). libheif 1.23.5 counts an image's memory
      against its context's limits, keyed by their address (`security_limits.cc`). The
      shim's HEIC reader (`load_heif`) freed its context while the decoded image lived on;
      a context allocated later at the same address (another thread's, in parallel tests;
      a preview overlapping a conversion, in the app) had its count taken below zero when
      that image was released. The reader's context now lives until its image is released
      (`Loaded::context` in `native/heic_shim.c`). Gate: `concurrent_encodes_keep_libheif_memory_accounting_sound`
      (8 threads x 25 HEIC and PNG sources). It does not reproduce on Linux, where glibc's
      per-thread arenas rarely reuse an address across threads, so macOS CI is its real
      test. Worth reporting upstream: the tracker could refuse to re-register a live key.
- [x] **One C runtime on Windows** (2026-10-01): everything on the static runtime —
      `.cargo/config.toml` `+crt-static` for `x86_64-pc-windows-msvc`, and
      `CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded` for libjpeg-turbo, libaom and libavif in
      `build.rs`; libheif stays a DLL on its own runtime. **Trap found:** CI's
      workflow-wide `RUSTFLAGS` *replaces* the config's target rustflags, so the first CI
      run built the dynamic runtime and went green meaninglessly (`LNK4098` still there);
      CI's Windows jobs now set the flag in `RUSTFLAGS` themselves, while `release.yml` (no
      `RUSTFLAGS`) takes it from the config. Verified by CI run 36895266768: `LNK4098` 0
      times (was 3), both editions' Windows suites green (263 passed on the standard), and
      the installed NSIS app passes the Windows smoke test. Not run on a Windows machine
      outside CI. *Original item:* The MSVC test binaries link both `libcmt` (static CRT:
      libjxl from `jpegxl-src`, and `native/heic_jpeg.cc` to match it) and the dynamic CRT
      the Rust toolchain and the other `cc`-built C use, which the linker warns about
      (`LNK4098`, seen in run 36521408006). Two CRTs mean two heaps and two sets of CRT
      state in one process; nothing is known to cross between them, but it is the kind of
      thing that produces failures like the intermittent overflow above. Either build every
      C/C++ dependency against the static CRT (`+crt-static` for the Windows target) or
      libjxl against the dynamic one, and re-run the Windows parity, smoke and installer
      jobs.

## Phase 8 — Reference-CLI parity

**The claim this phase makes true:** for a still image, every result the reference
command-line encoder can produce, Skidbladnir can produce — `cwebp` for WebP, `avifenc`
for AVIF, `cjxl` for JPEG XL, `heif-enc -e kvazaar` for HEIC — proven by parity tests that
run the real tool and compare bytes. The owner's scoping (2026-09-28): AVIF moves to
libavif + libaom so `avifenc`'s surface is reachable; HEIC stays on Kvazaar (x265 is GPL)
and its claim says "with Kvazaar"; multi-image features are queued below, not built now.

- [x] WebP: every `cwebp` option, one field per flag. `-preset` and `-z` are buttons the
      Rust core applies, as they are shorthands on the command line too. 246 parity cases,
      including `cwebp`'s libpng gamma correction (`png_gamma.rs`, libpng 1.6.58's tables).
- [x] JPEG XL: every `cjxl` option for a still image. 202 parity cases.
- [x] AVIF: libavif + libaom replace `ravif`; every `avifenc` option for a still image,
      its still path ported as `native/avif_shim.c`. 191 parity cases.
- [x] HEIC: every `heif-enc` option Kvazaar can honour, its still path ported as
      `native/heic_shim.c`. 147 parity cases. `-L` cannot work with Kvazaar (it asks for a
      `chroma` parameter Kvazaar lacks), so lossless is `-p lossless=true`; `-b` only
      affects 16-bit input, which Kvazaar refuses; `--enable-metadata-compression` needs a
      libheif built with zlib, which the default build is not.
- [x] Each format's window exposes its whole surface (a panel per format, the flag in
      every control's help), and presets round-trip it: presets store the `EncodeJob`
      itself. The accessibility audit covers every control with every section open.
- [x] Decode JPEG input the way each reference tool does: libjpeg-turbo 3.2.0 (a pinned
      submodule, linked statically) with libjpeg's defaults for `cwebp` and `cjxl`, and
      `avifenc`'s and `heif-enc`'s own JPEG readers compiled into their shims, reading from
      memory, so their direct YCbCr copies happen exactly when theirs do. zune-jpeg had
      differed by up to 3 levels on a quarter of the samples.
- [x] CI builds every reference from the sources the app links, reading JPEG through
      the same libjpeg-turbo (`scripts/build-reference-tools.sh`: `cwebp`, `img2webp`,
      `gif2webp`, `avifenc`, `cjxl`, `heif-enc`), and the `parity` job requires all 2,861
      cases.
- [x] Merged with 0.12 and 0.13 (2026-09-29), shipped as 0.14.0. The GPL edition's x265
      controls are `heif-enc`'s `-L` and `-p` parameters in the per-format HEIC settings;
      0.12's job-level metadata toggles load into each format's own options (`cwebp
      -metadata`, `avifenc --ignore-*`, `cjxl -x strip=`, and which kinds `heif-enc` keeps);
      the animation and GIF gates (`img2webp`, `gif2webp`) and 0.13's whole-surface
      WebP-, JPEG- and TIFF-file gates run against the per-CLI encoder and pass, 2,254 WebP
      cases in all. Malformed metadata that `cwebp -metadata` refuses is still refused.
      The one result not reproduced is 0.13's deliberate divergence: `cwebp` premultiplies
      a straight-alpha TIFF through libtiff, and Skidbladnir keeps its colours.
- [x] **PNM/PAM input** (2026-10-01): binary `P5`, `P6` and `P7`, read by
      `src/pnm.rs` as libwebp 1.6.0's `imageio/pnmdec.c` reads them (line-at-a-time header,
      `(v * 255 + maxval / 2) / maxval`) and handed to libjxl as `cjxl`'s
      `lib/extras/dec/pnm.cc` hands them over (`log2(maxval + 1)` bits,
      `JXL_BIT_DEPTH_FROM_CODESTREAM`, perceptual sRGB). **Gate:** `tests/pnm.rs` — 143 of 143
      byte-identical to `cwebp`, 20 of 20 to `cjxl`; `cwebp`'s refusals (`P1`-`P4`, a
      one-line header, truncation) checked against `cwebp`. `cwebp` reads no `P1`-`P4`, and
      neither does `cjxl`, so no reference exists for them.
- [x] **PFM input** (2026-10-01), as `cjxl` reads it: 32-bit floats handed to libjxl as
      they are (bottom-up rows, the scale's sign as byte order). **Gate:**
      `matches_cjxl_reading_pfm` — 12 of 12 byte-identical. Other formats get it clamped to
      0..1 at 8/16 bits, with no reference to match (only `cjxl` reads PFM).
- [ ] **libjxl 0.12.0 fails a lossless float encode at effort 3** ("Residual overflow",
      `enc_encoding.cc:314`) — `cjxl` itself fails the same PFM, so the PFM gate pins that
      both refuse it. It matches libjxl's open issue #4902, "Lossless fp32 encoding fails
      when the image mixes negative and positive values" (a suspected 0.12.0 regression;
      the fixture's samples run from -0.05 to 1.06), and #4905 reports the same error with
      patches. Check whether a libjxl release fixes it before the next libjxl bump, and
      turn the pin into a parity case then.
      <https://github.com/libjxl/libjxl/issues/4902> ·
      <https://github.com/libjxl/libjxl/issues/4905>
      Checked 2026-10-01: #4902 is still open, and PR #4948 ("modular: wrap residual
      subtraction for lossless fp32 predictors", open since 20 August 2026) says it fixes
      it, with a regression test. libjxl 0.12.0 is still the newest tag; watch for a
      release carrying #4948. <https://github.com/libjxl/libjxl/pull/4948>
      Re-checked 2026-10-03: #4902 and #4948 still open, 0.12.0 still the newest tag.
- [x] **A still GIF into JPEG XL, as `cjxl` reads it** (2026-10-01). `cjxl` reads GIF with
      its own reader (`lib/extras/dec/gif.cc`): three colour channels, alpha only when a
      pixel is transparent, perceptual sRGB — and it counts GIF a lossy input, so with no
      `-d`/`-q` it encodes losslessly. Skidbladnir took the generic path (relative intent,
      distance 1). **Gate:** `matches_cjxl_reading_a_still_gif` in `tests/gif.rs` — four
      still GIFs (opaque, transparent index, local palette + interlaced, a loop extension) x
      five settings including no target: 20 of 20 byte-identical to `cjxl`. Mutation: the
      generic path fails 16, the distance-1 default fails 8.
- [x] **TIFF into HEIC, held to `heif-enc`** (2026-10-01; test only, nothing needed
      changing): `heif-enc` reads TIFF with libtiff (`heifio/decoder_tiff.cc`).
      `matches_heif_enc_across_tiff_inputs` — 8-bit RGB (plain and LZW) and straight-alpha
      RGBA x two settings: 6 of 6 byte-identical. A 16-bit TIFF is refused by Kvazaar in
      `heif-enc` too.
- [x] **WebP into HEIC, held to `heif-enc`** (fixed 2026-10-01). `heif-enc` reads a *lossy*
      WebP straight into YCbCr 4:2:0 planes (`heifio/decoder_webp.cc`: `WebPDecode` into
      `MODE_YUV(A)`, never RGB), where Skidbladnir went through RGB — 4 of 8 conversions
      differed (every lossy one). A lossy WebP now reaches the shim as its own 4:2:0 planes
      (`encoder::webp_yuv420_planes`, the same decode the `cwebp` route uses, and a third
      input layout in `native/heic_shim.c`). **Gate:** `matches_heif_enc_across_webp_inputs`
      — lossless and lossy, with and without alpha, and an odd-sized lossy one: 9 of 9
      byte-identical. GPL edition: the same input path, built and checked by CI only.
- [x] **HEIC into HEIC, held to `heif-enc`** (fixed 2026-10-01). `heif-enc` decodes a HEIF
      input with libheif in the file's own colourspace and chroma, NCLX passed through and
      transformations applied, and keeps its first Exif and XMP blocks
      (`heifio/decoder_heif.cc`). Skidbladnir decoded to RGB and **dropped the source's Exif
      and XMP**. The shim now loads an untouched HEIC itself, as that reader does
      (`load_heif` in `native/heic_shim.c`); the window's Exif/XMP toggles still leave
      them out. With the ICC toggle off, the pixels go through RGB (the reader cannot drop
      a profile). **Gate:** `matches_heif_enc_across_heic_inputs` — sources written by
      `heif-enc` (RGB, RGBA, gray, odd size, every kind of metadata) x two settings: 10 of
      10 byte-identical, plus a direct check that Exif/XMP are kept and left out as asked.
      Mutation: the old route fails it (on the dropped metadata first). AVIF-in-HEIF input
      is not covered: neither our libheif nor the reference `heif-enc` decodes AV1.
- [x] **Decided (owner, 2026-10-01): correct TIFF colours by default, with an opt-in to
      reproduce the tools.** `EncodeJob::tiff_alpha_like_reference` ("TIFF input → Read
      transparency as the official tool does", off by default): WebP from a straight-alpha
      TIFF is premultiplied as `cwebp`/libtiff do, and HEIC from a premultiplied TIFF takes
      the stored samples as `heif-enc` does. **Gates:**
      `matches_reference_cwebp_through_a_straight_alpha_tiff_when_asked` (8- and 16-bit x
      138 settings: 276 of 276 byte-identical to `cwebp`) and
      `matches_heif_enc_through_a_premultiplied_tiff_when_asked` (2 of 2 to `heif-enc`, and
      the default checked to differ). Mutation: either branch removed fails 275 of 276 and
      2 of 2. This also settles the straight-alpha exception below.
- [x] (owner decision) **Premultiplied-alpha TIFF into HEIC.** Decided above. Since 2026-10-01 an
      associated-alpha TIFF is un-multiplied as `cwebp` does, for every format. Reading
      `heifio/decoder_tiff.cc`, `heif-enc` instead takes associated alpha as plain alpha
      (it un-multiplies nothing unless `--premultiplied-alpha` marks the image), so its
      HEIC of such a TIFF is darker where semi-transparent — the mirror image of the
      straight-alpha exception kept for `cwebp`. **Run 2026-10-01:** a 16x16 TIFF stored as
      premultiplied (100, 50, 25) at alpha 128 — straight colour (200, 100, 50), which is
      how ImageMagick reads it — comes out of `heif-enc -e kvazaar -q 100` and `heif-dec`
      as (100, 50, 25) at alpha 128. Decide whether HEIC keeps true colours (as now) or
      follows `heif-enc`, then pin it with a test either way.
- [x] **Y4M input into AVIF, held to `avifenc`** (2026-10-01): `avifenc`'s own reader
      (`apps/shared/y4m.c`) is compiled into the shim, reading from memory through a
      `tmpfile`, so a Y4M's planes are encoded as they are, with `avifenc`'s Y4M rules (`-y`
      and `-r` ignored, `-d` must match, no `-d D,E`). The first frame only. For every other
      output, and for the preview, it is converted to RGB with libavif's defaults for an
      unsignalled image (BT.601, the file's range); no reference tool makes that conversion
      for WebP or JPEG XL. **Gate:** `matches_avifenc_across_y4m_inputs` — 4:2:0, 4:2:2,
      4:4:4, 4:4:4 with alpha, mono, 10- and 12-bit, full range, odd size x four settings,
      plus lossless and `-d 10`, and both refusals: 40 of 40. Mutation: the RGB route fails
      40 of 40.
- [x] **Y4M into HEIC, held to `heif-enc`** (2026-10-01). `heif-enc`'s reader
      (`heifio/decoder_y4m.cc`) takes every Y4M as 8-bit 4:2:0 planes whatever its `C` tag,
      and refuses a frame line with parameters. Where that reading is the file's (8-bit
      4:2:0, or no tag) the planes go to the shim as they are, through the lossy-WebP
      planes layout; any other Y4M, which `heif-enc` misreads or refuses, takes the RGB
      conversion instead — correct colours, as with the TIFF cases, and no reference.
      **Gate:** `matches_heif_enc_across_y4m_inputs` 8 of 8 (no tag, `C420jpeg`,
      `C420mpeg2`, 61x45 `C420` x two qualities); the RGB route fails 8 of 8.
- [x] **PGX input, held to `cjxl`** (2026-10-01): `src/pnm.rs` reads it as
      `lib/extras/dec/pgx.cc` does, refusals included (signed, over 16 bits). `cjxl` hands
      libjxl the samples at their container's full range, which is right only at 8 and 16
      bits — run here, a 12-bit 4095 decodes as 256 of 4095 — so those two depths are
      matched and every other depth is read as meant (rescaled), with no reference.
      **Gate:** `matches_cjxl_reading_pgx` 16 of 16 (8- and 16-bit, both byte orders, CRLF,
      the optional space; four settings), the 12-bit file checked to be read right and to
      differ from `cjxl`, and both refusals. Mutation: the generic route fails 16 of 16.
- [x] Raw YUV input, as `cwebp -s` reads it (2026-10-02): a `.yuv` file (known by its
      extension; raw planes have no signature) is 8-bit I420 at `EncodeJob::yuv_size`,
      set under "Raw YUV input" in both windows. Into WebP the planes go straight into a
      YUV picture, as `ReadYUV` puts them, and through libwebp's own YUV-to-ARGB on the
      paths that need it; the other formats get that RGB. Parity gate
      `matches_reference_cwebp_through_raw_yuv`: 138 of 138 settings byte for byte on a
      67x49 fixture (odd, so the chroma planes round up); with the planes sent through
      RGB instead, 77 diverge. A wrong length and a missing size are refused, as `cwebp`
      refuses them.
- [x] `heif-enc --raw` has no result to reproduce in this build (checked 2026-10-02): its
      reader (`heifio/decoder_raw.cc`) makes one typed component in a custom colour space,
      which only libheif's uncompressed codec can write, and the libheif Skidbladnir ships
      and the reference `heif-enc` are built with HEVC encoders only. Run here:
      `heif-enc --raw-width 8 --raw-height 8 --raw-type uint8 m.raw -o m.heic` fails with
      "Unsupported feature: Unsupported color conversion", and this `heif-enc` has no `-U`.
      Revisit only if uncompressed HEIF output is ever added.
- [x] EXR input, as `cjxl` reads it (2026-10-02): `src/exr_input.rs` reads with the `exr`
      crate (BSD-3-Clause, pure Rust, already in the tree through `image`) and picks what
      libjxl's `exr.cc` picks — R/G/B or the first layer's triple, else the first channel as
      gray, that prefix's alpha (premultiplied), the display window with the data window
      placed in it, linear light with sRGB primaries or the file's chromaticities, white
      luminance as the intensity target, and its refusals (`UINT`, subsampled, mixed
      types). JPEG XL gets the samples as stored (half or float), alpha flagged
      premultiplied unless `--premultiply` says otherwise; the other formats get the sRGB
      curve, clipped at white. The reference `cjxl` is now built with OpenEXR
      (`build-reference-tools.sh` fails if it is missing; CI installs `libopenexr-dev` and
      Homebrew's `openexr`). Gate `matches_cjxl_through_exr`: 134 cases over 10 files (half
      and float, RGB/RGBA/gray, a named layer, data windows inside and past the display
      window, P3 chromaticities with 203 nits, uncompressed, RLE, ZIP, PIZ, PXR24, B44,
      DWAA and DWAB), 126 byte for byte and 8 that `cjxl` and Skidbladnir both refuse (libjxl 0.12.0 fails
      `JxlEncoderProcessOutput` on half-float alpha at `-d 25`, `-q 0` and resampling 2-8).
      Dropping the premultiplied flag fails 84, and declaring halves as floats 114, of the
      first 124 (before the DWA files were added).
- [x] EXR channels beyond colour and alpha (2026-10-02): written as `cjxl` writes them, each
      a named optional extra channel after the alpha, with a value-initialised info (not
      `JxlEncoderInitExtraChannelInfo`'s) at its own half/float depth, and its buffer after
      the frame. 15 more cases in `matches_cjxl_through_exr` (RGBA with a float depth and a
      half mask, gray with a depth, two layers with the second as extras), 149 in all;
      leaving out the names, or the channels, fails all 15.
- [x] JPEG XL to JPEG XL, as `cjxl` converts it (2026-10-02): `cjxl` decodes a `.jxl`
      input with libjxl (`DecodeImageJXL`, `coalescing=false`), and so does Skidbladnir now
      (`jxl::decode_like_cjxl`, the decoder already linked): float samples at the
      codestream's depth through an image callback (as `use_image_callback` has it), alpha
      kept premultiplied or not as coded, orientation undone and the Exif box's offset
      stripped and orientation reset (`ResetExifOrientation`), XMP and JUMBF, the data's
      colour as an encoding or else ICC, every extra channel past the first alpha with its
      own description and name, and the decoder's basic info and frame header (with its
      name) handed to the encoder as `cjxl` hands `ppf.info` over. Gate
      `matches_cjxl_from_jpeg_xl`: 126 cases over six sources `cjxl` wrote (lossless and
      lossy RGBA, 16-bit gray, Exif orientation 6 with XMP, float from PFM, EXR with extra
      channels; the option surface and every hint on the first), 124 byte for byte and 2
      that both refuse (124 in a debug build; see below). Mutations: no orientation reset fails 6; ICC for every colour, 20.
- [x] A lossy JPEG XL made lossless matches too, in an optimised build (2026-10-02). In a
      debug build it differed in 2 of 9,216 samples by one level: the `cmake` crate builds
      libjxl at the Cargo profile's optimisation, so a debug test links an unoptimised
      decoder whose float maths round differently from the reference's Release build. Not
      a SIMD target: both builds carry AVX2, SSE4 and SSE2 only (`nm`). `cargo test
      --release` (CI's parity job, and what ships) matches all 126; the gate leaves the two
      out only under `debug_assertions`.
- [x] A JPEG XL's IPTC and `jhgm` boxes (2026-10-02): read as `cjxl` reads them and written
      back as it writes them (IPTC as an `xml ` box after the XMP and JUMBF, then `jhgm`;
      `--container=0` strips them with the rest). 6 more cases, from `cjxl`'s own container
      with the two boxes appended: 132 in all, release build; dropping the boxes fails 5
      (the sixth is `--container=0`).
- [ ] JPEG XL to JPEG XL, not matched yet: several frames or layers (animation, deferred with
      the multi-image work) and a cropped layer — each converted from its pixels instead.
- [ ] HTJ2K-compressed EXR: the `exr` crate cannot decompress it (`compression/mod.rs`
      returns "yet unimplemented compression method").
- [ ] Animated GIF and APNG into `cjxl` (with the multi-image work below).
- [x] Animated WebP's own options as controls (2026-10-01): `-mixed`, `-min_size`,
      `-kmin`/`-kmax`, `img2webp -loop` and `gif2webp -loop_compatibility`
      (`WebpAnimation`). Found on the way: under `-mixed` the encoder still reads the
      config's own `lossless`, which `gif2webp` sets lossy and `img2webp` leaves lossless.
      **Gates:** `matches_reference_img2webp` 42 cases (was 18),
      `matches_reference_gif2webp_with_animation_options` 70 of 70; every option
      mutation-tested.
- [x] `img2webp -d` given once for every frame (2026-10-01): `WebpAnimation::frame_duration`,
      "Set every frame's duration". **Gate:** two cases in `matches_reference_img2webp`
      (46 of 46 now); ignoring it fails 4.
- [x] `img2webp`'s frame options (2026-10-02): `WebpAnimation::frame_changes`, each "from
      frame N on: lossy or lossless, quality, method, exact, duration", applied per frame in
      frame order as `img2webp` carries an option from the frame it precedes to every later
      one (`animation::for_frame`; `-lossy`/`-lossless` ignored under `-mixed`, as there).
      In both windows' Animation section as a list of changes (the gpui window gained
      `SKID_EXPAND` to open a disclosure for a check). Gate: 10 cases at two loop counts in
      `matches_reference_img2webp` (66 in all), the harness putting each change's flags
      before its frame; ignoring the changes fails 18, applying them out of order 4. GIF
      input has no such options in `gif2webp`, so there they apply on top, with no
      reference. Smoke test 43/43; axe clean with a change added in every
      "every control showing" state.
- [x] GPL edition: `heif-enc`'s `-p x265:<param>` beyond the fixed set of x265 controls
      (2026-10-01): `HeicSettings::x265_parameters`, a list of `KEY=VALUE` passed after the
      controls (so one overrides them), refused in the standard edition and dropped when a
      GPL settings file loads there. **Gate** (GPL build): `free_form_x265_parameters_reach_x265`
      — the controls' own value given again changes nothing, another value does, a
      parameter with no control does, the result decodes, and an unknown parameter is an
      error. Checked by decoding, like all x265 output.
- [x] Parity on macOS again (2026-10-01): the `parity` job is a matrix over
      `ubuntu-latest` and `macos-latest` (Apple silicon), with `--no-fail-fast`. Its first
      macOS run found a test bug, not an encoder one: the AVIF, HEIC and JPEG XL gates named
      scratch directories after the clock, which ticks in microseconds on macOS, so
      parallel tests shared and deleted one directory; a counter replaced it. Every gate
      then passed on macOS in CI run 36956355147.
- [x] Parity on Windows: the reference tools would have to build there (`makefile.unix`
      does not; CMake might). A first slice, sized 2026-10-02 from libwebp's sources:
      libwebp's CMake builds `cwebp` with MSVC, but on Windows it reads PNG, JPEG and TIFF
      through WIC whenever `wincodec.h` is found (`cmake/deps.cmake`, `HAVE_WINCODEC_H`), a
      different decoder from the libpng/libjpeg-turbo/libtiff the gates match. So: configure
      with `-DHAVE_WINCODEC_H=0` (the check is skipped when the variable is set) and run only
      the gates that never hand cwebp a PNG, JPEG or TIFF — the PAM control surface, the
      cwebp-defaults reset, WebP and PNM sources, raw YUV — as a Windows parity leg.
      **First slice done (2026-10-02):** CI's `parity-windows` job runs the four
      `tests/parity.rs` gates of that kind, and on its first run every case matched with MSVC
      builds of both sides — 138 control-surface settings, 552 WebP-to-WebP conversions, 138
      raw YUV conversions and the defaults reset (run 37092442114). Then, all passing on
      their first runs (37094421926) and now gates: `img2webp`, `gif2webp` and `webpmux` too,
      and the image readers the Linux references use (libjpeg-turbo from our submodule;
      libpng, libtiff and giflib from vcpkg; static runtime, no WIC) — every
      `tests/parity.rs`, `tests/animation.rs` and `tests/gif.rs` gate and `tests/pnm.rs`'s
      cwebp ones on Windows; and `cjxl` from libjxl's CMake with MSVC — every JPEG XL gate
      but EXR's and the JPEG XL-source one (libjxl finds OpenEXR only through pkg-config),
      and `tests/pnm.rs`'s PNM, PFM and PGX ones. **Done (2026-10-03):** OpenEXR for that
      `cjxl` (vcpkg's `openexr`, found through vcpkg's `pkgconf`; `/Zc:lambda`, since MSVC's
      legacy lambda parser refuses libjxl 0.12.0's `exr.cc`), so the JPEG XL-source gate
      runs there (132 of 132); `avifenc` built by `scripts/build-reference-avifenc.sh`,
      which now runs under Git Bash with MSVC (every `tests/avif_parity.rs` gate); and
      `heif-enc` from a second libheif built by `scripts/build-libheif.sh` with vcpkg's
      readers on the dynamic runtime (every `tests/heic_parity.rs` gate). All gates in CI
      run 37171820482. Two real bugs came out of it, below. The EXR gate itself stays out
      on Windows (next item).
- [x] **EXR on Windows: `cjxl` writes other bytes for 10 of 149** (found 2026-10-03, CI run
      37171820482; diagnosed and settled 2026-10-04): every case of the two fixtures whose
      data window leaves part of the display window uncovered. Not Skidbladnir and not
      OpenEXR: libjxl 0.12.0's `lib/extras/dec/exr.cc` `malloc`s the display-window image
      (`PackedImage`, `packed_image.cc`) and copies only the data window in ("TODO(eustas):
      should we deal with unpopulated pixels?"), so `cjxl` encodes whatever the allocator
      returned there: zero on a fresh glibc or macOS heap, other bytes on Windows. Shown on
      the dev host: with `MALLOC_PERTURB_=165` Linux's `cjxl` diverges on exactly the same 10
      of 149. Skidbladnir makes those pixels zero, so `matches_cjxl_through_exr` now holds
      those two fixtures to `cjxl` reading the same image with the zeros written out (the
      data window widened to the display window), every `cjxl` run in `tests/jxl_parity.rs`
      is under `MALLOC_PERTURB_` (a case that matches only by luck fails on Linux too), and
      the EXR gate runs on Windows. 149 of 149 on the dev host and on Windows (CI run
      37253214479, `parity with cjxl / windows`); moving the placement one pixel fails the 10. Still unfixed on libjxl's main branch
      (<https://github.com/libjxl/libjxl/blob/main/lib/extras/dec/exr.cc>, the TODO at line
      464; <https://github.com/libjxl/libjxl/blob/main/lib/extras/packed_image.cc>, line 110).
- [ ] Report libjxl's uninitialised EXR border upstream (an issue on libjxl/libjxl; none
      found by searching its tracker on 2026-10-04): `cjxl` encodes undefined pixels for any
      EXR whose data window does not cover its display window. The routine does not file
      issues on its own; the reproduction is the `MALLOC_PERTURB_` run above.
- [x] **Every Windows libheif lacked sharp YUV** (found and fixed 2026-10-03 by the Windows
      `heif-enc` parity job): `scripts/build-libheif.sh` looked for `sharpyuv.lib`, MSVC
      builds `libsharpyuv.lib`, and libheif's configure quietly built without it. `heif-enc`
      then refuses `-C sharp-yuv`; the app only prefers it, so a Windows HEIC made with Sharp
      YUV used the default downsampler. The script now finds it and fails without it.
      Gate: the Windows `heic_parity` run, 8 of 8 tests, sharp YUV cases included.
- [x] **libaom, libavif and libjpeg-turbo were debug-quality builds on Windows** (found and
      fixed 2026-10-03 by the Windows `avifenc` parity job): with Visual Studio's generator
      `cmake-rs` 0.1.58 replaces `CMAKE_<LANG>_FLAGS_RELEASE` with the compiler's base
      flags minus `/O`, so CMake's `/O2 /Ob2 /DNDEBUG` never reached them: unoptimised,
      assertions on. A libaom assertion (`av1/encoder/bitstream.c:2982`) ended the test
      process on a superres setting. `build.rs`'s `msvc_release_build` now sets the Release
      flags itself. Gate: the Windows `avif_parity` run, 4 of 4 tests, byte for byte.
- [x] **libjxl on Windows has the same unoptimised build** (fixed 2026-10-04, though not
      the way first suggested): `cmake-rs` replaces the Release flags only when no generator
      is named, and reads `CMAKE_GENERATOR` from the environment, so `build.rs`
      (`libjxl_msvc_generator`) names the Visual Studio generator for `jpegxl-src`'s build in
      release builds, and fails the build if libjxl's `CMakeCache.txt` then lacks `/O2` and
      `/DNDEBUG` (a cache configured before is removed first). Every Windows parity job
      passed with it, the build-time check included (CI run 37253214479: `cjxl` 7 of 7
      tests, `avifenc`, `heif-enc`, libwebp's tools). *Original item:* `jpegxl-src` builds it with
      `cmake-rs` and ClangCL, so the flag replacement above applies, and `build.rs` cannot
      reach that configure. The JPEG XL gates pass there all the same (byte parity held
      with an unoptimised libjxl), so this is speed, and assertion code shipped, not output.
      The fix is the one already suggested for `jpegxl-src`'s licence header: our own
      CMake invocation over the pinned libjxl source, through `msvc_release_build`; the
      Windows JPEG XL gates must stay green through it (Linux showed an unoptimised
      libjxl rounding differently on decode).
- [ ] Build libjxl from a pinned source with our own CMake call instead of `jpegxl-src`'s:
      its crates.io metadata says BSD-3-Clause but its `lib.rs` carries a GPL header (it only
      runs at build time and nothing of it is linked; Phase 7's JPEG XL item). Not a
      submodule alone: libjxl's own dependencies (highway, brotli, skcms) are nested
      submodules, and every CI job checks out `submodules: true` without recursion, so the
      source tarball and all 12 checkouts would change with it. Pins at libjxl v0.12.0
      (`git ls-tree v0.12.0 third_party/`, 2026-10-04): highway `457c8917`, brotli
      `028fb5a2`, skcms `96d9171c`; the library build needs only those three (sjpeg, lcms,
      libpng, zlib, googletest, libjpeg-turbo and testdata are for tools, tests or
      alternatives). The reference `cjxl` builds (`scripts/build-reference-tools.sh`, the
      Windows `cjxl` job) take their source from `jpegxl-src` too and move with it. Top-level
      submodules for the three would need no change to the checkouts, but libjxl cannot
      simply be pointed at them: `third_party/skcms.cmake` hard-codes
      `${PROJECT_SOURCE_DIR}/third_party/skcms`, so `build.rs` would assemble a source
      overlay (libjxl with the three in its `third_party/`) in `OUT_DIR` and configure that.

**Multi-image features the tools have, deferred by the owner (2026-09-28):**

- [ ] Animation and image sequences: `avifenc` from Y4M or several inputs (timescale,
      keyframe interval, repetition count), `cjxl` from GIF/APNG, and `heif-enc -S`/`-V`.
      (Animated WebP and GIF to animated WebP shipped in 0.12, Phase 7, held to
      `img2webp` and `gif2webp`.)
- [ ] Several inputs into one file: `avifenc --grid` from separate cells, `avifenc
      --layered`, `heif-enc` with several images and `-T` tiled input.
- [ ] `heif-enc` metadata tracks, MIME items, and the other experimental item types.

**Formats:**

- [x] **SVG input** (2026-10-01): `src/svg.rs` rasterises with `resvg` 0.48 (Apache-2.0 OR
      MIT) at the file's own size, transparency kept, straight colour, text in the
      system's fonts. Only `data:` images embedded in the file are drawn: no path or URL an
      SVG names is opened (`never_reads_another_file`, which fails with `resvg`'s default
      resolver). Capped at 100 megapixels. Sniffed by content (the sniff window grew from 64
      bytes to 1 KiB for SVG's XML declaration and comments). Tested to every format; no
      reference tool reads SVG.
- [x] SVG drawn at the resize's size (2026-10-01): an uncropped SVG with a resize is
      redrawn at the target size before any format's encoder sees it
      (`encoder::svg_at_resize`), the missing dimension derived as libwebp's rescaler
      derives it, so sizes match every other source's. Tested: a hard edge survives a 2x
      enlargement to lossless WebP (fails without it).
- [x] `.svgz` input (2026-10-01): `resvg`'s `svgz` feature, and the sniff looks inside the
      gzip stream, from the start of the file alone.
- [x] SVG drawn at the crop's size (2026-10-02): with a crop and a resize both set, only
      the crop's rectangle (in the drawing's own pixels) is drawn, straight at the resized
      size (`svg::rasterise_region`), the missing dimension derived from the crop as
      libwebp's rescaler derives it. A crop alone still keeps the raster's own pixels.
      Tested: sizes match libwebp's crop-then-rescale for four crops, a crop outside the
      drawing is refused, and a hard edge survives an 8x enlargement of a crop to lossless
      WebP (fails with the old raster route).
- [ ] **SVG output** (owner request, 2026-09-28) — needs the owner's answer to the question
      below first. The original notes, input included: *Input* means rasterising:
      `resvg` is the obvious renderer (pure Rust, no C toolchain); it needs a size — the
      SVG's own, or the resize — and a background for the areas SVG leaves transparent.
      *Output* is the open design question, because a raster has no vectors to write:
      either trace it (`vtracer`, which suits logos and flat art and mangles photographs)
      or wrap the encoded raster in an SVG `<image>` (lossless in content, and pointless
      unless something requires an `.svg`). Decide which one users mean before building
      either; they may want both. Check each crate's licence against ISC first.

## Cross-cutting

- [x] **The gpui window is the default** (1.3.0, PR #67, an owner-requested session rather
      than a routine run): `crates/skidbladnir-gpui`, opened by `src-tauri` through the
      default `gpui` feature, with the Nuxt webview kept as a fallback (`SKIDBLADNIR_UI=webview`,
      or automatically when gpui cannot open a window). Every UI change goes into both.
      <https://github.com/basic-automation/Skidbladnir/pull/67>
- [x] Something automated opens the gpui window (2026-10-02): `scripts/gpui-smoke.sh` launches
      it with its debug variables (`SKID_FORMAT`, `SKID_INPUTS`, `SKID_CONVERT`) and a scratch
      configuration, has it convert a PNG to WebP, AVIF, JPEG XL and HEIC and a raw `.yuv` at
      the size its settings give, and checks each file's header on disk and that the process
      it started is still the one running (the webview fallback relaunches and exits). 5 of 5
      on the dev host (Hyprland); forced onto the webview window, every one fails.
- [x] `scripts/gpui-smoke.sh` is a CI gate (2026-10-02): on Linux in the window job, both
      editions, with Mesa's lavapipe on Xvfb, and on Windows against the installed app
      (Direct3D, WARP on the GPU-less runner; the script writes the real `%APPDATA%`
      configuration there, which it allows only under `CI=true`). Both passed 5 of 5 on
      their first run (37092442114), the first time anything automated opened the gpui
      window on Windows. macOS is covered by the release workflow, which launches the app
      and screenshots its window.
- [x] An accessibility audit of the gpui window (2026-10-03): `scripts/gpui-a11y-audit.py`
      reads the window as a screen reader does, AT-SPI on Linux (in a private D-Bus session
      per state, with its own accessibility bus) and UI Automation on Windows (pywinauto),
      in ten states: each format with every disclosure open, the preset form, the settings,
      an open select, a queued file's menu, the preview and About. It fails on nameless
      controls and images, buttons sharing a name, sliders out of range, required text
      fields missing or unreadable, and a field that will not take the focus. CI steps in
      both Linux window legs and the Windows window job. **It found that no text or number
      field reached a screen reader at all**: gpuikit's input implements none of gpui's
      accessibility methods, so the role and name set on it were dropped. `Announced`
      (`controls.rs`) now reports them, with the value as a text run; 15 problems in the 5
      first states with the old code. Also two buttons named only "Apply" and a bare
      "Collapse"/"Expand", renamed in both windows; and Convert, announced as "Convert"
      whatever was queued and as available when dimmed, is now named as drawn ("Convert 2
      files") and marked disabled when it does nothing. A field focused over AT-SPI and typed
      into with `wtype` reads back what was typed (dev host, Hyprland).
- [ ] A person using the gpui window with a screen reader: Orca, Narrator or NVDA, and
      VoiceOver. The audit above checks what they are given, not how it sounds or flows.
      macOS is not audited at all (AccessKit's NSAccessibility tree; reading it from a CI
      runner needs an accessibility permission granted to the reader).
- [x] Dragging files onto the gpui window, on X11 (2026-10-03): `scripts/gpui-drop-test.py`
      plays a file manager's side of XDND with python-xlib (offers a PNG as
      `text/uri-list`, answers the window's selection request, drops), then presses
      Convert through AT-SPI and checks the WebP on disk. CI steps in both Linux window
      legs (Xvfb); passes here through XWayland, and fails with the drop left out.
- [x] Keyboard reach of the gpui window (2026-10-03): `scripts/gpui-keyboard-test.py` tabs
      through each format's panel (every disclosure open), reading the focus from AT-SPI,
      and fails on a control or radio group Tab never reaches, a trap, a nameless focus, or
      a Shift+Tab cycle that is not Tab's reversed. Clean in all four panels here; taking
      Lossless out of the tab order fails it. CI steps in both Linux window legs. In the
      WebP panel it also presses the keys inside controls (the arrows, Page Up, Home and End
      on a slider; the arrows in a radio group; Space on a check box); reversing the
      slider's arrows fails it; and a select (Down opens it on the current choice, Down and
      Enter choose, Escape closes). And the queue's menu (2026-10-04): Enter on Queue opens
      it on its first item, Down/Up/Home/End move the highlight, Space ticks "Include
      subfolders" and leaves it open, Escape closes it with the focus on Queue. Space is
      pressed only once the check item is highlighted, since the other two open file
      dialogs. Swapping Up and Down in the app fails 6 of those checks; the menu is
      highlight-only, so the focus never leaves Queue and the Escape check guards only
      against losing it. Since 2026-10-04 the test reads the focus once it has moved and
      held still, not at a fixed pause after the key: master's run 37180306916 lost a Tab
      stop ("Tab never reaches 'App settings'"), which a read before the focus moved would
      explain (not reproduced, so not proven).
- [x] **A select's value did not reach AT-SPI** (found and fixed 2026-10-03 by the keyboard
      test): the trigger is a `ComboBox` with `aria_value`, but AccessKit 0.18.1 gives a
      combo box no Text interface (`supports_text_ranges` covers text inputs, labels and
      documents only), and the open list is a pop-up elsewhere in the tree, so a screen
      reader could say "libwebp preset, combo box" but not "photo". Each select now carries
      its choice as a selected option of its own (`chosen_option` in `controls.rs`), as a
      browser exposes a `<select>`; AT-SPI's Selection reports it. The audit checks every
      select says what it holds; without the option, 9 of its 10 states fail. On Windows
      the audit reads UI Automation's Value (or selected item) for the same check (since
      2026-10-04; passes in CI run 37253214479).
- [x] A real drop onto the gpui window on Windows (2026-10-04): `scripts/gpui-drop-test.py`
      opens Explorer on a folder holding a PNG, presses the mouse on the file, moves onto the
      window and lets go (OLE drag and drop from the real file manager), then invokes "Convert
      1 file" through UI Automation and checks the WebP on disk. A step in the Windows window
      job; passed on its first run (CI run 37255781900) and since.
- [ ] A drop onto the gpui window on Wayland (a native Wayland drag source) is still
      untested: a Wayland drag needs a real pointer press's serial, which a script cannot
      fake the way it plays XDND on X11.
- [ ] AccessKit's AT-SPI mapping reports every button enabled: accesskit_atspi_common
      0.18.1 (`node.rs`) adds `Enabled | Sensitive` unless the role is one of those
      `is_read_only_supported` lists (text inputs, check boxes, sliders and the like), so a
      disabled Convert is announced as available on Linux. The window marks it disabled
      (UI Automation passes that on); report upstream, and take the fix when AccessKit has
      one.
- [ ] The gpui window on macOS and Windows has not been opened by a person (CI builds and
      tests it; the Windows installer smoke runs the webview window).
- [x] CI opens the gpui window on macOS (2026-10-04): the macOS rust job runs
      `scripts/gpui-smoke.sh` against its debug build, converting a PNG to every format and
      a raw YUV file (5 of 5 in CI run 37258119163). Before, only `release.yml`'s launch
      screenshot had shown it there.
- [ ] The gpui window's macOS accessibility tree, read by the audit (`AxReader`,
      2026-10-04; a reported-only CI step). The hosted runner's reader is trusted for
      accessibility. CI run 37258119163 read all 10 states: every control named, every
      select's value and slider range given, Convert disabled with nothing queued. The one
      problem in each was a text field that, asked to take the focus (`AXFocused`), did not
      report it. Bringing the app to the front first (`AXFrontmost`, `AXMain`) did not
      change that (run 37259957909). AccessKit's adapter does pass the request on
      (`accesskit_macos` 0.26.3 `setAccessibilityFocused:` → `Action::Focus`), but reports
      a node focused only while the host view has focus, which gpui gives it only while the
      window is key (`gpui-macos` 1.17.2 `window.rs`, `update_view_focus_state(is_active)`);
      a bare binary started from CI is never the active app. The reader now activates it
      (`NSRunningApplication.activateWithOptions_`) and says whether that worked; if macOS
      refuses (macOS 14 restricts activation by another process), skip the focus check on
      macOS with that reason. Make the step required once it is clean.
- [ ] **On CI's Xvfb, some private AT-SPI sessions never get the window's controls**: the
      audit's and keyboard test's single retry absorbs it (every run of 2026-10-04, mostly
      the GPL leg; once on master, run 37180306916). The diagnosis printed since then (CI
      run 37258119163) shows the app registered on the AT-SPI desktop with its window as
      its one child, but nothing inside the window for 90 seconds, so it is AccessKit's
      tree for the window that does not arrive, not the app's connection. A registry
      started 3 seconds late does not reproduce it on the dev host. The likely mechanism,
      read from `gpui-unofficial` 1.17.2 `window.rs` (not proven): on activation gpui hands
      AccessKit a tree of the window alone and asks for a frame to send the real one, but
      a frame sends the tree only if accessibility was active from its start, so an
      activation that lands mid-frame leaves the window empty until something draws again,
      which in a window nobody touches is never. Since 2026-10-04 the audit (and the keyboard
      test, through it) resizes the window by a pixel and back when it is listed but empty for
      5 seconds, and says so in the log; whether that recovers a session is CI's to show.
      A person's first key or pointer move would draw the frame, so it matters little
      outside tests, but it is gpui's to fix: report it upstream once confirmed.
- [x] The saving percentage is computed **once**, in Rust. `savingOf()` is gone from
      `pages/index.vue`; the conversion report and the preview both carry the core's
      `saving_percent`, and the window only formats it (commit 235dbc0 — this box was left
      unticked when it landed).

- [x] Every new Rust dependency goes in `[workspace.dependencies]`, consumed with
      `{ workspace = true }`. Holds for all of them: `base64`, `image`, `libwebp-sys`,
      `serde`, `serde_json`, `skidbladnir-encode`, `tauri`, `tauri-build`,
      `tauri-plugin-dialog`, `thiserror`. A standing rule, not a one-off task — re-check
      it whenever a dependency is added.
- [x] Keep dependencies current every run — both `cargo update` and the frontend's
      lockfile. Done this run: `cargo update` found nothing to move (already at the
      highest compatible versions) and `npm update` refreshed the frontend lockfile.
      `vue-router` 5.x is available and deliberately **not** taken: Nuxt 4 is on
      vue-router 4, so forcing the major would break the framework, not modernise it.
- [x] `cargo deny` (advisories + licences + bans + sources), as `deny.toml` and its own
      CI job. All four checks pass. Two things it found on its first run:
      - **A wildcard dependency of our own making.** `skidbladnir-encode` was declared as a
        bare `{ path = ... }`, which resolves as `*`. Now carries `version = "0.1"`.
      - Six **unmaintained** advisories reaching the tree transitively through Tauri —
        `proc-macro-error` (RUSTSEC-2024-0370) and five `unic-*` crates
        (RUSTSEC-2025-0075/0080/0081/0098/0100), each reporting "No safe upgrade is
        available". These are unmaintained notices, not vulnerabilities. `unmaintained` is
        therefore scoped to `workspace`, so an unmaintained crate *we* chose still fails
        while Tauri's transitive tree does not produce an ignore-list that never shrinks.
        Vulnerabilities and yanked crates still fail unconditionally.
      The graph is checked against the Windows and macOS target triples as well as Linux,
      so a Windows-only crate's licence or advisory is not invisible from this host.
- [x] The README screenshot is the Tauri app, committed in-tree at
      `resources/images/screenshot.webp` rather than hot-linked from
      `basicautomation.io`, so it cannot rot independently of the code. Captured from the
      running release binary over the WebDriver harness and encoded by `cwebp` at the
      app's own default settings — 91 KB for 1892x1720.
- [x] Re-captured for 0.6.0 (Format selector, AVIF panel, Preview panel), and the capture
      is now **genuinely scripted**: `scripts/screenshot.sh` runs in CI's window job and
      uploads the PNGs as the `screenshots` artifact. It has to be CI — on the dev host
      WebKitWebDriver's screenshot endpoint hangs under XWayland. The README images are
      those PNGs cropped to their content and encoded by the reference `cwebp` at the
      app's defaults. The preview is shot in WebP because CI's WebKitGTK cannot show AVIF.
- [ ] Re-capture whenever the UI changes materially: download the `screenshots` artifact
      from the change's CI run, crop, encode. Cheap; the risk is forgetting.
- [x] Dependabot now has an in-tree config (`.github/dependabot.yml`) covering cargo,
      the Nuxt frontend's npm tree, the Electron app's npm tree and github-actions, with
      the Tauri and Nuxt crates/packages grouped so they update together instead of
      opening mutually-conflicting PRs. Its PRs are gated by CI, which now exists.
- [x] Notices for the window's font (Fira Code, OFL-1.1) and its eleven Iconify icon
      collections (CC BY 4.0 attribution for Codicons, IconaMoon and Subway; the MIT, ISC,
      Apache-2.0 and OFL texts for the rest), and libpng's licence (`png_gamma.rs`), in both
      editions' notices (2026-10-01). Texts the packages lack are in
      `LICENSES/third-party/` with their sources; the generator refuses to run if
      `frontend/package.json`'s icon collections differ from the ones it describes.
- [x] Notices for the Rust crates (2026-10-02): `cargo-about` 0.9.2 (`about.toml`,
      `about.hbs`) lists the 700-odd crates compiled into the app on any of the five
      platforms it is built for (no build or dev dependencies, not Skidbladnir's own),
      grouped by licence text, into `LICENSES/third-party/rust-crates.md`, which
      `third-party-notices.py` appends to both editions' notices. CI's `deny` job installs
      the same cargo-about and fails if the listing is stale. MIT is preferred where a
      crate offers a choice: Apache-2.0 first came out larger (918 KB against 550 KB),
      because crates' copies of the Apache text differ in their whitespace.
- [x] Notices for the JavaScript packages in the webview window's bundle (2026-10-02): a
      Vite plugin (`frontend/tools/javascript-notices.ts`, active only when
      `SKIDBLADNIR_JS_NOTICES` names a file) lists every npm package with a module that kept
      code in the client bundle after tree-shaking — 35 packages (Vue, Nuxt, Nuxt UI, Reka
      UI, Floating UI, VueUse, the Tauri API…), every one with a licence file — grouped by
      licence text into `LICENSES/third-party/javascript.md`, appended to both editions'
      notices. CI's Linux rust leg rebuilds it and fails if it is stale. A clean rebuild is
      byte-identical.
- [x] Notices for the Ubuntu libraries inside the AppImage (2026-10-02). linuxdeploy copies
      most bundled packages' Debian copyright files into the image
      (`usr/share/doc/<package>/copyright`), **but not all**: the new check found 10 of the
      110 packages behind its 172 libraries without theirs, on every Linux leg and edition —
      dconf-gsettings-backend, glib-networking, libbz2-1.0, libcap2, libdbus-1-3 and others
      the GTK plugin pulls in — so every AppImage to date shipped those libraries without
      their licences. `scripts/appimage-notices.py` maps each bundled library to its runtime
      package (`dpkg -S`, through symlinks), release.yml bundles the AppImage again with the
      missing files (`tauri bundle --bundles appimage --config <fragment>`, re-signed), and
      the check fails the release if any are still missing. Both editions' notices say
      where the files are and that the sources are Ubuntu 22.04's (jammy). Release dry run
      37092442062: 110 of 110 on all four Linux legs, installs and updater manifests green.
- [ ] Dependabot's cargo and npm PRs now fail CI's notices checks until someone
      regenerates `LICENSES/third-party/rust-crates.md` or `javascript.md`, and the
      notices, on the PR branch (that check is the point: a new package's licence must
      reach the notices). A workflow
      that regenerates and pushes them for Dependabot's branches would save the manual
      step; it needs a token that can push to them, so it is a security decision.
- [ ] Two Dependabot alerts with no clean fix (2026-10-01): `glib` 0.18 (unsound
      `VariantStrIter`, fixed in 0.20) comes with Tauri 2's GTK 3 bindings, and `esbuild`
      0.27 (a Windows dev-server file read, fixed in 0.28.1) is pinned `^0.27` by
      `fontless` under `@nuxt/fonts`. Neither is in what ships; take each fix when its
      parent allows it.
      Re-checked 2026-10-02: both still open (`glib` 0.18.5 in `Cargo.lock`; `fontless` 1.2.1
      is the newest). A third, alert #45, high: `node-forge` 1.4.0 (RSA PKCS#1 v1.5
      signatures accepting extra nested `DigestAlgorithm` elements, CVE-2026-85393, no fixed
      release) comes through `nuxt` → `@nuxt/cli` → `listhen`, the dev server's certificate
      helper, in development scope only; the bundle listing
      (`LICENSES/third-party/javascript.md`) confirms it is not in what ships. Take a fix
      when `listhen` or `node-forge` has one. <https://github.com/advisories/GHSA-86w9-cpqp-85rv>
      Re-checked 2026-10-04: the same three open (#43 `glib`, #44 `esbuild`, #45
      `node-forge`, no patched version); `glib` 0.18.5, `fontless` 1.2.1, `listhen` 1.10.1,
      `node-forge` 1.4.0 are still what resolves.
- [x] README: an acknowledgements and non-affiliation section (2026-10-01).
- [x] README: a dated "How it compares" table (2026-10-01): Squoosh, XnConvert,
      Converseen and the raw CLIs, each from its own page
      (<https://github.com/GoogleChromeLabs/squoosh>, its `codecs/` directory,
      <https://www.xnview.com/en/xnconvert/>, <https://converseen.fasterland.net/>), "not
      stated" wherever a page does not say. Re-check it when it is more than a few months old.
- [ ] README: a short demo clip.
