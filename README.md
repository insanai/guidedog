# Guidedog

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Deutsch](README.de.md)

**Write once. Explain well. Publish on the web and on paper.**

Guidedog builds HTML documentation sites and PDF books from reStructuredText and Markdown. It is written in Rust. Its conversion engine is **Guidedoc**, and Typst, embedded in the program, typesets the PDF pages. No Python environment or LaTeX installation is needed.

Sphinx inspires the authoring workflow. Guidedog is an independent system: configuration is declarative TOML, API extraction is static, and Python configuration and extensions are never executed. Complete Sphinx compatibility is not a goal.

## Start a project

```sh
guidedog quickstart docs
guidedog build html docs
guidedog build pdf docs
guidedog serve docs
```

The project contains `conf.toml`, `index.rst`, `_templates/layout.html`, `_templates/book.typ`, and `_static/`. Templates are ordinary files that we can edit. HTML provides navigation, search, code highlighting, diagrams, and equations. PDF books provide chapters, cross-references, figures, indexes, and language-aware typesetting.

The default theme is light. Readers can choose dark or the system theme. The manual detects the browser language on its landing page and repository overview. A saved choice takes precedence; direct links to an edition or chapter remain explicit requests.

```sh
guidedog convert notes.md --to pdf --output notes.pdf
guidedog clean docs
guidedog migrate conf.py
```

API documentation is read from source without running it: Python (`autodoc`), Odin (`odindoc`), and, new in 0.7, Rust crates (the `rust` domain and the `rust_autoapi_*` settings).

## Install

Each [release](https://github.com/insanai/guidedog/releases) has an archive for Linux (x86-64 and ARM64) and macOS (Apple silicon and Intel), plus a Windows x86-64 ZIP. The `guidedog` archive holds `guidedog`, `guidedog-pages`, and `graphdog`. The Typst compiler is embedded, tree-sitter is compiled in, and Graphviz is linked statically, so nothing else needs to be installed. The Windows ZIP bundles Graphviz DLLs; we keep the whole extracted folder together. Linux needs glibc 2.35 or later; macOS needs version 12 or later. We download the archive for our system and `SHA256SUMS`, then check and install it:

```sh
sha256sum -c --ignore-missing SHA256SUMS      # on macOS: shasum -a 256 -c --ignore-missing SHA256SUMS
tar -xzf guidedog-linux-amd64.tar.gz
sudo install guidedog-linux-amd64/guidedog \
    guidedog-linux-amd64/guidedog-pages guidedog-linux-amd64/graphdog /usr/local/bin/
```

On macOS, install from the official [Guidedog Homebrew tap](https://github.com/insanai/homebrew-guidedog). Homebrew selects the Apple silicon or Intel archive. The tap also supports Linux:

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
guidedog --version
```

The programs are not code-signed: on macOS, remove the quarantine from a browser download with `xattr -d com.apple.quarantine guidedog`. The release notes describe each platform's limits.

## Build from source

Install Rust 1.99 or later (with [rustup](https://rustup.rs)), a C compiler (tree-sitter's grammars are C), and the Graphviz development files: `libgraphviz-dev` on Debian and Ubuntu, `graphviz-devel` on Fedora, `graphviz` from Homebrew on macOS. (Or build Graphviz statically, as the releases do: `cargo xtask graphviz`, then build with `GUIDEDOG_GRAPHVIZ_STATIC=target/graphviz/16.1.0`; the programs then need no Graphviz at run time.) Then build the workspace:

```sh
cargo build --release -p guidedog-cmd -p guidedog-pages-cmd -p graphdog-cmd
target/release/guidedog --version
```

The programs are in `target/release`. On macOS with Homebrew, point the linker at Graphviz first: `export LIBRARY_PATH="$(brew --prefix graphviz)/lib"`. The Typst compiler is embedded by default (the `embedded` feature of `gd-typst`); without it, Guidedog runs an installed `typst` 0.15.1 for PDF. On Linux, the embedded compiler's package downloader links a vendored OpenSSL, which needs `perl` and `make` to build.

Before sending a change, run what CI runs:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p check              # source limits: function and file length, line width
```

[RELEASING.md](RELEASING.md) describes the release workflow.

## Translate with Guidedog

```sh
mkdir -p docs/manual/_editions
guidedog build -b gettext docs/manual docs/manual/locales/templates -W
guidedog intl update -p docs/manual/locales/templates \
    -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog intl stat -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog build -b html docs/manual docs/manual/_editions/de \
    -D language=de -D 'html_extra_path=[]' -W
```

Translate and review `msgstr` values in the PO files. Guidedog extracts POT files, merges catalogs, reports progress, and builds each edition. It does not invent translations. Commands, paths, code, API names, and reference targets stay unchanged. Only the manual and repository overview are translated; API and GDS remain English. `cargo xtask build-site` assembles all editions, their PDF books, and the discussions into the website.

GitHub readers choose a README language using the links above. Automatic language selection belongs to the generated documentation website.

## Publish to GitHub Pages

`guidedog-pages` publishes a site from our own machine. It builds the project with the installed `guidedog` and sends the whole site to a branch of its own in one Git push. GitHub applies the push only if the branch still has the commit that was read. Then `guidedog-pages` waits until GitHub Pages has deployed that commit. Tokens stay in the system's credential store, and a binding on our machine decides where a profile may publish.

```sh
guidedog-pages init --repository=octo/manual --project=docs
guidedog-pages auth import          # a fine-grained token, read without echo
guidedog-pages bind                 # review the target, then allow this machine
guidedog-pages publish
guidedog-pages setup                # deploy GitHub Pages from the branch
guidedog-pages watch                # publish again whenever the sources change
```

The same guided flow is part of `guidedog` itself: the GitHub Pages plugin is linked in and adds `guidedog publish pages` (built with Guidedog's own engine) and `guidedog pages workflow`. GitHub Actions is the other route, for publishing after every push. `guidedog publish --publisher=pages` runs the same publisher. See [Publish to GitHub Pages](docs/manual/howto/github-pages.rst) and [Write a plugin](docs/manual/plugins.rst).

Build plugins transform source text, complete HTML pages, and static assets through a typed, versioned contract. A plugin is an ordinary Rust crate. We link it into our own `guidedog` with `gd_cli::run_with`, or we run it as a separate program that `guidedog plugin register` pins by SHA-256. A plugin can also add commands to `guidedog` itself, listed by `guidedog --help`. `guidedog publish --timeout=SECONDS` bounds each complete exchange with a publisher (default: 300 seconds); choose a value longer than `--wait`. After an unknown outcome, recover before publishing again.

## Keep discussions

`guidedog dsc` keeps proposals and decisions as reStructuredText records, one file each, and builds them into a website and PDF books. Each catalog has a prefix: Guidedog's design records use `gds`, and another project might use `eds`.

```sh
guidedog dsc init docs/eds --prefix=eds --name="Enso Discussions"
cd docs/eds
guidedog dsc new "Cache policy" --labels=architecture
guidedog dsc promote cache-policy
guidedog dsc state 1 --to=accepted --reason="Review converged"
guidedog dsc build
```

A catalog's `discussion.toml` sets its lifecycle states, record types, and numbering, and `guidedog dsc config` prints it. The commands come from the discussion plugin, which every `guidedog` links, so nothing else needs to be installed. The Windows ZIP bundles Graphviz DLLs; we keep the whole extracted folder together. Guidedog's own `docs/gds` is one such catalog: we run `guidedog dsc check` in that folder. See [Keep a discussion catalog](docs/manual/howto/discussions.rst).

`graphdog` renders Graphviz DOT to SVG, PNG, or laid-out DOT with the same library Guidedog's diagrams use, as a subset of the `dot` command: `echo 'digraph { a -> b }' | graphdog -Tsvg -o graph.svg`.

## Read and embed

- [Manual](docs/manual/index.rst): tutorials, topics, practical tasks, reference, and internals.
- [API reference](docs/api/index.rst): generated from the Rust sources by Guidedog itself and included in the manual's PDF appendix. `cargo doc --workspace --no-deps` builds the rustdoc pages too.
- [Engine guide](crates/guidedoc/README.md): workspaces, requests, adapters, and passes; [examples](examples/README.md) are small programs we can run with `cargo run`.
- [Templates](docs/manual/templates.rst): customize HTML with Jinja and books with Typst.
- [GDS](docs/gds/README.md): numbered design records and implementation decisions.
- [Discussion library](crates/gd-discussion/README.md): the catalogs that `guidedog dsc` keeps.
- [Plugin library](crates/gd-plugins/README.md): typed build services and the publisher interface used by `guidedog publish` and `guidedog-pages`.
- [Reusable crates](docs/manual/internals/crates.rst): the engine, libraries, API generators, and plugin SDK that other Rust projects can take from crates.io, and how they depend on each other.

Each crate under `crates/` has a short README with its API and its design decisions. Guidedoc owns its document tables and reuses them between conversions, so a conversion in steady state allocates nothing. It returns structured diagnostics with a hint. The host manages files, budgets, caching, publication, and the embedded compilers. `--untrusted` restricts capabilities; it is not a process sandbox or a limit on all native resource use. The beta scope is trusted documentation projects on macOS and Linux.

## Credits and license

Created by **Vikrant Rathore**, with assistance from **Ronak Rathore** and **Kanak Rathore**. Contributions are welcome through issues and pull requests. Documentation credits use names or handles, not personal email addresses; Git identity settings are managed separately.

Thanks to Sphinx and Georg Brandl, Docutils and David Goodger, Typst, Tree-sitter and Max Brunsfeld, Graphviz, the Rust project and its community, and Ginger Bill and the Odin community.

Guidedog is released under the [GNU Lesser General Public License v3.0 or later](LICENSE) (LGPL-3.0-or-later), which adds permissions to the [GNU General Public License v3.0](COPYING). For other licensing terms, contact the authors. [NOTICE](NOTICE) says how to reach them and lists the third-party components, such as the Rust crates and the bundled fonts, that keep their own licenses. An additional permission allows Guidedog to be linked and distributed with Graphviz; see [NOTICE](NOTICE).
