#!/usr/bin/env bash
# Drive the built Skidbladnir window over WebDriver and check it actually works.
#
# WHY THIS EXISTS
# ---------------
# `cargo test` proves the encode core. It cannot prove that the window opens, that the
# frontend renders, or that the IPC boundary carries anything — and those are exactly the
# things that have broken silently during this migration: a release binary that pointed at
# a dev server and showed "Connection refused", and a synchronous command that made
# cancellation impossible. Both looked fine in the source.
#
# USAGE
#   cargo tauri build --no-bundle     # from src-tauri/
#   scripts/smoke-test.sh             # from the repo root
#
# Pass --app <path> to test a specific binary. Exits non-zero on the first failed check.
set -euo pipefail

HARNESS="${SKIDBLADNIR_WEBDRIVER_HARNESS:-$HOME/.claude/scheduled-tasks/_shared/tauri-webdriver.sh}"
APP=""
while [ $# -gt 0 ]; do
	case "$1" in
		--app) APP="$2"; shift 2;;
		*) echo "unknown argument: $1" >&2; exit 2;;
	esac
done

if [ ! -x "$HARNESS" ]; then
	echo "SKIP: no WebDriver harness at $HARNESS (set SKIDBLADNIR_WEBDRIVER_HARNESS)." >&2
	echo "      The window is therefore UNVERIFIED by this run." >&2
	exit 0
fi

if [ -z "$APP" ]; then
	# Honour the shared CARGO_TARGET_DIR rather than assuming ./target.
	target=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
	APP="$target/release/skidbladnir"
fi

if [ ! -x "$APP" ]; then
	echo "FAIL: no binary at $APP. Build it with: cd src-tauri && cargo tauri build --no-bundle" >&2
	exit 1
fi

scratch=$(mktemp -d)
cleanup() {
	"$HARNESS" stop >/dev/null 2>&1 || true
	rm -rf "$scratch"
}
trap cleanup EXIT

echo "==> starting $APP"
"$HARNESS" start --app "$APP" >/dev/null
sleep 5

failures=0
check() {
	local name="$1" script="$2" expected="$3"
	local got
	got=$("$HARNESS" exec --script "$script" 2>&1 | tail -1)
	if [[ "$got" == *"$expected"* ]]; then
		printf 'ok   %s\n' "$name"
	else
		printf 'FAIL %s\n       expected to contain: %s\n       got: %s\n' "$name" "$expected" "$got"
		failures=$((failures + 1))
	fi
}

# The window is serving the EMBEDDED frontend, not a dev server. This is the check that
# would have caught the custom-protocol bug.
check "loads from the bundled frontend" 'return location.href' 'tauri://localhost'
check "renders the app" 'return document.querySelector("h1")?.textContent?.trim()' 'Skidbladnir'
check "IPC returns the linked encoder version" \
	'const I=window.__TAURI_INTERNALS__; return await I.invoke("encoder_version")' 'libwebp encoder'
check "IPC returns the core defaults" \
	'const I=window.__TAURI_INTERNALS__; const s=await I.invoke("default_settings"); return String(s.quality)+"/"+String(s.method)' '75/4'
check "the lossy controls are present" \
	'return String(document.querySelectorAll("[role=slider]").length)' '9'
check "every focusable control has a name" \
	'const f=[...document.querySelectorAll("button,input,[role=slider],[role=radio],[role=checkbox]")];
	 const n=e=>e.getAttribute("aria-label")||e.closest("label")?.textContent?.trim()||e.textContent?.trim()||null;
	 return String(f.filter(e=>!n(e)).length)' '0'

# A real conversion, end to end, through the running app: a genuine PNG in, a genuine
# WebP out, checked on disk. Anything less is a check that cannot fail.
python3 - "$scratch/smoke.png" <<'PNG'
import struct, sys, zlib

width = height = 48
raw = bytearray()
for y in range(height):
	raw.append(0)  # filter: none
	for x in range(width):
		raw += bytes(((x * 5) % 256, (y * 3) % 256, 120, 255))

def chunk(tag, data):
	body = tag + data
	return struct.pack('>I', len(data)) + body + struct.pack('>I', zlib.crc32(body) & 0xffffffff)

png = (
	b'\x89PNG\r\n\x1a\n'
	+ chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
	+ chunk(b'IDAT', zlib.compress(bytes(raw), 9))
	+ chunk(b'IEND', b'')
)
open(sys.argv[1], 'wb').write(png)
PNG

mkdir -p "$scratch/out"
check "converts an image end to end" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings');
	 const r=await I.invoke('convert_image', { settings: s, input: '$scratch/smoke.png', outputDirectory: '$scratch/out' });
	 return r.width + 'x' + r.height + ' ' + (r.outputBytes > 0)" '48x48 true'

if [ -s "$scratch/out/smoke.webp" ]; then
	printf 'ok   %s\n' "wrote a non-empty WebP to disk ($(wc -c < "$scratch/out/smoke.webp") bytes)"
else
	printf 'FAIL %s\n' "no WebP was written to $scratch/out"
	failures=$((failures + 1))
fi

# The file-safety rule: converting a WebP into its own directory must be refused.
check "refuses to overwrite the source" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings');
	 try { await I.invoke('convert_image', { settings: s, input: '$scratch/out/smoke.webp', outputDirectory: '$scratch/out' }); return 'NOT REFUSED'; }
	 catch (e) { return String(e); }" 'refusing to overwrite the source'

echo
if [ "$failures" -gt 0 ]; then
	echo "$failures check(s) failed."
	exit 1
fi
echo "all checks passed"
