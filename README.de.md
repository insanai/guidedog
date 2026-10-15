# Guidedog

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Deutsch](README.de.md)

**Einmal schreiben. Klar erklären. Im Web und auf Papier veröffentlichen.**

Guidedog erzeugt HTML-Dokumentation und PDF-Bücher aus reStructuredText und Markdown. Es ist in Rust geschrieben. Die Konvertierungsengine heißt **Guidedoc**; Typst, im Programm eingebettet, setzt die PDF-Seiten. Python oder LaTeX sind nicht nötig.

Sphinx dient als Vorbild für den Schreibablauf. Guidedog ist ein eigenständiges System: deklarative TOML-Konfiguration, statische API-Analyse und keine Ausführung von Python-Konfiguration oder -Erweiterungen. Vollständige Sphinx-Kompatibilität ist kein Ziel.

## Ein Projekt beginnen

```sh
guidedog quickstart docs
guidedog build html docs
guidedog build pdf docs
guidedog serve docs
```

Das Projekt enthält `conf.toml`, `index.rst`, `_templates/layout.html`, `_templates/book.typ` und `_static/`. Vorlagen sind gewöhnliche Dateien, die wir bearbeiten können. HTML bietet Navigation, Suche, hervorgehobenen Code, Diagramme und Gleichungen. PDF bietet Kapitel, Querverweise, Abbildungen, Register und sprachgerechten Satz.

Standard ist das helle Thema; Dunkel- und Systemthema sind wählbar. Die Einstiegsseiten des Handbuchs und der Repositoryübersicht erkennen die Browsersprache. Eine gespeicherte Wahl hat Vorrang; direkte Links zu einer Ausgabe oder einem Kapitel bleiben maßgeblich.

```sh
guidedog convert notes.md --to pdf --output notes.pdf
guidedog clean docs
guidedog migrate conf.py
```

API-Dokumentation wird aus dem Quellcode gelesen, ohne ihn auszuführen: Python (`autodoc`), Odin (`odindoc`) und, neu in 0.7, Rust-Crates (die `rust`-Domäne und die Einstellungen `rust_autoapi_*`).

## Installieren

Jedes [Release](https://github.com/insanai/guidedog/releases) enthält ein Archiv für Linux (x86-64 und ARM64) und macOS (Apple Silicon und Intel). Das `guidedog`-Archiv enthält `guidedog`, `dsc`, `guidedog-pages` und `graphdog`, das `dscc`-Archiv nur `dscc`. Der Typst-Compiler ist eingebettet, tree-sitter ist einkompiliert. Graphviz wird aus dem System gelinkt; installieren Sie daher zuerst seine Bibliotheken: unter Debian und Ubuntu `sudo apt install libgvc6` (oder das Paket `graphviz`), unter Fedora `sudo dnf install graphviz`, unter macOS `brew install graphviz`. Linux braucht glibc 2.35 oder neuer, macOS Version 12 oder neuer. Wir laden das Archiv für unser System und `SHA256SUMS` herunter, prüfen es und installieren es dann:

```sh
sha256sum -c --ignore-missing SHA256SUMS      # unter macOS: shasum -a 256 -c --ignore-missing SHA256SUMS
tar -xzf guidedog-linux-amd64.tar.gz
sudo install guidedog-linux-amd64/guidedog guidedog-linux-amd64/dsc \
    guidedog-linux-amd64/guidedog-pages guidedog-linux-amd64/graphdog /usr/local/bin/
```

Installieren Sie Guidedog unter macOS aus dem offiziellen [Homebrew-Tap](https://github.com/insanai/homebrew-guidedog). Homebrew wählt das Archiv für Apple Silicon oder Intel und installiert Graphviz mit. Der Tap unterstützt auch Linux:

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
guidedog --version
```

Die Programme sind nicht codesigniert: Unter macOS entfernen wir die Quarantäne einer im Browser heruntergeladenen Datei mit `xattr -d com.apple.quarantine guidedog`. Die Versionshinweise beschreiben die Grenzen jeder Plattform.

## Aus dem Quellcode bauen

Installieren Sie Rust 1.90 oder neuer (mit [rustup](https://rustup.rs)), einen C-Compiler (die Grammatiken von tree-sitter sind in C geschrieben) und die Entwicklungsdateien von Graphviz: `libgraphviz-dev` unter Debian und Ubuntu, `graphviz-devel` unter Fedora, `graphviz` aus Homebrew unter macOS. Bauen Sie dann den Workspace:

```sh
cargo build --release -p guidedog-cmd -p dsc-cmd -p dscc-cmd -p guidedog-pages-cmd -p graphdog-cmd
target/release/guidedog --version
```

Die Programme liegen in `target/release`. Unter macOS mit Homebrew zeigen wir dem Linker zuerst Graphviz: `export LIBRARY_PATH="$(brew --prefix graphviz)/lib"`. Der Typst-Compiler ist standardmäßig eingebettet (das Feature `embedded` von `gd-typst`); ohne ihn nutzt Guidedog für PDF ein installiertes `typst` 0.15.1. Unter Linux linkt der Paket-Downloader des eingebetteten Compilers ein mitgeliefertes OpenSSL, das zum Bauen `perl` und `make` braucht.

Führen Sie vor dem Einreichen einer Änderung dasselbe aus wie die CI:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p check              # Quellgrenzen: Länge von Funktionen und Dateien, Zeilenbreite
```

[RELEASING.md](RELEASING.md) beschreibt den Releaseablauf.

## Mit Guidedog übersetzen

```sh
mkdir -p docs/manual/_editions
guidedog build -b gettext docs/manual docs/manual/locales/templates -W
guidedog intl update -p docs/manual/locales/templates \
    -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog intl stat -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
guidedog build -b html docs/manual docs/manual/_editions/de \
    -D language=de -D 'html_extra_path=[]' -W
```

Übersetzen und prüfen Sie die `msgstr`-Werte der PO-Dateien. Guidedog extrahiert POT, führt Kataloge zusammen, zeigt den Stand und baut jede Ausgabe. Übersetzungen verfasst es nicht selbst. Befehle, Pfade, Code, API-Namen und Verweisziele bleiben unverändert. Nur Handbuch und Repositoryübersicht werden übersetzt; API und GDS bleiben Englisch. `tools/build-site.sh` stellt alle Ausgaben, ihre PDF-Bücher und die Diskussionen zur Website zusammen.

Auf GitHub wählen Leser die README-Sprache über die Links oben. Die automatische Sprachwahl erfolgt auf der erzeugten Dokumentationswebsite.

## Auf GitHub Pages veröffentlichen

`guidedog-pages` veröffentlicht eine Website von unserem eigenen Rechner aus. Es baut das Projekt mit dem installierten `guidedog` und sendet die ganze Website in einem einzigen Git-Push auf einen eigenen Branch. GitHub übernimmt den Push nur, wenn der Branch noch auf dem gelesenen Commit steht. Danach wartet es, bis GitHub Pages diesen Commit bereitgestellt hat. Tokens bleiben im Zugangsdatenspeicher des Systems, und eine Bindung auf unserem Rechner entscheidet, wohin ein Profil veröffentlichen darf.

```sh
guidedog-pages init --repository=octo/manual --project=docs
guidedog-pages auth import          # ein Fine-grained Token, ohne Echo eingelesen
guidedog-pages bind                 # Ziel prüfen, dann diesen Rechner zulassen
guidedog-pages publish
guidedog-pages setup                # GitHub Pages aus dem Branch bereitstellen
guidedog-pages watch                # erneut veröffentlichen, sobald sich die Quellen ändern
```

GitHub Actions ist der andere Weg, um nach jedem Push zu veröffentlichen. `guidedog publish --publisher=pages` baut mit Guidedogs eigener Engine und startet denselben Publisher. Siehe [Auf GitHub Pages veröffentlichen](docs/manual/howto/github-pages.rst) und [Ein Plugin schreiben](docs/manual/plugins.rst).

Build-Plugins verändern Quelltext, vollständige HTML-Seiten und statische Assets über einen typisierten, versionierten Vertrag. Ein Plugin ist ein gewöhnliches Rust-Crate. Wir linken es mit `gd_cli::run_with` in unser eigenes `guidedog`, oder wir führen es als eigenes Programm aus, das `guidedog plugin register` über seinen SHA-256 festhält. `guidedog publish --timeout=SECONDS` begrenzt jeden vollständigen Austausch mit einem Publisher (Standard: 300 Sekunden); wählen Sie einen Wert, der länger als `--wait` ist. Bei ungewissem Ausgang führen Sie vor einer erneuten Veröffentlichung die Wiederherstellung aus.

## Diskussionen führen

`dsc` bewahrt Vorschläge und Entscheidungen als reStructuredText-Aufzeichnungen, eine Datei je Aufzeichnung, und baut daraus mit Guidedog eine Website und PDF-Bücher. Jeder Katalog hat ein Präfix: Die Designaufzeichnungen von Guidedog verwenden `gds`, ein anderes Projekt etwa `eds`.

```sh
dsc init docs/eds --prefix=eds --name="Enso Discussions"
cd docs/eds
dsc new "Cache policy" --labels=architecture
dsc promote cache-policy
dsc state 1 --to=accepted --reason="Review converged"
dsc build
```

Die `discussion.toml` eines Katalogs legt die Zustände seines Lebenszyklus, seine Aufzeichnungstypen und seine Nummerierung fest, und `dsc config` gibt sie aus. `dsc` liegt im Guidedog-Archiv und baut mit dem installierten `guidedog`. `dscc` ist `dsc` mit eingebautem Guidedog, für einen Rechner, auf dem sonst nichts installiert ist; jedes Release enthält ein `dscc`-Archiv für jede Plattform. `guidedog gds` führt dieselben Befehle für das eigene `docs/gds` von Guidedog aus. Siehe [Einen Diskussionskatalog führen](docs/manual/howto/discussions.rst).

`graphdog` setzt Graphviz-DOT in SVG, PNG oder gelayoutetes DOT um, mit derselben Bibliothek wie die Diagramme von Guidedog, als Teilmenge des Befehls `dot`: `echo 'digraph { a -> b }' | graphdog -Tsvg -o graph.svg`.

## Lesen und einbetten

- [Handbuch](docs/manual/index.rst): Tutorials, Themen, Aufgaben, Referenz und interne Struktur.
- [API-Referenz](docs/api/index.rst): von Guidedog selbst aus dem Rust-Quellcode erzeugt und im PDF-Handbuch als Anhang enthalten. `cargo doc --workspace --no-deps` erzeugt zusätzlich die rustdoc-Seiten.
- [Engine-Leitfaden](crates/guidedoc/README.md): Workspaces, Anfragen, Adapter und Durchläufe; die [Beispiele](examples/README.md) sind kleine Programme, die wir mit `cargo run` ausführen.
- [Vorlagen](docs/manual/templates.rst): HTML mit Jinja und Bücher mit Typst anpassen.
- [GDS](docs/gds/README.md): nummerierte Entwurfsunterlagen und Implementierungsentscheidungen.
- [Diskussionsbibliothek](crates/gd-discussion/README.md): die Kataloge, die `dsc`, `dscc` und `guidedog gds` führen.
- [Plugin-Bibliothek](crates/gd-plugins/README.md): typisierte Build-Dienste und die Publisher-Schnittstelle für `guidedog publish` und `guidedog-pages`.
- [Wiederverwendbare Crates](docs/manual/internals/crates.rst): die Engine, die Bibliotheken, die API-Generatoren und das Plugin-SDK, die andere Rust-Projekte von crates.io nutzen können, und ihre Abhängigkeiten untereinander.

Jedes Crate unter `crates/` hat ein kurzes README mit seiner API und den Designentscheidungen der Portierung. Guidedoc besitzt seine Dokumenttabellen und verwendet sie zwischen Konvertierungen wieder, sodass eine Konvertierung im eingeschwungenen Zustand keinen Speicher anfordert. Guidedoc liefert strukturierte Diagnosen mit einem Hinweis. Der Host verwaltet Dateien, Budgets, Caches, Veröffentlichung und die eingebetteten Compiler. `--untrusted` begrenzt Fähigkeiten, ist aber keine Prozesssandbox und begrenzt nicht sämtliche nativen Ressourcen. Die Beta richtet sich an vertrauenswürdige Dokumentationsprojekte unter macOS und Linux.

Guidedog 0.6 und früher waren in Odin geschrieben. Version 0.7 ist eine Portierung nach Rust, die Befehle, Optionen, `conf.toml`-Schlüssel, Direktiven, Rollen und Ausgabe beibehält; das [Änderungsprotokoll](CHANGELOG.md) nennt, was sich geändert hat.

## Danksagung und Lizenz

Erstellt von **Vikrant Rathore**, unterstützt von **Ronak Rathore** und **Kanak Rathore**. Beiträge über Issues und Pull Requests sind willkommen. Autorenangaben verwenden Namen oder Handles, keine persönlichen E-Mail-Adressen. Git-Identitätseinstellungen werden separat verwaltet.

Dank an Sphinx und Georg Brandl, Docutils und David Goodger, Typst, Tree-sitter und Max Brunsfeld, Graphviz, das Rust-Projekt und seine Gemeinschaft sowie Ginger Bill und die Odin-Gemeinschaft, deren Sprache die erste Implementierung von Guidedog getragen hat.

Guidedog steht unter der [GNU Affero General Public License v3.0 oder neuer](LICENSE) (AGPL-3.0-or-later). Für andere Lizenzbedingungen wenden Sie sich an die Autoren. [NOTICE](NOTICE) beschreibt, wie man sie erreicht, und nennt die Komponenten Dritter, etwa die Rust-Crates und die mitgelieferten Schriften, die ihre eigenen Lizenzen behalten. Eine zusätzliche Erlaubnis gestattet, Guidedog zusammen mit Graphviz zu linken und weiterzugeben; siehe [NOTICE](NOTICE).
