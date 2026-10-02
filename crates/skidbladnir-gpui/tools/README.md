# Comparison tools

What was used to match the gpui window to the installed Tauri one and to benchmark them.
They expect a work directory (`SKID_WORK`, or the directory they sit in) holding:
`target/` (the spike built with `CARGO_TARGET_DIR` there), `libheif-gpl/` (from
`scripts/build-libheif.sh --edition gpl --prefix …`), `venv/` (Python with pillow, numpy,
websocket-client, fonttools, brotli), `xdg/com.basicautomation.skidbladnir/` (a copy of the
preferences and presets, with `outputDirectory` pointed at `out/`), `test.png`, `batch-1…6.png`.

- `wk.py` — evaluates JavaScript (stdin) inside the running Tauri window through
  WebKitGTK's remote inspector. Start the app with
  `WEBKIT_INSPECTOR_HTTP_SERVER=127.0.0.1:9333`.
- `dump.js`, `popdump.js`, `focus*.js`, `cls.js` — read the old window's computed layout,
  pop-up geometry, keyboard focus outlines and state classes through `wk.py`.
- `shoot.sh` — screenshots a window by process name; `state.sh` puts both apps in the same
  state and screenshots each; `diff.py`, `stack.py`, `rows.py` compare the shots.
- `shift_baseline.py` — moves Fira Code's ascent/descent split in the baked fonts (see the
  spike README, "Text").
- `bench.py N` — N rounds of: startup to drawn UI, idle PSS and CPU, preview latency and
  PSS, a six-image convert. Both apps run against `xdg/` and write into `out/`.
