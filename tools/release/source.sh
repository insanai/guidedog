#!/bin/sh
# Writes the source archive of the release, dist/guidedog-<version>-source.tar.gz:
# exactly the tracked files of the current commit (git archive), in one folder named
# guidedog-<version>. Native libraries, fonts and build outputs are not tracked, so they
# are not in it; their build scripts pin and check what they download. OUT sets the
# output folder (default dist).
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
out=${OUT:-$root/dist}
version=$("$root/tools/release/check-version.sh")
if [ -n "$(git -C "$root" status --porcelain)" ]; then
    echo "The working tree has changes; commit them so the archive matches its commit." >&2
    exit 1
fi
mkdir -p "$out"
archive=$out/guidedog-$version-source.tar.gz
git -C "$root" archive --format=tar --prefix="guidedog-$version/" HEAD | gzip -n -9 > "$archive"
echo "$archive"
