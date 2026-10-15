#!/bin/sh
# Smoke-tests the discussion programs the way a new user runs them: a new catalog, a
# draft, a promotion, a decision, and its HTML site and PDF books.
#
#     tools/release/smoke-discussion.sh dsc-PROGRAM GUIDEDOG   dsc with that guidedog
#     tools/release/smoke-discussion.sh dscc-PROGRAM           dscc with nothing else
#
# Each program runs with an empty PATH: dsc builds with the guidedog it is given, which
# carries what it needs, and dscc with the Guidedog built into it.
set -eu
program=${1:?Usage: tools/release/smoke-discussion.sh PROGRAM [GUIDEDOG]}
guidedog=${2:-}
case $program in /*) ;; *) program=$PWD/$program ;; esac
case $guidedog in /* | "") ;; *) guidedog=$PWD/$guidedog ;; esac
# dsc starts guidedog itself, so on Windows it needs a Windows path, not Git Bash's.
if [ -n "$guidedog" ] && command -v cygpath >/dev/null 2>&1; then
    guidedog=$(cygpath -w "$guidedog")
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/home"
run() {
    env -i HOME="$work/home" PATH="$work/bin" ${SYSTEMROOT:+SYSTEMROOT="$SYSTEMROOT"} \
        ${WINDIR:+WINDIR="$WINDIR"} ${USERPROFILE:+USERPROFILE="$USERPROFILE"} \
        ${LOCALAPPDATA:+LOCALAPPDATA="$LOCALAPPDATA"} ${APPDATA:+APPDATA="$APPDATA"} \
        ${TEMP:+TEMP="$TEMP"} ${TMP:+TMP="$TMP"} "$program" "$@"
}
fail() { echo "discussion smoke test: $*" >&2; exit 1; }

run --version
engine=""
if [ -n "$guidedog" ]; then
    engine="--guidedog=$guidedog"
else
    run --version | grep -q 'Typst: built in' || fail "$program has no Typst built in"
fi

cd "$work"
run init eds --prefix=eds --name="Smoke Discussions" --author=Release
cd eds
run new "Cache policy" --labels=architecture
run promote cache-policy
run state 1 --to=accepted --reason="Reviewed in the release smoke test"
run check
# shellcheck disable=SC2086 # engine is one option or none
run build $engine --color=never
html=_build/html/records/0001-cache-policy.html
pdf=_build/pdf/eds-0001-cache-policy-en.pdf
[ -f "$html" ] || fail "no $html"
grep -q 'Authors' "$html" || fail "$html does not show the metadata as Authors"
[ -f "$pdf" ] || fail "no $pdf"
[ "$(head -c 5 "$pdf")" = "%PDF-" ] || fail "$pdf is not a PDF"
if [ -n "$guidedog" ]; then
    if run build --guidedog="$work/no-guidedog" 2>/dev/null; then
        fail "dsc built without a guidedog"
    fi
fi
echo "discussion smoke test passed: $(basename "$program"), an HTML site and PDF books"
