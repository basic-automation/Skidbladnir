# gpuikit spike

A throwaway prototype answering one question: **could gpui + [gpuikit](https://github.com/iamnbutler/gpuikit) replace the Tauri webview UI?**
It is not part of the product. It is its own Cargo workspace, so the root workspace, CI, clippy and `cargo deny` never build it.

It covers the WebP options panel (17 controls bound directly to `WebpSettings`), open and save through gpui's native file dialogs, debounced background encoding, and the before/after preview as GPU textures.
It does not cover AVIF, JPEG XL, HEIC, geometry, presets, batches, the updater, the Paleday theme or the frameless window.

## Running it

The encode crate needs its submodules, and a prebuilt libheif:

```sh
git submodule update --init third_party/aom third_party/libavif third_party/libwebp third_party/libjpeg-turbo third_party/libheif
cd spikes/gpuikit
SKIDBLADNIR_LIBHEIF_DIR=../../build/libheif cargo run --release -- path/to/image.png
```

The image argument is optional. Without it, use **Open image…**.

Use `--release`: the debug build encodes about 6× slower, because libwebp is built unoptimised.

## Pins

`gpuikit = "=0.9.0"` and `gpui-unofficial = "=1.17.2"`: the pair gpuikit's own lockfile resolves.
gpuikit says to expect breaking changes in every release.

## Findings (2026-10-01, Linux/Wayland, Hyprland)

**Worked first time**

- 17 controls, open/save, background encode and preview: about 450 lines in one file. It compiled after a single missing-trait-import fix.
- No IPC and no TypeScript copy of the settings. Each control's subscription writes the `WebpSettings` field the encoder reads.
- Open and save use gpui's own `prompt_for_paths` and `prompt_for_new_path`, with no plugin. Their options have **no file-type filter**, which the Tauri dialog has.
- The preview goes from RGBA straight to a `RenderImage`. There is no lossless-WebP re-wrap and no base64 `data:` URL, and so none of the `MAX_PREVIEW_PIXELS` memory pressure the Tauri preview guards against.
- Window on screen 483 ms after launch (release build, warm cache).
- `ldd` shows no WebKitGTK, GTK or libsoup. On Linux only xcb and xkbcommon are linked.

**Measured**

- Release binary: 67 MB, or 52 MB stripped. That includes the statically linked libaom, libavif, libjxl, libwebp and libjpeg-turbo. The 1.0.0 AppImage is 95 MB, but that is a bundle, not a like-for-like number.
- Resident memory with a 1600×1000 image loaded: about 240 MB, in a single process.
- Encode plus decode of that image at the default settings: 976 ms release, 6078 ms debug.

**Against the Tauri 1.0.0 build** (same machine, same 1600×1000 PNG; the host was under heavy load, so the times are rough, but the ratios held across repeated runs)

| | Tauri 1.0.0 | Spike |
| --- | --- | --- |
| Preview cost per settings change (`examples/preview_cost.rs`) | 3.4–3.7 s | 1.0 s |
| Data sent over IPC per preview | 2.5 MB of base64 JSON | none |
| Memory (PSS, every process), idle | 357 MB: app 115, WebKit web 196, WebKit network 45 | 117 MB |
| Memory with the image loaded | not measured (opening needs a mouse) | 139 MB |
| Launch to window mapped | ~445 ms (AppImage, including the FUSE mount) | ~297 ms |

Most of the preview gap is not the webview. Two-thirds of Tauri's cost is the lossless re-encode of the *original* (about 2.3 s), done again on every change so a webview can display it. Tauri also re-reads and re-decodes the source file each time. The encode itself costs the same in both. Caching the original's `data:` URL in the Tauri app would close most of the gap without a migration.

The window timing flatters Tauri: a mapped Tauri window still has to load the Nuxt bundle before anything shows.

**Problems found**

- **Keyboard access is missing in the controls the app depends on.** In gpuikit 0.9.0, `Switch`, `Slider`, `Select` and `ToggleGroup` have no focus handle. Tab never reaches them and Space and the arrow keys do nothing (checked on screen and in the source). `Button`, `Checkbox`, `TextField`, `Combobox` and `Listbox` are focusable. The current app's settings panel is fully keyboard-operable, so this blocks a migration until it is fixed upstream or worked around.
- **A slider thumb at its maximum overshoots the track end** (visible on Near-lossless at 100 and Segments at 4). Cosmetic.
- **State lives in two places.** gpuikit controls are entities that own their value and emit events. Setting values from code (presets, restoring preferences) means calling `set_value` on each control entity as well as writing the settings struct. Vue's `v-model` keeps one copy. A real port needs a small binding layer to keep the two in step.
- `Select` reports changes through a callback, not an event like the others, so it needs a `WeakEntity` back to the view.

**Not tested**

- The file dialogs themselves: the run above loaded its image from the command line, because only keyboard input could be scripted here.
- Saving.
- macOS and Windows.
- Screen readers.
- Theming to Paleday.
- Frameless window chrome.
- Animated WebP (gpui's `RenderImage` takes frames with delays, so it should work).
