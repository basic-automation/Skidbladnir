#!/usr/bin/env python3
"""Audit the gpui window's accessibility tree the way a screen reader reads it: over AT-SPI.

WHY THIS EXISTS
---------------
scripts/a11y-audit.sh runs axe-core against the webview window, but the window most people
get since 1.3.0 is the gpui one, which no audit had read. gpui hands its tree to AccessKit,
which publishes it on the AT-SPI bus only once an assistive technology switches
accessibility on. This script does that inside a private session: its own D-Bus session
bus, its own AT-SPI bus with a screen reader reported on, and the window
launched into it with a scratch configuration, so neither the user's desktop nor their
Skidbladnir settings are touched. Then it reads the tree through libatspi, as Orca does.

For every state below (each format with every disclosure open, the pop-ups, the preview
and the About view) it fails on:
- a control (button, check box, radio button, slider, combo box, text field, list option,
  menu item, link) with no accessible name: a screen reader would say only its role;
- two buttons with the same name, which a screen reader cannot tell apart;
- a slider whose value is missing or outside its own range;
- an image with no name;
- a text or number field the state must have that is not in the tree, or one whose text
  (its value) a screen reader cannot read;
- a text field that, asked over AT-SPI to take the focus, is not then reported focused;
- a state in which the window's tree has no controls at all (the audit read nothing).

USAGE
    scripts/gpui-a11y-audit.py [--app <binary>] [--edition standard|gpl] [--report <json>]

Linux only. Needs a display, dbus, at-spi2-core (the accessibility bus's configuration and
at-spi2-registryd) and the GObject bindings for libatspi (python3-gi, gir1.2-atspi-2.0). It
re-executes itself under dbus-run-session.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Each state: a label, the window's debug variables (crates/skidbladnir-gpui/src/app.rs,
# `debug_state`), the text fields it must have, and whether a PNG is queued first. Every
# format's panel with every disclosure open, then the pop-ups, the preview and the About
# view. Text and number fields are checked by name because gpuikit's input once reported
# none of them (see `Announced` in crates/skidbladnir-gpui/src/controls.rs), and nothing
# else would notice them missing.
STATES = [
	("webp", {"SKID_FORMAT": "webp", "SKID_EXPAND": "webp-lossy,webp-animation"}, ["Width", "Height", "Quality, exact value"], False),
	("avif", {"SKID_FORMAT": "avif", "SKID_EXPAND": "avif-layout,avif-quantizers,avif-transform,avif-libaom"}, ["Width", "Height"], False),
	("jxl", {"SKID_FORMAT": "jxl", "SKID_EXPAND": "jxl-filters,jxl-modular-section,jxl-codestream"}, ["Width", "Height", "Quality, exact value", "Photon noise"], False),
	("heic", {"SKID_FORMAT": "heic", "SKID_EXPAND": "heic-geometry,heic-x265-parameters"}, ["Width", "Height", "Add a compatible brand"], False),
	("preset form", {"SKID_FORMAT": "webp", "SKID_OPEN": "preset-form"}, ["Preset name"], False),
	("settings", {"SKID_FORMAT": "webp", "SKID_OPEN": "settings"}, [], False),
	("an open select", {"SKID_FORMAT": "webp", "SKID_OPEN": "select-webp-preset"}, [], False),
	("a file queued, its menu open", {"SKID_FORMAT": "webp", "SKID_OPEN": "queue"}, [], True),
	("the preview", {"SKID_FORMAT": "webp", "SKID_PREVIEW": "1"}, [], True),
	("about", {"SKID_FORMAT": "webp", "SKID_OPEN": "about:license"}, [], False),
]

# A 16x12 RGBA PNG, for the states with a file in the queue.
def write_png(path):
	import struct
	import zlib

	width, height = 16, 12
	raw = b"".join(b"\0" + bytes(v for x in range(width) for v in (x * 16, y * 20, 128, 255)) for y in range(height))

	def chunk(kind, data):
		return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

	with open(path, "wb") as out:
		out.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


# Atspi.Role names: by enum, not by name, since libatspi reports a push button as "button".
CONTROL_ROLES = ("PUSH_BUTTON", "TOGGLE_BUTTON", "CHECK_BOX", "RADIO_BUTTON", "SLIDER", "SPIN_BUTTON", "COMBO_BOX", "ENTRY", "PASSWORD_TEXT", "LIST_ITEM", "MENU_ITEM", "CHECK_MENU_ITEM", "RADIO_MENU_ITEM", "LINK", "SWITCH")

ACCESSIBILITY_CONF = ["/usr/share/defaults/at-spi2/accessibility.conf", "/etc/at-spi2/accessibility.conf"]
REGISTRIES = ["/usr/lib/at-spi2-registryd", "/usr/libexec/at-spi2-registryd", "/usr/lib/at-spi2-core/at-spi2-registryd"]

# A session bus with nothing to activate: whatever the app asks for (a keyring, systemd)
# fails at once instead of timing out.
SESSION_CONF = """<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=/tmp</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"""

# What at-spi-bus-launcher serves on the session bus, and all AccessKit reads from it: the
# accessibility bus's address, and whether a screen reader is on.
A11Y_BUS_XML = """<node>
  <interface name="org.a11y.Bus"><method name="GetAddress"><arg type="s" direction="out"/></method></interface>
  <interface name="org.a11y.Status">
    <property name="IsEnabled" type="b" access="readwrite"/>
    <property name="ScreenReaderEnabled" type="b" access="readwrite"/>
  </interface>
</node>"""


def first(paths):
	return next((path for path in paths if os.path.exists(path)), None)


def in_private_session(args):
	"""Re-run this script inside a fresh D-Bus session bus of its own."""
	if shutil.which("dbus-run-session") is None or shutil.which("dbus-daemon") is None:
		sys.exit("no dbus-run-session or dbus-daemon: install dbus")
	if first(ACCESSIBILITY_CONF) is None or first(REGISTRIES) is None:
		sys.exit("no AT-SPI bus configuration or registry: install at-spi2-core")
	with tempfile.NamedTemporaryFile("w", suffix=".conf", delete=False) as conf:
		conf.write(SESSION_CONF)
	try:
		env = dict(os.environ, SKIDBLADNIR_A11Y_AUDIT_SESSION="1")
		return subprocess.call(["dbus-run-session", f"--config-file={conf.name}", "--", sys.executable, os.path.abspath(__file__), *args], env=env)
	finally:
		os.unlink(conf.name)


def enable_accessibility():
	"""Start an accessibility bus and its registry, and say on the session bus that a screen
	reader is on, as at-spi-bus-launcher would. Returns the processes to stop afterwards.

	Not at-spi-bus-launcher itself: where it is built for dbus-broker (Arch), it needs systemd,
	which a private session does not have."""
	import threading
	import warnings

	from gi.repository import Gio, GLib

	# PyGObject deprecates register_object for a closure variant older distributions lack.
	warnings.filterwarnings("ignore", category=DeprecationWarning)

	bus = subprocess.Popen(["dbus-daemon", f"--config-file={first(ACCESSIBILITY_CONF)}", "--nofork", "--print-address=1"], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
	address = bus.stdout.readline().strip()
	if not address:
		sys.exit("the accessibility bus did not start")
	os.environ["AT_SPI_BUS_ADDRESS"] = address
	ready = threading.Event()

	def serve():
		context = GLib.MainContext.new()
		context.push_thread_default()
		session = Gio.DBusConnection.new_for_address_sync(Gio.dbus_address_get_for_bus_sync(Gio.BusType.SESSION, None), Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
		info = Gio.DBusNodeInfo.new_for_xml(A11Y_BUS_XML)

		def call(_connection, _sender, _path, _interface, method, _parameters, invocation):
			if method == "GetAddress":
				invocation.return_value(GLib.Variant("(s)", (address,)))

		session.register_object("/org/a11y/bus", info.interfaces[0], call, None, None)
		session.register_object("/org/a11y/bus", info.interfaces[1], None, lambda *_: GLib.Variant("b", True), lambda *_: True)
		session.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName", GLib.Variant("(su)", ("org.a11y.Bus", 4)), None, Gio.DBusCallFlags.NONE, -1, None)
		ready.set()
		GLib.MainLoop.new(context, False).run()

	threading.Thread(target=serve, daemon=True).start()
	if not ready.wait(10):
		sys.exit("could not serve org.a11y.Bus")
	registry = subprocess.Popen([first(REGISTRIES)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
	time.sleep(1)
	return [registry, bus]


def find_app(atspi, pid, timeout=30.0):
	"""The window's application node on the AT-SPI desktop, once it has controls in it."""
	deadline = time.monotonic() + timeout
	while time.monotonic() < deadline:
		desktop = atspi.get_desktop(0)
		for index in range(desktop.get_child_count()):
			app = desktop.get_child_at_index(index)
			if app is None:
				continue
			try:
				if app.get_process_id() == pid and count_controls(atspi, app) > 0:
					return app
			except Exception:  # noqa: BLE001 — a half-registered application raises; try again
				pass
		time.sleep(0.5)
	return None


def walk(node, depth=0):
	yield node, depth
	try:
		count = node.get_child_count()
	except Exception:  # noqa: BLE001
		return
	for index in range(count):
		child = node.get_child_at_index(index)
		if child is not None:
			yield from walk(child, depth + 1)


def is_control(atspi, node):
	"""A node a user operates: one of CONTROL_ROLES, or editable text."""
	role = node.get_role()
	if any(role == getattr(atspi.Role, name, None) for name in CONTROL_ROLES):
		return True
	return role == atspi.Role.TEXT and node.get_state_set().contains(atspi.StateType.EDITABLE)


def count_controls(atspi, app):
	return sum(1 for node, _ in walk(app) if is_control(atspi, node))


def audit(atspi, app, fields):
	"""Every control in the tree, and what is wrong with each."""
	controls, problems = [], []
	entries = []
	images = 0
	for node, _ in walk(app):
		if node.get_role() == atspi.Role.IMAGE:
			images += 1
			if not (node.get_name() or "").strip():
				problems.append("an image with no name: a screen reader cannot say what it shows")
		if not is_control(atspi, node):
			continue
		role = node.get_role_name()
		name = (node.get_name() or "").strip()
		entry = {"role": role, "name": name}
		if not name:
			description = (node.get_description() or "").strip()
			problems.append(f"{role} with no name" + (f" (description: {description!r})" if description else ""))
		if role == "slider":
			value = node.get_value_iface() if hasattr(node, "get_value_iface") else None
			if value is None:
				problems.append(f"slider {name!r} has no value")
			else:
				current, low, high = value.get_current_value(), value.get_minimum_value(), value.get_maximum_value()
				entry["value"] = [current, low, high]
				if not low <= current <= high:
					problems.append(f"slider {name!r} is at {current}, outside {low}..{high}")
		if is_text_field(atspi, node):
			entries.append((name, node))
			text = node.get_text_iface()
			if text is None:
				problems.append(f"text field {name!r} exposes no text: a screen reader cannot read its value")
			else:
				entry["value"] = atspi.Text.get_text(text, 0, atspi.Text.get_character_count(text))
		controls.append(entry)
	names = {name for name, _ in entries}
	problems += [f"no text field named {field!r}" for field in fields if field not in names]
	buttons = [control["name"] for control in controls if control["role"] == "button" and control["name"]]
	problems += [f"{buttons.count(name)} buttons are all named {name!r}: a screen reader cannot tell them apart" for name in sorted(set(buttons)) if buttons.count(name) > 1]
	if entries:
		problems += focus_problems(atspi, *entries[0])
	return controls, images, problems


def is_text_field(atspi, node):
	return node.get_role() in (atspi.Role.ENTRY, atspi.Role.PASSWORD_TEXT) or (node.get_role() == atspi.Role.TEXT and node.get_state_set().contains(atspi.StateType.EDITABLE))


def focus_problems(atspi, name, node):
	"""Ask the window to focus a field, as a screen reader's user would, and check the field
	is then the node reported focused: the window's keyboard focus and its accessibility
	focus must be the same element."""
	component = node.get_component_iface()
	if component is None or not component.grab_focus():
		return [f"text field {name!r} cannot be focused"]
	for _ in range(20):
		time.sleep(0.1)
		if node.get_state_set().contains(atspi.StateType.FOCUSED):
			return []
	return [f"text field {name!r} was asked to take the focus and is not reported focused"]


def run(args):
	import gi

	gi.require_version("Atspi", "2.0")
	from gi.repository import Atspi

	accessibility = enable_accessibility()
	Atspi.init()
	app_path = os.path.abspath(args.app)
	env_base = dict(os.environ)
	# A bare binary needs libheif beside it, as the installers ship it, or the edition's build.
	libheif = os.path.join(ROOT, "build", "libheif-gpl" if args.edition == "gpl" else "libheif", "lib")
	if not os.path.exists(os.path.join(os.path.dirname(app_path), "libheif.so.1")) and os.path.exists(os.path.join(libheif, "libheif.so.1")):
		env_base["LD_LIBRARY_PATH"] = libheif + (":" + env_base["LD_LIBRARY_PATH"] if env_base.get("LD_LIBRARY_PATH") else "")
	failures = 0
	report = {}
	try:
		for label, variables, fields, queued in STATES:
			with tempfile.TemporaryDirectory(prefix="skidbladnir-gpui-a11y.") as scratch:
				env = dict(env_base, XDG_CONFIG_HOME=os.path.join(scratch, "config"), XDG_DATA_HOME=os.path.join(scratch, "data"), **variables)
				if queued:
					write_png(os.path.join(scratch, "input.png"))
					env["SKID_INPUTS"] = os.path.join(scratch, "input.png")
				log = open(os.path.join(scratch, "app.log"), "wb")
				process = subprocess.Popen([app_path], env=env, stdout=log, stderr=subprocess.STDOUT)
				try:
					app = find_app(Atspi, process.pid)
					if app is None:
						failures += 1
						alive = process.poll() is None
						print(f"FAIL {label}: no accessibility tree with controls from the window (process {'running' if alive else 'exited'})")
						continue
					# Let the expanded panel, the pop-up SKID_OPEN opens after 1.5 s, or the
					# preview (encoded after 1.5 s), settle into the tree.
					time.sleep(5 if "SKID_PREVIEW" in variables else 3)
					controls, images, problems = audit(Atspi, app, fields)
					report[label] = controls
					roles = {}
					for control in controls:
						roles[control["role"]] = roles.get(control["role"], 0) + 1
					summary = ", ".join(f"{count} {role}" for role, count in sorted(roles.items()))
					if problems:
						failures += len(problems)
						print(f"FAIL {label}: {len(controls)} controls ({summary}); {len(problems)} problems:")
						for problem in problems:
							print(f"       {problem}")
					else:
						print(f"ok   {label}: {len(controls)} controls, every one named ({summary}){f'; {images} images, named' if images else ''}")
				finally:
					process.terminate()
					try:
						process.wait(timeout=10)
					except subprocess.TimeoutExpired:
						process.kill()
					log.close()
	finally:
		for process in accessibility:
			process.terminate()
	if args.report:
		with open(args.report, "w") as out:
			json.dump(report, out, indent=1)
	print(f"{len(STATES) - sum(1 for state in STATES if state[0] not in report)} of {len(STATES)} states read; {failures} problem(s)")
	return 1 if failures else 0


def main():
	parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
	parser.add_argument("--app", default=os.path.join(ROOT, "target", "release", "skidbladnir"))
	parser.add_argument("--edition", choices=["standard", "gpl"], default="standard")
	parser.add_argument("--report", help="write every control's role, name and value as JSON here")
	args = parser.parse_args()
	if not os.access(args.app, os.X_OK):
		sys.exit(f"no app at {args.app}: build it first, or pass --app")
	if "SKIDBLADNIR_A11Y_AUDIT_SESSION" not in os.environ:
		return in_private_session(sys.argv[1:])
	return run(args)


if __name__ == "__main__":
	sys.exit(main())
