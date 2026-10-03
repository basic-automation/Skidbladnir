#!/usr/bin/env bash
# Open the gpui window — the default one since 1.3.0 — and have it convert a PNG to every
# format, and a raw YUV file at the size its settings give, checking each result on disk.
#
# WHY THIS EXISTS
# ---------------
# scripts/smoke-test.sh drives the webview window over WebDriver, and there is no WebDriver
# for gpui, so until this script nothing automated opened the window most people get. It
# drives the window through its debug variables instead (crates/skidbladnir-gpui/src/app.rs,
# `debug_state`): SKID_FORMAT picks the format, SKID_INPUTS queues a file as a drop would,
# SKID_CONVERT presses Convert. The output directory comes from a preferences file written
# into a scratch configuration directory, so the user's own settings are never read or
# written.
#
# It also checks that the gpui window is what ran: when gpui cannot open a window, the app
# relaunches itself as the webview window (SKIDBLADNIR_UI=webview) and exits, so the process
# this script started must still be running when the converted file appears.
#
# USAGE
#   cargo build --release -p skidbladnir      # or cargo tauri build --no-bundle
#   scripts/gpui-smoke.sh [--app <binary>] [--edition standard|gpl]
#
# Needs a display (Wayland or X11) and a Vulkan driver gpui can use (on a machine with no GPU,
# Mesa's lavapipe); on Windows, Direct3D (WARP will do).
#
# On Linux the configuration is a scratch directory (XDG_CONFIG_HOME). On Windows and macOS the
# app's configuration folder cannot be redirected, so the script writes the real one; it
# does that only when CI is set, on a throwaway runner, and refuses on anyone's machine. A bare binary built outside a bundle needs libheif: beside it, as the
# installers ship it, or else this script points the loader at build/libheif (build/libheif-gpl
# for --edition gpl).
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
app="$root/target/release/skidbladnir"
edition=standard
while [ $# -gt 0 ]; do
	case "$1" in
		--app) app="$2"; shift 2;;
		--edition) edition="$2"; shift 2;;
		*) echo "unknown argument: $1" >&2; exit 2;;
	esac
done
[ -x "$app" ] || { echo "no app at $app: build it first, or pass --app" >&2; exit 2; }
app=$(cd "$(dirname "$app")" && pwd)/$(basename "$app")
libheif="$root/build/libheif/lib"
[ "$edition" = gpl ] && libheif="$root/build/libheif-gpl/lib"
if [ ! -e "$(dirname "$app")/libheif.so.1" ] && [ -e "$libheif/libheif.so.1" ]; then
	export LD_LIBRARY_PATH="$libheif${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi

case "$(uname -s)" in
	MINGW* | MSYS* | CYGWIN*) platform=windows;;
	Darwin) platform=macos;;
	*) platform=linux;;
esac
if [ "$platform" != linux ] && [ "${CI:-}" != true ]; then
	echo "on $platform this would overwrite your own Skidbladnir settings; it runs only in CI (CI=true)" >&2
	exit 2
fi
if command -v python3 >/dev/null && python3 -c '' 2>/dev/null; then PY=python3; else PY=python; fi
# A path as the app takes it: Windows paths on Windows.
native() {
	if [ "$platform" = windows ]; then cygpath -w "$1"; else printf '%s' "$1"; fi
}

scratch=$(mktemp -d "${TMPDIR:-/tmp}/skidbladnir-gpui-smoke.XXXXXX")
pid=
cleanup() {
	[ -n "$pid" ] && kill "$pid" 2>/dev/null || true
	rm -rf "$scratch"
}
trap cleanup EXIT
mkdir -p "$scratch/data" "$scratch/out"
case "$platform" in
	windows) config="$(cygpath -u "$APPDATA")/com.basicautomation.skidbladnir";;
	macos) config="$HOME/Library/Application Support/com.basicautomation.skidbladnir";;
	*) config="$scratch/config/com.basicautomation.skidbladnir";;
esac
mkdir -p "$config"
# The size a raw .yuv input is read at (cwebp's -s), for the raw YUV check below. JSON-escaped,
# for a Windows path's backslashes.
"$PY" -c 'import json, sys; print(json.dumps({"settings": {"yuvSize": {"width": 21, "height": 13}}, "outputDirectory": sys.argv[1]}))' "$(native "$scratch/out")" > "$config/preferences.json"

# A 40x30 RGBA PNG with a gradient and a transparent band.
"$PY" - "$scratch/input.png" <<'PNG'
import struct, sys, zlib
w, h = 40, 30
raw = b''.join(b'\0' + bytes(v for x in range(w) for v in (x * 6, y * 8, 128, 0 if 10 <= y < 15 else 255)) for y in range(h))
chunk = lambda kind, data: struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
open(sys.argv[1], 'wb').write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))
PNG

# Raw I420 planes, 21x13 (odd, so the chroma planes round up), which only the size above
# makes readable.
"$PY" - "$scratch/raw.yuv" <<'YUV'
import sys
w, h = 21, 13
uv = ((w + 1) // 2) * ((h + 1) // 2)
open(sys.argv[1], 'wb').write(bytes((x * 9 + y * 5) % 220 + 16 for y in range(h) for x in range(w)) + bytes(128 for _ in range(2 * uv)))
YUV

failures=0
for job in input.png:webp input.png:avif input.png:jxl input.png:heic raw.yuv:webp; do
	input="${job%%:*}" format="${job##*:}"
	output="$scratch/out/${input%.*}.$format"
	XDG_CONFIG_HOME="$scratch/config" XDG_DATA_HOME="$scratch/data" SKID_FORMAT="$format" SKID_INPUTS="$(native "$scratch/$input")" SKID_CONVERT=1 "$app" > "$scratch/$input.$format.log" 2>&1 &
	pid=$!
	written=
	for _ in $(seq 1 300); do
		if [ -s "$output" ] && ! ls "$scratch/out"/.skidbladnir-* >/dev/null 2>&1; then written=1; break; fi
		kill -0 "$pid" 2>/dev/null || break
		sleep 0.1
	done
	alive=$(kill -0 "$pid" 2>/dev/null && echo yes || echo no)
	kill "$pid" 2>/dev/null || true
	wait "$pid" 2>/dev/null || true
	pid=
	magic_ok=$("$PY" - "$output" "$format" <<'MAGIC'
import sys
try:
    head = open(sys.argv[1], 'rb').read(16)
except OSError:
    head = b''
ok = {'webp': head[:4] == b'RIFF' and head[8:12] == b'WEBP', 'avif': head[4:12] == b'ftypavif', 'jxl': head[:2] == b'\xff\x0a' or head[4:8] == b'JXL ', 'heic': head[4:12] == b'ftypheic'}[sys.argv[2]]
print('yes' if ok else 'no')
MAGIC
)
	if [ -n "$written" ] && [ "$alive" = yes ] && [ "$magic_ok" = yes ]; then
		echo "ok   the gpui window converts $input to $format ($(wc -c < "$output") bytes)"
	else
		echo "FAIL the gpui window converts $input to $format: written=${written:-no} started-process-alive=$alive header-ok=$magic_ok"
		sed 's/^/       /' "$scratch/$input.$format.log" | tail -n 5
		failures=$((failures + 1))
	fi
done

echo
if [ "$failures" -gt 0 ]; then
	echo "$failures check(s) failed."
	exit 1
fi
echo "all checks passed"
