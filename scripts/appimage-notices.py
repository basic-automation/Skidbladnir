#!/usr/bin/env python3
"""Check that an AppImage carries the licence of every library it bundles.

    scripts/appimage-notices.py <AppImage>...

The AppImage bundles the shared libraries of the Ubuntu release it is built on (GTK,
WebKitGTK, GLib and the rest, most of them LGPL), so it runs where they are missing.
linuxdeploy, which tauri-bundler uses to build it, copies each package's Debian copyright
file into the image as usr/share/doc/<package>/copyright. This makes that a checked fact
rather than an assumption: every bundled library is looked up with `dpkg -S` on the build
machine, and its package's copyright file must be inside the image. Skidbladnir's own
libheif is left out; THIRD-PARTY-NOTICES.md covers it.

Run on the machine that built the AppImage, after building it (release.yml does), since the
packages are looked up in its dpkg database. Exits non-zero, naming them, if any library has
no package or its package has no copyright file inside.
"""
import os, pathlib, subprocess, sys, tempfile

OURS = ("libheif.so",)


def owner(name: str) -> str | None:
	"""The package that installed a file of this name in a library directory, without its
	architecture."""
	found = subprocess.run(["dpkg", "-S", f"*/{name}"], capture_output=True, text=True)
	for line in found.stdout.splitlines():
		packages, _, path = line.partition(": ")
		if "/lib/" in path and os.path.basename(path) == name:
			return packages.split(",")[0].strip().split(":")[0]
	return None


def check(appimage: pathlib.Path) -> list[str]:
	problems = []
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
			if not (root / "usr/share/doc" / package / "copyright").is_file():
				problems.append(f"{appimage.name}: no usr/share/doc/{package}/copyright for {', '.join(names)}")
		print(f"{appimage.name}: {len(libraries)} bundled libraries from {len(packages)} packages, {len(packages) - sum(1 for p in problems if 'no usr/share/doc' in p)} with their copyright file inside")
	return problems


if len(sys.argv) < 2:
	sys.exit(__doc__)
problems = [problem for path in sys.argv[1:] for problem in check(pathlib.Path(path))]
if problems:
	sys.exit("\n".join(problems))
