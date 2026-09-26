#!/usr/bin/env bash
# Capture the README screenshots from the running app over WebDriver.
#
#   scripts/screenshot.sh --app <binary> --out <directory>
#
# Writes two PNGs:
#   webp.png   WebP selected, with a file previewed — the whole WebP control surface and
#              the Preview panel.
#   avif.png   AVIF selected — the Format selector and the AVIF panel.
# The preview is deliberately taken in WebP: CI's WebKitGTK (Ubuntu 24.04) cannot display
# AVIF, so an AVIF preview there shows the "cannot display" notice rather than an image.
# They are PNG so the encode for the README is a separate, deliberate step — the committed
# images are encoded by `cwebp` at the app's own default settings.
#
# WHERE IT RUNS: CI's window job, under xvfb-run with a tall virtual screen. On the dev
# host (Hyprland, XWayland) WebKitWebDriver's screenshot endpoint hangs indefinitely and
# the compositor clamps the window to the monitor, so capturing is a CI job's work.
set -euo pipefail

HARNESS="${SKIDBLADNIR_WEBDRIVER_HARNESS:-$(dirname "$0")/webdriver.sh}"
APP="" OUT=""
while [ $# -gt 0 ]; do
	case "$1" in
		--app) APP="$2"; shift 2;;
		--out) OUT="$2"; shift 2;;
		*) echo "unknown argument: $1" >&2; exit 2;;
	esac
done
[ -n "$APP" ] && [ -n "$OUT" ] || { echo "usage: $0 --app <binary> --out <directory>" >&2; exit 2; }
if command -v python3 >/dev/null && python3 -c '' 2>/dev/null; then PY=python3; else PY=python; fi
mkdir -p "$OUT"

scratch=$(mktemp -d)
cleanup() {
	"$HARNESS" stop >/dev/null 2>&1 || true
	rm -rf "$scratch"
}
trap cleanup EXIT

# A sample worth looking at in the preview: a smooth two-colour ramp with a transparent
# band, so the checkerboard behind the preview shows that transparency survives.
"$PY" - "$scratch/sample.png" <<'PNG'
import struct, sys, zlib
w, h = 480, 300
rows = bytearray()
for y in range(h):
	rows.append(0)
	for x in range(w):
		t = x / (w - 1)
		r, g, b = int(255 * (1 - t) + 199 * t), int(184 * (1 - t) + 146 * t), int(108 * (1 - t) + 234 * t)
		rows += bytes((r, g, b, 0 if 40 <= y < 90 else 255))
def chunk(tag, data):
	body = tag + data
	return struct.pack('>I', len(data)) + body + struct.pack('>I', zlib.crc32(body) & 0xffffffff)
open(sys.argv[1], 'wb').write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(bytes(rows), 9)) + chunk(b'IEND', b''))
PNG

"$HARNESS" start --app "$APP" >/dev/null
sleep 4
STATE="${XDG_RUNTIME_DIR:-/tmp}/skidbladnir-wd-${SKIDBLADNIR_WD_PORT:-4444}.json"
session=$("$PY" -c 'import json,sys; print(json.load(open(sys.argv[1]))["session"])' "$STATE")
base="http://127.0.0.1:${SKIDBLADNIR_WD_PORT:-4444}/session/$session"

# Tall enough to show every panel without scrolling, at the width the window opens at.
curl -s --max-time 30 -X POST -H 'Content-Type: application/json' -d '{"width":1000,"height":2300}' "$base/window/rect" >/dev/null
sleep 2

shot() {
	curl -s --max-time 60 "$base/screenshot" | "$PY" -c 'import base64,json,sys; open(sys.argv[1], "wb").write(base64.b64decode(json.load(sys.stdin)["value"]))' "$1"
	echo "wrote $1"
}

format() {
	"$HARNESS" exec --script "[...document.querySelectorAll('[aria-label=\"Output format\"] [role=radio]')][$1].click(); await new Promise(r => setTimeout(r, 400)); return 'ok'" >/dev/null
}

format 1
shot "$OUT/avif.png"

format 0
"$HARNESS" exec --script "const sleep = ms => new Promise(r => setTimeout(r, ms));
	await window.__TAURI_INTERNALS__.invoke('plugin:event|emit', { event: 'tauri://drag-drop', payload: { paths: ['$scratch/sample.png'], position: { x: 10, y: 10 } } });
	await sleep(800);
	[...document.querySelectorAll('button')].find(b => b.textContent.trim() === 'Preview').click();
	for (let i = 0; i < 300 && document.querySelectorAll('figure img').length < 2; i++) await sleep(100);
	await sleep(800);
	return 'ok'" >/dev/null
shot "$OUT/webp.png"
