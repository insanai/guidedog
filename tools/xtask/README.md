# xtask cheat sheet

Guidedog's repository tasks, in Rust (no shell or Python scripts). `.cargo/config.toml`
aliases `cargo xtask` to `cargo run --quiet --package xtask --`.

```sh
cargo xtask --help                       # every task
cargo xtask tables --check               # generated tables match tools/data (CI)
cargo xtask tables                       # regenerate them
cargo xtask fetch-fonts                  # Noto Serif CJK into docs/shared/fonts
cargo xtask build-site [--no-pdf]        # the website into build/site
cargo xtask release --help               # build, package, source, checksums, formula,
                                         # publish, check-version, check-run,
                                         # credential-store, smoke, smoke-*
```

| Module | Does |
|---|---|
| `site` | the landing page, manual (7 languages, PDF books) and discussions in one folder |
| `fonts` | downloads with the installed `curl`, checks SHA-256 with gd-sha |
| `release/*` | the release workflow's steps and smoke tests |
| `tables/unicode` | case folding, id folding, NFD, categories, rST punctuation |
| `tables/entities` | HTML5 entities from WHATWG `entities.json` |
| `tables/styles`, `tables/python` | Pygments and Sphinx styles, read from their Python as data |

External programs run through `std::process::Command` with argument vectors; failures
are gd-report reports with a hint (exit 1, usage problems exit 2).

Changes from the shell scripts they replace:

- `release check-run` parses GitHub's answers with gd-json instead of `jq`; checksums are
  computed with gd-sha instead of `sha256sum`/`shasum`, and `publish` verifies
  `SHA256SUMS` itself.
- `release credential-store` runs itself again inside `dbus-run-session` instead of an
  inline `sh -c` script.
- `release publish --dry-run` and `release build --dry-run` print the commands without
  running them.
- The smoke tests keep the same set of variables (TMPDIR and the Windows ones) for every
  program; `cygpath` is gone, since Rust's paths are already native.
- `build.json`'s `requires` no longer ends with a stray space.
- `tables` checks `tools/data/SHA256SUMS` before it reads the data.
