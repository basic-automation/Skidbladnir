# gpui spike: the Tauri window, rebuilt to match

Skidbladnir's whole window, rebuilt with [gpui](https://www.gpui.rs) (and
[gpuikit](https://github.com/iamnbutler/gpuikit)) to match the installed Tauri build:
**Skidbladnir-GPL 1.1.0** on Linux (since brought up to 1.2.0) (WebKitGTK), Hyprland, at 1.5× scale.

The point was a fair comparison. A smaller mock-up would make any benchmark meaningless, so
everything the old window draws is here:

- the frame, rail, sidebar and header;
- all four format panels, plus Crop, Resize and TIFF input;
- the update banner, preview, progress and results;
- the queue, settings, preset and select pop-ups;
- focus outlines, keyboard behaviour and drag-and-drop.

The window's own storage and preview code (`src-tauri/src/{preferences,presets,preview}.rs`)
is compiled in unchanged, so both apps read the same files and do the same work.

It is its own Cargo workspace. The root workspace, CI and `cargo deny` never build it.

## Running it

```sh
git submodule update --init third_party/aom third_party/libavif third_party/libwebp third_party/libjpeg-turbo third_party/libheif third_party/x265 third_party/libde265
scripts/build-libheif.sh --edition gpl          # once; the reference build is the GPL edition
cd spikes/gpuikit
SKIDBLADNIR_LIBHEIF_DIR=$PWD/../../build/libheif-gpl cargo run --release --features gpl
```

`SKIDBLADNIR_LIBHEIF_DIR` must be absolute: the encode crate's build script resolves it from
its own directory.

Debug switches, for putting the window in a given state:

| Variable | Effect |
| --- | --- |
| `SKID_FORMAT` | Selects the output format, without saving it |
| `SKID_INPUTS` | Queues files, as a drop would |
| `SKID_PREVIEW` | Runs a preview |
| `SKID_CONVERT` | Runs a conversion |
| `SKID_OPEN` | Opens a pop-up |
| `SKID_SCROLL` | Scrolls the window |
| `SKID_CHOOSE` | Opens "Choose images…" |
| `SKID_VERSION` | Pretends to be an older version, to try an update |
| `SKID_PROBE` | Prints layout bounds |
| `SKID_BENCH` | Prints timings |

## How it was matched

The reference is the running 1.1.0 window itself, not the source and not Chromium. WebKitGTK
lays the same CSS out differently from Chromium (see below), so measurements taken in Chromium
would have been wrong.

Started with `WEBKIT_INSPECTOR_HTTP_SERVER`, the Tauri window serves WebKit's remote inspector.
`tools/wk.py` runs JavaScript in it to read every element's computed box, font, colour, focus
outline and state classes, and to open its pop-ups. The spike's own boxes were then compared
number for number (`SKID_PROBE`), and both windows were screenshotted in the same states and
diffed (`tools/`).

Checked this way, with old and new side by side:

- The WebP, AVIF, JPEG XL and HEIC screens, top to bottom.
- The queue menu, settings popover and select menus, open.
- A queued image, and a finished preview (images, captions, saving).
- The keyboard: Tab order, the focus outline each control draws, Enter, Space and the arrow
  keys in menus and radio groups.

Horizontal positions match to within half a pixel; vertically, see "Not possible" item 1.

What it took:

- **Tailwind's text line heights as WebKitGTK computes them.** `calc(1 / 0.75)` is 15.99996px,
  which WebKitGTK truncates to a 15px line box. Chromium makes it 16px.
- **Fira Code baked from the frontend's own woff2** into static 450 and 600 cuts. gpui can't
  load woff2 and doesn't apply variable-font weights. The cuts' ascent/descent split is moved
  0.1em (`tools/shift_baseline.py`) so gpui's baseline lands where WebKitGTK draws it.
- **Each Nuxt UI control's own focus outline,** measured per control:
  - 3px of accent at 25% for primary controls;
  - 3px of near-black at 25% for neutral buttons;
  - 2px of the text colour on the plain buttons;
  - violet only on the disclosure.
- **Reka's keyboard model:**
  - a radio group is one Tab stop;
  - gpui's own Enter/Space click is used rather than doubled;
  - a select opens on its chosen item;
  - the queue menu opens on its first item;
  - a dropdown takes focus off its trigger, a select doesn't.
- **Pop-ups placed as Reka places them:**
  - selects open 8px below their trigger and exactly as wide, and flip above when there's no
    room;
  - the settings popover is bottom-aligned 8px right of the gear;
  - the queue menu is sized to its content, with the item's empty trailing slot.
- **Hover colour fades** (`src/fade.rs`). Tailwind's `transition-colors`, 150ms on
  `cubic-bezier(0.4, 0, 0.2, 1)`, reversing mid-way as CSS does. It covers every hover
  background and the menu highlights. gpui's `.hover()` style switches instantly, so each
  element records its hover state, and the window draws frames while a fade is moving.
- **"Choose images…" with the old dialog's Images filter** (`src/dialogs.rs`). gpui's dialog
  has no filters. On Linux the desktop's file-chooser portal is asked directly, through the
  same `ashpd` gpui uses, with each extension in both cases. Elsewhere it uses `rfd`, as
  tauri-plugin-dialog does.
- **CSS line breaking** (`src/css_text.rs`). gpui's wrapper counts the space after a word
  toward the line, and breaks at any punctuation. CSS lets that space hang and doesn't break
  at `/`, `(` or `,`. Text is measured with gpui's own shaper and broken the CSS way, so every
  help paragraph breaks where the old window breaks it.

## gpuikit

Of gpuikit, the matched window uses only `Input`/`InputState` (the text and number fields)
and `init`.

Its sliders, switches, selects, toggle groups and buttons couldn't be made to match:

- **Sizes and outlines are fixed.** It draws its own, with no way to set Nuxt UI's.
- **Keyboard access is missing on the controls this window needs.** `Switch`, `Slider`,
  `Select` and `ToggleGroup` take no focus at all in 0.9.0.
- **The input has no `text-align`.** The slider readout is right-aligned by sizing the field
  to its text.

Everything else is plain gpui. The window is about 4,400 lines of Rust, against about 2,100
lines of Vue that lean on Nuxt UI.

## Not possible (or not done)

1. **Sub-pixel-identical layout at a fractional scale.**
   - **What differs:** gpui renders at the compositor's fractional scale and snaps every box to
     whole device pixels, rounding halves down. WebKitGTK renders at 2× and lets the compositor
     downsample. So a 15px line is 22 device pixels instead of 22.5, and a 1px border is 1
     instead of 1.5.
   - **Effect:** stacked content drifts up by up to half a device pixel per line, about 10.7
     CSS px by the Resize heading. Text is sharper than the old window's downsampled text.
   - **Fix:** needs a patch to gpui's Wayland backend (render at 2×) or to its rounding. gpui
     has no setting for either. At 1× or 2× the two would agree.
2. **The colour chooser isn't the platform's own.** `<input type="color">` opens whatever the
   engine provides: GTK's chooser under WebKitGTK, a popup under WebView2, the colour panel on
   macOS. gpui has none, so the swatch opens one in-app picker on every platform
   (`src/color_picker.rs`): a saturation and brightness square, a hue strip and a `#rrggbb`
   field, styled like the window's other pop-ups.
3. **Selecting text.** The old window lets paths, the version line and results be selected and
   copied. gpui text isn't selectable without a custom element; not done.
4. **Untested here:**
    - dragging files over the window (no pointer automation);
    - macOS and Windows, including their update installs;
    - the standard (Kvazaar) edition;
    - a screen reader reading the window (the roles and names are set, but no screen reader
      was run against it).

## Done since the first comparison

- **Updates, every way the app is installed** (`src/updater.rs`). The same manifest and
  minisign check as tauri-plugin-updater, then the same install it would do:
  - AppImage: the running file is replaced and relaunched;
  - .deb and .rpm: `pkexec dpkg -i` / `pkexec rpm -U`, then relaunch;
  - Windows: the NSIS installer runs with `/P /UPDATE /R`, the MSI with `msiexec /i … /passive`,
    and the app quits for it;
  - macOS: the `.app.tar.gz` replaces the bundle, asking for an administrator when the bundle
    isn't writable.

  How the copy was installed is read from the bundle-type marker tauri-bundler writes into the
  binary, with fallbacks (`$APPIMAGE`, the bundle path, `dpkg -S`, `rpm -qf`). Tried end to end
  against the real 1.2.0 GPL release into a throwaway file (`examples/update_install.rs`),
  including the refusal of a file with one byte changed.
- **Resizing from the edges.** 6px grips on each side and corner of a floating window. They're
  left off any side the compositor tiles, and off a maximised window.
- **Keyboard and screen readers:**
  - landmarks (navigation, complementary, main) and the page heading;
  - the format rail as a radio group;
  - the queue menu as a menu, with a checkbox item;
  - every button, toggle, slider, select, number and text field named;
  - progress, alerts and status messages;
  - the preview images, named;
  - number fields stepping with the arrows, Page Up/Down, Home and End;
  - the brands tags (comma adds, Backspace removes);
  - the colour picker's square and hue strip as sliders on the arrows;
  - Escape closing a pop-up and returning focus to the button that opened it;
  - Page Up/Down, Home and End scrolling the window.
- **1.2.0:** "Replace existing files", the About view and its licence texts, the output-plan
  warnings, cwebp's defaults, the WebP animation options, the x265 parameters, and the new
  input types.

## Benchmarks

Both apps were measured on the same machine, in the same tile (1274×691 at 1.5×), with
update checks off and an isolated config directory (`tools/bench.py`):

- the same preferences and presets;
- output to a scratch folder;
- the same inputs: a 1600×1000 RGBA PNG for the preview, six of them for the convert.

Medians of 7 rounds, each round starting fresh processes. The machine was quiet (load
average under 3):

| | Tauri 1.1.0 | gpui spike |
| --- | --- | --- |
| Launch to UI drawn | 976 ms | 678 ms |
| Memory, idle (PSS, all processes) | 380 MB | 109 MB |
| Memory after a preview | 438 MB | 147 MB |
| Idle CPU over 10 s | 0.01 s | 0.03 s |
| Preview, press to both images drawn | 2,750 ms | 2,712 ms |
| Convert six images to WebP | 5,886 ms | 5,901 ms |

**Launch** is from starting the process until the Convert button's colour is on screen, not
just until a window maps. The old window maps before its web UI loads.
- **Tauri:** 961–978 ms.
- **Spike:** 482–699 ms; it alternates between about 490 and about 680, which looks like the
  font and GPU caches.

**Idle CPU** is negligible for both: tens of milliseconds over ten seconds.

**Memory.** The Tauri figure is the app process plus WebKitGTK's web and network processes,
plus the AppImage runtime. The spike is one process.

**Preview and convert.** These cost the same, because the work is the same code:

- The spike calls the Tauri app's own `preview()` and its convert path, and draws the result.
- Decoding the two preview images to GPU textures takes the spike about 30ms.
- Almost all of the roughly 2.7s preview is the lossless re-encode of the *original* image,
  done so a webview can display it. A gpui app wouldn't need that step; Tauri could cache it.

## Assets

`assets/fonts` holds Fira Code (SIL OFL 1.1), baked from `@fontsource-variable/fira-code`.
`assets/icons` holds the same Iconify icons the frontend bundles, under their collections'
licences: Lucide, Material Symbols, MDI, Iconoir, VS Code Icons, Bootstrap Icons, Codicons,
Elusive, Subway, IconaMoon, Google Material Icons.

The updater reads its own edition's manifest: `latest.json` for the standard edition,
`latest-gpl.json` for the GPL one, as `tauri.gpl.conf.json` points the Tauri app at it. `skid--jxl-format.svg` and the logo are the
app's own.
