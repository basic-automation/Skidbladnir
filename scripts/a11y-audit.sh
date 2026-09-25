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
for tool in tauri-driver WebKitWebDriver curl python3; do
	command -v "$tool" >/dev/null || { echo "SKIP: $tool is not installed; the window is UNAUDITED." >&2; exit 0; }
done

if [ -z "$APP" ]; then
	target=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
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

report=$("$HARNESS" exec --script '
const results = await window.axe.run(document, { resultTypes: ["violations"] });
return JSON.stringify(results.violations.map(v => ({ id: v.id, impact: v.impact, help: v.help, nodes: v.nodes.length })));
')

printf '%s' "$report" | THRESHOLD="$THRESHOLD" python3 -c '
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
'
