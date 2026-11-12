#!/bin/bash
# Puts the Buildroot commit this image is built with (tools/build/BUILDROOT_COMMIT) into DIR (default ~/buildroot):
#
#   tools/build/fetch-buildroot.sh [DIR]
#
# A shallow fetch of that one commit; an existing checkout at the right commit is left alone.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
commit=$(tr -d '[:space:]' < "$here/BUILDROOT_COMMIT")
dir=${1:-$HOME/buildroot}
url=${BUILDROOT_URL:-https://github.com/buildroot/buildroot.git}

if [ -d "$dir/.git" ] && [ "$(git -C "$dir" rev-parse HEAD)" = "$commit" ]; then
	echo "Buildroot $commit is already in $dir"
	exit 0
fi
[ -e "$dir" ] && [ -n "$(ls -A "$dir" 2>/dev/null)" ] && { echo "$dir exists and is not Buildroot $commit" >&2; exit 1; }
mkdir -p "$dir"
git -C "$dir" init -q
git -C "$dir" fetch -q --depth 1 "$url" "$commit"
git -C "$dir" checkout -q FETCH_HEAD
echo "Buildroot $(git -C "$dir" rev-parse HEAD) in $dir"
