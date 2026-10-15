#!/bin/sh
# Checks that an earlier release workflow run may be published by
# .github/workflows/release-recover.yml: it ran release.yml for the push of TAG, at the
# commit TAG names, has finished, and every job but publishing succeeded.
#
#     GH_TOKEN=... GITHUB_REPOSITORY=insanai/guidedog tools/release/check-run.sh RUN TAG
#
# Needs gh and jq.
set -eu
run=${1:?Usage: tools/release/check-run.sh RUN_ID TAG}
tag=${2:?Usage: tools/release/check-run.sh RUN_ID TAG}
repo=${GITHUB_REPOSITORY:?GITHUB_REPOSITORY names the repository, such as insanai/guidedog}
fail() { echo "Run $run cannot be published: $*" >&2; exit 1; }

info=$(gh api "repos/$repo/actions/runs/$run")
field() { echo "$info" | jq -r "$1"; }
[ "$(field .path)" = .github/workflows/release.yml ] ||
    fail "it ran $(field .path), not .github/workflows/release.yml."
[ "$(field .event)" = push ] && [ "$(field .head_branch)" = "$tag" ] ||
    fail "it ran for $(field .event) of $(field .head_branch), not for pushing $tag."
commit=$(gh api "repos/$repo/commits/$tag" --jq .sha)
[ "$(field .head_sha)" = "$commit" ] ||
    fail "it built $(field .head_sha), but $tag names $commit."
[ "$(field .status)" = completed ] || fail "it is still $(field .status); wait for it."

jobs=$(gh api --paginate "repos/$repo/actions/runs/$run/jobs" \
    --jq '.jobs[] | [.name, (.conclusion // "none")] | @tsv')
echo "$jobs"
for gate in version qualify build smoke source homebrew; do
    echo "$jobs" | cut -f1 | grep -Eq "^$gate( \(|$)" || fail "it has no $gate job."
done
bad=$(echo "$jobs" | grep -Ev '^(publish|refresh)' | awk -F '\t' '$2 != "success"' || true)
[ -z "$bad" ] || fail "these jobs did not succeed:
$bad"
echo "Run $run built $tag at $commit and passed every gate."
