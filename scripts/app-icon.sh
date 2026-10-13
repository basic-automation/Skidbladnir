#!/usr/bin/env bash
# Regenerate the app icon (src-tauri/icons/*) from the hero illustration.
#
#   scripts/app-icon.sh
#
# resources/icons/hero.svg is the 1224×792 hero scene. The icon is a square crop of it
# around the ship and its sail — the 600×600 region at (335, 40) — with the sunset sky
# and sea kept, and corners rounded; `cargo tauri icon` then writes every size from it.
# Only the files the repo tracks are replaced: it also writes Android and iOS sets, which
# this desktop app does not ship. Needs rsvg-convert (librsvg) and ImageMagick 7.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# 1224 × 1024/600: the crop comes out at 1024px.
rsvg-convert -w 2089 "$root/resources/icons/hero.svg" -o "$work/hero.png"
magick "$work/hero.png" -crop 1024x1024+572+68 +repage \
	\( -size 1024x1024 xc:none -fill white -draw "roundrectangle 0,0 1023,1023 180,180" \) \
	-alpha set -compose DstIn -composite "$root/resources/icons/icon.png"

cargo tauri icon "$root/resources/icons/icon.png" -o "$work/icons" >/dev/null 2>&1
for tracked in $(git -C "$root" ls-files src-tauri/icons); do
	cp "$work/icons/$(basename "$tracked")" "$root/$tracked"
done
echo "wrote resources/icons/icon.png and src-tauri/icons/*"
