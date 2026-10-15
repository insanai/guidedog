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
cargo xtask sphinxdiff S_HTML G_HTML -v  # compare a Sphinx and a Guidedog build
cargo xtask sphinxdiff-roots SPHINX_BUILD ROOTS WORK [RECORD]  # Sphinx's test roots
cargo xtask rstref [--ids] DOCUTILS CORPUS        # store the Docutils reference trees
cargo xtask verify-docutils DOCUTILS WORK [RECORD] # recheck them byte for byte
```

| Module | Does |
|---|---|
| `site` | the landing page, manual (7 languages, PDF books) and discussions in one folder |
| `fonts` | downloads with the installed `curl`, checks SHA-256 with gd-sha |
| `release/*` | the release workflow's steps and smoke tests |
| `tables/unicode` | case folding, id folding, NFD, categories, rST punctuation |
| `tables/entities` | HTML5 entities from WHATWG `entities.json` |
| `tables/styles`, `tables/python` | Pygments and Sphinx styles, read from their Python as data |
| `sphinxdiff/*` | Sphinx vs Guidedog HTML: pages, ids, links, numbers, numrefs, objects.inv, warnings |
| `docutils/*` | the RST corpus's Docutils trees and ids.txt, and their verification record |
| `evidence` | date, host, revision and program version for verification records |

External programs run through `std::process::Command` with argument vectors; failures
are gd-report reports with a hint (exit 1, usage problems exit 2).

Notes:

- `release check-run` parses GitHub's answers with gd-json; checksums are computed with
  gd-sha, and `publish` verifies `SHA256SUMS` itself.
- `release credential-store` runs itself again inside `dbus-run-session`.
- `release publish --dry-run` and `release build --dry-run` print the commands without
  running them.
- The smoke tests pass the same set of variables (TMPDIR and the Windows ones) to every
  program; Rust's paths are already native, so no path conversion is needed.
- `tables` checks `tools/data/SHA256SUMS` before it reads the data.
- `sphinxdiff-roots` runs the comparison in process for each root (no second binary);
  `rstref` refuses a setting `corpus.txt` names that it does not know instead of skipping
  it; `verify-docutils` refuses a work folder that holds the repository, since it empties
  it first.
