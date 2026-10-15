#!/bin/sh
# Runs the native credential adapter of gd-pages against an isolated, real Linux Secret
# Service (gnome-keyring through secret-tool). No GitHub token is used, and no existing
# desktop keyring is opened or changed. Needs dbus-run-session, gnome-keyring, and
# libsecret-tools.
set -eu
cd "$(dirname "$0")/../.."
root=$(mktemp -d)
trap 'rm -rf "$root"' EXIT HUP INT TERM
export XDG_DATA_HOME="$root/data" XDG_CONFIG_HOME="$root/config"
export XDG_CACHE_HOME="$root/cache" XDG_RUNTIME_DIR="$root/run"
mkdir -m 700 "$XDG_RUNTIME_DIR"
# GUIDEDOG_TEST_REQUIRE_STORE turns an unreachable store into a test failure.
dbus-run-session -- sh -eu -c '
    printf "%s\n" guidedog-fixture-password |
        gnome-keyring-daemon --unlock --components=secrets >/dev/null
    GUIDEDOG_TEST_REQUIRE_STORE=1 cargo test --locked -p gd-pages --lib the_credential_store_keeps_tokens
'
