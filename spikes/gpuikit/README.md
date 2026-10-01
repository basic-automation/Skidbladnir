# gpui spike: the Tauri window, rebuilt to match

Skidbladnir's whole window, rebuilt with [gpui](https://www.gpui.rs) (and
[gpuikit](https://github.com/iamnbutler/gpuikit)) to match the installed Tauri build:
**Skidbladnir-GPL 1.1.0** on Linux (WebKitGTK), Hyprland, at 1.5× scale.

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
2. **Colour transitions.** The old window fades hover colours over 150ms (`transition-colors`).
   gpui has no style transitions, so hovers switch instantly. Each could be animated by hand;
   not done.
3. **The colour chooser.** `<input type="color">` opens GTK's colour dialog. gpui has none, so
   the Blend background swatch only shows the colour.
4. **File-type filters in Open.** gpui's `PathPromptOptions` has no filters, so "Choose
   images…" lists every file.
5. **Selecting text.** The old window lets paths, the version line and results be selected and
   copied. gpui text isn't selectable without a custom element; not done.
6. **Installing updates.** The check and the banner are implemented against the same manifest.
   Downloading, verifying and swapping the binary is tauri-plugin-updater's, and has no
   counterpart here (`cargo-packager-updater` is the likely replacement).
7. **Resizing the frameless window from its edges.** Possible with gpui's `start_window_resize`;
   not done. Untested under a tiling compositor.
8. **Number-field stepping.** UInputNumber's arrow-key stepping isn't reproduced on gpuikit's
   input.
9. **HEIC compatible-brands tags.** A hand-built tag input. Backspace-to-remove and comma-to-add
   are missing.
10. **Screen readers.** Roles and names aren't wired on the hand-built controls. gpui has
    AccessKit underneath; not done.
11. **Untested here:**
    - dragging files over the window (no pointer automation);
    - macOS and Windows;
    - the standard (Kvazaar) edition.

## Benchmarks

Both apps were measured on the same machine, in the same tile (1274×691 at 1.5×), with
update checks off and an isolated config directory (`tools/bench.py`):

- the same preferences and presets;
- output to a scratch folder;
- the same inputs: a 1600×1000 RGBA PNG for the preview, six of them for the convert.

Medians of 5 rounds, each round starting fresh processes:

| | Tauri 1.1.0 | gpui spike |
| --- | --- | --- |
| Memory, idle (PSS, all processes) | 350 MB | 109 MB |
| Memory after a preview | 405 MB | 155 MB |
| Idle CPU over 10 s | 0.01 s | 0.05 s |
| Preview, press to both images drawn | ≈ equal (see below) | ≈ equal |
| Convert six images to WebP | ≈ equal (see below) | ≈ equal |

TIMINGS

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
Elusive, Subway, IconaMoon, Google Material Icons. `skid--jxl-format.svg` and the logo are the
app's own.
