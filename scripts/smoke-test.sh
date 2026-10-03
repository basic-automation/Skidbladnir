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
# Pass --app <path> to test a specific binary, and --edition gpl for a build with the `gpl`
# feature (the default is standard): the edition decides what HEIC it can write. Exits
# non-zero on the first failed check.
set -euo pipefail

# The harness ships with the repository, so this check runs anywhere the tools are
# installed rather than only on the one machine that had it under $HOME.
# SKIDBLADNIR_WEBDRIVER_HARNESS overrides it.
HARNESS="${SKIDBLADNIR_WEBDRIVER_HARNESS:-$(dirname "$0")/webdriver.sh}"
APP=""
EDITION=standard
while [ $# -gt 0 ]; do
	case "$1" in
		--app) APP="$2"; shift 2;;
		--edition) EDITION="$2"; shift 2;;
		*) echo "unknown argument: $1" >&2; exit 2;;
	esac
done

if [ ! -x "$HARNESS" ]; then
	echo "FAIL: no WebDriver harness at $HARNESS" >&2
	exit 1
fi

# The platform differences, all in one place. On Windows (Git Bash) the native driver is
# msedgedriver, Python is `python`, the binary has an .exe suffix, the embedded frontend
# is served from http://tauri.localhost rather than tauri://localhost, and the app needs
# Windows paths — it cannot open the POSIX /tmp paths Git Bash hands out.
case "$(uname -s)" in
	MINGW* | MSYS* | CYGWIN*) ON_WINDOWS=1 ;;
	*) ON_WINDOWS=0 ;;
esac
if command -v python3 >/dev/null && python3 -c '' 2>/dev/null; then PY=python3; else PY=python; fi
if [ "$ON_WINDOWS" = 1 ]; then
	NATIVE_DRIVER=msedgedriver EXE=.exe BUNDLED_ORIGIN=http://tauri.localhost
else
	NATIVE_DRIVER=WebKitWebDriver EXE="" BUNDLED_ORIGIN=tauri://localhost
fi
# A path as the app should receive it: forward-slashed Windows form (C:/...) on Windows,
# which also needs no escaping inside the JavaScript strings below.
app_path() {
	if [ "$ON_WINDOWS" = 1 ]; then cygpath -m "$1"; else printf '%s' "$1"; fi
}

# Without these the window cannot be driven at all. Say so and skip, rather than
# reporting a pass that checked nothing.
for tool in tauri-driver "$NATIVE_DRIVER" curl "$PY"; do
	if ! command -v "$tool" >/dev/null; then
		echo "SKIP: $tool is not installed, so the window is UNVERIFIED by this run." >&2
		echo "      tauri-driver: cargo install tauri-driver" >&2
		echo "      WebKitWebDriver: webkit2gtk-driver (Debian/Ubuntu) or webkitgtk-6.0 (Arch)" >&2
		echo "      msedgedriver: msedgedriver-tool, which matches the installed WebView2" >&2
		exit 0
	fi
done

if [ -z "$APP" ]; then
	# Honour the shared CARGO_TARGET_DIR rather than assuming ./target.
	target=$(cargo metadata --format-version 1 --no-deps | "$PY" -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
	APP="$target/release/skidbladnir$EXE"
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
check "loads from the bundled frontend" 'return location.href' "$BUNDLED_ORIGIN"
check "renders the app" 'return document.querySelector("h1")?.textContent?.trim()' 'Skidbladnir'
check "IPC returns the linked encoder version" \
	'const I=window.__TAURI_INTERNALS__; return await I.invoke("encoder_version")' 'libwebp encoder'
check "IPC returns the core defaults" \
	'const I=window.__TAURI_INTERNALS__; const s=await I.invoke("default_settings"); return s.format+" "+String(s.webp.quality)+"/"+String(s.webp.method)' 'webp 75/4'
# cwebp's sliders in the default (lossy) state: quality, method, alpha quality, and the
# nine lossy tuning ones.
check "the WebP controls are present" \
	'return String(document.querySelectorAll("[role=slider]").length)' '12'
check "every focusable control has a name" \
	'const f=[...document.querySelectorAll("button,input,[role=slider],[role=radio],[role=checkbox]")];
	 const n=e=>e.getAttribute("aria-label")||e.closest("label")?.textContent?.trim()||e.textContent?.trim()||null;
	 return String(f.filter(e=>!n(e)).length)' '0'

# A real conversion, end to end, through the running app: a genuine PNG in, a genuine
# WebP out, checked on disk. Anything less is a check that cannot fail.
"$PY" - "$scratch/smoke.png" <<'PNG'
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
in_png=$(app_path "$scratch/smoke.png")
out_dir=$(app_path "$scratch/out")
check "converts an image end to end" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings');
	 const r=await I.invoke('convert_image', { settings: s, input: '$in_png', outputDirectory: '$out_dir' });
	 return r.width + 'x' + r.height + ' ' + (r.outputBytes > 0) + ' ' + typeof r.savingPercent" '48x48 true number'

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
	 try { await I.invoke('convert_image', { settings: s, input: '$out_dir/smoke.webp', outputDirectory: '$out_dir' }); return 'NOT REFUSED'; }
	 catch (e) { return String(e); }" 'refusing to overwrite the source'

# Replacing existing files can be turned off: the file already there is kept as it was.
before=$(wc -c < "$scratch/out/smoke.webp")
check "keeps an existing output when asked not to replace it" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings'); s.webp.quality=5;
	 try { await I.invoke('convert_image', { settings: s, input: '$in_png', outputDirectory: '$out_dir', replaceExisting: false }); return 'NOT KEPT'; }
	 catch (e) { return String(e); }" 'already exists'
if [ "$(wc -c < "$scratch/out/smoke.webp")" = "$before" ]; then printf 'ok   %s\n' "the kept file is unchanged"; else printf 'FAIL %s\n' "the kept file changed"; failures=$((failures + 1)); fi
check "warns before two inputs write the same file" \
	"const I=window.__TAURI_INTERNALS__;
	 const p=await I.invoke('plan_outputs', { format: 'webp', inputs: [{ path: '$in_png', relative: null }, { path: '$(app_path "$scratch/smoke.tif")', relative: null }], outputDirectory: '$out_dir' });
	 return p.collisions.length + ' ' + p.collisions[0].inputs.length + ' ' + p.existing.length" '1 2 1'
check "starts from cwebp's defaults" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings');
	 const w=await I.invoke('webp_cwebp_defaults', { webp: s.webp });
	 return [w.quality, w.passes, w.alphaFiltering, w.autofilter, w.multiThreading].join(' ')" "75 1 fast false true"
check "the About view reads the bundled licence" \
	"const I=window.__TAURI_INTERNALS__;
	 const t=await I.invoke('legal_document', { document: 'license' });
	 const n=await I.invoke('legal_document', { document: 'notices' });
	 return t.startsWith('ISC License') + ' ' + (n.length > 10000)" "true true"

# AVIF: the format switch, a real conversion checked on disk, and whether this platform's
# webview can display the AVIF preview at all (WebKitGTK and WebView2 each decide that).
check "switches to AVIF" \
	'[...document.querySelectorAll("[aria-label=\"Output format\"] [role=radio]")][1].click();
	 await new Promise(r => setTimeout(r, 300));
	 return String(document.querySelectorAll("[role=slider]").length) + " " + String(!!document.querySelector("[aria-label=\"libwebp preset\"]"))' '1 false'
check "converts an image to AVIF end to end" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings'); s.format='avif'; s.avif.speed=10;
	 const r=await I.invoke('convert_image', { settings: s, input: '$in_png', outputDirectory: '$out_dir' });
	 return r.outputPath.split(/[\\\\/]/).pop() + ' ' + r.width + 'x' + r.height" 'smoke.avif 48x48'
if [ "$(head -c 12 "$scratch/out/smoke.avif" 2>/dev/null | tail -c 8)" = "ftypavif" ]; then
	printf 'ok   %s\n' "wrote a real AVIF to disk ($(wc -c < "$scratch/out/smoke.avif") bytes)"
else
	printf 'FAIL %s\n' "no AVIF file at $scratch/out/smoke.avif"
	failures=$((failures + 1))
fi
# The AVIF preview is decoded in Rust and shown as lossless WebP, so it must display on
# every platform's web engine — including Ubuntu's WebKitGTK, which cannot decode AVIF.
check "the webview displays the AVIF preview" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings'); s.format='avif'; s.avif.speed=10;
	 const p=await I.invoke('preview_encode', { settings: s, input: '$in_png' });
	 return await new Promise(res => { const i=new Image(); i.onload=()=>res('displays ' + i.naturalWidth + 'x' + i.naturalHeight); i.onerror=()=>res('cannot display'); i.src=p.encoded; })" 'displays 48x48'
# JPEG XL: a real conversion checked on disk, and the preview, which is decoded in Rust
# because most web engines cannot display JPEG XL yet.
check "converts an image to JPEG XL end to end" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings'); s.format='jxl'; s.jxl.effort=3;
	 const r=await I.invoke('convert_image', { settings: s, input: '$in_png', outputDirectory: '$out_dir' });
	 return r.outputPath.split(/[\\\\/]/).pop() + ' ' + r.width + 'x' + r.height" 'smoke.jxl 48x48'
if [ "$(head -c 2 "$scratch/out/smoke.jxl" 2>/dev/null | od -An -tx1 | tr -d ' ')" = "ff0a" ]; then
	printf 'ok   %s\n' "wrote a real JPEG XL to disk ($(wc -c < "$scratch/out/smoke.jxl") bytes)"
else
	printf 'FAIL %s\n' "no JPEG XL file at $scratch/out/smoke.jxl"
	failures=$((failures + 1))
fi
check "the webview displays the JPEG XL preview" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings'); s.format='jxl'; s.jxl.effort=3;
	 const p=await I.invoke('preview_encode', { settings: s, input: '$in_png' });
	 return await new Promise(res => { const i=new Image(); i.onload=()=>res('displays ' + i.naturalWidth + 'x' + i.naturalHeight); i.onerror=()=>res('cannot display'); i.src=p.encoded; })" 'displays 48x48'
# HEIC: a real conversion through the libheif this app ships (Kvazaar, or x265 in the GPL
# edition), checked on disk, and the preview, decoded in Rust because only Safari displays
# HEIC.
check "converts an image to HEIC end to end" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings'); s.format='heic';
	 const r=await I.invoke('convert_image', { settings: s, input: '$in_png', outputDirectory: '$out_dir' });
	 return r.outputPath.split(/[\\\\/]/).pop() + ' ' + r.width + 'x' + r.height" 'smoke.heic 48x48'
if [ "$(head -c 12 "$scratch/out/smoke.heic" 2>/dev/null | tail -c 8)" = "ftypheic" ]; then
	printf 'ok   %s\n' "wrote a real HEIC to disk ($(wc -c < "$scratch/out/smoke.heic") bytes)"
else
	printf 'FAIL %s\n' "no HEIC file at $scratch/out/smoke.heic"
	failures=$((failures + 1))
fi
check "the webview displays the HEIC preview" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings'); s.format='heic';
	 const p=await I.invoke('preview_encode', { settings: s, input: '$in_png' });
	 return await new Promise(res => { const i=new Image(); i.onload=()=>res('displays ' + i.naturalWidth + 'x' + i.naturalHeight); i.onerror=()=>res('cannot display'); i.src=p.encoded; })" 'displays 48x48'
# The edition is the one this was built as, and what only x265 writes follows it: the GPL
# edition writes -L lossless and 10-bit HEIC, and the standard edition refuses 10-bit.
if [ "$EDITION" = gpl ]; then
	check "is the GPL edition" \
		"const e=await window.__TAURI_INTERNALS__.invoke('edition'); return e.name + ' ' + e.license + ' x265 ' + e.x265" 'gpl GPL-3.0-or-later x265 true'
	check "the HEIC panel shows x265's controls" \
		'[...document.querySelectorAll("[aria-label=\"Output format\"] [role=radio]")][3].click(); await new Promise(r => setTimeout(r, 300));
		 return "headings=" + [...document.querySelectorAll("h2")].map(h => h.textContent.trim()).filter(t => t === "x265").join(",") + " aq=" + !!document.querySelector("[aria-label=\"Adaptive quantisation\"]")' 'headings=x265 aq=true'
	mkdir -p "$scratch/x265"
	check "writes lossless HEIC through x265" \
		"const I=window.__TAURI_INTERNALS__;
		 const s=await I.invoke('default_settings'); s.format='heic'; s.heic.lossless=true; s.heic.preset='ultrafast';
		 const r=await I.invoke('convert_image', { settings: s, input: '$in_png', outputDirectory: '$(app_path "$scratch/x265")' });
		 return r.outputPath.split(/[\\\\/]/).pop() + ' ' + r.width + 'x' + r.height" 'smoke.heic 48x48'
	check "the webview displays a 10-bit 4:4:4 HEIC preview" \
		"const I=window.__TAURI_INTERNALS__;
		 const s=await I.invoke('default_settings'); s.format='heic'; s.heic.bitDepth='ten'; s.heic.chroma='444';
		 const p=await I.invoke('preview_encode', { settings: s, input: '$in_png' });
		 return await new Promise(res => { const i=new Image(); i.onload=()=>res('displays ' + i.naturalWidth + 'x' + i.naturalHeight); i.onerror=()=>res('cannot display'); i.src=p.encoded; })" 'displays 48x48'
else
	check "is the standard edition" \
		"const e=await window.__TAURI_INTERNALS__.invoke('edition'); return e.name + ' ' + e.license + ' x265 ' + e.x265" 'standard ISC x265 false'
	check "the HEIC panel has no x265 controls" \
		'[...document.querySelectorAll("[aria-label=\"Output format\"] [role=radio]")][3].click(); await new Promise(r => setTimeout(r, 300));
		 return "headings=" + [...document.querySelectorAll("h2")].map(h => h.textContent.trim()).filter(t => t === "x265").join(",") + " aq=" + !!document.querySelector("[aria-label=\"Adaptive quantisation\"]")' 'headings= aq=false'
	check "refuses HEIC only x265 can write" \
		"const I=window.__TAURI_INTERNALS__;
		 const s=await I.invoke('default_settings'); s.format='heic'; s.heic.bitDepth='ten';
		 try { await I.invoke('validate_settings', { settings: s }); return 'accepted'; } catch (e) { return String(e); }" 'only the GPL edition of Skidbladnir has'
fi
# AVIF input: the AVIF written above, converted back to WebP in another folder.
mkdir -p "$scratch/from-avif"
check "reads AVIF input" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings');
	 const r=await I.invoke('convert_image', { settings: s, input: '$out_dir/smoke.avif', outputDirectory: '$(app_path "$scratch/from-avif")' });
	 return r.outputPath.split(/[\\\\/]/).pop() + ' ' + r.width + 'x' + r.height" 'smoke.webp 48x48'

# Animated WebP: converted with every frame, refused (not cut down) for AVIF, and
# previewed as an animation the webview can show. The fixture was written by libwebp's own
# img2webp; its provenance is in crates/skidbladnir-encode/tests/animation.rs.
cp "$(dirname "$0")/../crates/skidbladnir-encode/tests/fixtures/animated.webp" "$scratch/anim.webp"
anim_in=$(app_path "$scratch/anim.webp")
check "converts an animated WebP end to end" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings');
	 const r=await I.invoke('convert_image', { settings: s, input: '$anim_in', outputDirectory: '$out_dir' });
	 return r.outputPath.split(/[\\\\/]/).pop() + ' ' + r.width + 'x' + r.height" 'anim.webp 24x16'
frames=$("$PY" -c 'import sys; print(open(sys.argv[1], "rb").read().count(b"ANMF"))' "$scratch/out/anim.webp" 2>/dev/null || echo 0)
if [ "$frames" = 3 ]; then
	printf 'ok   %s\n' "wrote an animated WebP with all 3 frames ($(wc -c < "$scratch/out/anim.webp") bytes)"
else
	printf 'FAIL %s\n' "expected 3 frames in $scratch/out/anim.webp, found $frames"
	failures=$((failures + 1))
fi
check "refuses to cut an animation down to an AVIF still" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings'); s.format='avif';
	 try { await I.invoke('convert_image', { settings: s, input: '$anim_in', outputDirectory: '$out_dir' }); return 'NOT REFUSED'; }
	 catch (e) { return String(e); }" 'AVIF, JPEG XL and HEIC output take still images only'
# GIF input, with the ImageMagick-written fixture from tests/gif.rs.
cp "$(dirname "$0")/../crates/skidbladnir-encode/tests/fixtures/animated.gif" "$scratch/clip.gif"
gif_in=$(app_path "$scratch/clip.gif")
check "converts an animated GIF to animated WebP" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings');
	 const r=await I.invoke('convert_image', { settings: s, input: '$gif_in', outputDirectory: '$out_dir' });
	 const found=await I.invoke('inspect_dropped_paths', { paths: ['$gif_in'] });
	 return r.outputPath.split(/[\\\\/]/).pop() + ' ' + r.width + 'x' + r.height + ' ' + found[0].format + ' animated=' + found[0].animated" 'clip.webp 24x16 GIF animated=true'
check "previews an animation the webview can show" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings');
	 const p=await I.invoke('preview_encode', { settings: s, input: '$anim_in' });
	 const shown=await new Promise(res => { const i=new Image(); i.onload=()=>res(i.naturalWidth + 'x' + i.naturalHeight); i.onerror=()=>res('not displayable'); i.src=p.encoded; });
	 return p.frames + ' frames, shown ' + shown" '3 frames, shown 24x16'

# HEIC from a lossy WebP (heif-enc's reader takes its YCbCr planes as they are) and from a
# HEIC (decoded in its own colourspace, metadata kept): the WebP and HEIC written above.
mkdir -p "$scratch/to-heic"
for source in smoke.webp smoke.heic; do
	check "converts $source to HEIC" \
		"const I=window.__TAURI_INTERNALS__;
		 const s=await I.invoke('default_settings'); s.format='heic';
		 const r=await I.invoke('convert_image', { settings: s, input: '$out_dir/$source', outputDirectory: '$(app_path "$scratch/to-heic")' });
		 return r.outputPath.split(/[\\\\/]/).pop() + ' ' + r.width + 'x' + r.height" "smoke.heic 48x48"
done

# The TIFF alpha opt-in crosses IPC: off by default, and on, a straight-alpha TIFF converts
# as cwebp converts it (premultiplied), which is a different file.
cp "$(dirname "$0")/../crates/skidbladnir-encode/tests/fixtures/tiff-rgba8.tif" "$scratch/straight.tif"
check "reads TIFF alpha correctly unless asked otherwise" \
	"const I=window.__TAURI_INTERNALS__;
	 const s=await I.invoke('default_settings'); s.webp.lossless=true; s.webp.exact=true;
	 const input='$(app_path "$scratch/straight.tif")';
	 const off=await I.invoke('preview_encode', { settings: s, input });
	 const asked=structuredClone(s); asked.tiffAlphaLikeReference=true;
	 const on=await I.invoke('preview_encode', { settings: asked, input });
	 return 'default ' + s.tiffAlphaLikeReference + ', on differs ' + (on.encoded !== off.encoded)" 'default false, on differs true'

# PNM, PAM and PFM input: recognised by content, and converted to WebP (as cwebp reads PNM)
# and to JPEG XL (as cjxl reads all three).
"$PY" - "$scratch" <<'PNM'
import struct, sys
d, w, h = sys.argv[1], 20, 12
open(d + '/netpbm-p6.ppm', 'wb').write(b'P6\n%d %d\n255\n' % (w, h) + bytes((x * 12 + c * 40) % 256 for y in range(h) for x in range(w) for c in range(3)))
open(d + '/netpbm-pam.pam', 'wb').write(b'P7\nWIDTH %d\nHEIGHT %d\nDEPTH 4\nMAXVAL 65535\nTUPLTYPE RGB_ALPHA\nENDHDR\n' % (w, h) + b''.join(struct.pack('>HHHH', x * 3000, y * 5000, 30000, 65535 - x * 1000) for y in range(h) for x in range(w)))
open(d + '/netpbm-pfm.pfm', 'wb').write(b'PF\n%d %d\n-1.0\n' % (w, h) + b''.join(struct.pack('<fff', x / w, y / h, 0.5) for y in range(h) for x in range(w)))
open(d + '/netpbm-svg.svg', 'wb').write(b'<?xml version="1.0"?>\n<svg xmlns="http://www.w3.org/2000/svg" width="%d" height="%d"><rect width="10" height="12" fill="#c33"/><circle cx="15" cy="6" r="5" fill="#33c" fill-opacity="0.5"/></svg>' % (w, h))
open(d + '/netpbm-pgx.pgx', 'wb').write(b'PG ML + 12 %d %d\n' % (w, h) + b''.join(struct.pack('>H', (x * 200 + y * 100) % 4096) for y in range(h) for x in range(w)))
PNM
mkdir -p "$scratch/from-netpbm"
for kind in p6:ppm:PNM pam:pam:PNM pfm:pfm:PFM pgx:pgx:PGX svg:svg:SVG; do
	IFS=: read -r name extension format <<< "$kind"
	check "reads $format input (.$extension)" \
		"const I=window.__TAURI_INTERNALS__;
		 const input='$(app_path "$scratch/netpbm-$name.$extension")', out='$(app_path "$scratch/from-netpbm")';
		 const found=await I.invoke('inspect_dropped_paths', { paths: [input] });
		 const s=await I.invoke('default_settings');
		 const w=await I.invoke('convert_image', { settings: s, input, outputDirectory: out });
		 s.format='jxl';
		 const j=await I.invoke('convert_image', { settings: s, input, outputDirectory: out });
		 return found[0].format + ' ' + [w, j].map(r => r.outputPath.split(/[\\\\/]/).pop() + ' ' + r.width + 'x' + r.height).join(' ')" "$format netpbm-$name.webp 20x12 netpbm-$name.jxl 20x12"
done

# Y4M: AVIF through avifenc's own reader, WebP through the conversion to RGB.
"$PY" - "$scratch/smoke.y4m" <<'Y4M'
import sys
w, h = 20, 12
planes = bytes((x * 9 + y * 5) % 256 for y in range(h) for x in range(w)) + bytes(128 for _ in range(2 * (w // 2) * (h // 2)))
open(sys.argv[1], 'wb').write(b'YUV4MPEG2 W20 H12 F30:1 Ip A1:1 C420jpeg\nFRAME\n' + planes)
Y4M
check "reads Y4M input" \
	"const I=window.__TAURI_INTERNALS__;
	 const input='$(app_path "$scratch/smoke.y4m")', out='$(app_path "$scratch/from-netpbm")';
	 const found=await I.invoke('inspect_dropped_paths', { paths: [input] });
	 const s=await I.invoke('default_settings');
	 const w=await I.invoke('convert_image', { settings: s, input, outputDirectory: out });
	 s.format='avif';
	 const a=await I.invoke('convert_image', { settings: s, input, outputDirectory: out });
	 return found[0].format + ' ' + [w, a].map(r => r.outputPath.split(/[\\\\/]/).pop() + ' ' + r.width + 'x' + r.height).join(' ')" "Y4M smoke.webp 20x12 smoke.avif 20x12"

# Raw YUV: a .yuv is offered to the queue by its extension, refused with no size, and read
# at the size the settings give (cwebp's -s). Odd, so the chroma planes round up.
"$PY" - "$scratch/raw.yuv" <<'YUV'
import sys
w, h = 21, 13
uv = ((w + 1) // 2) * ((h + 1) // 2)
open(sys.argv[1], 'wb').write(bytes((x * 9 + y * 5) % 220 + 16 for y in range(h) for x in range(w)) + bytes(128 for _ in range(2 * uv)))
YUV
check "reads raw YUV input at the size given" \
	"const I=window.__TAURI_INTERNALS__;
	 const input='$(app_path "$scratch/raw.yuv")', out='$(app_path "$scratch/from-netpbm")';
	 const found=await I.invoke('inspect_dropped_paths', { paths: [input] });
	 const s=await I.invoke('default_settings');
	 const unsized=await I.invoke('convert_image', { settings: s, input, outputDirectory: out }).then(() => 'converted', () => 'refused');
	 s.yuvSize={ width: 21, height: 13 };
	 const w=await I.invoke('convert_image', { settings: s, input, outputDirectory: out });
	 s.format='avif';
	 const a=await I.invoke('convert_image', { settings: s, input, outputDirectory: out });
	 return found[0].format + ' ' + unsized + ' ' + [w, a].map(r => r.outputPath.split(/[\\\\/]/).pop() + ' ' + r.width + 'x' + r.height).join(' ')" "YUV refused raw.webp 21x13 raw.avif 21x13"

echo
if [ "$failures" -gt 0 ]; then
	echo "$failures check(s) failed."
	exit 1
fi
echo "all checks passed"
