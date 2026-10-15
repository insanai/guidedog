#!/bin/sh
# Smoke-tests guidedog-pages and guidedog publish the way a new user starts, offline: a
# pages.toml, the program registered with guidedog, and publications that stop before
# reaching GitHub because the target is not yet authorized on this machine.
#
#     tools/release/smoke-pages.sh PAGES-PROGRAM GUIDEDOG
#
# Machine state goes to a temporary folder (GUIDEDOG_CONFIG_DIR, GUIDEDOG_CACHE_DIR), so
# the user's registry, bindings, and tokens are never touched.
set -eu
program=${1:?Usage: tools/release/smoke-pages.sh PAGES-PROGRAM GUIDEDOG}
guidedog=${2:?Usage: tools/release/smoke-pages.sh PAGES-PROGRAM GUIDEDOG}
case $program in /*) ;; *) program=$PWD/$program ;; esac
case $guidedog in /*) ;; *) guidedog=$PWD/$guidedog ;; esac
registered=$program
# guidedog registers a program by its absolute path, which on Windows is a Windows path.
if command -v cygpath >/dev/null 2>&1; then registered=$(cygpath -w "$program"); fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export GUIDEDOG_CONFIG_DIR="$work/state" GUIDEDOG_CACHE_DIR="$work/cache"
fail() { echo "pages smoke test: $*" >&2; exit 1; }
# expect CODE COMMAND...: the command fails with the problem CODE, reported as JSON.
expect() {
    code=$1
    shift
    if "$@" --diagnostics=json >"$work/out" 2>"$work/err"; then
        fail "$* succeeded; it should stop with $code"
    fi
    if ! grep -q "\"code\":\"$code\"" "$work/err"; then
        cat "$work/err" >&2
        fail "$* did not report $code"
    fi
}

version=$("$program" --version | head -n 1)
case $version in "guidedog-pages "*) ;; *) fail "$program reports \"$version\"" ;; esac
cd "$work"
"$guidedog" quickstart docs -q -p Demo -a Release >/dev/null
"$program" init --repository=octo/manual --project=docs --color=never
grep -q 'repository = "octo/manual"' pages.toml || fail "init wrote no repository"
expect pages.usage "$program" publish --no-such-option
expect pages.unbound "$program" publish --dry-run --guidedog="$guidedog"
"$guidedog" plugin register pages "$registered" --yes
"$guidedog" plugin inspect pages | grep -q 'is the one registered' || fail "inspect failed"
expect pages.unbound "$guidedog" publish --publisher=pages --config=pages.toml --dry-run
"$guidedog" plugin unregister pages
expect cli.plugin-missing "$guidedog" publish --publisher=pages --config=pages.toml
echo "pages smoke test passed: $(basename "$program") registered; each publication stopped safely"
