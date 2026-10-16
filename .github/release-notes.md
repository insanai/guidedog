Guidedog builds documentation websites and PDF books from the same reStructuredText or Markdown files.

Version 1.0.0-beta.2 fixes PDF content that could run past the bottom of a page when a table or code listing had a caption.

- **Long tables continue across pages.** All rows remain visible, and table headers repeat. This fixes [issue #1](https://github.com/insanai/guidedog/issues/1).
- **Captioned code listings continue too.** The review found the same unbreakable outer figure around long listings.
- **Existing projects inherit the fix.** The pagination defaults live in the shared renderer, so built-in books, copied templates and standalone PDF conversion agree. We do not need to replace our book template.
- **Checks cover the output.** Regression tests inspect the laid-out pages and verify that all 120 entries appear once, span pages and retain repeated table headers.

The [review record](https://insanai.github.io/guidedog/manual/evidence/pdf-pagination.html) explains the root cause. The [changelog](https://github.com/insanai/guidedog/blob/v1.0.0-beta.2/CHANGELOG.md) records the changes.

## Downloads

| File | For |
| --- | --- |
| `guidedog-linux-amd64.tar.gz` | Linux x86-64 with glibc 2.35 or later (Ubuntu 22.04, Debian 12, Fedora 36 or newer) |
| `guidedog-linux-arm64.tar.gz` | Linux ARM64, with the same requirements |
| `guidedog-macos-arm64.tar.gz` | macOS 12 or later on Apple silicon |
| `guidedog-macos-amd64.tar.gz` | macOS 12 or later on Intel |
| `guidedog-windows-amd64.zip` | Windows x86-64, with bundled Graphviz DLLs |
| `guidedog-1.0.0-beta.2-source.tar.gz` | The source of this release |
| `guidedog.rb` | A Homebrew formula that installs the macOS or Linux archive |
| `SHA256SUMS` | The SHA-256 checksum of every file above |

Each `guidedog` archive holds one folder with `guidedog`, `README.md`, `LICENSE`, `NOTICE`, `CHANGELOG.md`, `SOURCE.txt` (the commit, the Rust toolchain, and the exact build command), and `guidedog.build.json` (the version, target, toolchain, and the programs' checksums).

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
- Graphviz 16.1.0 is linked statically, with the dot and neato layout families and SVG and DOT output (no PNG). Its licence and Expat's are in the archive (`COPYING-graphviz`, `COPYING-expat`), and `SOURCE.txt` names their source tarballs.
- Linux builds link the system's glibc. HTTP and HTTPS are built in; no system `curl` program is needed. The Pages plugin stores tokens through the Secret Service (`secret-tool` from libsecret-tools) when a desktop session offers one; without one, give a token to a single command with `GUIDEDOG_PAGES_TOKEN` and `--credential=env`.
- Windows x86-64 bundles the official Graphviz 16.1.0 runtime beside `guidedog.exe`. Keep the whole extracted folder together.
- PDF books use Typst's embedded fonts and the system's fonts. Books in Chinese, Japanese, Korean, or Hindi need fonts for those scripts installed.

## License

Guidedog is released under the GNU Lesser General Public License v3.0 or later (LGPL-3.0-or-later), with an additional permission to link and distribute it with Graphviz; see `LICENSE`, `COPYING` and `NOTICE`. For other licensing terms, contact the authors.

## Credits

Created by Vikrant Rathore, with assistance from Ronak Rathore and Kanak Rathore.

Thanks to Sphinx and Georg Brandl, Docutils and David Goodger, Typst, Tree-sitter and Max Brunsfeld, Graphviz, and the Rust project and its community.
