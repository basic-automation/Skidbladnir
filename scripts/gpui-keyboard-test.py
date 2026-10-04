#!/usr/bin/env python3
"""Tab through the gpui window and check the keyboard reaches every control.

WHY THIS EXISTS
---------------
The README says the whole window works from the keyboard. For the webview window the
smoke test and axe check it; nothing had checked the gpui window. This presses Tab, as a
keyboard user does, and after each press reads which control has the focus from the
window's accessibility tree (AT-SPI). It fails when a control that says it can take the
focus is never reached, when Tab stops moving (a trap), or when the focus lands on a
nameless control. Shift+Tab must walk the same cycle backwards. In the WebP panel it also
presses the keys inside controls: the arrows, Page Up, Home and End on a slider, the arrows
in a radio group, Space on a check box.

Each format's panel is checked with every disclosure open. It runs in the private AT-SPI
session scripts/gpui-a11y-audit.py sets up, with a scratch configuration, and the window on
X11 (WAYLAND_DISPLAY unset), with keys sent by xdotool. Keys go only to the test's own
window: before each press the window must hold the X input focus (and, under Hyprland,
be the active window), or the test stops without sending it.

USAGE
    scripts/gpui-keyboard-test.py [--app <binary>] [--edition standard|gpl]
"""

import argparse
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
spec = importlib.util.spec_from_file_location("gpui_a11y_audit", os.path.join(ROOT, "scripts", "gpui-a11y-audit.py"))
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)

STATES = [state for state in audit.STATES if state[0] in ("webp", "avif", "jxl", "heic")]


class NotOurs(Exception):
	"""The test's window does not have the keyboard: no key is sent."""


def own_window(pid):
	"""Make sure keys go to this process's window, and only to it."""
	if shutil.which("hyprctl") and os.environ.get("HYPRLAND_INSTANCE_SIGNATURE"):
		subprocess.run(["hyprctl", "dispatch", "focuswindow", f"pid:{pid}"], stdout=subprocess.DEVNULL, check=False)
		time.sleep(0.2)
		active = json.loads(subprocess.run(["hyprctl", "-j", "activewindow"], capture_output=True, text=True).stdout or "{}")
		if active.get("pid") != pid:
			raise NotOurs(f"the active window is pid {active.get('pid')}, not the test's {pid}")
	focused = subprocess.run(["xdotool", "getwindowfocus", "getwindowpid"], capture_output=True, text=True).stdout.strip()
	if focused != str(pid):
		windows = subprocess.run(["xdotool", "search", "--pid", str(pid)], capture_output=True, text=True).stdout.split()
		for window in windows:
			subprocess.run(["xdotool", "windowfocus", window], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
		time.sleep(0.2)
		focused = subprocess.run(["xdotool", "getwindowfocus", "getwindowpid"], capture_output=True, text=True).stdout.strip()
		if focused != str(pid):
			raise NotOurs(f"X input focus is on pid {focused or 'none'}, not the test's {pid}")


def press(pid, keys):
	own_window(pid)
	subprocess.run(["xdotool", "key", "--clearmodifiers", keys], check=True)
	time.sleep(0.15)


def focused(reader, app):
	atspi = reader.atspi
	for node in reader.walk(app):
		if node.get_state_set().contains(atspi.StateType.FOCUSED):
			return node
	return None


def describe(node):
	return f"{node.get_role_name()} {node.get_name()!r}" if node is not None else "nothing"


def walk_cycle(reader, app, pid, key, limit):
	"""Press `key` until the focus comes back to the first control it reached; the nodes it
	visited, that one last. Nodes are told apart by their AT-SPI path, which AccessKit
	derives from gpui's element id: two controls may share a name."""
	visited = []
	for _ in range(limit):
		press(pid, key)
		node = focused(reader, app)
		if node is None:
			return visited, "the focus left every control (nothing reports focused)"
		if visited and node.path == visited[-1].path:
			return visited, f"{key} did not move the focus from {describe(node)}"
		visited.append(node)
		if len(visited) > 1 and node.path == visited[0].path:
			return visited, None
	return visited, f"{key} did not come back to where it started in {limit} presses"


def inner_keys(reader, app, pid):
	"""The keys inside controls, in the WebP panel: the arrows, Home and End on a slider,
	the arrows in a radio group, Space on a check box. Each control is focused over AT-SPI
	first, as a screen reader's user would move to it."""
	atspi = reader.atspi
	problems = []

	def find(role, name):
		return next((node for node in reader.walk(app) if node.get_role() == role and node.get_name() == name), None)

	def focus(node):
		component = node.get_component_iface()
		if component is None or not component.grab_focus():
			return False
		for _ in range(20):
			if node.get_state_set().contains(atspi.StateType.FOCUSED):
				return True
			time.sleep(0.1)
		return False

	slider = find(atspi.Role.SLIDER, "Quality")
	if slider is None or not focus(slider):
		problems.append("the Quality slider cannot be focused")
	else:
		value = lambda: find(atspi.Role.SLIDER, "Quality").get_value_iface().get_current_value()
		start = value()
		for key, expected in (("Right", start + 1), ("Left", start), ("Page_Up", min(start + 10, 100)), ("End", 100), ("Home", 0)):
			press(pid, key)
			if value() != expected:
				problems.append(f"{key} on the Quality slider gives {value()}, not {expected}")

	quality = find(atspi.Role.RADIO_BUTTON, "Quality")
	if quality is None or not focus(quality):
		problems.append("the Quality radio button cannot be focused")
	else:
		press(pid, "Right")
		size = find(atspi.Role.RADIO_BUTTON, "File size")
		if size is None or not size.get_state_set().contains(atspi.StateType.CHECKED) or not size.get_state_set().contains(atspi.StateType.FOCUSED):
			problems.append("Right from the Quality radio button does not check and focus File size")
		press(pid, "Left")
		if not find(atspi.Role.RADIO_BUTTON, "Quality").get_state_set().contains(atspi.StateType.CHECKED):
			problems.append("Left from File size does not check Quality again")

	lossless = find(atspi.Role.CHECK_BOX, "Lossless")
	if lossless is None or not focus(lossless):
		problems.append("the Lossless check box cannot be focused")
	else:
		before = lossless.get_state_set().contains(atspi.StateType.CHECKED)
		press(pid, "space")
		if find(atspi.Role.CHECK_BOX, "Lossless").get_state_set().contains(atspi.StateType.CHECKED) == before:
			problems.append("Space does not toggle the Lossless check box")
	return problems


def run(args):
	reader = audit.AtspiReader()
	failures = 0
	try:
		for index, (label, variables, *_) in enumerate(STATES):
			if index != args.state:
				continue
			with tempfile.TemporaryDirectory(prefix="skidbladnir-gpui-keys.") as scratch:
				env = dict(os.environ, XDG_CONFIG_HOME=os.path.join(scratch, "config"), XDG_DATA_HOME=os.path.join(scratch, "data"), **variables)
				env.pop("WAYLAND_DISPLAY", None)
				libheif = os.path.join(ROOT, "build", "libheif-gpl" if args.edition == "gpl" else "libheif", "lib")
				if os.path.exists(os.path.join(libheif, "libheif.so.1")) and not os.path.exists(os.path.join(os.path.dirname(os.path.abspath(args.app)), "libheif.so.1")):
					env["LD_LIBRARY_PATH"] = libheif + (":" + env["LD_LIBRARY_PATH"] if env.get("LD_LIBRARY_PATH") else "")
				with open(os.path.join(scratch, "app.log"), "wb") as log:
					process = subprocess.Popen([os.path.abspath(args.app)], env=env, stdout=log, stderr=subprocess.STDOUT)
					try:
						app = reader.find(process.pid)
						if app is None:
							failures += 1
							print(f"FAIL {label}: no accessibility tree")
							continue
						time.sleep(2)
						atspi = reader.atspi
						# What Tab must reach: every control that can take the focus, scrolled into
						# view or not, except the window itself; and every radio group once,
						# since Tab enters a group at its checked radio and the arrow keys move
						# within it (the ARIA radio group pattern), though AT-SPI calls every
						# radio focusable.
						focusable = [node for node in reader.walk(app) if node.get_state_set().contains(atspi.StateType.FOCUSABLE) and node.get_role() not in (atspi.Role.FRAME, atspi.Role.WINDOW)]
						radios = [node for node in focusable if node.get_role() == atspi.Role.RADIO_BUTTON]
						stops = [node for node in focusable if node.get_role() != atspi.Role.RADIO_BUTTON]
						groups = {}
						for radio in radios:
							group = radio.get_parent()
							groups.setdefault(group.path, group)
						limit = len(focusable) * 2 + 10
						forward, problem = walk_cycle(reader, app, process.pid, "Tab", limit)
						problems = [problem] if problem else []
						reached = {node.path for node in forward}
						reached_groups = {node.get_parent().path for node in forward if node.get_role() == atspi.Role.RADIO_BUTTON}
						problems += [f"Tab never reaches {describe(node)}" for node in stops if node.path not in reached]
						problems += [f"Tab never reaches the radio group {group.get_name()!r}" for path, group in groups.items() if path not in reached_groups]
						problems += [f"the focus lands on {describe(node)}, which has no name" for node in forward if not (node.get_name() or "").strip()]
						backward, problem = walk_cycle(reader, app, process.pid, "shift+Tab", limit)
						if problem:
							problems.append(problem)
						elif [node.path for node in backward][:-1] != [node.path for node in reversed(forward)][1:]:
							problems.append("Shift+Tab does not walk Tab's cycle backwards")
						if label == "webp":
							inner = inner_keys(reader, app, process.pid)
							problems += inner
							if not inner:
								print("ok   webp: the arrows, Page Up, Home and End move the Quality slider; the arrows move a radio group's choice; Space toggles a check box")
						if problems:
							failures += len(problems)
							print(f"FAIL {label}: {len(forward)} Tab stops for {len(stops)} controls and {len(groups)} radio groups; {len(problems)} problems:")
							for line in problems[:20]:
								print(f"       {line}")
						else:
							print(f"ok   {label}: Tab reaches all {len(stops)} controls and {len(groups)} radio groups in a cycle of {len(forward) - 1} stops; Shift+Tab walks it back")
					except NotOurs as error:
						failures += 1
						print(f"STOP {label}: {error}; no keys were sent")
						break
					finally:
						process.terminate()
						try:
							process.wait(timeout=10)
						except subprocess.TimeoutExpired:
							process.kill()
	finally:
		reader.close()
	return 1 if failures else 0


def main():
	parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
	parser.add_argument("--app", default=os.path.join(ROOT, "target", "release", "skidbladnir"))
	parser.add_argument("--edition", choices=["standard", "gpl"], default="standard")
	parser.add_argument("--state", type=int, help=argparse.SUPPRESS)
	args = parser.parse_args()
	if not os.access(args.app, os.X_OK):
		sys.exit(f"no app at {args.app}: build it first, or pass --app")
	if not os.environ.get("DISPLAY") or shutil.which("xdotool") is None:
		sys.exit("needs an X display (DISPLAY), or XWayland, and xdotool")
	if "SKIDBLADNIR_A11Y_AUDIT_SESSION" not in os.environ:
		# Each state in a private session of its own, as the audit runs them.
		failed = sum(1 for index in range(len(STATES)) if audit.in_private_session(["--app", args.app, "--edition", args.edition, "--state", str(index)], __file__))
		print("all checks passed" if not failed else f"{failed} of {len(STATES)} states with problems")
		return 1 if failed else 0
	return run(args)


if __name__ == "__main__":
	sys.exit(main())
