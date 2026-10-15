#!/bin/sh
# Packages the programs tools/release/build.sh put in build/ into the release archives of
# one platform:
#
#     tools/release/package.sh linux-amd64      dist/guidedog-linux-amd64.tar.gz
#                                               dist/dscc-linux-amd64.tar.gz
#
# The guidedog archive holds one folder, guidedog-<os>-<arch>, with guidedog, dsc,
# guidedog-pages, and graphdog; the dscc archive holds dscc alone. Each folder also has
# README.md, LICENSE, NOTICE, CHANGELOG.md, SOURCE.txt (where the exact source is and how
# it was built) and NAME.build.json (version, commit, target, toolchain, and the programs'
# SHA-256). Graphviz is not in the archives: the programs link the system's libraries.
# The working tree must be clean, so the archives match their commit. OUT sets the output
# folder (default dist).
set -eu
package=${1:-}
root=$(cd "$(dirname "$0")/../.." && pwd)
out=${OUT:-$root/dist}
case $package in
    linux-amd64) target=x86_64-unknown-linux-gnu ;;
    linux-arm64) target=aarch64-unknown-linux-gnu ;;
    macos-arm64) target=aarch64-apple-darwin ;;
    macos-amd64) target=x86_64-apple-darwin ;;
    *) echo "Usage: tools/release/package.sh linux-amd64|linux-arm64|macos-arm64|macos-amd64" >&2
       exit 2 ;;
esac
case $package in
    linux-*) requires="Linux with glibc 2.35 or later, and the Graphviz libraries
(Debian/Ubuntu: apt install libgvc6; Fedora: dnf install graphviz)" ;;
    macos-*) requires="macOS 12 or later, and Graphviz from Homebrew (brew install graphviz)" ;;
esac

for file in build/guidedog build/dsc build/dscc build/guidedog-pages build/graphdog \
    build/build.command build/rustc.version README.md LICENSE NOTICE CHANGELOG.md; do
    if [ ! -f "$root/$file" ]; then
        echo "$file is missing; build the programs with tools/release/build.sh first." >&2
        exit 1
    fi
done
if [ -n "$(git -C "$root" status --porcelain)" ]; then
    echo "The working tree has changes; commit them so the archive matches its commit:" >&2
    git -C "$root" status --short >&2
    exit 1
fi

version=$("$root/tools/release/check-version.sh")
commit=$(git -C "$root" rev-parse HEAD)
command=$(cat "$root/build/build.command")
rustc=$(cat "$root/build/rustc.version")
if command -v sha256sum >/dev/null 2>&1; then sum() { sha256sum "$1" | cut -d' ' -f1; }
else sum() { shasum -a 256 "$1" | cut -d' ' -f1; }; fi
releases=https://github.com/insanai/guidedog/releases/download
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir -p "$out"

# archive NAME PROGRAMS...: writes dist/NAME-PACKAGE.tar.gz with the programs.
archive() {
    name=$1
    shift
    folder=$stage/$name-$package
    mkdir -p "$folder"
    sums=""
    for program in "$@"; do
        cp "$root/build/$program" "$folder/$program"
        chmod 755 "$folder/$program"
        [ -n "$sums" ] && sums="$sums, "
        sums="$sums\"$program\": \"$(sum "$folder/$program")\""
    done
    for file in README.md LICENSE NOTICE CHANGELOG.md; do cp "$root/$file" "$folder/"; done
    source_text "$@" > "$folder/SOURCE.txt"
    cat > "$folder/$name.build.json" <<JSON
{
  "name": "$name",
  "version": "$version",
  "commit": "$commit",
  "source": "https://github.com/insanai/guidedog/tree/v$version",
  "package": "$package",
  "target": "$target",
  "rustc": "$rustc",
  "command": "$(echo "$command" | sed 's/"/\\"/g')",
  "typst_embedded": true,
  "graphviz": "linked from the system",
  "requires": "$(echo "$requires" | tr '\n' ' ')",
  "binary": "$name",
  "binary_sha256": "$(sum "$folder/$name")",
  "programs": {$sums}
}
JSON
    (cd "$stage" && tar -cf - "$name-$package") | gzip -n -9 > "$out/$name-$package.tar.gz"
    echo "$out/$name-$package.tar.gz"
}

# source_text PROGRAMS...: the SOURCE.txt of an archive.
source_text() {
    cat <<SOURCE
Guidedog $version for $package ($target)

Corresponding source: https://github.com/insanai/guidedog/tree/v$version
Commit: $commit
Source archive: $releases/v$version/guidedog-$version-source.tar.gz

Built with $rustc and Cargo.lock, by:

    $command

Programs: $*
Typst: embedded Typst 0.15.1. Graphviz: linked from the system.
Requires: $requires.
Licenses and notices: LICENSE and NOTICE.
SOURCE
}

archive guidedog guidedog dsc guidedog-pages graphdog
archive dscc dscc
