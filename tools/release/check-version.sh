#!/bin/sh
# Checks that a release names one version everywhere, and prints it:
#
#     tools/release/check-version.sh                 VERSION, Cargo.toml, and the tag in
#                                                    GITHUB_REF
#     tools/release/check-version.sh build/guidedog  also what the program reports (or
#                                                    build/dsc, build/dscc, ...: each
#                                                    begins with its own name)
#
# The VERSION file holds the version (0.7.0), and [workspace.package] in Cargo.toml holds
# the same version, which every crate inherits. A tag build (GITHUB_REF=refs/tags/...)
# must be tagged v<VERSION>, and `guidedog --version` must begin "guidedog <VERSION>".
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)

if [ ! -f "$root/VERSION" ]; then
    echo "There is no VERSION file at the repository root." >&2
    echo "Create it with the release version on one line, such as 0.7.0." >&2
    exit 1
fi
version=$(tr -d ' \r\n' < "$root/VERSION")
if ! echo "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$'; then
    echo "VERSION holds \"$version\", which is not a version such as 0.7.0." >&2
    exit 1
fi

# The first version line after [workspace.package] in the root Cargo.toml.
cargo=$(awk '/^\[workspace\.package\]/ { inside = 1; next }
    /^\[/ { inside = 0 }
    inside && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }' "$root/Cargo.toml")
if [ "$cargo" != "$version" ]; then
    echo "Cargo.toml's [workspace.package] version is \"$cargo\", but VERSION is $version." >&2
    echo "Bump both together." >&2
    exit 1
fi

ref=${GITHUB_REF:-}
case $ref in
    refs/tags/*)
        if [ "$ref" != "refs/tags/v$version" ]; then
            echo "The tag ${ref#refs/tags/} does not match VERSION $version." >&2
            echo "Tag the commit v$version, or correct VERSION and tag again." >&2
            exit 1
        fi ;;
esac

if [ $# -gt 0 ]; then
    reported=$("$1" --version | head -n 1 | tr -d '\r')
    name=$(basename "$1" .exe)
    case $reported in
        "$name $version" | "$name $version "*) ;;
        *)
            echo "$1 reports \"$reported\", but VERSION is $version." >&2
            echo "Rebuild after bumping VERSION and Cargo.toml together." >&2
            exit 1 ;;
    esac
fi
echo "$version"
