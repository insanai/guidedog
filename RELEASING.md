# Releasing Guidedog

Releases are built, checked, and published by GitHub Actions from a `v*` tag. Nothing is
uploaded by hand, and a published release is never changed.

## Workflows

- **Verify** (`.github/workflows/ci.yml`) runs on every push to `main` and every pull
  request, on Linux (x86-64) and macOS (Apple silicon): the source limits
  (`cargo run -p check`), the data tables (`cargo xtask tables --check`), `cargo build`, `cargo clippy` with warnings as errors,
  `cargo test --workspace`, a real isolated Linux Secret Service check, the example
  programs, and the smoke tests of the built programs. Changes to documentation only
  are skipped, apart from the files the tests read; the Pages workflow builds the
  documentation.
- **Release** (`.github/workflows/release.yml`) runs on a `v*` tag, or by hand as a
  candidate. Its jobs:
  - `version`: the tag, `VERSION`, the `[workspace.package]` version in `Cargo.toml`,
    and later each program's `--version` agree (`cargo xtask release check-version`).
  - `qualify`: on linux-amd64 and macos-arm64, the source limits, the data tables
    (`cargo xtask tables --check`), clippy, every test
    suite, and the Linux Secret Service check. macOS tests use the Keychain and fail if
    it cannot be reached.
  - `build`: per platform, natively on that platform, `cargo xtask release build` runs
    `cargo build --release --locked` for `guidedog`, with the embedded Typst compiler and linked Pages and discussion plugins. Graphviz 16.1.0 is linked
    statically: `cargo xtask graphviz` builds it (cached between runs) and the build
    sets `GUIDEDOG_GRAPHVIZ_STATIC`. The integration tests run again against the release
    build. `cargo xtask release package` packages `guidedog` with its linked libraries and plugins, with their notices, `SOURCE.txt`, and a build manifest
    (`guidedog.build.json`).
  - `smoke`: each archive, unpacked on a fresh machine of its platform with nothing
    installed, runs quickstart and builds HTML and PDF with an empty
    `PATH`; `guidedog dsc` keeps and builds a discussion catalog
    (`cargo xtask release smoke-discussion`); the linked Pages plugin initializes and stops safely offline (`smoke-pages`). Graphviz is exercised by the HTML and PDF diagram builds.
  - `source`: the source archive (`git archive` of the tagged commit).
  - `homebrew`: renders `guidedog.rb`, which has no dependencies and
    installs `guidedog`, then installs and
    tests it from a local tap.
  - `publish` (tags only): writes `SHA256SUMS`, attests build provenance, creates a draft
    release with `.github/release-notes.md`, uploads every file, and publishes it as a prerelease when the version has a suffix, or as the latest stable release otherwise. It stops if the release is already published.
  - `refresh`: runs the Pages workflow, so the website names the new release.
- **Recover release publication** (`.github/workflows/release-recover.yml`) publishes the
  artifacts of an earlier tagged run whose only failure was publishing.

The steps the workflows run are `cargo xtask release <step>` (tools/xtask, Rust with no
shell scripts) and run locally too; `cargo xtask release --help` lists them. The Rust compiler is pinned in `rust-toolchain.toml` (1.99.0). The Typst
compiler is pinned in the root `Cargo.toml` (`typst = "=0.15.1"`), and `Cargo.lock` pins
every other crate; release builds use `--locked`.

| Platform | Rust target | Built and smoke-tested on |
|---|---|---|
| `linux-amd64` | `x86_64-unknown-linux-gnu` | `ubuntu-22.04` (glibc 2.35) |
| `linux-arm64` | `aarch64-unknown-linux-gnu` | `ubuntu-22.04-arm` |
| `macos-arm64` | `aarch64-apple-darwin` | `macos-15` |
| `macos-amd64` | `x86_64-apple-darwin` | `macos-15-intel` |
| `windows-amd64` | `x86_64-pc-windows-msvc` | `windows-2022` |

Each archive is built natively, with the static Graphviz built on that platform
(`cargo xtask graphviz`). The archives carry Graphviz's and Expat's licences
(`COPYING-graphviz`, `COPYING-expat`), and `SOURCE.txt` names their source tarballs with
their SHA-256. Windows ships a ZIP with the official Graphviz 16.1.0 DLLs and configuration beside the three executables. The prepare action verifies the upstream archive SHA-256. A fresh Windows runner extracts the ZIP, checks the programs against the manifest, and runs HTML, PDF, discussion, Pages and graph rendering smoke tests.

## Build a release locally

```sh
cargo xtask release build                   # build/guidedog with libraries and plugins
cargo xtask release check-version build/guidedog
cargo xtask release smoke build/guidedog
cargo xtask release package linux-amd64    # dist/guidedog-linux-amd64.tar.gz
cargo xtask release checksums dist         # dist/SHA256SUMS
```

`release package` refuses a working tree with uncommitted changes, so an archive always
matches its commit.

## Plugin protocol compatibility

Before a release that touches the plugin protocol or the publisher SDK, qualify protocol
1.0 against the released 0.6.0 host and SDK in both directions:

```sh
cargo xtask release plugin-compatibility [--work DIR]
```

It downloads the 0.6.0 Linux x86-64 (or Apple silicon) archive and source archive,
checks each against its pinned SHA-256, and builds this checkout's `guidedog`, a test-only Pages process adapter and the `gd-plugins` publisher and build-plugin examples. The 0.6.0
`guidedog` then registers, inspects and dry-runs the new Pages process adapter and publishes
with the new folder publisher (dry run, first, unchanged and changed publication); it must
refuse the build-plugin example cleanly. The new `guidedog` does the same with the 0.6.0
`guidedog-pages` and the 0.6.0 folder publisher example (built from its source archive when
that toolchain is installed). Everything after the download runs offline. 0.6.0 has no build plugins and no
release ships a build-plugin program, so the new-host direction covers publishers only.
It prints a record of versions, pins and results, and exits 1 on any incompatibility.

## Cut a release

1. Update `VERSION` and the `version` in `[workspace.package]` of the root `Cargo.toml` together (every crate and program inherits it), and the `version` of each reusable crate's entry in `[workspace.dependencies]` (the dependency gate in `tests/integration` checks them). Run `cargo build` so `Cargo.lock` follows, and update the release setting in each documentation project's `conf.toml`. `cargo xtask release check-version` checks that they agree.
2. Add the release to `CHANGELOG.md`, and write `.github/release-notes.md`: what changed,
   the downloads, how to verify and install, and the platform notes.
3. Push to `main` and wait for Verify to pass.
4. Run Release by hand on `main` (Actions, Release, Run workflow, platform `all`). This
   candidate run builds and tests everything but publishes nothing. Download its
   artifacts and try them if you like.
5. Tag the same commit and push the tag:

   ```sh
   git tag -a v1.0.0-beta.1 -m "Guidedog 1.0.0-beta.1"
   git push origin v1.0.0-beta.1
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
2. Run **Recover release publication** with the tag (`v1.0.0-beta.1`) and the ID of the tagged
   run (the number in its URL).

It checks with `cargo xtask release check-run` that the run built that tag at the commit the
tag names and that every other job succeeded, then publishes that run's artifacts. A
draft left by the failed attempt is resumed; a published release is left alone.
Re-running only the failed jobs of the original run works too.

## Homebrew tap

Each release carries `guidedog.rb`, a formula that installs the release archive for the
machine; Graphviz is built in, so it has no dependencies. The official tap is
[insanai/homebrew-guidedog](https://github.com/insanai/homebrew-guidedog):

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
```

The tap's Update workflow checks published releases every hour, including prereleases, and selects the greatest version. We can also run it manually after publication with an explicit tag. It verifies the formula and checksum manifest against GitHub's asset digests, checks all four archive URLs and hashes, and installs and tests the candidate on Apple silicon and Intel macOS before committing it. The release workflow remains the source of the packages and formula; the tap does not rebuild the engine or change published assets. No cross-repository write token is needed.

Beta releases use GitHub's prerelease flag. Scheduled tap updates include `v1.0.0-beta.1` and later prereleases, then follow the stable release when its version is greater. The updater rejects downgrades.
