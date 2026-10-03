#!/usr/bin/env python3
"""Check that an AppImage carries the licence of every library it bundles.

    scripts/appimage-notices.py [--missing-config <file>] <AppImage>...

The AppImage bundles the shared libraries of the Ubuntu release it is built on (GTK,
WebKitGTK, GLib and the rest, most of them LGPL), so it runs where they are missing.
linuxdeploy, which tauri-bundler uses to build it, copies most packages' Debian copyright
files into the image as usr/share/doc/<package>/copyright, but not all: the libraries its
GTK plugin adds (GIO modules, the SVG pixbuf loader, the GSettings backend) and some of
their dependencies arrive without theirs (found by the 1.4.0 release dry run: 13 packages,
from libbz2 to libselinux). This makes the licences a checked fact: every bundled library
is looked up with `dpkg -S` on the build machine — through its symlinks, so an unversioned
name counts as the runtime package that holds the library, not the -dev package that holds
the link — and its package's copyright file must be inside the image. Skidbladnir's own
libheif is left out; THIRD-PARTY-NOTICES.md covers it.

With --missing-config, the copyright files the image lacks are written as a Tauri
configuration fragment (`bundle.linux.appimage.files`), for bundling the AppImage again
with them (release.yml does), and the check itself is not failed.

Run on the machine that built the AppImage, after building it, since the packages are
looked up in its dpkg database. Exits non-zero, naming them, if any library has no package,
or (without --missing-config) its package has no copyright file inside.
"""
import json, os, pathlib, subprocess, sys, tempfile

OURS = ("libheif.so",)


def package_of(path: str) -> str | None:
	found = subprocess.run(["dpkg", "-S", path], capture_output=True, text=True)
	for line in found.stdout.splitlines():
		packages, _, owned = line.partition(": ")
		if owned == path:
			return packages.split(",")[0].strip().split(":")[0]
	return None


def owner(name: str) -> str | None:
	"""The package that holds the library a bundled file of this name is, without its
	architecture: found by name in a library directory, then followed through symlinks."""
	found = subprocess.run(["dpkg", "-S", f"*/{name}"], capture_output=True, text=True)
	for line in found.stdout.splitlines():
		packages, _, path = line.partition(": ")
		if "/lib/" not in path or os.path.basename(path) != name:
			continue
		real = os.path.realpath(path)
		return (package_of(real) if real != path else None) or packages.split(",")[0].strip().split(":")[0]
	return None


def check(appimage: pathlib.Path) -> tuple[list[str], dict[str, str]]:
	"""The problems with one AppImage, and the copyright files it lacks as
	`{path in the image: path on this machine}`."""
	problems, missing = [], {}
	with tempfile.TemporaryDirectory() as work:
		subprocess.run([str(appimage.resolve()), "--appimage-extract"], cwd=work, check=True, capture_output=True)
		root = pathlib.Path(work, "squashfs-root")
		libraries = sorted({path.name for path in (root / "usr/lib").rglob("*.so*") if path.is_file() and not path.name.startswith(OURS)})
		packages: dict[str, list[str]] = {}
		for name in libraries:
			package = owner(name)
			if package is None:
				problems.append(f"{appimage.name}: {name} is from no installed package")
				continue
			packages.setdefault(package, []).append(name)
		for package, names in sorted(packages.items()):
			inside = f"/usr/share/doc/{package}/copyright"
			if (root / inside.lstrip("/")).is_file():
				continue
			if os.path.isfile(inside):
				missing[inside] = os.path.realpath(inside)
			else:
				problems.append(f"{appimage.name}: {package} has no copyright file on this machine either, for {', '.join(names)}")
		print(f"{appimage.name}: {len(libraries)} bundled libraries from {len(packages)} packages, {len(packages) - len(missing)} with their copyright file inside")
		for inside in sorted(missing):
			print(f"{appimage.name}: lacks {inside}")
	return problems, missing


arguments = sys.argv[1:]
config = None
if arguments[:1] == ["--missing-config"] and len(arguments) >= 2:
	config, arguments = pathlib.Path(arguments[1]), arguments[2:]
if not arguments:
	sys.exit(__doc__)
problems, missing = [], {}
for path in arguments:
	found, lacking = check(pathlib.Path(path))
	problems += found
	missing.update(lacking)
if config is not None:
	config.write_text(json.dumps({"bundle": {"linux": {"appimage": {"files": missing}}}}, indent=2) if missing else "", encoding="utf-8")
elif missing:
	problems += [f"the AppImage lacks {inside}" for inside in sorted(missing)]
if problems:
	sys.exit("\n".join(problems))
