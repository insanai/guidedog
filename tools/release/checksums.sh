#!/bin/sh
# Writes DIR/SHA256SUMS for every release file in DIR (the archives and guidedog.rb), in
# the format `sha256sum -c` reads, sorted by name:
#
#     tools/release/checksums.sh dist
#     cd dist && sha256sum -c SHA256SUMS          (shasum -a 256 -c on macOS)
set -eu
dir=${1:?Usage: tools/release/checksums.sh DIR}
cd "$dir"
set --
for file in *.tar.gz *.zip *.rb; do
    [ -f "$file" ] && set -- "$@" "$file"
done
if [ $# -eq 0 ]; then
    echo "$dir holds no release files (*.tar.gz, *.zip, *.rb)." >&2
    exit 1
fi
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$@" | LC_ALL=C sort -k 2 > SHA256SUMS
else
    shasum -a 256 "$@" | LC_ALL=C sort -k 2 > SHA256SUMS
fi
cat SHA256SUMS
