Guidedog builds documentation websites, dated articles and PDF books from reStructuredText and Markdown.

Version 1.0.0-beta.15 fixes a race in the file locks, and gives every command one lock that is proved correct.

- **No false "busy".** A command could be told that the plugin registry, a publisher's state, a discussion catalog or a site folder was busy when no command held its lock (`plugins.busy`, `pages.busy`, `PREFIX.busy`, `ssg.conflict`, `ssg.site`). On Unix a lock belongs to the open file, and a child process holds a copy of every descriptor until it starts its program; a holder that only closed its lock file had not released the lock while such a copy lived. A holder now unlocks before it closes, which releases the lock for every copy at once.
- **One lock.** `gd_plugins::files::lock_exclusive` and `lock_exclusive_private`, with `FileLock` and `LockError`, take an advisory lock on a lock file without waiting and release it when the `FileLock` drops. The registry, `gd-pages`, `gd-discussion`, both locks of `gd-ssg` and the example publisher use it. Problem codes, messages, exit statuses and lock file names are unchanged.
- **No waiting in `lock_catalog`.** `gd_discussion::lock_catalog` reports a lock another command holds at once, as the other commands do; it no longer asks again for 200 ms.
- **Proofs.** GDS 0019, "File locks", models the lock, the takers, child processes and crashes. It proves mutual exclusion, no false refusal, crash recovery, and that readers see whole files and no update is lost. Lean 4.19 checks the proofs with its core library alone (`docs/gds/proofs`), and `cargo xtask proofs` runs the check. Tests enumerate the same model and reproduce the defect without timing.

[GDS 0019](https://insanai.github.io/guidedog/discussions/records/0019-file-locks.html), the [plugin internals](https://insanai.github.io/guidedog/manual/internals/plugins.html) and the [changelog](https://github.com/insanai/guidedog/blob/v1.0.0-beta.15/CHANGELOG.md) describe the defect, the lock and the proofs.

## Downloads

| File | For |
| --- | --- |
| `guidedog-linux-amd64.tar.gz` | Linux x86-64 with glibc 2.35 or later (Ubuntu 22.04, Debian 12, Fedora 36 or newer) |
| `guidedog-linux-arm64.tar.gz` | Linux ARM64, with the same requirements |
| `guidedog-macos-arm64.tar.gz` | macOS 12 or later on Apple silicon |
| `guidedog-macos-amd64.tar.gz` | macOS 12 or later on Intel |
| `guidedog-windows-amd64.zip` | Windows x86-64, with bundled Graphviz DLLs |
| `guidedog-1.0.0-beta.15-source.tar.gz` | The source of this release |
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
