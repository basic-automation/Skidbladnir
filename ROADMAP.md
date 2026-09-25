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

- [x] Tighten the CSP. Everything except inline script is now locked down:
      `default-src 'self'`, `img-src 'self' data:` (the preview's two images and nothing
      else), `font-src 'self'` (the Inter webfont is bundled, not fetched),
      `connect-src 'self' ipc: http://ipc.localhost` — the app talks to Rust over the IPC
      bridge and should never open a socket — plus `object-src`, `frame-src`,
      `frame-ancestors` and `form-action` all `'none'`, and `base-uri 'self'`.
- [ ] Remove `script-src 'unsafe-inline'`. Nuxt emits **three** inline scripts of its own
      (a 44-byte importmap and two runtime bootstraps of 1213 and 154 bytes), so the
      directive cannot simply be dropped — the window would go blank, and a CSP that
      blocks the bundle shows up as a blank window rather than an error, so any attempt
      must be re-verified over WebDriver.
      The fix is SHA-256 hashes, and the blocker is *when*: `tauri::generate_context!`
      reads `tauri.conf.json` at **compile** time, while the hashes are only known after
      `nuxt generate` runs, so it needs a build step that computes them and injects them
      between the two — and one that does not leave the tracked config file dirty.
      Worth doing; not a one-line change, and mis-scoping it as one is how it would get
      half-done.
- [x] `scripts/smoke-test.sh` — a committed WebDriver smoke test, so "the window renders
      and IPC answers" is a repeatable check rather than something each run redoes by hand.
      Nine checks: the window loads from `tauri://localhost` (**not** a dev server — the
      check that would have caught the `custom-protocol` bug), the app renders, the IPC
      returns the linked encoder version and the core's defaults, the lossy control set is
      present, every focusable control has an accessible name, a real PNG converts to a
      real WebP on disk, and converting a WebP into its own directory is refused.
      It was mutation-tested: a wrong expectation and a missing binary both fail it.
- [x] Make the smoke test self-contained. `scripts/webdriver.sh` is now an in-repo
      WebDriver harness (tauri-driver → WebKitWebDriver → the app), so the check no longer
      reaches into a file under `$HOME` and can run anywhere the tools are installed. It
      skips loudly, naming the missing tool and how to install it, rather than passing
      when it checked nothing.
- [ ] Run `scripts/smoke-test.sh` in CI. The harness dependency is gone; what remains is
      runner setup — `xvfb` for a display, the `webkit2gtk-driver` package, and
      `cargo install tauri-driver`. Note `execute/sync`, not `execute/async`: the async
      endpoint waits for a completion callback, so a script that simply returns hangs
      until the driver times out (this cost a debugging round already).

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
- [ ] CMYK JPEG input is still **untested** against libjpeg as `cwebp` drives it. The same
      class of bug as the 16-bit PNG one above, in a decoder path nothing has exercised —
      it needs a real CMYK JPEG fixture, which the `image` crate cannot write.

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
- [ ] Run `scripts/a11y-audit.sh` in CI alongside the smoke test — same runner setup, and
      it needs `frontend/node_modules` present for axe-core.

## Phase 5 — Tri-platform packaging and release

- [x] `cargo tauri build` green on **Linux** for `.deb` — **this was not actually blocked
      on `patchelf`.** Only the AppImage target needs it; `cargo tauri build --bundles deb`
      produces a valid 3.8 MB `Skidbladnir_<version>_amd64.deb` on the dev host today,
      containing `usr/bin/skidbladnir` and the hicolor icon set.
- [ ] AppImage on Linux still needs `patchelf` (owner-gated locally: `sudo pacman -S
      patchelf`). CI installs it from apt, so the AppImage is produced there — it simply
      cannot be produced or checked on the dev host.
- [ ] `cargo tauri build` green on **Windows** (MSI/NSIS) — CI-verified, the dev host
      cannot build Windows targets.
- [ ] `cargo tauri build` green on **macOS** (`.dmg`) — CI-verified, no local Mac.
- [x] A `release.yml` workflow that builds all three targets and attaches them to the
      GitHub release for a tag. Triggers on `v*` tags, or manually with a tag input so a
      release whose build failed can be retried without moving the tag. `fail-fast: false`,
      so one platform failing does not deny the release the artifacts the others produced,
      and the collect step **fails loudly** if a platform produced no installer rather
      than uploading nothing.
      **Unverified until a tag exists** — a tag-triggered workflow cannot be exercised
      before the tag it reacts to.
- [x] Decide whether the Tauri updater is in scope. **Not for the 0.x line.** Reasons, in
      order of weight:
      1. The updater requires a **signing keypair**, and its private key is owner-gated —
       it must never pass through this routine. An updater configured without one is not
       an updater; an updater whose key lives somewhere convenient is a way to push
       arbitrary code to every install.
      2. It is an auto-update channel for an app that is **mid-migration and pre-parity**.
       Shipping a mechanism that silently replaces a user's binary before the binary
       itself is stable is the wrong order.
      3. The app has no telemetry and no crash reporting, so a bad auto-update would be
       invisible to us and unattributable by the user.
      Revisit when the Tauri app is the shipping app and a tri-platform release has gone
      out at least once. Until then, releases are downloaded deliberately from the
      releases page.
- [ ] (owner-gated, when the updater is revisited) Generate and store an updater signing
      keypair. `cargo tauri signer generate`. The private key and its password belong in
      the repository's Actions secrets and nowhere else; the routine must never see them.
- [ ] First tri-platform release of the Tauri app.

## Phase 6 — Retire Electron

Only once Phase 3 parity is `[x]` and a Tauri release has shipped.

- [ ] Remove `main.js`, `index.html`, `index.css` and the Electron dependencies.
- [ ] Remove the `resources/win/bin` cwebp-download step from the README.
- [ ] Final Electron release tagged as the last of its line, so users on it have a
      pinned artifact.

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
- [ ] Add AVIF output. The Rust options, with the trade-off that matters:
      - **`ravif` 0.13** (BSD-3-Clause) — pure Rust on top of `rav1e`, no C toolchain, but
        rav1e is slow and exposes a narrow control surface.
      - **`libavif-sys` 0.17** (BSD-2-Clause, libavif 1.0.4) — the reference implementation
        with the full control surface, at the cost of a C dependency per platform, which
        is the same trade already accepted for libwebp.
      Parity is not the constraint here (there is no existing AVIF behaviour to preserve),
      so the question is control surface versus build complexity. **The encode core is
      already format-agnostic in shape** — `EncodeSettings` is WebP-specific, so this needs
      a decision on whether settings become an enum over formats or each format gets its
      own type. Record that decision before writing code.
- [ ] Watch **Tauri 3**, do not adopt it. `3.0.0-alpha` releases began appearing in
      September 2026, bringing a CEF runtime option, plugin-API changes
      (`js_init_script` → `initialization_script`) and removed deprecated APIs. Adopting an
      alpha mid-migration would be trading a known platform for an unknown one; the app
      stays on Tauri 2.x until 3 is stable and the migration has shipped.
      <https://github.com/tauri-apps/tauri/releases>
- [x] Confirm the encoder is current. **No libwebp upgrade is pending:** 1.6.0
      (9 July 2025) is still the newest release, and it is exactly what `libwebp-sys`
      vendors and what the parity test compares against, so the parity claim is against
      current upstream. Re-check each run.
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
- [ ] Re-encode animated WebP rather than refusing it, using libwebp's `WebPAnimEncoder`.
      This needs a per-frame settings story (do the advanced controls apply to every
      frame?) and a demuxer for the input, so it is a real piece of work, not a flag.

## Cross-cutting

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
- [ ] Re-capture the screenshot whenever the UI changes materially. It is now a scripted
      step (WebDriver screenshot → crop → encode), so this is cheap; the risk is
      forgetting, not effort.
- [x] Dependabot now has an in-tree config (`.github/dependabot.yml`) covering cargo,
      the Nuxt frontend's npm tree, the Electron app's npm tree and github-actions, with
      the Tauri and Nuxt crates/packages grouped so they update together instead of
      opening mutually-conflicting PRs. Its PRs are gated by CI, which now exists.
