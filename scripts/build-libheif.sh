#!/usr/bin/env bash
# Build the libheif Skidbladnir ships: one shared library with the Kvazaar HEVC encoder
# (BSD-3-Clause) and the libde265 HEVC decoder (LGPL-3.0) built into it, and nothing else.
#
#   scripts/build-libheif.sh [--prefix <dir>] [--with-heif-enc] [--jpeg <prefix>]
#
# --with-heif-enc also builds libheif's heif-enc, linked to this same libheif: the reference
# tests/heic_parity.rs compares against (it needs the libpng and libjpeg development files).
# --jpeg points its JPEG reader at a libjpeg installed under <prefix> rather than the
# system's (see scripts/build-reference-tools.sh). Never ship that build; the app's libheif
# is the one without them.
#
# Installs into build/libheif (or --prefix, or $SKIDBLADNIR_LIBHEIF_DIR): lib/ holds the
# shared library (bin/heif.dll on Windows, beside the import library in lib/), include/
# its headers. crates/skidbladnir-encode/build.rs links it from there.
#
# WHY SHARED: libheif and libde265 are LGPL. Linking them dynamically and shipping the
# library beside the app is what lets an ISC app use them. Kvazaar and libde265 are
# static *inside* libheif, so there is exactly one library to ship.
#
# WHY SO MUCH IS OFF: plugin loading is off, so the shipped libheif can never pick up a
# GPL encoder (x265) that happens to be installed on the user's machine, and every other
# codec is off because the app does not use it.
#
# Sharp YUV (`-C sharp-yuv` in heif-enc) comes from libwebp's libsharpyuv, built from the
# libwebp submodule (the version libwebp-sys vendors) and linked into libheif statically.
#
# Sources are the pinned submodules under third_party/. Needs CMake and a C/C++ compiler;
# on Windows it runs from Git Bash with Visual Studio's compiler. For an Intel build on an
# Apple Silicon Mac, set SKIDBLADNIR_TARGET_ARCH=x86_64.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
prefix="${SKIDBLADNIR_LIBHEIF_DIR:-$root/build/libheif}"
examples=OFF
jpeg=""
while [ $# -gt 0 ]; do
	case "$1" in
		--prefix) prefix="$2"; shift 2;;
		--with-heif-enc) examples=ON; shift;;
		--jpeg) jpeg="$2"; shift 2;;
		*) echo "unknown argument: $1" >&2; exit 2;;
	esac
done
for module in libheif kvazaar libde265 libwebp; do
	[ -f "$root/third_party/$module/CMakeLists.txt" ] || { echo "third_party/$module is missing: run git submodule update --init third_party/$module" >&2; exit 1; }
done

# CMake on Windows wants native paths.
native() { if command -v cygpath >/dev/null; then cygpath -m "$1"; else echo "$1"; fi; }
mkdir -p "$prefix"
prefix=$(cd "$prefix" && pwd)
work="$prefix/.work"
deps="$work/deps"

common=(-DCMAKE_BUILD_TYPE=Release -DCMAKE_POSITION_INDEPENDENT_CODE=ON -DCMAKE_INSTALL_LIBDIR=lib)
case "$(uname -s)" in
	Darwin)
		common+=(-DCMAKE_OSX_DEPLOYMENT_TARGET=10.15)
		[ -n "${SKIDBLADNIR_TARGET_ARCH:-}" ] && common+=("-DCMAKE_OSX_ARCHITECTURES=$SKIDBLADNIR_TARGET_ARCH")
		;;
	MINGW* | MSYS* | CYGWIN*)
		# The same C runtime Rust links against, so the DLL and the app share one.
		common+=(-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDLL)
		;;
esac
jobs=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)

build() {
	local name=$1 source=$2 install=$3
	shift 3
	echo "==> $name"
	cmake -S "$(native "$source")" -B "$(native "$work/$name")" "${common[@]}" "-DCMAKE_INSTALL_PREFIX=$(native "$install")" "$@"
	cmake --build "$(native "$work/$name")" --config Release --parallel "$jobs"
	cmake --install "$(native "$work/$name")" --config Release
}

build kvazaar "$root/third_party/kvazaar" "$deps" \
	-DBUILD_SHARED_LIBS=OFF -DBUILD_TESTS=OFF -DBUILD_KVAZAAR_BINARY=OFF -DGIT_SUBMODULE=OFF

build libde265 "$root/third_party/libde265" "$deps" \
	-DBUILD_SHARED_LIBS=OFF -DENABLE_SDL=OFF -DENABLE_DECODER=OFF -DENABLE_ENCODER=OFF

# libsharpyuv alone: the target, not libwebp's whole install.
echo "==> libsharpyuv"
cmake -S "$(native "$root/third_party/libwebp")" -B "$(native "$work/libwebp")" "${common[@]}" -DBUILD_SHARED_LIBS=OFF \
	-DWEBP_BUILD_ANIM_UTILS=OFF -DWEBP_BUILD_CWEBP=OFF -DWEBP_BUILD_DWEBP=OFF -DWEBP_BUILD_GIF2WEBP=OFF -DWEBP_BUILD_IMG2WEBP=OFF \
	-DWEBP_BUILD_VWEBP=OFF -DWEBP_BUILD_WEBPINFO=OFF -DWEBP_BUILD_WEBPMUX=OFF -DWEBP_BUILD_EXTRAS=OFF
cmake --build "$(native "$work/libwebp")" --config Release --parallel "$jobs" --target sharpyuv
sharpyuv=$(find "$work/libwebp" -maxdepth 2 \( -name libsharpyuv.a -o -name sharpyuv.lib \) | head -1)

# The two static libraries are compiled into libheif, so their headers must not declare
# their functions as DLL imports.
static_defines="-DKVZ_STATIC_LIB -DLIBDE265_STATIC_BUILD"
off=()
for codec in X265 X264 OpenH264_DECODER AOM_DECODER AOM_ENCODER DAV1D RAV1E SvtEnc JPEG_DECODER JPEG_ENCODER OpenJPEG_DECODER OpenJPEG_ENCODER OPENJPH_ENCODER FFMPEG_DECODER UVG266 VVDEC VVENC; do
	off+=("-DWITH_$codec=OFF")
done
extra=()
[ "$(uname -s)" = Darwin ] && extra+=(-DCMAKE_INSTALL_NAME_DIR=@rpath)
build libheif "$root/third_party/libheif" "$prefix" \
	-DBUILD_SHARED_LIBS=ON -DENABLE_PLUGIN_LOADING=OFF \
	-DWITH_KVAZAAR=ON -DWITH_KVAZAAR_PLUGIN=OFF -DWITH_LIBDE265=ON -DWITH_LIBDE265_PLUGIN=OFF \
	"${off[@]}" \
	-DWITH_LIBSHARPYUV=ON "-DLIBSHARPYUV_INCLUDE_DIR=$(native "$root/third_party/libwebp")" "-DLIBSHARPYUV_LIBRARY=$(native "$sharpyuv")" \
	-DWITH_UNCOMPRESSED_CODEC=OFF -DWITH_HEADER_COMPRESSION=OFF \
	-DWITH_EXAMPLES=$examples -DWITH_GDK_PIXBUF=OFF -DBUILD_TESTING=OFF -DWITH_FUZZERS=OFF \
	-DBUILD_DOCUMENTATION=OFF \
	"-DCMAKE_PREFIX_PATH=$(native "$deps")${jpeg:+;$(native "$jpeg")}" \
	"-DCMAKE_C_FLAGS=$static_defines" "-DCMAKE_CXX_FLAGS=$static_defines" \
	"${extra[@]}"

echo "libheif installed in $prefix:"
ls "$prefix/lib" "$prefix/bin" 2>/dev/null || true
