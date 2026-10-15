# Releasing Guidedog

Releases are built, checked, and published by GitHub Actions from a `v*` tag. Nothing is
uploaded by hand, and a published release is never changed.

## Workflows

- **Verify** (`.github/workflows/ci.yml`) runs on every push to `main` and every pull
  request, on Linux (x86-64) and macOS (Apple silicon): the source limits
  (`cargo run -p check`), `cargo build`, `cargo clippy` with warnings as errors,
  `cargo test --workspace`, a real isolated Linux Secret Service check, the example
  programs, and the smoke tests of the built programs. Changes to documentation only
  are skipped, apart from the files the tests read; the Pages workflow builds the
  documentation.
- **Release** (`.github/workflows/release.yml`) runs on a `v*` tag, or by hand as a
  candidate. Its jobs:
  - `version`: the tag, `VERSION`, the `[workspace.package]` version in `Cargo.toml`,
    and later each program's `--version` agree (`tools/release/check-version.sh`).
  - `qualify`: on linux-amd64 and macos-arm64, the source limits, clippy, every test
    suite, and the Linux Secret Service check. macOS tests use the Keychain and fail if
    it cannot be reached.
  - `build`: per platform, natively on that platform, `tools/release/build.sh` runs
    `cargo build --release --locked` for `guidedog` (with the embedded Typst compiler),
    `dsc`, `dscc`, `guidedog-pages`, and `graphdog`. Graphviz is linked from the
    runner's system libraries. The integration tests run again against the release
    build. `tools/release/package.sh` packages `guidedog`, `dsc`, `guidedog-pages`, and
    `graphdog` together, and `dscc` alone, each with its notices, `SOURCE.txt`, and a
    build manifest (`guidedog.build.json`, `dscc.build.json`).
  - `smoke`: each archive, unpacked on a fresh machine of its platform with only the
    Graphviz runtime installed, runs quickstart and builds HTML and PDF with an empty
    `PATH`; `dsc` keeps and builds a discussion catalog with the packaged `guidedog`,
    and `dscc` with nothing else (`tools/release/smoke-discussion.sh`); `guidedog-pages`
    registers and stops safely offline (`smoke-pages.sh`); `graphdog` renders a graph
    (`smoke-graphdog.sh`).
  - `source`: the source archive (`git archive` of the tagged commit).
  - `homebrew`: renders `guidedog.rb`, which depends on Homebrew's `graphviz` and
    installs `guidedog`, `dsc`, `guidedog-pages`, and `graphdog`, then installs and
    tests it from a local tap.
  - `publish` (tags only): writes `SHA256SUMS`, attests build provenance, creates a draft
    release with `.github/release-notes.md`, uploads every file, and publishes it as the
    latest release. It stops if the release is already published.
  - `refresh`: runs the Pages workflow, so the website names the new release.
- **Recover release publication** (`.github/workflows/release-recover.yml`) publishes the
  artifacts of an earlier tagged run whose only failure was publishing.

The scripts the workflows call are in `tools/release` and run locally too. The Typst
compiler is pinned in the root `Cargo.toml` (`typst = "=0.15.1"`), and `Cargo.lock` pins
every other crate; release builds use `--locked`.

| Platform | Rust target | Built and smoke-tested on |
|---|---|---|
| `linux-amd64` | `x86_64-unknown-linux-gnu` | `ubuntu-22.04` (glibc 2.35) |
| `linux-arm64` | `aarch64-unknown-linux-gnu` | `ubuntu-22.04-arm` |
| `macos-arm64` | `aarch64-apple-darwin` | `macos-15` |
| `macos-amd64` | `x86_64-apple-darwin` | `macos-15-intel` |

Each archive is built natively, because the programs link the Graphviz of the platform
they are built on. Windows has no release archive yet.

## Build a release locally

```sh
tools/release/build.sh                  # build/guidedog, dsc, dscc, guidedog-pages, graphdog
tools/release/check-version.sh build/guidedog
tools/release/smoke.sh build/guidedog
tools/release/package.sh linux-amd64    # dist/guidedog-linux-amd64.tar.gz, dist/dscc-...
tools/release/checksums.sh dist         # dist/SHA256SUMS
```

`package.sh` refuses a working tree with uncommitted changes, so an archive always
matches its commit.

## Cut a release

1. Update `VERSION` and the `version` in `[workspace.package]` of the root `Cargo.toml` together (every crate and program inherits it), and the `version` of each reusable crate's entry in `[workspace.dependencies]` (the dependency gate in `tests/integration` checks them). Run `cargo build` so `Cargo.lock` follows, and update the release setting in each documentation project's `conf.toml`. `tools/release/check-version.sh` checks that they agree.
2. Add the release to `CHANGELOG.md`, and write `.github/release-notes.md`: what changed,
   the downloads, how to verify and install, and the platform notes.
3. Push to `main` and wait for Verify to pass.
4. Run Release by hand on `main` (Actions, Release, Run workflow, platform `all`). This
   candidate run builds and tests everything but publishes nothing. Download its
   artifacts and try them if you like.
5. Tag the same commit and push the tag:

   ```sh
   git tag -a v0.7.0 -m "Guidedog 0.7.0"
   git push origin v0.7.0
   ```

6. The tagged run publishes the release when every job passes. Check the release page,
   the attached `SHA256SUMS`, and the website.

7. After publication, update `GUIDEDOG_VERSION` and `GUIDEDOG_SHA256` in
   `docs/manual/howto/github-pages.rst` from the published Linux amd64 archive's
   verified checksum. The workflow block is shared by every manual language.
   Do this after publication, when the final archive hash exists; do not guess it
   from a candidate. Commit the documentation update and wait for Website to pass.
   Run the official tap's Update workflow and check that its tested formula names
   the new version. Record the release checks in the manual's evidence directory.

A failing build or test job means the tag cannot be released as it is. Fix the problem
on `main` and release the next patch version; do not move a pushed tag.

## Recover a failed publication

If every job of the tagged run passed except `publish` or `refresh` (a GitHub outage, a
permissions problem), publish the files that run already built:

1. Fix the cause, if it is in the repository settings.
2. Run **Recover release publication** with the tag (`v0.7.0`) and the ID of the tagged
   run (the number in its URL).

It checks with `tools/release/check-run.sh` that the run built that tag at the commit the
tag names and that every other job succeeded, then publishes that run's artifacts. A
draft left by the failed attempt is resumed; a published release is left alone.
Re-running only the failed jobs of the original run works too.

## Homebrew tap

Each release carries `guidedog.rb`, a formula that installs the release archive for the
machine and depends on Homebrew's `graphviz`. The official tap is
[insanai/homebrew-guidedog](https://github.com/insanai/homebrew-guidedog):

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
```

The tap's Update workflow checks the latest stable release daily and can be run
manually after publication. It verifies the formula and checksum manifest against
GitHub's asset digests, checks all four archive URLs and hashes, and installs and
tests the candidate on Apple silicon before committing it. The tap also tests
formula changes on Apple silicon and Intel macOS. The release workflow remains the
source of the packages and formula; the tap does not rebuild the engine or change
published assets. No cross-repository write token is needed.
