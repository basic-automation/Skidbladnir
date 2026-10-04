#!/usr/bin/env bash
# Build the avifenc that tests/avif_parity.rs compares Skidbladnir's AVIF output against.
#
#   scripts/build-reference-avifenc.sh [--prefix <dir>] [--jpeg <prefix>] [--cmake-arg <arg>]...
#
# Byte parity is only meaningful against an avifenc built the way Skidbladnir's libavif is
# built: the same pinned libavif and libaom (third_party/), no libyuv (it changes the
# RGB-to-YUV conversion), and libwebp's sharpyuv at the version libwebp-sys vendors. A
# distribution's avifenc is almost always linked with libyuv, so it is not a reference.
#
# sharpyuv is built from third_party/libwebp, the libwebp libwebp-sys vendors. avifenc's own
# PNG and JPEG readers use the system libpng, libjpeg and zlib, so their development files
# are needed, and its configure step fetches libargparse from GitHub. --jpeg uses a libjpeg
# installed under <prefix> instead of the system's (see scripts/build-reference-tools.sh).
# Each --cmake-arg goes to every configure step: on Windows (Git Bash, Visual Studio's
# compiler) that is how the C runtime and vcpkg's libpng and zlib are chosen.
#
# Installs avifenc into build/reference-avifenc (or --prefix) and prints its path.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
prefix="$root/build/reference-avifenc"
jpeg=""
extra=()
while [ $# -gt 0 ]; do
	case "$1" in
		--prefix) prefix="$2"; shift 2;;
		--jpeg) jpeg="$2"; shift 2;;
		--cmake-arg) extra+=("$2"); shift 2;;
		*) echo "unknown argument: $1" >&2; exit 2;;
	esac
done
for module in libavif aom libwebp; do
	[ -f "$root/third_party/$module/CMakeLists.txt" ] || { echo "third_party/$module is missing: run git submodule update --init third_party/$module" >&2; exit 1; }
done

# CMake on Windows wants native paths.
native() { if command -v cygpath >/dev/null; then cygpath -m "$1"; else echo "$1"; fi; }
mkdir -p "$prefix/.work"
prefix=$(cd "$prefix" && pwd)
work="$prefix/.work"
jobs=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)
# Each step's output goes to a log, shown only if the step fails.
quiet() { "$@" > "$work/step.log" 2>&1 || { tail -n 60 "$work/step.log" >&2; exit 1; }; }
# The libraries as each toolchain names them: libfoo.a, or foo.lib with MSVC.
library() { find "$1" \( -name "lib$2.a" -o -name "$2.lib" \) | head -1; }

echo "==> libsharpyuv"
quiet cmake -S "$(native "$root/third_party/libwebp")" -B "$(native "$work/libwebp")" -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DWEBP_BUILD_ANIM_UTILS=OFF -DWEBP_BUILD_CWEBP=OFF -DWEBP_BUILD_DWEBP=OFF -DWEBP_BUILD_GIF2WEBP=OFF -DWEBP_BUILD_IMG2WEBP=OFF -DWEBP_BUILD_VWEBP=OFF -DWEBP_BUILD_WEBPINFO=OFF -DWEBP_BUILD_WEBPMUX=OFF -DWEBP_BUILD_EXTRAS=OFF ${extra[@]+"${extra[@]}"}
quiet cmake --build "$(native "$work/libwebp")" --config Release --parallel "$jobs" --target sharpyuv
sharpyuv=$(library "$work/libwebp" sharpyuv)

echo "==> libaom"
# CMAKE_POLICY_VERSION_MINIMUM as crates/skidbladnir-encode/build.rs builds it, for CMake 4.
quiet cmake -S "$(native "$root/third_party/aom")" -B "$(native "$work/aom")" -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DCONFIG_AV1_DECODER=1 -DCONFIG_AV1_ENCODER=1 -DENABLE_DOCS=OFF -DENABLE_EXAMPLES=OFF -DENABLE_TESTDATA=OFF -DENABLE_TESTS=OFF -DENABLE_TOOLS=OFF -DCMAKE_POLICY_VERSION_MINIMUM=3.5 "-DCMAKE_INSTALL_PREFIX=$(native "$work/aom-install")" -DCMAKE_INSTALL_LIBDIR=lib ${extra[@]+"${extra[@]}"}
quiet cmake --build "$(native "$work/aom")" --config Release --parallel "$jobs"
quiet cmake --install "$(native "$work/aom")" --config Release
aom=$(library "$work/aom-install/lib" aom)

echo "==> libavif and avifenc"
quiet cmake -S "$(native "$root/third_party/libavif")" -B "$(native "$work/libavif")" -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
	-DAVIF_CODEC_AOM=SYSTEM -DAVIF_CODEC_AOM_DECODE=ON -DAVIF_CODEC_AOM_ENCODE=ON \
	"-DAOM_INCLUDE_DIR=$(native "$work/aom-install/include")" "-DAOM_LIBRARY=$(native "$aom")" \
	-DAVIF_LIBYUV=OFF -DAVIF_LIBSHARPYUV=SYSTEM "-DLIBSHARPYUV_INCLUDE_DIR=$(native "$root/third_party/libwebp")" "-DLIBSHARPYUV_LIBRARY=$(native "$sharpyuv")" \
	-DAVIF_JPEG=SYSTEM -DAVIF_ZLIBPNG=SYSTEM -DAVIF_BUILD_APPS=ON -DAVIF_BUILD_TESTS=OFF -DAVIF_ENABLE_WERROR=OFF \
	${jpeg:+"-DCMAKE_PREFIX_PATH=$(native "$jpeg")"} ${extra[@]+"${extra[@]}"}
quiet cmake --build "$(native "$work/libavif")" --config Release --parallel "$jobs" --target avifenc
built=$(find "$work/libavif" -maxdepth 2 \( -name avifenc -o -name avifenc.exe \) -type f | head -1)
cp "$built" "$prefix/"
echo "$prefix/$(basename "$built")"
