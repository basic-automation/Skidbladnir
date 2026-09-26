#!/usr/bin/env bash
# A minimal WebDriver harness for the Skidbladnir window, with no dependencies outside
# this repository.
#
#   tauri-driver (:4444)  ->  WebKitWebDriver (:4445)  ->  the app
#
# WHY IT LIVES HERE
# -----------------
# The smoke test used to reach into a harness under $HOME, which meant it could never run
# anywhere but one machine — and a check that only one machine can run is not a check the
# project has. This is the same chain, self-contained.
#
# REQUIREMENTS
#   tauri-driver      cargo install tauri-driver
#   native driver     Linux: WebKitWebDriver (Debian/Ubuntu: webkit2gtk-driver · Arch:
#                     webkitgtk-6.0). Windows: msedgedriver on PATH, matching the
#                     installed WebView2 — msedgedriver-tool fetches the right one.
#                     macOS: none exists; tauri-driver does not support it.
#   a display         Linux only: DISPLAY or WAYLAND_DISPLAY (xvfb-run when headless)
#   curl, python3     (or `python`, which is what the Windows runner calls it)
#
# WAYLAND IS THE TRAP. Under Wayland the app dies during startup with a Gdk protocol error
# and WebDriver reports the useless "session deleted because of page crash or hang", which
# reads exactly like the app crashing on its own. It is GTK under automation, not the app.
# `start` forces the X11/XWayland backend whenever it sees a Wayland session.
set -euo pipefail

CMD="${1:-}"; shift || true
APP=""; SCRIPT=""; OUT=""; PORT="${SKIDBLADNIR_WD_PORT:-4444}"; NATIVE_PORT="${SKIDBLADNIR_WD_NATIVE_PORT:-4445}"
while [ $# -gt 0 ]; do
	case "$1" in
		--app) APP="$2"; shift 2;;
		--script) SCRIPT="$2"; shift 2;;
		# A file, for scripts too large to pass as an argument — injecting axe-core is
		# half a megabyte and blows the argv limit.
		--script-file) SCRIPT="$(cat "$2")"; shift 2;;
		--out) OUT="$2"; shift 2;;
		*) echo "unknown argument: $1" >&2; exit 2;;
	esac
done

STATE="${XDG_RUNTIME_DIR:-/tmp}/skidbladnir-wd-$PORT.json"

# Git Bash on Windows reports MINGW*/MSYS*. The native driver, the display requirement and
# the Python executable's name all differ there.
case "$(uname -s)" in
	MINGW* | MSYS* | CYGWIN*) ON_WINDOWS=1 ;;
	*) ON_WINDOWS=0 ;;
esac
if command -v python3 >/dev/null && python3 -c '' 2>/dev/null; then PY=python3; else PY=python; fi
if [ "$ON_WINDOWS" = 1 ]; then NATIVE_DRIVER=msedgedriver; else NATIVE_DRIVER=WebKitWebDriver; fi

json_field() {
	"$PY" -c 'import json,sys; d=json.load(sys.stdin); print(d.get(sys.argv[1], ""))' "$1"
}

case "$CMD" in
start)
	[ -n "$APP" ] || { echo "start needs --app <binary>" >&2; exit 2; }
	command -v tauri-driver >/dev/null || { echo "tauri-driver not on PATH (cargo install tauri-driver)" >&2; exit 3; }
	command -v "$NATIVE_DRIVER" >/dev/null || { echo "$NATIVE_DRIVER not on PATH" >&2; exit 3; }
	if [ "$ON_WINDOWS" = 0 ]; then
		[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] || { echo "no display; run under xvfb-run on a headless machine" >&2; exit 3; }
	fi

	if [ -n "${WAYLAND_DISPLAY:-}" ]; then
		# GTK3 under WebKitWebDriver aborts on Wayland; XWayland is fine.
		export GDK_BACKEND=x11
	fi

	# tauri-driver is a native Windows program there: it cannot open a Git Bash path like
	# /d/a/..., so hand it the Windows form.
	if [ "$ON_WINDOWS" = 1 ]; then APP=$(cygpath -w "$APP"); fi
	driver_log="${STATE%.json}.driver.log"
	tauri-driver --port "$PORT" --native-port "$NATIVE_PORT" >"$driver_log" 2>&1 &
	driver_pid=$!
	for _ in $(seq 1 50); do
		curl -sf -o /dev/null "http://127.0.0.1:$PORT/status" && break
		sleep 0.2
	done

	payload=$("$PY" -c 'import json,sys; print(json.dumps({"capabilities":{"alwaysMatch":{"tauri:options":{"application":sys.argv[1]}}}}))' "$APP")
	# -s, not -sf: when the session is refused the driver's error body is the diagnosis,
	# and -f would throw it away.
	response=$(curl -s --max-time 120 -X POST -H 'Content-Type: application/json' -d "$payload" "http://127.0.0.1:$PORT/session" || true)
	session=$(printf '%s' "$response" | "$PY" -c 'import json,sys
try:
    d = json.load(sys.stdin)
except Exception:
    sys.exit(1)
print(d.get("value", {}).get("sessionId") or d.get("sessionId") or "")' 2>/dev/null || true)

	if [ -z "$session" ]; then
		kill "$driver_pid" 2>/dev/null || true
		echo "could not start a session. Driver response: $response" >&2
		echo "--- tauri-driver log ($driver_log) ---" >&2
		cat "$driver_log" >&2 || true
		exit 4
	fi
	printf '{"session":"%s","port":"%s","driver":"%s"}\n' "$session" "$PORT" "$driver_pid" > "$STATE"
	echo "session $session"
	;;

exec)
	[ -f "$STATE" ] || { echo "no session; run start first" >&2; exit 2; }
	session=$(json_field session < "$STATE")
	script_file=$(mktemp)
	printf '%s' "$SCRIPT" > "$script_file"
	payload_file=$(mktemp)
	"$PY" -c 'import json,sys
script = open(sys.argv[1], encoding="utf-8").read()
json.dump({"script": script, "args": []}, open(sys.argv[2], "w", encoding="utf-8"))' "$script_file" "$payload_file"
	# execute/sync, not execute/async: the async endpoint waits for the script to invoke
	# a completion callback, so a script that simply returns hangs until the driver gives
	# up. WebKitWebDriver resolves a returned promise on the sync endpoint, which is what
	# lets these scripts use `await`.
	response=$(curl -s --max-time 120 -X POST -H 'Content-Type: application/json' --data-binary "@$payload_file" \
		"http://127.0.0.1:$PORT/session/$session/execute/sync")
	rm -f "$script_file" "$payload_file"
	# Unwrap WebDriver's {"value": ...} envelope, so callers compare against what the
	# script actually returned rather than against JSON scaffolding.
	printf '%s' "$response" | "$PY" -c 'import json,sys
raw = sys.stdin.read()
try:
    value = json.loads(raw).get("value")
except Exception:
    sys.stdout.write(raw)
else:
    sys.stdout.write(value if isinstance(value, str) else json.dumps(value))'
	echo
	;;

stop)
	if [ -f "$STATE" ]; then
		session=$(json_field session < "$STATE")
		driver=$(json_field driver < "$STATE")
		curl -sf -X DELETE "http://127.0.0.1:$PORT/session/$session" >/dev/null 2>&1 || true
		[ -n "$driver" ] && kill "$driver" 2>/dev/null || true
		rm -f "$STATE"
	fi
	echo "stopped"
	;;

*)
	echo "usage: $0 {start --app <binary> | exec --script <js> | stop}" >&2
	exit 2
	;;
esac
