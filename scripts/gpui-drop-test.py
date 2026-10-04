#!/usr/bin/env python3
"""Drop a file onto the gpui window the way a file manager does, then convert it.

WHY THIS EXISTS
---------------
scripts/gpui-smoke.sh queues files through SKID_INPUTS, which takes the drop handler's path
in code but not the drop itself, so whether a real drag reached the gpui window had never
been tested. On X11 a drag between applications is a protocol, XDND
(https://www.freedesktop.org/wiki/Specifications/XDND/): this script plays the file
manager's side of it. It offers a PNG as text/uri-list, moves the pointer into the window,
answers the window's request for the data, and drops. Then it presses Convert through the
window's accessibility tree, as a screen reader's user would, and checks that the
converted file is written. It runs inside the private AT-SPI session
scripts/gpui-a11y-audit.py sets up, with a scratch configuration.

USAGE
    scripts/gpui-drop-test.py [--app <binary>] [--edition standard|gpl]

Linux, X11 only (under Wayland it runs the window through XWayland: WAYLAND_DISPLAY is
unset for it). Needs python-xlib and xdotool, and what the audit needs.
"""

import argparse
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.parse

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
spec = importlib.util.spec_from_file_location("gpui_a11y_audit", os.path.join(ROOT, "scripts", "gpui-a11y-audit.py"))
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class DragSource:
	"""The file manager's half of XDND, version 5, offering one file."""

	def __init__(self, path):
		from Xlib import X, display

		self.X = X
		self.display = display.Display()
		self.root = self.display.screen().root
		self.window = self.root.create_window(-10, -10, 1, 1, 0, X.CopyFromParent)
		self.atom = lambda name: self.display.intern_atom(name)
		self.uri = ("file://" + urllib.parse.quote(path) + "\r\n").encode()

	def target(self, pid):
		"""The app's top-level window that accepts drops (has XdndAware)."""
		found = subprocess.run(["xdotool", "search", "--pid", str(pid)], capture_output=True, text=True).stdout.split()
		for wid in found:
			window = self.display.create_resource_object("window", int(wid))
			if window.get_full_property(self.atom("XdndAware"), self.X.AnyPropertyType):
				return window
		return None

	def send(self, target, kind, data):
		from Xlib.protocol import event

		message = event.ClientMessage(window=target, client_type=self.atom(kind), data=(32, data))
		target.send_event(message, event_mask=0)
		self.display.flush()

	def wait(self, kind, target, seconds=15):
		"""Answer the target's selection requests until a client message of this kind comes."""
		from Xlib.protocol import event

		deadline = time.monotonic() + seconds
		while time.monotonic() < deadline:
			while self.display.pending_events():
				received = self.display.next_event()
				if received.type == self.X.SelectionRequest:
					requestor = received.requestor
					requestor.change_property(received.property, received.target, 8, self.uri)
					reply = event.SelectionNotify(time=received.time, requestor=requestor, selection=received.selection, target=received.target, property=received.property)
					requestor.send_event(reply, event_mask=0)
					self.display.flush()
				elif received.type == self.X.ClientMessage and received.client_type == self.atom(kind):
					return received
			time.sleep(0.05)
		return None

	def drop(self, target):
		geometry = target.get_geometry()
		origin = target.translate_coords(self.root, 0, 0)
		x, y = -origin.x + geometry.width // 2, -origin.y + geometry.height // 2
		subprocess.run(["xdotool", "mousemove", str(x), str(y)], check=True)
		self.window.set_selection_owner(self.atom("XdndSelection"), self.X.CurrentTime)
		self.display.flush()
		uri_list = self.atom("text/uri-list")
		copy = self.atom("XdndActionCopy")
		self.send(target, "XdndEnter", [self.window.id, 5 << 24, uri_list, 0, 0])
		self.send(target, "XdndPosition", [self.window.id, 0, (x << 16) | y, self.X.CurrentTime, copy])
		if self.wait("XdndStatus", target) is None:
			return "the window never answered XdndPosition"
		# Once the window has the data (it asks for it on the first position), a second
		# position is answered after it has seen the paths.
		self.send(target, "XdndPosition", [self.window.id, 0, (x << 16) | y, self.X.CurrentTime, copy])
		self.wait("XdndStatus", target, 5)
		self.send(target, "XdndDrop", [self.window.id, 0, self.X.CurrentTime, 0, 0])
		if self.wait("XdndFinished", target) is None:
			return "the window never sent XdndFinished"
		return None


def run(args):
	reader = audit.AtspiReader()
	failures = []
	try:
		with tempfile.TemporaryDirectory(prefix="skidbladnir-gpui-drop.") as scratch:
			out = os.path.join(scratch, "out")
			os.makedirs(out)
			config = os.path.join(scratch, "config", "com.basicautomation.skidbladnir")
			os.makedirs(config)
			with open(os.path.join(config, "preferences.json"), "w") as preferences:
				json.dump({"outputDirectory": out}, preferences)
			source_path = os.path.join(scratch, "dropped image.png")
			audit.write_png(source_path)
			env = dict(os.environ, XDG_CONFIG_HOME=os.path.join(scratch, "config"), XDG_DATA_HOME=os.path.join(scratch, "data"), SKID_FORMAT="webp")
			env.pop("WAYLAND_DISPLAY", None)
			libheif = os.path.join(ROOT, "build", "libheif-gpl" if args.edition == "gpl" else "libheif", "lib")
			if os.path.exists(os.path.join(libheif, "libheif.so.1")) and not os.path.exists(os.path.join(os.path.dirname(os.path.abspath(args.app)), "libheif.so.1")):
				env["LD_LIBRARY_PATH"] = libheif + (":" + env["LD_LIBRARY_PATH"] if env.get("LD_LIBRARY_PATH") else "")
			with open(os.path.join(scratch, "app.log"), "wb") as log:
				process = subprocess.Popen([os.path.abspath(args.app)], env=env, stdout=log, stderr=subprocess.STDOUT)
				try:
					app = reader.find(process.pid)
					if app is None:
						return report(["the window's accessibility tree never appeared"])
					convert = next((node for node in reader.walk(app) if node.get_role() == reader.atspi.Role.PUSH_BUTTON and node.get_name() == "Convert"), None)
					if convert is None:
						return report(["no Convert button in the tree"])
					# Named for what it will convert, as it is drawn: "Convert" with nothing queued.
					if convert.get_name() != "Convert":
						return report([f"with nothing queued Convert is named {convert.get_name()!r}"])
					source = DragSource(source_path)
					target = source.target(process.pid)
					if target is None:
						return report(["no window of the app accepts drops (XdndAware)"])
					problem = source.drop(target)
					if problem:
						return report([problem])
					print("ok   the window took the drop (XdndFinished)")
					action = convert.get_action_iface()
					if action is None or not any(action.get_action_name(index) == "click" for index in range(action.get_n_actions())):
						return report(["Convert has no click action"])
					# The drop queues the file, and Convert's name says so. (Not its enabled state:
					# AccessKit's AT-SPI mapping reports every button enabled.)
					for _ in range(100):
						if convert.get_name() == "Convert 1 file":
							break
						time.sleep(0.1)
					else:
						return report([f"after the drop Convert is named {convert.get_name()!r}: the file was not queued"])
					print("ok   the dropped file is queued (Convert is named 'Convert 1 file')")
					action.do_action(next(index for index in range(action.get_n_actions()) if action.get_action_name(index) == "click"))
					output = os.path.join(out, "dropped image.webp")
					for _ in range(150):
						if os.path.exists(output) and os.path.getsize(output) > 0:
							break
						time.sleep(0.1)
					if not os.path.exists(output):
						failures.append(f"Convert wrote nothing to {out}: {sorted(os.listdir(out))}")
						log.flush()
						with open(os.path.join(scratch, "app.log"), encoding="utf-8", errors="replace") as written:
							print(written.read()[-2000:])
					else:
						with open(output, "rb") as written:
							head = written.read(16)
						if head[:4] != b"RIFF" or head[8:12] != b"WEBP":
							failures.append("the converted file is not a WebP")
						else:
							print(f"ok   the dropped PNG converts to WebP ({os.path.getsize(output)} bytes)")
				finally:
					process.terminate()
					try:
						process.wait(timeout=10)
					except subprocess.TimeoutExpired:
						process.kill()
	finally:
		reader.close()
	return report(failures)


def report(failures):
	for failure in failures:
		print(f"FAIL {failure}")
	print("all checks passed" if not failures else f"{len(failures)} check(s) failed")
	return 1 if failures else 0


def main():
	parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
	parser.add_argument("--app", default=os.path.join(ROOT, "target", "release", "skidbladnir"))
	parser.add_argument("--edition", choices=["standard", "gpl"], default="standard")
	args = parser.parse_args()
	if not os.access(args.app, os.X_OK):
		sys.exit(f"no app at {args.app}: build it first, or pass --app")
	if not os.environ.get("DISPLAY"):
		sys.exit("needs an X display (DISPLAY), or XWayland")
	if "SKIDBLADNIR_A11Y_AUDIT_SESSION" not in os.environ:
		return audit.in_private_session(sys.argv[1:], __file__)
	return run(args)


if __name__ == "__main__":
	sys.exit(main())
