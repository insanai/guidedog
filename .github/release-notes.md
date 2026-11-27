Guidedog builds documentation websites, dated articles and PDF books from reStructuredText and Markdown.

Version 1.0.0-beta.16 builds the editions of a site at the same time, as many as fit in memory, and publishes the same site as before.

- **Parallel site builds.** `guidedog ssg build` builds each edition (each mount in each language) in a `guidedog build` child process. As many run at once as fit in nine tenths of the memory available and as the processors allow. The Guidedog website, 23 editions with seven PDF books, builds in 13 s instead of 56 to 66 s from an empty build folder, with the same files.
- **Within memory.** An edition reserves its peak memory at its last build, kept in `_build/ssg-projects/memory.toml`, with a quarter more. An edition heavier than the room is built alone, as before. Linux, macOS and Windows report the memory available and each build's peak; `GUIDEDOG_AVAILABLE_MIB` states it where a system's report misleads.
- **The sequential outcome.** The first failing edition in plan order is reported, as before, and the editions' reports follow in their order. `--jobs=N` limits the editions at once, and `--jobs=1` builds one after another in the `guidedog` process.
- **One build per output.** Every project build locks its records folder: a second build of the same output stops with `build.busy` (exit 4) instead of mixing its files with the first. The default layout's builders share that folder, so `guidedog build html` and `guidedog build pdf` of one project no longer run at once.
- **Proofs.** GDS 0020, "Parallel site builds within memory", proves in Lean that the running editions stay within the room unless one runs alone, that every run ends, that a parallel build has the outcome of the sequential build, that two editions never write the same file, and that a catalog several editions write is read whole. `cargo xtask proofs` checks 30 theorems.

[GDS 0020](https://insanai.github.io/guidedog/discussions/records/0020-parallel-site-builds.html), the [command reference](https://insanai.github.io/guidedog/manual/reference/cli.html) and the [changelog](https://github.com/insanai/guidedog/blob/v1.0.0-beta.16/CHANGELOG.md) describe the scheduler, the options and the proofs.

## Downloads

| File | For |
| --- | --- |
| `guidedog-linux-amd64.tar.gz` | Linux x86-64 with glibc 2.35 or later (Ubuntu 22.04, Debian 12, Fedora 36 or newer) |
| `guidedog-linux-arm64.tar.gz` | Linux ARM64, with the same requirements |
| `guidedog-macos-arm64.tar.gz` | macOS 12 or later on Apple silicon |
| `guidedog-macos-amd64.tar.gz` | macOS 12 or later on Intel |
| `guidedog-windows-amd64.zip` | Windows x86-64, with bundled Graphviz DLLs |
| `guidedog-1.0.0-beta.16-source.tar.gz` | The source of this release |
| `guidedog.rb` | A Homebrew formula that installs the macOS or Linux archive |
| `SHA256SUMS` | The SHA-256 checksum of every file above |

Each `guidedog` archive holds one folder with `guidedog`, `README.md`, `LICENSE`, `COPYING`, `NOTICE`, `COPYING-bravura`, `CHANGELOG.md`, `SOURCE.txt` (the commit, the Rust toolchain, and the exact build command), and `guidedog.build.json` (the version, target, toolchain, and the program's checksum). Unix archives also carry `COPYING-graphviz` and `COPYING-expat`.

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

With Homebrew, on macOS or Linux, install from the official tap. It installs `guidedog`:

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
```

Or unpack the archive and put `guidedog` on our `PATH`:

```sh
tar -xzf guidedog-macos-arm64.tar.gz
sudo install guidedog-macos-arm64/guidedog /usr/local/bin/
guidedog --version
```

## Platform notes

- The executable is not code-signed. On macOS, a file downloaded with a browser is quarantined; run `xattr -d com.apple.quarantine guidedog` for the executable, or download with `curl`.
- Bravura 1.482 is embedded for musical notation (`COPYING-bravura`). Graphviz 16.1.1 is linked statically on Unix, with the dot and neato layout families and SVG and DOT output (no PNG); its license and Expat's are in the archive (`COPYING-graphviz`, `COPYING-expat`), and `SOURCE.txt` names their source tarballs.
- Linux builds link the system's glibc. HTTP and HTTPS are built in; no system `curl` program is needed. The Pages plugin stores tokens through the Secret Service (`secret-tool` from libsecret-tools) when a desktop session offers one; without one, give a token to a single command with `GUIDEDOG_PAGES_TOKEN` and `--credential=env`.
- Windows x86-64 bundles the official Graphviz 16.1.1 runtime beside `guidedog.exe`. Keep the whole extracted folder together.
- PDF books and score lyrics use Typst's embedded fonts and the system's fonts (plus `pdf_font_paths`). Books or lyrics in Chinese, Japanese, Korean, or Hindi need fonts for those scripts installed or configured.

## License

Guidedog is released under the GNU Lesser General Public License v3.0 or later (LGPL-3.0-or-later), with an additional permission to link and distribute it with Graphviz; see `LICENSE`, `COPYING` and `NOTICE`. Bravura is licensed under the SIL Open Font License 1.1 (`COPYING-bravura`). For other licensing terms, contact the authors.

## Credits

Created by Vikrant Rathore, with assistance from Ronak Rathore and Kanak Rathore.

Thanks to Sphinx and Georg Brandl, Docutils and David Goodger, Typst, Steinberg Media Technologies and Daniel Spreadbury (Bravura and SMuFL), Tree-sitter and Max Brunsfeld, Graphviz, and the Rust project and its community.
