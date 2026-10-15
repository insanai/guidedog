#!/bin/sh
# Renders the Homebrew formula DIR/guidedog.rb from tools/release/guidedog.rb.in, with the
# version and the SHA-256 of the four macOS and Linux archives in DIR:
#
#     tools/release/formula.sh dist
#
# The formula installs the release binary for the machine (no build from source), so
# every archive it names must be in DIR.
set -eu
dir=${1:?Usage: tools/release/formula.sh DIR}
root=$(cd "$(dirname "$0")/../.." && pwd)
version=$("$root/tools/release/check-version.sh")
if command -v sha256sum >/dev/null 2>&1; then sum() { sha256sum "$1" | cut -d' ' -f1; }
else sum() { shasum -a 256 "$1" | cut -d' ' -f1; }; fi

sed_script="s/@VERSION@/$version/g"
for package in macos-arm64 macos-amd64 linux-arm64 linux-amd64; do
    archive=$dir/guidedog-$package.tar.gz
    if [ ! -f "$archive" ]; then
        echo "$archive is missing; the formula needs all four macOS and Linux archives." >&2
        exit 1
    fi
    key=$(echo "$package" | tr 'a-z-' 'A-Z_')
    sed_script="$sed_script; s/@$key@/$(sum "$archive")/g"
done
sed "$sed_script" "$root/tools/release/guidedog.rb.in" > "$dir/guidedog.rb"
if grep -q '@[A-Z0-9_]*@' "$dir/guidedog.rb"; then
    echo "$dir/guidedog.rb still holds a placeholder." >&2
    exit 1
fi
echo "$dir/guidedog.rb"
