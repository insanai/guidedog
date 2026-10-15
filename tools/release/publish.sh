#!/bin/sh
# Publishes the release files in DIR as the GitHub release for TAG, never replacing a
# published release:
#
#     GH_TOKEN=... GITHUB_REPOSITORY=insanai/guidedog tools/release/publish.sh v0.2.0 dist
#
# It checks DIR/SHA256SUMS against the files, creates a draft release with the notes in
# .github/release-notes.md (or resumes an existing draft, replacing its files), uploads
# every file, and then publishes the draft as the latest release. A release that is
# already published is left untouched and the script fails, so assets never change
# after people may have downloaded them.
#
#     tools/release/publish.sh --check v0.2.0      only fail if v0.2.0 is published
set -eu
check_only=no
if [ "${1:-}" = --check ]; then check_only=yes; shift; fi
tag=${1:?Usage: tools/release/publish.sh [--check] TAG DIR}
root=$(cd "$(dirname "$0")/../.." && pwd)
repo=${GITHUB_REPOSITORY:?GITHUB_REPOSITORY names the repository, such as insanai/guidedog}
version=$("$root/tools/release/check-version.sh")
if [ "$tag" != "v$version" ]; then
    echo "The tag $tag does not match VERSION $version." >&2
    exit 1
fi

# release_state prints draft, published or missing; anything else stops the script.
release_state() {
    errors=$(mktemp)
    if draft=$(gh release view "$tag" --repo "$repo" --json isDraft --jq .isDraft \
        2> "$errors"); then
        if [ "$draft" = true ]; then echo draft; else echo published; fi
    elif grep -q 'release not found' "$errors"; then
        echo missing
    else
        cat "$errors" >&2
        echo "Cannot tell whether $tag is published, so nothing was uploaded." >&2
        rm -f "$errors"
        return 1
    fi
    rm -f "$errors"
}

state=$(release_state)
if [ "$state" = published ]; then
    echo "The release $tag is already published; its files are not replaced." >&2
    echo "Publish a new version instead." >&2
    exit 1
fi
[ "$check_only" = yes ] && { echo "The release $tag is not published ($state)."; exit 0; }

dir=${2:?Usage: tools/release/publish.sh [--check] TAG DIR}
(
    cd "$dir"
    if command -v sha256sum >/dev/null 2>&1; then sha256sum -c --quiet SHA256SUMS
    else shasum -a 256 -c --quiet SHA256SUMS; fi
)
if [ "$state" = draft ]; then
    echo "Resuming the draft release $tag."
else
    gh release create "$tag" --repo "$repo" --verify-tag --draft \
        --title "Guidedog $version" --notes-file "$root/.github/release-notes.md"
fi

set --
for file in "$dir"/*; do set -- "$@" "$file"; done
gh release upload "$tag" --repo "$repo" --clobber "$@"
gh release edit "$tag" --repo "$repo" --draft=false --latest
echo "Published $tag with $# files."
