#!/usr/bin/env bash
# The smallest thread stack each encoder's heaviest settings survive, in a release build.
#
# Each case of crates/skidbladnir-encode/examples/stack-probe.rs is run with every thread's
# stack limited, bisecting between 32 KiB and 64 MiB to 16 KiB: the encode's own thread, and,
# through `ulimit -s`, the threads the encoders start themselves, which take glibc's default
# from it. (On Windows a thread started without a size gets the executable's default, 1 MiB;
# the encoders' worker threads do most of the work, so it is all of them that matter.) A
# stack too small crashes the probe, which is what is being looked for; core dumps are off
# for it. The cases run in parallel. Linux only.
#
#   scripts/stack-probe.sh [--dev] [case ...]     # every case if none are named
#
# --dev measures a debug build (the tests' profile), with libwebp's C unoptimised, instead.
# Prints one line per case: the smallest stack that encoded, in KiB.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
profile=release
if [ "${1:-}" = --dev ]; then
	profile=debug
	shift
fi
cargo build $([ "$profile" = release ] && echo --release) --quiet -p skidbladnir-encode --example stack-probe
target=$(cd "$root" && cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
probe="$target/$profile/examples/stack-probe"
ulimit -c 0
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
# Write the inputs once, before the cases race to.
"$probe" webp $((64 << 20)) "$scratch" >/dev/null

if [ $# -eq 0 ]; then
	mapfile -t names < <("$probe" list)
else
	names=("$@")
fi

bisect() {
	local name="$1" low=$((32 << 10)) high=$((64 << 20)) middle status
	if ! (ulimit -s $((high >> 10)); "$probe" "$name" "$high" "$scratch") 2>/dev/null; then
		echo "$name: fails even with $((high >> 10)) KiB"
		return
	fi
	while [ $((high - low)) -gt $((16 << 10)) ]; do
		middle=$(((low + high) / 2 / 4096 * 4096))
		status=0
		(ulimit -s $((middle >> 10)); exec "$probe" "$name" "$middle" "$scratch") >/dev/null 2>&1 || status=$?
		case "$status" in
			0) high=$middle;;
			3) echo "$name: the encode failed at $((middle >> 10)) KiB, not for want of stack"; return;;
			*) low=$middle;;
		esac
	done
	echo "$name: $((high >> 10)) KiB"
}

for name in "${names[@]}"; do
	bisect "$name" &
done
wait
