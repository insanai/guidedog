Guidedog 0.7.0 is the Rust port. Guidedog, its Guidedoc engine and every supporting
library are rewritten in Rust as one Cargo workspace. Commands, options, `conf.toml`
keys, directives, roles and output are unchanged, so existing projects build as before.

- **One toolchain.** `cargo build --release -p guidedog-cmd` builds Guidedog from source;
  the Odin toolchain and the native build scripts are gone.
- **Typst embedded, Graphviz from the system.** The Typst 0.15.1 compiler is built in.
  The programs link the system's Graphviz libraries, which you install once with your
  package manager or Homebrew (see Platform notes).
- **Rust API documentation.** The `rust` domain and the `rust_autoapi_*` settings
  document Rust crates from source with tree-sitter.
- **Plugins in Rust.** Publisher interface 1.1, build-plugin interface 1.0 and both
  process protocols 1.0 are kept. Plugins are Rust crates: register them as programs, or
  link them into your own `guidedog` with `gd_cli::run_with`.
- **`graphdog`** joins the archive: a `dot`-compatible DOT renderer with clear errors.

This remains a beta for trusted documentation projects; installed plugins are separate
native processes, not an operating-system sandbox.

See the [changelog](https://github.com/insanai/guidedog/blob/v0.7.0/CHANGELOG.md).

## Downloads

| File | For |
| --- | --- |
| `guidedog-linux-amd64.tar.gz` | Linux x86-64 with glibc 2.35 or later (Ubuntu 22.04, Debian 12, Fedora 36 or newer) and Graphviz |
| `guidedog-linux-arm64.tar.gz` | Linux ARM64, with the same requirements |
| `guidedog-macos-arm64.tar.gz` | macOS 12 or later on Apple silicon, with Graphviz from Homebrew |
| `guidedog-macos-amd64.tar.gz` | macOS 12 or later on Intel, with Graphviz from Homebrew |
| `dscc-linux-amd64.tar.gz`, `dscc-linux-arm64.tar.gz` | `dscc` alone, for Linux, with the same requirements |
| `dscc-macos-arm64.tar.gz`, `dscc-macos-amd64.tar.gz` | `dscc` alone, for macOS 12 or later |
| `guidedog-0.7.0-source.tar.gz` | The source of this release |
| `guidedog.rb` | A Homebrew formula that installs the macOS or Linux archive |
| `SHA256SUMS` | The SHA-256 checksum of every file above |

Each `guidedog` archive holds one folder with `guidedog`, `dsc`, `guidedog-pages`, and `graphdog`, `README.md`, `LICENSE`, `NOTICE`, `CHANGELOG.md`, `SOURCE.txt` (the commit, the Rust toolchain, and the exact build command), and `guidedog.build.json` (the version, target, toolchain, and the programs' checksums). Each `dscc` archive holds `dscc` with the same files, and `dscc.build.json`.

## Verify

Download `SHA256SUMS` next to the archive and check it:

```sh
sha256sum -c --ignore-missing SHA256SUMS          # Linux
shasum -a 256 -c --ignore-missing SHA256SUMS      # macOS
```

The archives also have build provenance from GitHub Actions, which the GitHub CLI checks:

```sh
gh attestation verify guidedog-linux-amd64.tar.gz --repo insanai/guidedog
```

## Install

With Homebrew, on macOS or Linux, install from the official tap. It installs Graphviz, `guidedog`, `dsc`, `guidedog-pages`, and `graphdog`:

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
```

Or install Graphviz, unpack the archive, and put the programs on your `PATH`:

```sh
brew install graphviz                       # macOS; on Debian/Ubuntu: sudo apt install libgvc6
tar -xzf guidedog-macos-arm64.tar.gz
sudo install guidedog-macos-arm64/guidedog guidedog-macos-arm64/dsc \
    guidedog-macos-arm64/guidedog-pages guidedog-macos-arm64/graphdog /usr/local/bin/
guidedog --version
```

`dscc` installs the same way from its own archive.

## Platform notes

- The programs are not code-signed. On macOS, a file downloaded with a browser is quarantined; run `xattr -d com.apple.quarantine guidedog` once for each program, or download with `curl`.
- Graphviz is linked from the system. The Linux archives are built on Ubuntu 22.04 against Graphviz 2.42 (`libgvc.so.6`): Debian and Ubuntu need `libgvc6`; distributions with another Graphviz library version should build from source (`cargo build --release -p guidedog-cmd`). The macOS archives link Homebrew's Graphviz (`/opt/homebrew` on Apple silicon, `/usr/local` on Intel).
- Linux builds link the system's glibc and a static OpenSSL for Typst's package downloader; HTTP requests elsewhere go through the `curl` program. `guidedog-pages` stores tokens through the Secret Service (`secret-tool` from libsecret-tools) when a desktop session offers one; without one, give a token to a single command with `GUIDEDOG_PAGES_TOKEN` and `--credential=env`.
- Windows has no release archive in this version.
- PDF books use Typst's embedded fonts and the system's fonts. Books in Chinese, Japanese, Korean, or Hindi need fonts for those scripts installed.

## License

Guidedog, `dsc`, `dscc`, `guidedog-pages`, and `graphdog` are released under the GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later), with an additional permission to link and distribute them with Graphviz; see `LICENSE` and `NOTICE`. For other licensing terms, contact the authors.

## Credits

Created by Vikrant Rathore, with assistance from Ronak Rathore and Kanak Rathore.

Thanks to Sphinx and Georg Brandl, Docutils and David Goodger, Typst, Tree-sitter and Max Brunsfeld, Graphviz, the Rust project and its community, and Ginger Bill and the Odin community, whose language carried Guidedog's first implementation.
