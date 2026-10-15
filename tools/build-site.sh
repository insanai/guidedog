#!/bin/sh
# Builds the Guidedog website with Guidedog and puts it together in one static folder:
#
#     /                       the landing page (docs/site); its editions in zh-cn/ ja/ ...
#     /manual/                the manual (docs/manual); its editions in manual/zh-cn/ ...
#     /manual/books/EDITION/  each edition's PDF book, guidedog-manual-LANGUAGE.pdf
#     /discussions/           the discussions (docs/gds), with every record's PDF in pdf/
#     /404.html, /.nojekyll   for GitHub Pages
#
# Every link is relative, so the folder works under any base path, such as /guidedog/.
# Only 404.html names the base path (html_context.site_base in docs/site/conf.toml),
# because GitHub Pages shows it at any depth.
#
# Usage: sh tools/build-site.sh [--guidedog PATH] [--out DIR] [--no-pdf]
#
#   --guidedog PATH   the guidedog to build with (or set GUIDEDOG). Without one, the script
#                     builds target/release/guidedog with Cargo, as the README describes.
#   --out DIR         where the site goes (default: build/site in the repository).
#   --no-pdf          leave out the PDF books, for a quick look; their links then fail.
#
# The CJK PDF books need the fonts that `sh tools/fetch-fonts.sh` fetches. A VERSION file at
# the repository root, when there is one, sets the version every page shows. The site is
# put together next to DIR and replaces it only when every build succeeds, so a failed run
# leaves the previous site in place; the intermediate folders are removed either way.
set -eu

guidedog=${GUIDEDOG:-}
out=
pdf=1
while [ $# -gt 0 ]; do
    case $1 in
    --guidedog | --out)
        if [ $# -lt 2 ]; then
            echo "error: $1 needs a value" >&2
            echo "hint: write a path after it, such as --out build/site" >&2
            exit 2
        fi
        if [ "$1" = --out ]; then out=$2; else guidedog=$2; fi
        shift 2
        ;;
    --no-pdf) pdf=0; shift ;;
    -h | --help) sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *)
        echo "error: unknown option $1" >&2
        echo "hint: sh tools/build-site.sh --help lists the options" >&2
        exit 2
        ;;
    esac
done

# Paths given are relative to where the script was started; the builds run from the
# repository's root.
absolute() { case $1 in /* | "") echo "$1" ;; *) echo "$PWD/$1" ;; esac; }
out=$(absolute "$out")
case $guidedog in
*/*) guidedog=$(absolute "$guidedog") ;;
?*) guidedog=$(command -v "$guidedog" || echo "$guidedog") ;;
esac
cd "$(dirname "$0")/.."
out=${out:-$PWD/build/site}

languages="en zh_CN ja ko hi es de"
work=$out.work
next=$out.next

# edition LANGUAGE: the folder of a language's edition, "" for English, else "ja/".
edition() {
    case $1 in
    en) echo "" ;;
    *) echo "$1/" | tr _ - | tr '[:upper:]' '[:lower:]' ;;
    esac
}

cleanup() { rm -rf "$work" "$next" "$out.old"; }
trap cleanup EXIT
trap 'exit 130' INT TERM
cleanup

started=$(date +%s)
step() {
    now=$(date +%s)
    echo "== $1 ($((now - started)) s)"
}

if [ -z "$guidedog" ]; then
    step "building guidedog"
    if ! command -v cargo >/dev/null 2>&1; then
        echo "error: cargo is not on PATH, and no guidedog was given" >&2
        echo "hint: install Rust with rustup, or pass --guidedog PATH (or set GUIDEDOG)" >&2
        exit 1
    fi
    cargo build --release -p guidedog-cmd
    guidedog=${CARGO_TARGET_DIR:-$PWD/target}/release/guidedog
fi
if [ ! -x "$guidedog" ]; then
    echo "error: $guidedog is not an executable file" >&2
    echo "hint: build it with cargo build --release -p guidedog-cmd" >&2
    exit 1
fi

set --
if [ -f VERSION ]; then
    release=$(tr -d ' \t\r\n' < VERSION)
    set -- -D "release=$release" -D "version=${release%.*}"
fi

build() { "$guidedog" build -q -W "$@"; }

mkdir -p "$work" "$next"
for lang in $languages; do
    ed=$(edition "$lang")
    up=${ed:+../}
    step "landing page ($lang)"
    links=".. _manual: ${up}manual/${ed}index.html
.. _manual-pdf: ${up}manual/books/${ed:-en/}guidedog-manual-$lang.pdf
.. _manual-tutorial: ${up}manual/${ed}tutorial/first-project.html
.. _discussions: ${up}discussions/index.html
.. _manual-discussions: ${up}manual/${ed}howto/discussions.html"
    build -b html docs/site "$work/site-$lang" -D "language=$lang" -D "rst_epilog=$links" "$@"
    # The landing page's search box searches the manual, so its own search page is unused,
    # and its pages use the web fonts (woff2) of the folder it copies, not the print ones.
    rm -f "$work/site-$lang/search.html" "$work/site-$lang/_static/searchindex.js" \
        "$work/site-$lang/_static/"*.otf
    if [ "$lang" = en ]; then
        cp -R "$work/site-$lang/." "$next/"
    else
        # GitHub Pages serves only the root's 404.html and .nojekyll.
        rm -f "$work/site-$lang/404.html" "$work/site-$lang/.nojekyll"
        mv "$work/site-$lang" "$next/${ed%/}"
    fi

    step "manual, website ($lang)"
    build -b html docs/manual "$work/manual-$lang" -j auto -D "language=$lang" \
        -D 'html_extra_path=[]' -D 'html_context.site_home=../' "$@"
    mkdir -p "$next/manual"
    if [ "$lang" = en ]; then
        cp -R "$work/manual-$lang/." "$next/manual/"
    else
        mv "$work/manual-$lang" "$next/manual/${ed%/}"
    fi

    if [ "$pdf" = 1 ]; then
        step "manual, PDF book ($lang)"
        build -b pdf docs/manual "$work/book-$lang" -D "language=$lang" \
            -D 'html_extra_path=[]' "$@"
        mkdir -p "$next/manual/books/${ed:-en/}"
        mv "$work/book-$lang/guidedog-manual-$lang.pdf" "$next/manual/books/${ed:-en/}"
    fi
done

step "discussions, website"
build -b html docs/gds "$work/gds" -j auto -D 'html_context.pdf_downloads=pdf/' \
    -D 'html_context.site_home=../' "$@"
cp -R "$work/gds/." "$next/discussions/"
if [ "$pdf" = 1 ]; then
    step "discussions, PDF"
    build -b pdf docs/gds "$work/gds-pdf" "$@"
    mkdir -p "$next/discussions/pdf"
    mv "$work/gds-pdf/"*.pdf "$next/discussions/pdf/"
fi

# Only the published files: no build state or hidden files besides the root's .nojekyll.
find "$next" -name '.*' ! -path "$next/.nojekyll" -prune -exec rm -rf {} +
if [ ! -f "$next/404.html" ] || [ ! -f "$next/.nojekyll" ]; then
    echo "error: the landing page build wrote no 404.html or .nojekyll" >&2
    echo "hint: check html_additional_pages and extensions in docs/site/conf.toml" >&2
    exit 1
fi

rm -rf "$out.old"
if [ -d "$out" ]; then mv "$out" "$out.old"; fi
mv "$next" "$out"
rm -rf "$out.old"

step "done: ${out#"$PWD"/}"
# Sizes as a download sees them (du counts a file with several links once).
for part in "$out" "$out/manual" "$out/manual/books" "$out/discussions"; do
    bytes=$(find "$part" -type f -exec cat {} + | wc -c)
    echo "${part#"$PWD"/}: $(awk -v b="$bytes" 'BEGIN { printf "%.1f", b / 1048576 }') MiB"
done
echo "files: $(find "$out" -type f | wc -l | tr -d ' ')"
