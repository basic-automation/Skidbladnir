# Skidbladnir — Roadmap

Single source of truth for the work queue. Every item is a `[ ]`/`[x]` checkbox in
phase order. Tick `[x]` only when the item genuinely shipped and was verified.
Discovered work becomes a new `[ ]` in the right phase. No status tables, no run
logs, no prose essays — git history and the PRs are the record.

**The migration in one line:** Skidbladnir is an Electron 9 GUI wrapping `cwebp`.
It is moving to **Rust + Tauri 2** with a **Nuxt + Tailwind** frontend, shipping on
**Windows, Linux and macOS**, without losing a single one of the encoder controls
the Electron app exposes today.

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
- [ ] Cache the Nuxt build in CI as well as npm's download cache; today every job
      regenerates the frontend from scratch.
- [ ] Add `.github/dependabot.yml` covering the `cargo`, `npm` and `github-actions`
      ecosystems. Dependabot is enabled on the repo but there is no config file in-tree,
      so its coverage is whatever the GitHub UI was set to and is not reviewable here.
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
      because the AppImage target needs `patchelf`, which is owner-gated; **no installer
      or AppImage has been built.**
- [x] First green dev window that renders the Nuxt shell — verified over WebDriver
      against the Nuxt dev server on 127.0.0.1:1420: `document.title` is "Skidbladnir",
      the shell renders, and `encoder_version` / `default_settings` return real values.
      The shared harness at `~/.claude/scheduled-tasks/_shared/tauri-webdriver.sh` drives
      this app correctly; its first green run against Skidbladnir is this one.
- [ ] (owner-gated) `patchelf` is not installed on the dev host and AppImage bundling
      needs it — `sudo pacman -S patchelf`. Until then, verify the bare binary and
      report the AppImage as not built.

- [ ] Decide the CSP properly. The window currently runs with
      `script-src 'self' 'unsafe-inline'` because Nuxt emits an inline `<script
      type="importmap">`. That is weaker than a desktop app needs; either make Nuxt drop
      the importmap or move to hashes/nonces, and re-verify over WebDriver — a CSP that
      silently blocks the bundle shows up as a blank window, not an error.
- [ ] Add a WebDriver smoke test to CI or to a committed script, so "the window renders
      and IPC answers" is a repeatable check rather than something each run redoes by
      hand.

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
- [x] Define the `EncodeSettings` type — `crates/skidbladnir-encode/src/settings.rs`.
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
- [ ] Build a **version-matched** reference `cwebp` in CI, from the same libwebp source
      `libwebp-sys` vendors, so the parity test can be *enforced* there
      (`SKIDBLADNIR_REQUIRE_PARITY=1`) instead of skipping whenever apt or brew ships a
      different libwebp. Today CI reports the parity result but cannot fail on it.
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
- [ ] Support the remaining input formats the Electron app's file filter lists but which
      are only aliases today (`.jpe`, `.jif`, `.jfif`, `.jfi`) — content sniffing already
      handles them, but confirm against real files from each producer.
- [ ] Decide what to do about 16-bit PNG and CMYK JPEG input. The `image` crate reduces
      both to 8-bit RGBA, and whether that matches libpng/libjpeg as `cwebp` drives them
      is **untested** — the PNG parity test covers 8-bit RGBA only.

## Phase 3 — Frontend parity (Nuxt + Tailwind)

Parity means a user of the Electron app finds every control they had, not a
prettier subset.

- [x] File selection: input paths and output path, via `tauri-plugin-dialog`. The window
      holds only `dialog:allow-open` and no filesystem permission at all — every read and
      write happens in Rust against a path the user picked.
- [ ] Drag-and-drop of input files onto the window.
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
- [ ] Conversion progress. There is a busy state and a per-file result list, but no
      per-file progress while a batch runs and no indication of which file is in flight.
      libwebp exposes `WebPPicture::progress_hook`, which is the route to a real
      percentage.
- [x] Original vs converted size readout — a results table with before, after, the
      percentage change and the encoded dimensions, per file.
- [x] Dark mode (Tailwind), following the system preference.

- [ ] Drag-and-drop of input files onto the window is still open, and now needs
      `dragDropEnabled` in `tauri.conf.json` plus the webview's drag-drop event — note
      that Tauri disables the webview's own HTML5 drag-and-drop when its native handler is
      on, so the two approaches are mutually exclusive and the choice should be recorded.

## Phase 4 — Beyond parity

- [ ] Batch conversion: **partly done.** Multi-select and sequential conversion with
      per-file success/failure rows work today. Still missing: cancel, and a queue that
      reports progress as it goes rather than only when each file finishes.
- [ ] Preview: original vs encoded, side by side, before committing the write.
- [ ] Persist settings between launches.
- [ ] Named user presets (save/load an `EncodeSettings`).
- [ ] Recursive directory input with an output-structure mirror.
- [ ] Keyboard-navigable and screen-reader-labelled controls.

## Phase 5 — Tri-platform packaging and release

- [ ] `cargo tauri build` green on **Linux** (AppImage + `.deb`), blocked on `patchelf`.
- [ ] `cargo tauri build` green on **Windows** (MSI/NSIS) — CI-verified, the dev host
      cannot build Windows targets.
- [ ] `cargo tauri build` green on **macOS** (`.dmg`) — CI-verified, no local Mac.
- [ ] A `release.yml` workflow that builds all three targets and attaches them to a
      GitHub release.
- [ ] Decide whether the Tauri updater is in scope; if yes, a signing key is required
      and that is owner-gated (the key never passes through the routine).
- [ ] First tri-platform release of the Tauri app.

## Phase 6 — Retire Electron

Only once Phase 3 parity is `[x]` and a Tauri release has shipped.

- [ ] Remove `main.js`, `index.html`, `index.css` and the Electron dependencies.
- [ ] Remove the `resources/win/bin` cwebp-download step from the README.
- [ ] Final Electron release tagged as the last of its line, so users on it have a
      pinned artifact.

## Phase 7 — Beyond WebP

The README has promised JPEG 2000 "coming soon" since 2019. Decide it honestly.

- [ ] Research and decide the next format. **AVIF** and **JPEG XL** are the formats
      with current momentum; JPEG 2000 has essentially none outside medical and
      archival imaging. Record the decision and the evidence here.
- [ ] Either ship the chosen format, or strike the JPEG 2000 claim from the README.
- [ ] Decoding/inspection of existing WebP files (dimensions, mode, alpha), which the
      app cannot do at all today.

## Cross-cutting

- [ ] Every new Rust dependency goes in `[workspace.dependencies]`, consumed with
      `{ workspace = true }`.
- [ ] Keep dependencies current every run — both `cargo update` and the frontend's
      lockfile — not only when an advisory forces it.
- [ ] `cargo deny` (advisories + licenses + bans) once the workspace exists.
- [ ] Keep the screenshot in the README current as the UI changes; the one in the
      README today is the Electron app.
- [ ] The repo has Dependabot enabled but no CI — Dependabot PRs currently have
      nothing gating them. Phase 0's CI item fixes that; until then, review them
      by hand rather than trusting green.
