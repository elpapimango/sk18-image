#!/bin/bash
# Packs a built image for download and flashing with balenaEtcher (or Raspberry Pi Imager, dd, ...):
#
#   tools/build/release.sh <version> [--out DIR] [--image PATH]
#
# Writes dist/sk18-image-<version>.img.xz and dist/SHA256SUMS. balenaEtcher flashes the .img.xz as it is (it unpacks
# it on the fly). The image is a complete SD card image (partition table, U-Boot, kernel, root file system) of about 64 MB
# that fits any card of 1 GB or more.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
version=${1:?usage: release.sh <version> [--out DIR] [--image PATH]}
shift
out="$repo/dist"
image="$HOME/sk18-build/images/sdcard.img"
while [ $# -gt 0 ]; do
	case "$1" in
		--out) out=$2; shift ;;
		--image) image=$2; shift ;;
		*) echo "unknown argument $1" >&2; exit 2 ;;
	esac
	shift
done

# The release version is the version of the client workspace (what the board reports as its firmware version).
crate=$(grep -m 1 '^version = ' "$repo/client/Cargo.toml" | cut -d'"' -f2)
[ "$version" = "$crate" ] || { echo "version $version is not the client version $crate (client/Cargo.toml): change one of them" >&2; exit 1; }
[ -f "$image" ] || { echo "no image at $image: build it first (tools/build/build.sh)" >&2; exit 1; }
mkdir -p "$out"
name="sk18-image-$version"
xz -9 -T0 -c "$image" > "$out/$name.img.xz"
(
	cd "$out"
	# the checksum of the unpacked image too, for people who write it with dd
	printf '%s  %s\n' "$(sha256sum < "$image" | cut -d' ' -f1)" "$name.img" > "$name.img.sha256"
	sha256sum "$name.img.xz" > SHA256SUMS
	cat "$name.img.sha256" >> SHA256SUMS
	cat SHA256SUMS
)
ls -l "$out/$name.img.xz"
