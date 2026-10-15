#!/bin/sh
# Smoke-tests a guidedog program the way a new user runs it: quickstart, then an HTML site
# and a PDF book from a project with a Graphviz diagram, an equation, Markdown, and API
# documentation read from Python source without running Python.
#
#     tools/release/smoke.sh guidedog-linux-amd64/guidedog
#     tools/release/smoke.sh --external-typst target/release/guidedog
#
# By default the program runs with an empty PATH, so it must carry everything itself
# except the system libraries it links (libc and Graphviz): the embedded Typst compiler
# and tree-sitter. --external-typst keeps PATH, for builds without the `embedded`
# feature that run an installed typst program.
set -eu
external=no
if [ "${1:-}" = --external-typst ]; then external=yes; shift; fi
program=${1:?Usage: tools/release/smoke.sh [--external-typst] PROGRAM}
case $program in /*) ;; *) program=$PWD/$program ;; esac

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/home"
# guidedog runs with HOME and PATH only.
run() {
    if [ "$external" = yes ]; then
        "$program" "$@"
    else
        env -i HOME="$work/home" PATH="$work/bin" ${TMPDIR:+TMPDIR="$TMPDIR"} "$program" "$@"
    fi
}
fail() { echo "smoke test: $*" >&2; exit 1; }

version=$(run --version)
echo "$version"
if [ "$external" = no ]; then
    echo "$version" | grep -q '^typst 0\.15\.1 (embedded)' ||
        fail "$program does not embed Typst 0.15.1"
fi

cd "$work"
run quickstart docs -q -p Smoke -a Release
cat > docs/conf.toml <<'TOML'
project = "Smoke"
author = "Release"
extensions = ["sphinx.ext.graphviz", "sphinx.ext.mathjax", "myst_parser", "sphinx.ext.autodoc"]
autodoc_source_paths = ["."]
TOML
cat > docs/index.rst <<'RST'
Smoke
=====

.. toctree::

   notes

.. digraph:: flow

   source -> html
   source -> pdf

.. math::

   e^{i \pi} + 1 = 0

.. autofunction:: tool.run
RST
printf 'def run(x, y=1):\n    """Runs x."""\n' > docs/tool.py
# shellcheck disable=SC2016 # Markdown, not shell
printf '# Notes\n\nSome *Markdown*, and ``code``.\n' > docs/notes.md

run build html docs -W --color=never
run build pdf docs -W --color=never
html=docs/_build/html/index.html
pdf=docs/_build/pdf/smoke-en.pdf
[ -f "$html" ] || fail "no $html"
[ -f docs/_build/html/notes.html ] || fail "no notes.html from Markdown"
grep -q 'class="graphviz"' "$html" || fail "the diagram is missing from $html"
grep -q '<svg' "$html" || fail "no SVG in $html"
grep -q 'id="tool.run"' "$html" || fail "autodoc did not document tool.run"
[ -f "$pdf" ] || fail "no $pdf"
[ "$(head -c 5 "$pdf")" = "%PDF-" ] || fail "$pdf is not a PDF"
echo "smoke test passed: $(wc -c < "$pdf" | tr -d ' ') byte PDF and an HTML site"
