#!/usr/bin/env bash
# Run axe-core against the live Skidbladnir window and report accessibility violations.
#
# WHY A REAL AUDIT TOOL
# ---------------------
# The smoke test checks that every focusable control has an accessible name. That is one
# rule. axe-core checks colour contrast, ARIA validity, heading order, landmark structure
# and around ninety others — the things a hand-written check does not know to look for.
#
# axe-core is injected from node_modules rather than fetched, because the window runs
# under a CSP with no remote sources and a desktop app should not need the network to be
# testable.
#
# USAGE
#   npm --prefix frontend install        # once, for node_modules/axe-core
#   cd src-tauri && cargo tauri build --no-bundle
#   scripts/a11y-audit.sh                # from the repo root
#
# Exits non-zero if any violation at or above the severity threshold is found.
# --threshold takes: minor, moderate, serious, critical (default: serious).
set -euo pipefail

HARNESS="${SKIDBLADNIR_WEBDRIVER_HARNESS:-$(dirname "$0")/webdriver.sh}"
AXE="$(dirname "$0")/../frontend/node_modules/axe-core/axe.min.js"
APP=""
THRESHOLD="serious"
while [ $# -gt 0 ]; do
	case "$1" in
		--app) APP="$2"; shift 2;;
		--threshold) THRESHOLD="$2"; shift 2;;
		*) echo "unknown argument: $1" >&2; exit 2;;
	esac
done

if [ ! -f "$AXE" ]; then
	echo "SKIP: axe-core not found at $AXE. Run: npm --prefix frontend install" >&2
	exit 0
fi
if command -v python3 >/dev/null && python3 -c '' 2>/dev/null; then PY=python3; else PY=python; fi
for tool in tauri-driver WebKitWebDriver curl "$PY"; do
	command -v "$tool" >/dev/null || { echo "SKIP: $tool is not installed; the window is UNAUDITED." >&2; exit 0; }
done

if [ -z "$APP" ]; then
	target=$(cargo metadata --format-version 1 --no-deps | "$PY" -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
	APP="$target/release/skidbladnir"
fi
[ -x "$APP" ] || { echo "FAIL: no binary at $APP" >&2; exit 1; }

cleanup() { "$HARNESS" stop >/dev/null 2>&1 || true; }
trap cleanup EXIT

echo "==> starting $APP"
"$HARNESS" start --app "$APP" >/dev/null
sleep 5

# Inject the library, then run it. Split in two so the injection payload and the run are
# separate WebDriver calls rather than one very large one.
inject=$(mktemp)
cat "$AXE" > "$inject"
printf '\nreturn typeof window.axe;\n' >> "$inject"
"$HARNESS" exec --script-file "$inject" >/dev/null
rm -f "$inject"

# The window is audited in each state that shows different controls: as it opens (WebP),
# with AVIF selected (its own panel of sliders and radio groups), and with a preview on
# screen (images and captions). A state the audit never reaches is a state it cannot
# vouch for.
fixture=$(mktemp -d)
"$PY" - "$fixture/audit.png" <<'PNG'
import struct, sys, zlib
w = h = 32
rows = bytearray()
for y in range(h):
	rows.append(0)
	for x in range(w):
		rows += bytes(((x * 8) % 256, (y * 8) % 256, 128, 255))
def chunk(tag, data):
	body = tag + data
	return struct.pack('>I', len(data)) + body + struct.pack('>I', zlib.crc32(body) & 0xffffffff)
open(sys.argv[1], 'wb').write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(bytes(rows))) + chunk(b'IEND', b''))
PNG

failed_states=0
audit() {
	local state="$1" setup="$2"
	if [ -n "$setup" ]; then
		"$HARNESS" exec --script "const sleep=ms=>new Promise(r=>setTimeout(r,ms)); $setup" >/dev/null
	fi
	echo "--- $state"
	local report
	report=$("$HARNESS" exec --script '
const results = await window.axe.run(document, { resultTypes: ["violations"] });
return JSON.stringify(results.violations.map(v => ({ id: v.id, impact: v.impact, help: v.help, nodes: v.nodes.length })));
')
	printf '%s' "$report" | THRESHOLD="$THRESHOLD" "$PY" -c '
import json, os, sys

order = {"minor": 0, "moderate": 1, "serious": 2, "critical": 3}
threshold = order[os.environ["THRESHOLD"]]
raw = sys.stdin.read().strip()
try:
    violations = json.loads(raw)
except Exception:
    print(f"could not parse the audit result: {raw[:400]}")
    sys.exit(1)

if not violations:
    print("axe-core: no violations")
    sys.exit(0)

failed = 0
for v in violations:
    impact = v.get("impact") or "minor"
    marker = "FAIL" if order.get(impact, 0) >= threshold else "warn"
    if marker == "FAIL":
        failed += 1
    print(f"{marker} [{impact}] {v["id"]}: {v["help"]} ({v["nodes"]} node(s))")

print()
if failed:
    print(f"{failed} violation(s) at or above {os.environ["THRESHOLD"]}.")
    sys.exit(1)
print(f"no violations at or above {os.environ["THRESHOLD"]}.")
' || failed_states=$((failed_states + 1))
}

audit "as it opens (WebP)" ""
audit "with AVIF selected" \
	'[...document.querySelectorAll("[aria-label=\"Output format\"] [role=radio]")][1].click(); await sleep(300); return "ok"'
# Tauri's own drop event is the one way to hand the window a file without a native
# dialog, and a preview needs a file.
audit "with a preview on screen" \
	"await window.__TAURI_INTERNALS__.invoke('plugin:event|emit', { event: 'tauri://drag-drop', payload: { paths: ['$fixture/audit.png'], position: { x: 10, y: 10 } } });
	 await sleep(800);
	 [...document.querySelectorAll('button')].find(b => b.textContent.trim() === 'Preview').click();
	 for (let i = 0; i < 150 && document.querySelectorAll('figure img').length < 2; i++) await sleep(100);
	 await sleep(300); return 'ok'"
rm -rf "$fixture"

if [ "$failed_states" -gt 0 ]; then
	echo
	echo "$failed_states state(s) had violations."
	exit 1
fi
