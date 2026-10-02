#!/usr/bin/env python3
"""Write the updater manifest (`latest.json`) for a release from its signed installers.

The app's updater reads this file to learn what the newest version is and where each
platform downloads it. `release.yml` runs this once every platform has built, over the
installers and the `.sig` files `tauri build` wrote beside them, then publishes the result.

Only installers that were SIGNED are listed. A platform whose build failed, or that was
built without the signing key, is left out — its users are simply not offered this update,
which is the right failure: an unsigned entry would be refused by every client anyway.

The keys are the updater plugin's `{os}-{arch}-{bundle}` targets, so a copy installed from
the .deb is updated with a .deb, one from the .rpm with an .rpm and one from the AppImage
with an AppImage, rather than one format replacing another.

Each edition has its own manifest, and `--prefix` picks its installers out of a directory
that holds both: `Skidbladnir_` (the default) for the standard edition's latest.json,
`Skidbladnir-GPL_` for the GPL edition's latest-gpl.json. A GPL install reads only the
latter (tauri.gpl.conf.json), so it can never be updated to the standard edition, or the
other way round.

usage: updater-manifest.py --dir DIR --version X.Y.Z --tag vX.Y.Z --repo OWNER/NAME
                           [--prefix Skidbladnir_] [--notes-file FILE] [--out latest.json]
"""

import argparse
import datetime
import json
import pathlib
import re
import sys
import urllib.parse

# Installer file name -> updater target. macOS's archives are renamed per architecture by
# release.yml: tauri names both `Skidbladnir.app.tar.gz`, and two assets on one release
# cannot share a name.
TARGETS = [
	(re.compile(r"_amd64\.AppImage$"), "linux-x86_64-appimage"),
	(re.compile(r"_amd64\.deb$"), "linux-x86_64-deb"),
	(re.compile(r"_x86_64\.rpm$"), "linux-x86_64-rpm"),
	(re.compile(r"_aarch64\.AppImage$"), "linux-aarch64-appimage"),
	(re.compile(r"_arm64\.deb$"), "linux-aarch64-deb"),
	(re.compile(r"_aarch64\.rpm$"), "linux-aarch64-rpm"),
	(re.compile(r"_x64-setup\.exe$"), "windows-x86_64-nsis"),
	(re.compile(r"_aarch64\.app\.tar\.gz$"), "darwin-aarch64-app"),
	(re.compile(r"_x64\.app\.tar\.gz$"), "darwin-x86_64-app"),
]


def main() -> int:
	parser = argparse.ArgumentParser()
	parser.add_argument("--dir", required=True, type=pathlib.Path)
	parser.add_argument("--version", required=True)
	parser.add_argument("--tag", required=True)
	parser.add_argument("--repo", required=True)
	parser.add_argument("--prefix", default="Skidbladnir_")
	parser.add_argument("--notes-file", type=pathlib.Path)
	parser.add_argument("--out", default="latest.json", type=pathlib.Path)
	args = parser.parse_args()

	platforms = {}
	for installer in sorted(args.dir.iterdir()):
		if not installer.name.startswith(args.prefix):
			continue
		for pattern, target in TARGETS:
			if not pattern.search(installer.name):
				continue
			signature = installer.with_name(installer.name + ".sig")
			if not signature.is_file():
				print(f"skipping {installer.name}: not signed", file=sys.stderr)
				break
			if target in platforms:
				print(f"error: two installers for {target}", file=sys.stderr)
				return 1
			platforms[target] = {
				"signature": signature.read_text().strip(),
				"url": f"https://github.com/{args.repo}/releases/download/{args.tag}/{urllib.parse.quote(installer.name)}",
			}
			break

	if not platforms:
		print("error: no signed installers found, so there is nothing to offer", file=sys.stderr)
		return 1

	manifest = {
		"version": args.version,
		"pub_date": datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
		"platforms": platforms,
	}
	if args.notes_file and args.notes_file.is_file():
		manifest["notes"] = args.notes_file.read_text().strip()

	args.out.write_text(json.dumps(manifest, indent=2) + "\n")
	print(f"{args.out}: {args.version} for {', '.join(sorted(platforms))}")
	return 0


if __name__ == "__main__":
	sys.exit(main())
