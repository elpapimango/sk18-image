#!/bin/bash
# Builds the SK-18 SD card image (Buildroot, mainline U-Boot and Linux). Run it inside Linux (on Windows: in WSL).
#
#   tools/build/build.sh [--clean-kernel] [--clean-clients] [--out DIR] [--buildroot DIR]
#
#   --clean-kernel    rebuild the kernel from scratch (after a change to a kernel patch or to linux.fragment)
#   --clean-clients   rebuild sk18-keys, sk18-link and sk18-splash (after a change under client/)
#   --out DIR         Buildroot output directory (default ~/sk18-build)
#   --buildroot DIR   a Buildroot checkout (default ~/buildroot; the version is pinned in docs/)
#
# The first build needs network access (sources, the Rust crates). The image is DIR/images/sdcard.img; turn it into a file
# for balenaEtcher with tools/build/release.sh.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
out="$HOME/sk18-build"
buildroot="$HOME/buildroot"
targets=""
while [ $# -gt 0 ]; do
	case "$1" in
		--clean-kernel) targets="$targets linux-dirclean" ;;
		--clean-clients) targets="$targets sk18-keys-dirclean sk18-splash-dirclean sk18-link-dirclean" ;;
		--out) out=$2; shift ;;
		--buildroot) buildroot=$2; shift ;;
		-h|--help) sed -n '2,12p' "$0"; exit 0 ;;
		*) echo "unknown argument $1" >&2; exit 2 ;;
	esac
	shift
done

export BR2_EXTERNAL="$repo/br2-external"
cd "$buildroot"
[ -f "$out/.config" ] || make O="$out" waveshare_sk18_defconfig
rm -f "$out/build.status"
{
	# shellcheck disable=SC2086
	[ -z "$targets" ] || make O="$out" $targets
	make O="$out" -j"$(nproc)"
} 2>&1 | tee "$out/build.log" | tail -n 20
status=${PIPESTATUS[0]}
echo "exit=$status" > "$out/build.status"
date -Is >> "$out/build.status"
[ "$status" -eq 0 ] && ls -l "$out/images/sdcard.img"
exit "$status"
