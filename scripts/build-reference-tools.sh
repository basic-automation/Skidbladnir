#!/usr/bin/env bash
# Build the reference encoders the parity tests compare Skidbladnir against, each from the
# same source as the library Skidbladnir links, so a comparison is between the options and
# not between versions:
#
#   cwebp     third_party/libwebp (the libwebp libwebp-sys vendors), with img2webp and
#             gif2webp beside it, the references for animated WebP and GIF input, and
#             webpmux, the reference for the metadata chunks of an animated WebP
#   avifenc   third_party/libavif and third_party/aom, no libyuv (scripts/build-reference-avifenc.sh)
#   cjxl      the libjxl the jpegxl-src crate carries
#   heif-enc  third_party/libheif with Kvazaar (scripts/build-libheif.sh --with-heif-enc)
#
# All four read JPEG through one libjpeg-turbo, built from third_party/libjpeg-turbo — the
# version the app links — so JPEG parity compares encoders, not decoder versions. PNG goes
# through the system libpng, whose rules the app follows (libpng 1.6), and cwebp reads TIFF
# through the system libtiff, so their development files are needed, with zlib's and
# giflib's (for gif2webp), and OpenEXR's for cjxl, which reads EXR through it; avifenc's
# configure step fetches libargparse from GitHub.
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

echo "==> cwebp, img2webp, gif2webp, webpmux" >&2
# WEBP_LINK_STATIC off: libwebp's CMake leaves TIFF out of a statically linked cwebp.
cmake -S "$root/third_party/libwebp" -B "$work/libwebp" "${common[@]}" -DBUILD_SHARED_LIBS=OFF -DWEBP_LINK_STATIC=OFF -DWEBP_BUILD_CWEBP=ON -DWEBP_BUILD_DWEBP=OFF -DWEBP_BUILD_ANIM_UTILS=OFF -DWEBP_BUILD_GIF2WEBP=ON -DWEBP_BUILD_IMG2WEBP=ON -DWEBP_BUILD_VWEBP=OFF -DWEBP_BUILD_WEBPINFO=OFF -DWEBP_BUILD_WEBPMUX=ON -DWEBP_BUILD_EXTRAS=OFF "-DCMAKE_PREFIX_PATH=$jpeg" >/dev/null
cmake --build "$work/libwebp" --parallel "$jobs" --target cwebp img2webp gif2webp webpmux >/dev/null
for tool in cwebp img2webp gif2webp webpmux; do
	[ -x "$work/libwebp/$tool" ] || { echo "$tool was not built (gif2webp needs giflib's development files)" >&2; exit 1; }
	cp "$work/libwebp/$tool" "$prefix/bin/$tool"
done

echo "==> avifenc" >&2
"$root/scripts/build-reference-avifenc.sh" --prefix "$work/avif" --jpeg "$jpeg" >/dev/null
cp "$work/avif/avifenc" "$prefix/bin/avifenc"

echo "==> heif-enc" >&2
"$root/scripts/build-libheif.sh" --prefix "$prefix/heif" --with-heif-enc --jpeg "$jpeg" >/dev/null

echo "==> cjxl" >&2
libjxl=$(cd "$root" && cargo metadata --format-version 1 | python3 -c 'import json,sys; print([p for p in json.load(sys.stdin)["packages"] if p["name"] == "jpegxl-src"][0]["manifest_path"].rsplit("/", 1)[0] + "/libjxl")')
cmake -S "$libjxl" -B "$work/libjxl" "${common[@]}" -DBUILD_SHARED_LIBS=OFF -DBUILD_TESTING=OFF -DJPEGXL_ENABLE_TOOLS=ON -DJPEGXL_ENABLE_DOXYGEN=OFF -DJPEGXL_ENABLE_MANPAGES=OFF -DJPEGXL_ENABLE_BENCHMARK=OFF -DJPEGXL_ENABLE_EXAMPLES=OFF -DJPEGXL_ENABLE_JNI=OFF -DJPEGXL_ENABLE_SJPEG=OFF -DJPEGXL_ENABLE_OPENEXR=ON -DJPEGXL_BUNDLE_LIBPNG=OFF -DJPEGXL_ENABLE_PLUGINS=OFF "-DCMAKE_PREFIX_PATH=$jpeg" >/dev/null
# libjxl leaves EXR out quietly when it cannot find OpenEXR; the EXR gate needs it in.
grep -q '^OpenEXR_FOUND:INTERNAL=1' "$work/libjxl/CMakeCache.txt" || { echo "cjxl would be built without EXR: install OpenEXR's development files" >&2; exit 1; }
cmake --build "$work/libjxl" --parallel "$jobs" --target cjxl >/dev/null
cp "$work/libjxl/tools/cjxl" "$prefix/bin/cjxl"

echo "SKIDBLADNIR_REFERENCE_CWEBP=$prefix/bin/cwebp"
echo "SKIDBLADNIR_REFERENCE_IMG2WEBP=$prefix/bin/img2webp"
echo "SKIDBLADNIR_REFERENCE_GIF2WEBP=$prefix/bin/gif2webp"
echo "SKIDBLADNIR_REFERENCE_WEBPMUX=$prefix/bin/webpmux"
echo "SKIDBLADNIR_REFERENCE_AVIFENC=$prefix/bin/avifenc"
echo "SKIDBLADNIR_REFERENCE_CJXL=$prefix/bin/cjxl"
echo "SKIDBLADNIR_REFERENCE_HEIF_ENC=$prefix/heif/bin/heif-enc"
