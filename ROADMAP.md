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
- [ ] Add a CI workflow (`.github/workflows/ci.yml`): build + test the Rust workspace
      and the frontend on `ubuntu-latest`, `windows-latest`, `macos-latest`. CI pins
      **stable** Rust even though the dev host defaults to nightly.
- [ ] Add `rustfmt.toml` and a clippy configuration matching the owner's other Rust
      repos (hard tabs, `-D warnings` in CI).
- [ ] Decide and record the app's product identity: the repo is `Skidbladnir`, the
      Electron `package.json` says `ba-nextgenimg`, the 2021 rewrite branch says
      `skidblad`, and the README header says `ba | Next Generation Image`. Pick one
      name, one appId, one binary name; write it here as the answer.
- [ ] Triage `origin/copilot/add-resize-control-option` — the only unmerged branch.
      Either land it on the Electron app or close it as superseded by the Tauri port.
- [ ] Declare `origin/Skidbladnir` (the cold 2021 Vue 3 POC) dead, and say so in this
      file, so no future run mistakes it for the migration target.

## Phase 1 — Tauri 2 + Nuxt + Tailwind scaffold

The new app lands **alongside** the Electron app, not on top of it. `master` keeps
building and running the Electron app until Phase 6 retires it.

- [ ] Choose the layout: a Cargo workspace at the repo root with `src-tauri/`
      (Tauri app) + `crates/` (pure-Rust logic) and `frontend/` (Nuxt). Record the
      chosen layout here before creating directories.
- [ ] `cargo install tauri-cli` on the dev host (no `sudo` needed); pin the version.
- [ ] Scaffold the Tauri 2 app: `src-tauri/` with `tauri.conf.json`, an appId, the
      window config, and the existing `build/icon.png` wired as the app icon.
- [ ] Scaffold the Nuxt frontend in `frontend/` with Tailwind, configured for
      static generation (`ssr: false`) so Tauri can serve it from `dist/`.
- [ ] Wire `beforeDevCommand` / `beforeBuildCommand` / `frontendDist` so
      `cargo tauri dev` and `cargo tauri build` drive Nuxt correctly.
- [ ] First green `cargo tauri build` on Linux producing a runnable binary.
- [ ] First green `cargo tauri dev` window that renders the Nuxt shell.
- [ ] (owner-gated) `patchelf` is not installed on the dev host and AppImage bundling
      needs it — `sudo pacman -S patchelf`. Until then, verify the bare binary and
      report the AppImage as not built.

## Phase 2 — Encode core in Rust (the real work)

The Electron app shells out to `cwebp.exe`. The Rust port should not.

- [ ] **Decide the encode backend and record the decision here.** The options, with
      the trade-off that actually matters — can it reproduce every control the
      Electron UI exposes?
      - `webp` / `libwebp-sys` crate: binds libwebp's `WebPConfig`, which carries
        1:1 fields for every flag the current UI sets. Full parity, but it is a C
        dependency compiled per platform.
      - `image` crate: pure Rust, but its WebP encoder does not expose the advanced
        knobs (segments, SNS, filter strength, partition limit, passes, target
        size/PSNR) — it cannot reach parity.
      - Tauri **sidecar**: ship the real `cwebp` binary per platform and keep shelling
        out. Guaranteed parity and guaranteed behavioural identity, at the cost of
        bundling three binaries and keeping them updated.
      Recommended default unless research overturns it: **`libwebp-sys` via `WebPConfig`**,
      with the sidecar kept as the documented fallback.
- [ ] Define the `EncodeSettings` type — one Rust struct that is the single
      representation of an encode job, serializable across the Tauri IPC boundary.
- [ ] Implement the encoder and prove **parity against `cwebp` itself** with a test
      that encodes fixture images both ways and compares output. This is the gate for
      the whole phase: a port that silently changes output is a regression.
- [ ] Port the **mode** surface: lossy · lossless · near-lossless · JPEG-like · preset.
- [ ] Port the **preset** surface: `default`, `photo`, `picture`, `drawing`, `icon`, `text`.
- [ ] Port **quality** and **alpha quality**.
- [ ] Port **compression method** (`-m`) and **segments** (`-segments`).
- [ ] Port **spatial noise shaping** (`-sns`).
- [ ] Port the **filter** surface: strength, sharpness, strong/simple, auto-filter.
- [ ] Port **target size** and **target PSNR** (mutually exclusive with quality).
- [ ] Port **multi-pass** (`-pass`).
- [ ] Port **partition limit** (`-partition_limit`).
- [ ] Port **sharp YUV** (`-sharp_yuv`).
- [ ] Port **low memory** (`-low_memory`).
- [ ] Port **resize** (width/height).
- [ ] Port **multi-threading** (`-mt`).
- [ ] Report real before/after file sizes and the original size back to the UI — the
      Electron app does this and users rely on it.

## Phase 3 — Frontend parity (Nuxt + Tailwind)

Parity means a user of the Electron app finds every control they had, not a
prettier subset.

- [ ] File selection: input paths and output path, via Tauri's dialog plugin.
- [ ] Drag-and-drop of input files onto the window.
- [ ] The mode selector and its conditional control groups (lossy shows the lossy
      options; lossless hides them) — the Electron UI's show/hide logic is real
      behaviour, not decoration.
- [ ] Every Phase 2 control bound to the UI, with the same ranges and defaults.
- [ ] The advanced-options disclosure that hides the expert controls by default.
- [ ] Per-control help text — the Electron UI has an `-info` element for most
      controls; carry the copy over rather than rewriting it from scratch.
- [ ] Input/output validation and the error states the Electron UI shows.
- [ ] Conversion progress and the completed/converted state.
- [ ] Original vs converted size readout.
- [ ] Dark mode (Tailwind), since Electron never had it and it is cheap here.

## Phase 4 — Beyond parity

- [ ] Batch conversion with a real queue, per-file status, and cancel.
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
