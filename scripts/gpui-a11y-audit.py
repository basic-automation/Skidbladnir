#!/usr/bin/env python3
"""Audit the gpui window's accessibility tree the way a screen reader reads it.

WHY THIS EXISTS
---------------
scripts/a11y-audit.sh runs axe-core against the webview window, but the window most people
get since 1.3.0 is the gpui one, which no audit had read. gpui hands its tree to AccessKit,
which gives it to the platform's accessibility API: AT-SPI on Linux (what Orca reads), UI
Automation on Windows (what Narrator and NVDA read). This script reads it from there.

On Linux AccessKit publishes the tree only once an assistive technology switches
accessibility on. The script does that inside a private session: its own D-Bus session
bus, its own AT-SPI bus with a screen reader reported on, and the window launched into it
with a scratch configuration, so neither the user's desktop nor their Skidbladnir settings
are touched. On Windows UI Automation asks the window for its tree directly; the app's
configuration folder cannot be redirected there, so it runs only in CI.

For every state below (each format with every disclosure open, the pop-ups, the preview
and the About view) it fails on:
- a control (button, check box, radio button, slider, combo box, text field, list option,
  menu item, link) with no accessible name: a screen reader would say only its role;
- two buttons with the same name, which a screen reader cannot tell apart;
- a slider whose value is missing or outside its own range;
- a select that does not say what it holds (AT-SPI: no selected option);
- an image with no name;
- a text or number field the state must have that is not in the tree, or one whose text
  (its value) a screen reader cannot read;
- a text field that, asked to take the focus, is not then reported focused;
- a state in which the window's tree has no controls at all (the audit read nothing).

USAGE
    scripts/gpui-a11y-audit.py [--app <binary>] [--edition standard|gpl] [--report <json>]

Linux: needs a display, dbus, at-spi2-core (the accessibility bus's configuration and
at-spi2-registryd) and the GObject bindings for libatspi (python3-gi, gir1.2-atspi-2.0);
it re-executes itself under dbus-run-session, once per state. Windows: needs pywinauto.
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
# `debug_state`), the text fields it must have, whether a PNG is queued first, and how many
# images it must show (none if left out). Every
# format's panel with every disclosure open, then the pop-ups, the preview and the About
# view; the preview's two images must be there too. Text and number fields are checked by name because gpuikit's input once reported
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
	("the preview", {"SKID_FORMAT": "webp", "SKID_PREVIEW": "1"}, [], True, 2),
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


def in_private_session(args, script=__file__):
	"""Re-run this script (or another that uses it) with these arguments inside a fresh D-Bus
	session bus of its own."""
	if shutil.which("dbus-run-session") is None or shutil.which("dbus-daemon") is None:
		sys.exit("no dbus-run-session or dbus-daemon: install dbus")
	if first(ACCESSIBILITY_CONF) is None or first(REGISTRIES) is None:
		sys.exit("no AT-SPI bus configuration or registry: install at-spi2-core")
	with tempfile.NamedTemporaryFile("w", suffix=".conf", delete=False) as conf:
		conf.write(SESSION_CONF)
	try:
		env = dict(os.environ, SKIDBLADNIR_A11Y_AUDIT_SESSION="1")
		return subprocess.call(["dbus-run-session", f"--config-file={conf.name}", "--", sys.executable, os.path.abspath(script), *args], env=env)
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


class Node:
	"""One accessible element, as either platform's accessibility API reports it."""

	def __init__(self, role, name, description="", kind="other", value=None, text=None, focus=None, enabled=None):
		self.role = role  # the platform's own name for the role, for the report
		self.name = (name or "").strip()
		self.description = (description or "").strip()
		self.kind = kind  # "control", "field" (a text or number field, also a control), "image" or "other"
		self.value = value  # a slider's (current, minimum, maximum), or None for no value interface
		self.text = text  # a field's text, or None when it exposes none
		self.focus = focus  # for a field: asks the window to focus it; true if it then reports focused
		self.enabled = enabled  # whether the platform reports it enabled, where that is told (UIA)


class AtspiReader:
	"""Linux: AT-SPI, through libatspi, inside the private session `run` sets up."""

	def __init__(self):
		import gi

		gi.require_version("Atspi", "2.0")
		from gi.repository import Atspi

		self.atspi = Atspi
		self.services = enable_accessibility()
		Atspi.init()

	def close(self):
		for process in self.services:
			process.terminate()

	def find(self, pid, timeout=90.0):
		"""The window's application node on the AT-SPI desktop, once it has controls in it."""
		deadline = time.monotonic() + timeout
		while time.monotonic() < deadline:
			desktop = self.atspi.get_desktop(0)
			for index in range(desktop.get_child_count()):
				app = desktop.get_child_at_index(index)
				if app is None:
					continue
				try:
					if app.get_process_id() == pid and any(node.kind in ("control", "field") for node in self.nodes(app)):
						self.activate(pid)
						return app
				except Exception:  # noqa: BLE001 — a half-registered application raises; try again
					pass
			time.sleep(0.5)
		return None

	@staticmethod
	def activate(pid):
		"""On a bare X server (CI's Xvfb) no window manager gives the window the input focus,
		and a field in a window without it cannot hold the keyboard focus: give it with
		xdotool. A Wayland compositor focuses a new window itself."""
		if os.environ.get("WAYLAND_DISPLAY") or not os.environ.get("DISPLAY") or shutil.which("xdotool") is None:
			return
		windows = subprocess.run(["xdotool", "search", "--pid", str(pid)], capture_output=True, text=True).stdout.split()
		for window in windows:
			subprocess.run(["xdotool", "windowfocus", window], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

	def walk(self, node):
		yield node
		try:
			count = node.get_child_count()
		except Exception:  # noqa: BLE001
			return
		for index in range(count):
			child = node.get_child_at_index(index)
			if child is not None:
				yield from self.walk(child)

	def nodes(self, app):
		atspi = self.atspi
		for node in self.walk(app):
			role = node.get_role()
			editable = role == atspi.Role.TEXT and node.get_state_set().contains(atspi.StateType.EDITABLE)
			if role in (atspi.Role.ENTRY, atspi.Role.PASSWORD_TEXT) or editable:
				text = node.get_text_iface()
				yield Node(node.get_role_name(), node.get_name(), node.get_description(), "field", text=None if text is None else atspi.Text.get_text(text, 0, atspi.Text.get_character_count(text)), focus=lambda node=node: self.focus(node))
			elif any(role == getattr(atspi.Role, name, None) for name in CONTROL_ROLES):
				value = node.get_value_iface() if role == atspi.Role.SLIDER else None
				text = None
				if role == atspi.Role.COMBO_BOX:
					# What a screen reader says a select holds: its selected option.
					selection = node.get_selection_iface()
					chosen = [atspi.Selection.get_selected_child(selection, index) for index in range(atspi.Selection.get_n_selected_children(selection))] if selection else []
					text = ", ".join(child.get_name() or "" for child in chosen)
				yield Node(node.get_role_name(), node.get_name(), node.get_description(), "control", value=None if value is None else (value.get_current_value(), value.get_minimum_value(), value.get_maximum_value()), text=text)
			elif role == atspi.Role.IMAGE:
				yield Node(node.get_role_name(), node.get_name(), kind="image")

	def focus(self, node):
		component = node.get_component_iface()
		if component is None or not component.grab_focus():
			return False
		for _ in range(20):
			time.sleep(0.1)
			if node.get_state_set().contains(self.atspi.StateType.FOCUSED):
				return True
		return False


class UiaReader:
	"""Windows: UI Automation, what Narrator and NVDA read, through pywinauto."""

	CONTROLS = {"Button", "CheckBox", "RadioButton", "Slider", "Spinner", "ComboBox", "ListItem", "MenuItem", "Hyperlink", "TabItem", "TreeItem"}

	def __init__(self):
		from pywinauto import Application

		self.application = Application

	def close(self):
		pass

	def find(self, pid, timeout=90.0):
		deadline = time.monotonic() + timeout
		while time.monotonic() < deadline:
			try:
				window = self.application(backend="uia").connect(process=pid, timeout=5).top_window()
				if any(node.kind in ("control", "field") for node in self.nodes(window)):
					return window
			except Exception:  # noqa: BLE001 — not up yet; try again
				pass
			time.sleep(0.5)
		return None

	def walk(self, element):
		"""The window's own elements: not the title bar Windows adds to every top-level
		window's tree (its Minimise, Maximise and Close and its system menu), which a
		screen reader presents as the title bar, apart from the window's own controls."""
		for child in element.children():
			if child.element_info.control_type in ("TitleBar", "MenuBar"):
				continue
			yield child
			yield from self.walk(child)

	def nodes(self, window):
		for element in self.walk(window):
			info = element.element_info
			kind = info.control_type
			if kind == "Edit":
				try:
					text = element.iface_value.CurrentValue
				except Exception:  # noqa: BLE001 — no Value pattern
					text = None
				yield Node(kind, info.name, kind="field", text=text, focus=lambda element=element: self.focus(element))
			elif kind in self.CONTROLS:
				value = None
				if kind == "Slider":
					try:
						pattern = element.iface_range_value
						value = (pattern.CurrentValue, pattern.CurrentMinimum, pattern.CurrentMaximum)
					except Exception:  # noqa: BLE001 — no RangeValue pattern
						value = None
				node = Node(kind, info.name, kind="control", value=value, enabled=element.is_enabled())
				if kind == "ComboBox":
					# Reported, not yet checked: what UI Automation says a select holds, by its
					# Value pattern or its selected item (ROADMAP.md).
					held = None
					try:
						held = element.iface_value.CurrentValue
					except Exception:  # noqa: BLE001 — no Value pattern
						try:
							held = ", ".join(item.window_text() for item in element.get_selection())
						except Exception:  # noqa: BLE001 — no Selection pattern either
							held = None
					node.description = f"holds {held!r}"
				yield node
			elif kind == "Image":
				yield Node(kind, info.name, kind="image")

	@staticmethod
	def focus(element):
		try:
			element.set_focus()
		except Exception:  # noqa: BLE001
			return False
		for _ in range(20):
			time.sleep(0.1)
			if element.has_keyboard_focus():
				return True
		return False


def audit(nodes, fields, queued=False):
	"""Every control, the number of images, and what is wrong."""
	controls, problems, entries = [], [], []
	images = 0
	for node in nodes:
		if node.kind == "image":
			images += 1
			if not node.name:
				problems.append("an image with no name: a screen reader cannot say what it shows")
			continue
		if node.kind not in ("control", "field"):
			continue
		entry = {"role": node.role, "name": node.name}
		if node.role == "ComboBox" and node.description:
			print(f"     select {node.name!r} {node.description} (UI Automation)")
		if not node.name:
			problems.append(f"{node.role} with no name" + (f" (description: {node.description!r})" if node.description else ""))
		if node.role.lower() == "slider":
			if node.value is None:
				problems.append(f"slider {node.name!r} has no value")
			else:
				current, low, high = node.value
				entry["value"] = [current, low, high]
				if not low <= current <= high:
					problems.append(f"slider {node.name!r} is at {current}, outside {low}..{high}")
		if node.role == "combo box" and node.text is not None:
			entry["value"] = node.text
			if not node.text.strip():
				problems.append(f"select {node.name!r} does not say what it holds")
		if node.kind == "field":
			entries.append(node)
			if node.text is None:
				problems.append(f"text field {node.name!r} exposes no text: a screen reader cannot read its value")
			else:
				entry["value"] = node.text
		controls.append(entry)
	names = {node.name for node in entries}
	problems += [f"no text field named {field!r}" for field in fields if field not in names]
	buttons = [control["name"] for control in controls if control["role"].lower() in ("button", "push button") and control["name"]]
	problems += [f"{buttons.count(name)} buttons are all named {name!r}: a screen reader cannot tell them apart" for name in sorted(set(buttons)) if buttons.count(name) > 1]
	# With nothing queued Convert does nothing, so it must say it is disabled, where the
	# platform can tell (UI Automation; AccessKit reports every button enabled on AT-SPI).
	if not queued:
		problems += [f"{node.name!r} is reported enabled with nothing queued" for node in nodes if node.name == "Convert" and node.enabled]
	# Ask the window to focus a field, as a screen reader's user would: its keyboard focus
	# and its accessibility focus must be the same element.
	if entries and not entries[0].focus():
		problems.append(f"text field {entries[0].name!r} was asked to take the focus and is not reported focused")
	return controls, images, problems


def run(args):
	windows = sys.platform == "win32"
	reader = UiaReader() if windows else AtspiReader()
	app_path = os.path.abspath(args.app)
	env_base = dict(os.environ)
	# A bare binary needs libheif beside it, as the installers ship it, or the edition's build.
	libheif = os.path.join(ROOT, "build", "libheif-gpl" if args.edition == "gpl" else "libheif", "lib")
	if not windows and not os.path.exists(os.path.join(os.path.dirname(app_path), "libheif.so.1")) and os.path.exists(os.path.join(libheif, "libheif.so.1")):
		env_base["LD_LIBRARY_PATH"] = libheif + (":" + env_base["LD_LIBRARY_PATH"] if env_base.get("LD_LIBRARY_PATH") else "")
	failures = 0
	report = {}
	try:
		for index, (label, variables, fields, queued, *rest) in enumerate(STATES):
			if args.state is not None and index != args.state:
				continue
			min_images = rest[0] if rest else 0
			with tempfile.TemporaryDirectory(prefix="skidbladnir-gpui-a11y.") as scratch:
				# On Windows the configuration folder cannot be redirected: main() runs this
				# only in CI there.
				env = dict(env_base, XDG_CONFIG_HOME=os.path.join(scratch, "config"), XDG_DATA_HOME=os.path.join(scratch, "data"), **variables)
				if queued:
					write_png(os.path.join(scratch, "input.png"))
					env["SKID_INPUTS"] = os.path.join(scratch, "input.png")
				log = open(os.path.join(scratch, "app.log"), "wb")
				process = subprocess.Popen([app_path], env=env, stdout=log, stderr=subprocess.STDOUT)
				try:
					app = reader.find(process.pid)
					if app is None:
						failures += 1
						alive = process.poll() is None
						print(f"FAIL {label}: no accessibility tree with controls from the window (process {'running' if alive else 'exited'})")
						continue
					# The expanded panel, the pop-up SKID_OPEN opens after 1.5 s, and the preview
					# (encoded after 1.5 s) come later than the window, later still on a
					# software renderer: wait for what the state must have, then a moment more.
					deadline = time.monotonic() + 60
					while time.monotonic() < deadline:
						nodes = list(reader.nodes(app))
						names = {node.name for node in nodes if node.kind == "field"}
						if all(field in names for field in fields) and sum(1 for node in nodes if node.kind == "image") >= min_images:
							break
						time.sleep(1)
					time.sleep(2)
					controls, images, problems = audit(list(reader.nodes(app)), fields, queued)
					if images < min_images:
						problems.append(f"{images} images, not {min_images}")
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
		reader.close()
	if args.report:
		with open(args.report, "w") as out:
			json.dump(report, out, indent=1)
	if args.state is None:
		print(f"{len(report)} of {len(STATES)} states read; {failures} problem(s)")
	return 1 if failures else 0


def each_in_its_own_session(args):
	"""Linux: every state in a private session of its own. In one session, once the first
	window had gone, AT-SPI found none of the next windows' trees on CI's Xvfb (seen on one
	of two runs), so no state can be left to inherit another's registry."""
	report, failed = {}, 0
	with tempfile.TemporaryDirectory(prefix="skidbladnir-gpui-a11y-report.") as scratch:
		for index in range(len(STATES)):
			part = os.path.join(scratch, f"{index}.json")
			command = ["--app", args.app, "--edition", args.edition, "--state", str(index), "--report", part]
			status = in_private_session(command)
			read = False
			if os.path.exists(part):
				with open(part) as source:
					read = STATES[index][0] in json.load(source)
			if status and not read:
				# The tree never appeared (seen once in CI: one state of ten, on lavapipe).
				# Once more in a new session; a state that was read is never retried.
				print(f"     {STATES[index][0]}: no tree read; once more in a new session")
				status = in_private_session(command)
			failed += 1 if status else 0
			if os.path.exists(part):
				with open(part) as source:
					report.update(json.load(source))
	if args.report:
		with open(args.report, "w") as out:
			json.dump(report, out, indent=1)
	print(f"{len(report)} of {len(STATES)} states read; {failed} with problems")
	return 1 if failed else 0


def main():
	parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
	parser.add_argument("--app", default=os.path.join(ROOT, "target", "release", "skidbladnir.exe" if sys.platform == "win32" else "skidbladnir"))
	parser.add_argument("--edition", choices=["standard", "gpl"], default="standard")
	parser.add_argument("--report", help="write every control's role, name and value as JSON here")
	parser.add_argument("--state", type=int, help=argparse.SUPPRESS)
	args = parser.parse_args()
	if not os.access(args.app, os.X_OK):
		sys.exit(f"no app at {args.app}: build it first, or pass --app")
	if sys.platform == "win32":
		if os.environ.get("CI") != "true":
			sys.exit("on Windows the app's configuration folder cannot be redirected, so this runs only in CI (CI=true)")
		return run(args)
	if "SKIDBLADNIR_A11Y_AUDIT_SESSION" not in os.environ:
		return each_in_its_own_session(args)
	return run(args)


if __name__ == "__main__":
	sys.exit(main())
