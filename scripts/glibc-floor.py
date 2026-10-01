#!/usr/bin/env python3
"""Fail when a Linux build needs a newer glibc or libstdc++ than the oldest system it supports.

The Linux installers are built on Ubuntu 22.04 so that they start there and on Debian 12.
That holds only while nothing in them needs a symbol version those systems lack: glibc 2.35
(Ubuntu 22.04; Debian 12 has 2.36) and the libstdc++ of GCC 12 (GLIBCXX_3.4.30 and
CXXABI_1.3.13; Ubuntu 22.04 ships 12.3, Debian 12 ships 12.2). A build that needs more still
installs, then refuses to start with "version `GLIBC_2.39' not found". Built on Ubuntu 24.04,
0.14.0 did exactly that.

This reads the version needs (`readelf -V`) of every ELF file in what it is given. Those are
what the dynamic loader checks before the program runs. It prints the newest version each
family needs either way, and fails when one is above the floor. A need flagged WEAK is
optional to the loader, so it is reported but does not fail. A weak SYMBOL is different:
Rust's std binds pidfd_spawnp weakly, yet its GLIBC_2.39 need is not flagged weak, and
the loader refuses the binary on an older glibc all the same.

usage: glibc-floor.py [--glibc 2.35] [--glibcxx 3.4.30] [--cxxabi 1.3.13] PATH...

Each PATH is an ELF file, a directory (searched), a .deb (unpacked with dpkg-deb, or ar
and tar) or an .AppImage (unpacked with unsquashfs, or with its own --appimage-extract).
Exit status: 0 within the floor, 1 above it, 2 when a PATH cannot be read or holds no ELF.
"""

import argparse
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

FAMILIES = ("GLIBC", "GLIBCXX", "CXXABI")
# `  0x0010: Version: 1  File: libc.so.6  Cnt: 22`
NEED_FILE = re.compile(r"^\s*(?:0x)?[0-9a-f]+:\s+Version:\s+\d+\s+File:\s+(\S+)")
# `  0x01b0:   Name: GLIBC_2.2.5  Flags: none  Version: 3`
NEED_NAME = re.compile(r"^\s*0x[0-9a-f]+:\s+Name:\s+(\S+)\s+Flags:\s+(.*?)\s+Version:\s+\d+")
VERSIONED = re.compile(r"^(GLIBC|GLIBCXX|CXXABI)_(\d+(?:\.\d+)*)$")
C_LOCALE = {**os.environ, "LC_ALL": "C"}


def parse(version: str) -> tuple[int, ...]:
	return tuple(int(part) for part in version.split("."))


def show(version: tuple[int, ...]) -> str:
	return ".".join(str(part) for part in version)


def version_needs(path: pathlib.Path) -> list[tuple[str, str, bool]]:
	"""(library, version name, weak) for every version `path` needs."""
	result = subprocess.run(["readelf", "-V", "-W", str(path)], capture_output=True, text=True, env=C_LOCALE, check=False)
	needs = []
	inside = False
	library = "?"
	for line in result.stdout.splitlines():
		if line.startswith("Version needs section"):
			inside = True
			continue
		if not line.startswith(" "):
			inside = False
			continue
		if not inside:
			continue
		if match := NEED_FILE.match(line):
			library = match.group(1)
		elif match := NEED_NAME.match(line):
			needs.append((library, match.group(1), "WEAK" in match.group(2)))
	return needs


def symbols_needing(path: pathlib.Path, name: str, limit: int = 6) -> list[str]:
	"""A few of the symbols `path` binds to version `name`, to say what pulled it in."""
	result = subprocess.run(["objdump", "-T", str(path)], capture_output=True, text=True, env=C_LOCALE, check=False)
	found = []
	for line in result.stdout.splitlines():
		if match := re.search(r"\((\S+)\)\s+(\S+)\s*$", line):
			if match.group(1) == name:
				weak = " (weak symbol)" if re.search(r"^\S+\s+w\s", line) else ""
				found.append(match.group(2) + weak)
	return found[:limit]


def elf_files(root: pathlib.Path):
	if root.is_file():
		candidates = [root]
	else:
		candidates = (pathlib.Path(directory, name) for directory, _, names in os.walk(root) for name in names)
	for path in candidates:
		if path.is_symlink() or not path.is_file():
			continue
		try:
			with path.open("rb") as file:
				if file.read(4) == b"\x7fELF":
					yield path
		except OSError:
			continue


def appimage_offset(path: pathlib.Path) -> int:
	"""Where the squashfs starts: just past the runtime's ELF section header table."""
	with path.open("rb") as file:
		header = file.read(64)
	if header[:4] != b"\x7fELF":
		raise ValueError(f"{path} is not an AppImage (no ELF runtime)")
	order = "little" if header[5] == 1 else "big"
	if header[4] == 2:
		shoff, shentsize, shnum = header[0x28:0x30], header[0x3A:0x3C], header[0x3C:0x3E]
	else:
		shoff, shentsize, shnum = header[0x20:0x24], header[0x2E:0x30], header[0x30:0x32]
	return int.from_bytes(shoff, order) + int.from_bytes(shentsize, order) * int.from_bytes(shnum, order)


def unpack(path: pathlib.Path, scratch: pathlib.Path) -> pathlib.Path:
	"""The directory to search for `path`: itself, or where its package was unpacked."""
	quiet = {"stdout": subprocess.DEVNULL, "check": True}
	if path.name.endswith(".deb"):
		target = scratch / "root"
		if shutil.which("dpkg-deb"):
			subprocess.run(["dpkg-deb", "-x", str(path), str(target)], **quiet)
		else:
			members = scratch / "members"
			members.mkdir()
			subprocess.run(["ar", "x", str(path.resolve())], cwd=members, **quiet)
			target.mkdir()
			subprocess.run(["tar", "-xf", str(next(members.glob("data.tar*"))), "-C", str(target)], **quiet)
		return target
	if path.name.endswith(".AppImage"):
		target = scratch / "squashfs-root"
		if shutil.which("unsquashfs"):
			try:
				subprocess.run(["unsquashfs", "-n", "-o", str(appimage_offset(path)), "-d", str(target), str(path)], **quiet)
				return target
			except subprocess.CalledProcessError:
				# An unsquashfs too old for -o, say: the AppImage can unpack itself.
				shutil.rmtree(target, ignore_errors=True)
		subprocess.run([str(path.resolve()), "--appimage-extract"], cwd=scratch, **quiet)
		return target
	return path


def main() -> int:
	parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
	parser.add_argument("--glibc", default="2.35", help="newest GLIBC_ version allowed (default 2.35, Ubuntu 22.04)")
	parser.add_argument("--glibcxx", default="3.4.30", help="newest GLIBCXX_ version allowed (default 3.4.30, GCC 12)")
	parser.add_argument("--cxxabi", default="1.3.13", help="newest CXXABI_ version allowed (default 1.3.13, GCC 12)")
	parser.add_argument("paths", nargs="+", type=pathlib.Path)
	args = parser.parse_args()
	floor = {"GLIBC": parse(args.glibc), "GLIBCXX": parse(args.glibcxx), "CXXABI": parse(args.cxxabi)}
	annotate = "::error::" if os.environ.get("GITHUB_ACTIONS") == "true" else "error: "

	print("floor: " + ", ".join(f"{family}_{show(floor[family])}" for family in FAMILIES))
	status = 0
	for given in args.paths:
		if not given.exists():
			print(f"{annotate}{given} does not exist")
			status = max(status, 2)
			continue
		with tempfile.TemporaryDirectory(prefix="glibc-floor-") as scratch:
			try:
				root = unpack(given, pathlib.Path(scratch))
			except (subprocess.CalledProcessError, OSError, ValueError, StopIteration) as error:
				print(f"{annotate}cannot unpack {given}: {error}")
				status = max(status, 2)
				continue
			files = sorted(elf_files(root))
			print(f"\n{given}: {len(files)} ELF files")
			if not files:
				print(f"{annotate}{given} holds no ELF file, so nothing was checked")
				status = max(status, 2)
				continue
			newest: dict[str, tuple[tuple[int, ...], str]] = {}
			above: dict[tuple[pathlib.Path, str, str], list[str]] = {}
			weak_above, other = [], set()
			for path in files:
				shown = str(path.relative_to(root)) if path != root else path.name
				for library, name, weak in version_needs(path):
					match = VERSIONED.match(name)
					if not match:
						if name.split("_")[0] in FAMILIES:
							other.add(f"{name} from {library} ({shown})")
						continue
					family, version = match.group(1), parse(match.group(2))
					if weak:
						if version > floor[family]:
							weak_above.append((shown, name))
						continue
					if family not in newest or version > newest[family][0]:
						newest[family] = (version, shown)
					if version > floor[family]:
						above.setdefault((path, shown, name), []).append(library)
			for family in FAMILIES:
				if family in newest:
					version, where = newest[family]
					verdict = "above the floor" if version > floor[family] else "ok"
					print(f"  newest {family + '_':<8} needed: {show(version):<7} {verdict:<15} ({where})")
				else:
					print(f"  newest {family + '_':<8} needed: none")
			for shown, name in weak_above:
				print(f"  note: {shown} needs {name} only weakly; the loader does not require it")
			for needed in sorted(other):
				print(f"  note: needs {needed}")
			for (path, shown, name), libraries in above.items():
				symbols = ", ".join(symbols_needing(path, name)) or "no dynamic symbol names it"
				print(f"{annotate}{shown} needs {name} ({' and '.join(libraries)}), newer than the floor: {symbols}")
				status = max(status, 1)
	if status == 1:
		print("\nSomething above needs a newer system than the oldest one supported. Build it on that system, or raise the floor and say so where the Linux requirements are stated.")
	return status


if __name__ == "__main__":
	sys.exit(main())
