#!/usr/bin/env bash
# Build the libheif Skidbladnir ships: one shared library with an HEVC encoder and the
# libde265 HEVC decoder (LGPL-3.0) built into it, and nothing else.
#
#   scripts/build-libheif.sh [--edition standard|gpl] [--prefix <dir>]
#
# The edition decides the encoder, and with it the licence of the app that ships it:
# - standard (the default): Kvazaar (BSD-3-Clause). The app stays ISC.
# - gpl: x265 (GPL-2.0-or-later), with its 8-bit and 10-bit encoders linked together.
#   An app that ships this library is distributed under GPL-3.0-or-later as a whole; it
#   is the `gpl` feature's build (see README.md, "Editions").
# `SKIDBLADNIR_EDITION` sets the edition too, for CI.
#
# Installs into build/libheif, or build/libheif-gpl for the gpl edition (or --prefix, or
# $SKIDBLADNIR_LIBHEIF_DIR): lib/ holds the shared library (bin/heif.dll on Windows, beside
# the import library in lib/), include/ its headers, and EDITION names the edition it was
# built as. crates/skidbladnir-encode/build.rs links it from there and refuses a library
# built for the other edition, so a standard app can never link, and so never ship, x265.
#
# WHY SHARED: libheif and libde265 are LGPL. Linking them dynamically and shipping the
# library beside the app is what lets an ISC app use them. The encoder and libde265 are
# static *inside* libheif, so there is exactly one library to ship.
#
# WHY SO MUCH IS OFF: plugin loading is off, so the shipped libheif can never pick up an
# encoder (x265 above all) that happens to be installed on the user's machine, and every
# other codec is off because the app does not use it.
#
# Sources are the pinned submodules under third_party/. Needs CMake and a C/C++ compiler,
# and for the gpl edition on x86_64, nasm (x265's assembly). On Windows it runs from Git
# Bash with Visual Studio's compiler. For an Intel build on an Apple Silicon Mac, set
# SKIDBLADNIR_TARGET_ARCH=x86_64.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
prefix="${SKIDBLADNIR_LIBHEIF_DIR:-}"
edition="${SKIDBLADNIR_EDITION:-standard}"
while [ $# -gt 0 ]; do
	case "$1" in
		--prefix) prefix="$2"; shift 2;;
		--edition) edition="$2"; shift 2;;
		*) echo "unknown argument: $1" >&2; exit 2;;
	esac
done
case "$edition" in
	standard) encoder=kvazaar; prefix="${prefix:-$root/build/libheif}";;
	gpl) encoder=x265; prefix="${prefix:-$root/build/libheif-gpl}";;
	*) echo "unknown edition: $edition (standard or gpl)" >&2; exit 2;;
esac
for module in libheif "$encoder" libde265; do
	[ -f "$root/third_party/$module/CMakeLists.txt" ] || [ -f "$root/third_party/$module/source/CMakeLists.txt" ] || { echo "third_party/$module is missing: run git submodule update --init third_party/$module" >&2; exit 1; }
done

# CMake on Windows wants native paths.
native() { if command -v cygpath >/dev/null; then cygpath -m "$1"; else echo "$1"; fi; }
mkdir -p "$prefix"
prefix=$(cd "$prefix" && pwd)
work="$prefix/.work"
deps="$work/deps"
# No marker until this build has finished, so a failed or interrupted build can never be
# mistaken for a finished one of either edition.
rm -f "$prefix/EDITION"

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

# The static libraries are compiled into libheif, so their headers must not declare their
# functions as DLL imports.
static_defines="-DLIBDE265_STATIC_BUILD"
encoder_options=()
if [ "$edition" = standard ]; then
	build kvazaar "$root/third_party/kvazaar" "$deps" \
		-DBUILD_SHARED_LIBS=OFF -DBUILD_TESTS=OFF -DBUILD_KVAZAAR_BINARY=OFF -DGIT_SUBMODULE=OFF
	static_defines+=" -DKVZ_STATIC_LIB"
	encoder_options=(-DWITH_KVAZAAR=ON -DWITH_KVAZAAR_PLUGIN=OFF -DWITH_X265=OFF)
else
	# x265 builds one bit depth per library. The 10-bit one is built first without its C
	# API, then the 8-bit one links it in (LINKED_10BIT) and answers for both through
	# x265_api_get(10), which is how libheif asks for it. Both go into libheif.
	x265_options=(-DENABLE_SHARED=OFF -DENABLE_CLI=OFF -DENABLE_LIBNUMA=OFF -DENABLE_PIC=ON
		# x265 4.2 still declares CMake 2.8-era compatibility, which CMake 4 refuses.
		-DCMAKE_POLICY_VERSION_MINIMUM=3.5)
	# x265 picks its assembly from CMAKE_SYSTEM_PROCESSOR, which a cross build on an Apple
	# Silicon runner leaves at arm64: name the target so it assembles x86_64 with nasm.
	if [ "$(uname -s)" = Darwin ] && [ "${SKIDBLADNIR_TARGET_ARCH:-}" = x86_64 ]; then
		x265_options+=(-DCMAKE_SYSTEM_NAME=Darwin -DCMAKE_SYSTEM_PROCESSOR=x86_64)
	fi
	# MSVC names the static library x265-static.lib; everything else libx265.a.
	case "$(uname -s)" in
		MINGW* | MSYS* | CYGWIN*) x265_lib=x265-static.lib;;
		*) x265_lib=libx265.a;;
	esac
	# x265 names its version with `git describe`, and a shallow checkout (CI's) has no
	# tags: it then reads "unknown", and on Windows cannot version its resource file at
	# all. Tag the pinned commit locally with the version .gitmodules records for it.
	x265_version=$(git -C "$root" config -f .gitmodules submodule.third_party/x265.version || true)
	if [ -e "$root/third_party/x265/.git" ] && [ -n "$x265_version" ] && ! git -C "$root/third_party/x265" describe --tags --exact-match >/dev/null 2>&1; then
		git -C "$root/third_party/x265" tag "$x265_version" HEAD
	fi
	build x265-main10 "$root/third_party/x265/source" "$deps/x265-main10" \
		"${x265_options[@]}" -DHIGH_BIT_DEPTH=ON -DEXPORT_C_API=OFF
	main10="$deps/x265-main10/lib/$x265_lib"
	build x265 "$root/third_party/x265/source" "$deps/x265" \
		"${x265_options[@]}" "-DEXTRA_LIB=$(native "$main10")" -DLINKED_10BIT=ON
	# The 8-bit library first: it refers to the 10-bit one's symbols, and a Unix linker
	# resolves archives in order.
	encoder_options=(-DWITH_X265=ON -DWITH_X265_PLUGIN=OFF -DWITH_KVAZAAR=OFF
		"-DX265_INCLUDE_DIR=$(native "$deps/x265/include")"
		"-DX265_LIBRARY=$(native "$deps/x265/lib/$x265_lib");$(native "$main10")")
fi

build libde265 "$root/third_party/libde265" "$deps" \
	-DBUILD_SHARED_LIBS=OFF -DENABLE_SDL=OFF -DENABLE_DECODER=OFF -DENABLE_ENCODER=OFF

off=()
for codec in X264 OpenH264_DECODER AOM_DECODER AOM_ENCODER DAV1D RAV1E SvtEnc JPEG_DECODER JPEG_ENCODER OpenJPEG_DECODER OpenJPEG_ENCODER OPENJPH_ENCODER FFMPEG_DECODER UVG266 VVDEC VVENC; do
	off+=("-DWITH_$codec=OFF")
done
extra=()
[ "$(uname -s)" = Darwin ] && extra+=(-DCMAKE_INSTALL_NAME_DIR=@rpath)
# A fresh configure every time: switching editions in one prefix must not inherit the
# other edition's cached encoder.
rm -rf "$work/libheif"
build libheif "$root/third_party/libheif" "$prefix" \
	-DBUILD_SHARED_LIBS=ON -DENABLE_PLUGIN_LOADING=OFF \
	"${encoder_options[@]}" -DWITH_LIBDE265=ON -DWITH_LIBDE265_PLUGIN=OFF \
	"${off[@]}" \
	-DWITH_LIBSHARPYUV=OFF -DWITH_UNCOMPRESSED_CODEC=OFF -DWITH_HEADER_COMPRESSION=OFF \
	-DWITH_EXAMPLES=OFF -DWITH_GDK_PIXBUF=OFF -DBUILD_TESTING=OFF -DWITH_FUZZERS=OFF \
	-DBUILD_DOCUMENTATION=OFF \
	"-DCMAKE_PREFIX_PATH=$(native "$deps")" \
	"-DCMAKE_C_FLAGS=$static_defines" "-DCMAKE_CXX_FLAGS=$static_defines" \
	"${extra[@]}"

echo "$edition" > "$prefix/EDITION"
echo "libheif ($edition edition, $encoder) installed in $prefix:"
ls "$prefix/lib" "$prefix/bin" 2>/dev/null || true
