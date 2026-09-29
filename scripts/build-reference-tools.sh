#!/usr/bin/env bash
# Build the four reference encoders the parity tests compare Skidbladnir against, each from
# the same source as the library Skidbladnir links, so a comparison is between the options
# and not between versions:
#
#   cwebp     third_party/libwebp (the libwebp libwebp-sys vendors)
#   avifenc   third_party/libavif and third_party/aom, no libyuv (scripts/build-reference-avifenc.sh)
#   cjxl      the libjxl the jpegxl-src crate carries
#   heif-enc  third_party/libheif with Kvazaar (scripts/build-libheif.sh --with-heif-enc)
#
# All four read JPEG through one libjpeg-turbo, built from third_party/libjpeg-turbo — the
# version the app links — so JPEG parity compares encoders, not decoder versions. PNG goes
# through the system libpng, whose rules the app follows (libpng 1.6), so its development
# files are needed, with zlib's; avifenc's configure step fetches libargparse from GitHub.
#
#   scripts/build-reference-tools.sh [--prefix <dir>]
#
# Installs into build/reference (or --prefix) and prints the environment the parity tests
# read, one `NAME=value` line each, for `>> "$GITHUB_ENV"` or `export`.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
prefix="$root/build/reference"
while [ $# -gt 0 ]; do
	case "$1" in
		--prefix) prefix="$2"; shift 2;;
		*) echo "unknown argument: $1" >&2; exit 2;;
	esac
done
for module in libjpeg-turbo libwebp libavif aom libheif kvazaar libde265; do
	[ -f "$root/third_party/$module/CMakeLists.txt" ] || { echo "third_party/$module is missing: run git submodule update --init third_party/$module" >&2; exit 1; }
done
mkdir -p "$prefix/bin"
prefix=$(cd "$prefix" && pwd)
work="$prefix/.work"
jobs=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)
common=(-DCMAKE_BUILD_TYPE=Release -DCMAKE_POSITION_INDEPENDENT_CODE=ON -DCMAKE_INSTALL_LIBDIR=lib)

echo "==> libjpeg-turbo" >&2
jpeg="$work/jpeg-install"
cmake -S "$root/third_party/libjpeg-turbo" -B "$work/jpeg" "${common[@]}" -DENABLE_SHARED=OFF -DENABLE_STATIC=ON -DWITH_JPEG8=ON -DWITH_TURBOJPEG=OFF -DWITH_TOOLS=OFF -DWITH_TESTS=OFF "-DCMAKE_INSTALL_PREFIX=$jpeg" >/dev/null
cmake --build "$work/jpeg" --parallel "$jobs" >/dev/null
cmake --install "$work/jpeg" >/dev/null

echo "==> cwebp" >&2
cmake -S "$root/third_party/libwebp" -B "$work/libwebp" "${common[@]}" -DBUILD_SHARED_LIBS=OFF -DWEBP_BUILD_CWEBP=ON -DWEBP_BUILD_DWEBP=OFF -DWEBP_BUILD_ANIM_UTILS=OFF -DWEBP_BUILD_GIF2WEBP=OFF -DWEBP_BUILD_IMG2WEBP=OFF -DWEBP_BUILD_VWEBP=OFF -DWEBP_BUILD_WEBPINFO=OFF -DWEBP_BUILD_WEBPMUX=OFF -DWEBP_BUILD_EXTRAS=OFF "-DCMAKE_PREFIX_PATH=$jpeg" >/dev/null
cmake --build "$work/libwebp" --parallel "$jobs" --target cwebp >/dev/null
cp "$work/libwebp/cwebp" "$prefix/bin/cwebp"

echo "==> avifenc" >&2
"$root/scripts/build-reference-avifenc.sh" --prefix "$work/avif" --jpeg "$jpeg" >/dev/null
cp "$work/avif/avifenc" "$prefix/bin/avifenc"

echo "==> heif-enc" >&2
"$root/scripts/build-libheif.sh" --prefix "$prefix/heif" --with-heif-enc --jpeg "$jpeg" >/dev/null

echo "==> cjxl" >&2
libjxl=$(cd "$root" && cargo metadata --format-version 1 | python3 -c 'import json,sys; print([p for p in json.load(sys.stdin)["packages"] if p["name"] == "jpegxl-src"][0]["manifest_path"].rsplit("/", 1)[0] + "/libjxl")')
cmake -S "$libjxl" -B "$work/libjxl" "${common[@]}" -DBUILD_SHARED_LIBS=OFF -DBUILD_TESTING=OFF -DJPEGXL_ENABLE_TOOLS=ON -DJPEGXL_ENABLE_DOXYGEN=OFF -DJPEGXL_ENABLE_MANPAGES=OFF -DJPEGXL_ENABLE_BENCHMARK=OFF -DJPEGXL_ENABLE_EXAMPLES=OFF -DJPEGXL_ENABLE_JNI=OFF -DJPEGXL_ENABLE_SJPEG=OFF -DJPEGXL_ENABLE_OPENEXR=OFF -DJPEGXL_BUNDLE_LIBPNG=OFF -DJPEGXL_ENABLE_PLUGINS=OFF "-DCMAKE_PREFIX_PATH=$jpeg" >/dev/null
cmake --build "$work/libjxl" --parallel "$jobs" --target cjxl >/dev/null
cp "$work/libjxl/tools/cjxl" "$prefix/bin/cjxl"

echo "SKIDBLADNIR_REFERENCE_CWEBP=$prefix/bin/cwebp"
echo "SKIDBLADNIR_REFERENCE_AVIFENC=$prefix/bin/avifenc"
echo "SKIDBLADNIR_REFERENCE_CJXL=$prefix/bin/cjxl"
echo "SKIDBLADNIR_REFERENCE_HEIF_ENC=$prefix/heif/bin/heif-enc"
