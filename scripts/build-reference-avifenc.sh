#!/usr/bin/env bash
# Build the avifenc that tests/avif_parity.rs compares Skidbladnir's AVIF output against.
#
#   scripts/build-reference-avifenc.sh --sharpyuv <libsharpyuv.a> [--prefix <dir>]
#
# Byte parity is only meaningful against an avifenc built the way Skidbladnir's libavif is
# built: the same pinned libavif and libaom (third_party/), no libyuv (it changes the
# RGB-to-YUV conversion), and libwebp's sharpyuv at the version libwebp-sys vendors. A
# distribution's avifenc is almost always linked with libyuv, so it is not a reference.
#
# --sharpyuv is a static libsharpyuv from that libwebp version; CI and a dev host both get
# one from the libwebp checkout they build the reference cwebp from
# (`make -f makefile.unix sharpyuv/libsharpyuv.a`). avifenc's own PNG and JPEG readers use
# the system libpng, libjpeg and zlib, so their development files are needed, and its
# configure step fetches libargparse from GitHub.
#
# Installs avifenc into build/reference-avifenc (or --prefix) and prints its path.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
prefix="$root/build/reference-avifenc"
sharpyuv=""
while [ $# -gt 0 ]; do
	case "$1" in
		--prefix) prefix="$2"; shift 2;;
		--sharpyuv) sharpyuv="$2"; shift 2;;
		*) echo "unknown argument: $1" >&2; exit 2;;
	esac
done
[ -f "$sharpyuv" ] || { echo "--sharpyuv must name a static libsharpyuv built from libwebp $(cargo run -q --example libwebp-version -p skidbladnir-encode 2>/dev/null || echo '<libwebp-sys version>')" >&2; exit 2; }
for module in libavif aom; do
	[ -f "$root/third_party/$module/CMakeLists.txt" ] || { echo "third_party/$module is missing: run git submodule update --init third_party/$module" >&2; exit 1; }
done

mkdir -p "$prefix"
prefix=$(cd "$prefix" && pwd)
work="$prefix/.work"
jobs=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)

echo "==> libaom"
cmake -S "$root/third_party/aom" -B "$work/aom" -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DCONFIG_AV1_DECODER=1 -DCONFIG_AV1_ENCODER=1 -DENABLE_DOCS=OFF -DENABLE_EXAMPLES=OFF -DENABLE_TESTDATA=OFF -DENABLE_TESTS=OFF -DENABLE_TOOLS=OFF "-DCMAKE_INSTALL_PREFIX=$work/aom-install" -DCMAKE_INSTALL_LIBDIR=lib >/dev/null
cmake --build "$work/aom" --parallel "$jobs" >/dev/null
cmake --install "$work/aom" >/dev/null

echo "==> libavif and avifenc"
cmake -S "$root/third_party/libavif" -B "$work/libavif" -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
	-DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=ON -DAVIF_CODEC_AOM_ENCODE=ON \
	"-DAOM_INCLUDE_DIR=$work/aom-install/include" "-DAOM_LIBRARY=$work/aom-install/lib/libaom.a" \
	-DAVIF_LIBYUV=OFF -DAVIF_LIBSHARPYUV=SYSTEM "-DLIBSHARPYUV_INCLUDE_DIR=$root/third_party/sharpyuv" "-DLIBSHARPYUV_LIBRARY=$sharpyuv" \
	-DAVIF_JPEG=SYSTEM -DAVIF_ZLIBPNG=SYSTEM -DAVIF_BUILD_APPS=ON -DAVIF_BUILD_TESTS=OFF -DAVIF_ENABLE_WERROR=OFF >/dev/null
cmake --build "$work/libavif" --parallel "$jobs" --target avifenc >/dev/null
cp "$work/libavif/avifenc" "$prefix/avifenc"
echo "$prefix/avifenc"
