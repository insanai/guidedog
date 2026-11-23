Guidedog builds documentation websites, dated articles and PDF books from reStructuredText and Markdown.

Version 1.0.0-beta.14 adds mathematics that needs no script, self-contained HTML fragments, and a check of every formula.

- **MathML output.** `html_math_renderer = "mathml"`, `guidedog build --math=mathml` and `guidedog convert --math=mathml` write each formula as a MathML Core `math` element, so a page loads no MathJax and is readable with JavaScript off. Equation numbers, labels and `:eq:` references keep working. MathJax stays the default, and its output is unchanged. A formula that cannot be MathML keeps its LaTeX, with a warning.
- **Formulas written as MathML.** The `math` directive takes `:notation: mathml`. HTML writes the formula through an allowlist filter for MathML Core. PDF translates the same filtered MathML to Typst math, so the book shows a typeset, numbered formula; a formula the bounded translator cannot write shows its source, with a warning that names the cause.
- **Pictures inside a fragment.** `guidedog convert --embed-pictures` draws graphs, Typst diagrams and inline scores under the confinement of a build and writes them as SVG `data:` images, within `--max-pictures` and `--max-picture-kib`. A picture that fails or passes a bound stays code, with a warning.
- **Output despite errors.** `guidedog convert --keep-going` writes the output and still reports the errors of a document with an unknown directive or role, a missing included file or an unresolved reference.
- **A math check for every output.** Each LaTeX formula is checked once when a document is read. A warning points at the first fault and proposes a correction, such as "Did you mean `\frac`?". A command MathJax typesets is never reported as unknown. These are warnings, so `-W` and `--strict` builds that held such formulas now fail on them.

The [mathematics guide](https://insanai.github.io/guidedog/manual/topics/mathematics.html), [command reference](https://insanai.github.io/guidedog/manual/reference/cli.html) and [changelog](https://github.com/insanai/guidedog/blob/v1.0.0-beta.14/CHANGELOG.md) describe the options, bounds and library interfaces.

## Downloads

| File | For |
| --- | --- |
| `guidedog-linux-amd64.tar.gz` | Linux x86-64 with glibc 2.35 or later (Ubuntu 22.04, Debian 12, Fedora 36 or newer) |
| `guidedog-linux-arm64.tar.gz` | Linux ARM64, with the same requirements |
| `guidedog-macos-arm64.tar.gz` | macOS 12 or later on Apple silicon |
| `guidedog-macos-amd64.tar.gz` | macOS 12 or later on Intel |
| `guidedog-windows-amd64.zip` | Windows x86-64, with bundled Graphviz DLLs |
| `guidedog-1.0.0-beta.14-source.tar.gz` | The source of this release |
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
