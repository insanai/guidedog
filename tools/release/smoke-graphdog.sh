#!/bin/sh
# Smoke-tests graphdog with the system's Graphviz: a graph to SVG, and a syntax error
# reported with its location.
#
#     tools/release/smoke-graphdog.sh guidedog-linux-amd64/graphdog
set -eu
program=${1:?Usage: tools/release/smoke-graphdog.sh PROGRAM}
case $program in /*) ;; *) program=$PWD/$program ;; esac
fail() { echo "graphdog smoke test: $*" >&2; exit 1; }

"$program" --version
svg=$(echo 'digraph { source -> html; source -> pdf }' | "$program" -Tsvg)
case $svg in *"<svg"*) ;; *) fail "no SVG from a valid graph" ;; esac
if printf 'digraph {\n    a -> -> b\n}\n' | "$program" -Tsvg >/dev/null 2>&1; then
    fail "a broken graph rendered"
fi
echo "graphdog smoke test passed"
