#!/bin/sh
# Builds the release programs the way the release workflow does, copies them into build/,
# and records the exact command in build/build.command for the archive's SOURCE.txt:
#
#     tools/release/build.sh                              the host's target
#     tools/release/build.sh --target aarch64-apple-darwin
#
# It builds guidedog, dsc, dscc, guidedog-pages, and graphdog with `cargo build --release
# --locked`. The Typst compiler is embedded (gd-typst's default `embedded` feature) and
# tree-sitter is compiled in. Graphviz is linked from the system: install its development
# files first (Debian/Ubuntu: libgraphviz-dev; macOS: brew install graphviz). On macOS
# the script points the linker at Homebrew's Graphviz and targets macOS 12.
set -eu
cd "$(dirname "$0")/../.."

target=""
while [ $# -gt 0 ]; do
    case $1 in
        --target) target=${2:?--target needs a Rust target triple, such as x86_64-apple-darwin}
            shift ;;
        *) echo "Usage: tools/release/build.sh [--target TRIPLE]" >&2; exit 2 ;;
    esac
    shift
done

programs="guidedog dsc dscc guidedog-pages graphdog"
case $(uname -s) in
    MINGW* | MSYS* | CYGWIN*) exe=.exe ;;
    Darwin)
        exe=""
        export MACOSX_DEPLOYMENT_TARGET=${MACOSX_DEPLOYMENT_TARGET:-12.0}
        if command -v brew >/dev/null 2>&1; then
            LIBRARY_PATH="$(brew --prefix graphviz)/lib${LIBRARY_PATH:+:$LIBRARY_PATH}"
            export LIBRARY_PATH
        fi ;;
    *) exe="" ;;
esac

set -- cargo build --release --locked
for program in $programs; do set -- "$@" -p "$program-cmd"; done
[ -n "$target" ] && set -- "$@" --target "$target"

mkdir -p build
echo "$*" > build/build.command
echo "$*"
"$@"

out=${CARGO_TARGET_DIR:-target}/${target:+$target/}release
for program in $programs; do
    cp "$out/$program$exe" "build/$program$exe"
done
rustc --version > build/rustc.version
echo "build/: $programs"
