# Guidedog

[English](README.md) | [简体中文](README.zh-CN.md) | [日本語](README.ja.md) | [한국어](README.ko.md) | [हिन्दी](README.hi.md) | [Español](README.es.md) | [Deutsch](README.de.md)

**Einmal schreiben. Klar erklären. Im Web und auf Papier veröffentlichen.**

Guidedog erstellt eine Dokumentationswebsite und ein PDF-Buch aus denselben reStructuredText- oder Markdown-Dateien. Wir pflegen die Quelldateien an einer Stelle und veröffentlichen sie für Leser im Web und auf Papier. Jedes Release stellt eine einzige ausführbare Datei bereit, `guidedog`, in der PDF-Satz und Diagrammdarstellung bereits enthalten sind.

Sphinx dient als Vorbild für den Schreibablauf. Guidedog ist ein eigenständiges System: Die Konfiguration erfolgt deklarativ in TOML, die API-Analyse arbeitet statisch, und Python-Konfigurationen oder -Erweiterungen werden niemals ausgeführt. Vollständige Sphinx-Kompatibilität ist kein Ziel.

## Ein Projekt beginnen

```sh
guidedog quickstart docs
guidedog build html docs
guidedog build pdf docs
guidedog serve docs
```

Das Projekt enthält `conf.toml`, `index.rst`, `_templates/layout.html`, `_templates/book.typ` und `_static/`. Vorlagen sind gewöhnliche Dateien, die wir direkt bearbeiten können. HTML bietet Navigation, Suche, Syntaxhervorhebung, Diagramme und Gleichungen. PDF-Bücher bieten Kapitel, Querverweise, Abbildungen, Register und sprachgerechten Satz.

Standardmäßig ist das helle Design aktiv; Leser können auf das dunkle Design oder die Systemeinstellung umschalten. Die Einstiegsseite des Handbuchs und die Repositoryübersicht erkennen die Browsersprache automatisch. Eine gespeicherte Auswahl hat Vorrang; direkte Links zu einer Sprachausgabe oder einem Kapitel bleiben ausdrückliche Anforderungen.

```sh
guidedog convert notes.md --to pdf --output notes.pdf
guidedog clean docs
guidedog migrate conf.py
```

API-Dokumentation wird aus dem Quellcode gelesen, ohne ihn auszuführen: Python (`autodoc`), Odin (`odindoc`) und Rust-Crates (die `rust`-Domäne und die Einstellungen `rust_autoapi_*`).

## Installieren

Jedes [Release](https://github.com/insanai/guidedog/releases) bietet ein Archiv für Linux (x86-64 und ARM64) und macOS (Apple Silicon und Intel) sowie ein ZIP-Archiv für Windows (x86-64). Jedes Archiv enthält die ausführbare Datei `guidedog` samt Lizenzhinweisen. Die Veröffentlichung auf GitHub Pages, die Veröffentlichung statischer Websites und die Diskussionsverwaltung sind fest eingebundene Plugins; `gd-graphdog` kapselt Graphviz als Bibliothek. Der Typst-Compiler ist eingebettet, tree-sitter ist einkompiliert und Graphviz ist statisch gelinkt – es muss also nichts weiter installiert werden. Das Windows-ZIP enthält die benötigten Graphviz-DLLs; wir belassen den gesamten entpackten Ordner zusammen. Linux erfordert glibc 2.35 oder neuer, macOS Version 12 oder neuer. Wir laden das Archiv für unser System sowie `SHA256SUMS` herunter, prüfen die Prüfsumme und installieren das Programm:

```sh
sha256sum -c --ignore-missing SHA256SUMS      # unter macOS: shasum -a 256 -c --ignore-missing SHA256SUMS
tar -xzf guidedog-linux-amd64.tar.gz
sudo install guidedog-linux-amd64/guidedog /usr/local/bin/
```

Unter macOS lässt sich Guidedog über den offiziellen [Guidedog-Homebrew-Tap](https://github.com/insanai/homebrew-guidedog) installieren. Homebrew wählt automatisch das Archiv für Apple Silicon oder Intel aus. Der Tap unterstützt auch Linux:

```sh
brew tap insanai/guidedog
brew install insanai/guidedog/guidedog
guidedog --version
```

Die ausführbare Datei ist nicht codesigniert: Unter macOS entfernen wir die Quarantäne eines Browser-Downloads mit `xattr -d com.apple.quarantine guidedog`. Die Versionshinweise beschreiben die jeweiligen Plattformgrenzen.

## Aus dem Quellcode bauen

Installieren Sie Rust 1.99 oder neuer (mit [rustup](https://rustup.rs)), einen C-Compiler (die Grammatiken von tree-sitter sind in C geschrieben) sowie die Entwicklungsdateien von Graphviz: `libgraphviz-dev` unter Debian und Ubuntu, `graphviz-devel` unter Fedora, `graphviz` aus Homebrew unter macOS. (Alternativ lässt sich Graphviz wie in den Releases statisch bauen: `cargo xtask graphviz` und anschließend mit `GUIDEDOG_GRAPHVIZ_STATIC=target/graphviz/16.1.0` kompilieren; die Programme benötigen dann zur Laufzeit kein installiertes Graphviz.) Bauen Sie anschließend den Workspace:

```sh
cargo build --release -p guidedog-cmd
target/release/guidedog --version
```

Die ausführbare Datei liegt unter `target/release`. Unter macOS mit Homebrew weisen wir dem Linker zuerst den Pfad zu Graphviz zu: `export LIBRARY_PATH="$(brew --prefix graphviz)/lib"`. Der Typst-Compiler ist standardmäßig eingebettet (das Feature `embedded` von `gd-typst`); ohne dieses Feature ruft Guidedog für PDFs ein installiertes `typst` 0.15.1 auf. Unter Linux linkt der Paket-Downloader des eingebetteten Compilers ein mitgeliefertes OpenSSL, das zum Kompilieren `perl` und `make` benötigt.

Führen Sie vor dem Einreichen einer Änderung dieselben Prüfungen wie die CI aus:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p check              # Quellcodegrenzen: Funktions- und Dateilänge, Zeilenbreite
```

[RELEASING.md](RELEASING.md) beschreibt den Release-Ablauf.

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

Übersetzen und prüfen Sie die `msgstr`-Einträge in den PO-Dateien. Guidedog extrahiert POT-Dateien, führt Kataloge zusammen, berichtet über den Fortschritt und baut jede Sprachausgabe. Eigene Übersetzungen erfindet es nicht. Befehle, Pfade, Code, API-Namen und Verweisziele bleiben unverändert. Übersetzt werden nur das Handbuch und die Repositoryübersicht; API-Referenz und GDS bleiben auf Englisch. `cargo xtask build-site` fügt alle Sprachausgaben, ihre PDF-Bücher und die Diskussionen zur Gesamtwebsite zusammen.

Auf GitHub wählen Leser die Sprache der README über die Links ganz oben. Die automatische Spracherkennung gehört zur erzeugten Dokumentationswebsite.

## Auf GitHub Pages veröffentlichen

`guidedog publish pages` veröffentlicht über das eingebaute GitHub-Pages-Plugin. Es baut das Projekt mit Guidedogs eigener Engine, führt durch noch fehlende Einrichtungsschritte und veröffentlicht einen vollständigen Website-Stand. `guidedog pages` stellt Befehle für Konfiguration, Zugangsdaten, Wiederherstellung und Workflow-Generierung bereit. Ein separates Publisher-Programm muss nicht installiert werden.

```sh
guidedog publish pages --repository=octo/manual --project=docs
guidedog pages status
guidedog pages workflow --write
```

Für die skriptgesteuerte Einrichtung nutzen wir `guidedog pages init`, `auth import`, `bind` und `setup`. Mit dem generierten Workflow kann GitHub Actions die Website nach jedem Push automatisch bauen und bereitstellen. Siehe [Auf GitHub Pages veröffentlichen](docs/manual/howto/github-pages.rst).

Build-Plugins transformieren Quelltext, vollständige HTML-Seiten und statische Assets über einen typisierten, versionierten Vertrag. Ein Plugin ist ein gewöhnliches Rust-Crate. Wir binden es mit `gd_cli::run_with` direkt in unser eigenes `guidedog` ein oder führen es als separates Programm aus, das `guidedog plugin register` über seinen SHA-256-Hash fixiert. Ein Plugin kann `guidedog` zudem um eigene Unterbefehle erweitern, die in `guidedog --help` aufgeführt werden. `guidedog publish --timeout=SECONDS` begrenzt jeden vollständigen Austausch mit einem Publisher (Standard: 300 Sekunden); wählen Sie einen Wert, der größer als `--wait` ist. Nach einem ungewissen Ausgang sollte vor einer erneuten Veröffentlichung die Wiederherstellung ausgeführt werden.

## Diskussionen führen

`guidedog dsc` verwaltet Vorschläge und Entscheidungen als reStructuredText-Einträge – eine Datei pro Eintrag – und baut daraus eine Website sowie PDF-Bücher. Jeder Katalog besitzt ein Präfix: Die Entwurfsaufzeichnungen von Guidedog verwenden `gds`, ein anderes Projekt beispielsweise `eds`.

```sh
guidedog dsc init docs/eds --prefix=eds --name="Enso Discussions"
cd docs/eds
guidedog dsc new "Cache policy" --labels=architecture
guidedog dsc promote cache-policy
guidedog dsc state 1 --to=accepted --reason="Review converged"
guidedog dsc build
```

Die Datei `discussion.toml` eines Katalogs legt dessen Lebenszykluszustände, Eintragstypen und Nummerierung fest; `guidedog dsc config` gibt diese Konfiguration aus. Die Befehle stammen aus dem Diskussions-Plugin, das in jedes `guidedog` eingebunden ist, sodass nichts zusätzlich installiert werden muss. Guidedogs eigenes Verzeichnis `docs/gds` ist ein solcher Katalog: In diesem Ordner prüfen wir ihn mit `guidedog dsc check`. Siehe [Einen Diskussionskatalog führen](docs/manual/howto/discussions.rst).

`gd-graphdog` ist die Bibliothek, die Graphviz kapselt. Guidedog nutzt sie, um Dokumentationsdiagramme im selben Prozess zu rendern; sie ist kein eigenständiges CLI-Programm.

## Statische Websites veröffentlichen

`guidedog ssg` verwaltet Artikel, Forschungsnotizen, Sammlungen, Feeds, per Prüfsumme fixierte Theme-Archive und Websites mit mehreren Teilprojekten aus derselben `conf.toml` und demselben Dokumentbaum:

```sh
guidedog ssg init site --preset=journal --title="Field Notes"
guidedog ssg new posts/2026-04-17-zero-copy.rst --title="Zero-Copy Readers"
guidedog ssg schedule posts/2026-04-17-zero-copy.rst --at=2026-05-01T09:00:00Z
guidedog ssg publish posts/2026-04-17-zero-copy.rst
guidedog ssg build site
```

Seiten, die mit `:pdf: article` markiert sind, erzeugen neben ihrer HTML-Seite auch eigenständige PDFs in Druckqualität. Websites mit mehreren Teilprojekten binden Unterprojekte (wie `docs/manual`, `docs/api` und `docs/gds`) deklarativ über `[[site.mounts]]` ein. Siehe [Statische Websites veröffentlichen](docs/manual/howto/static-publishing.rst).

## Lesen und einbetten

- [Handbuch](docs/manual/index.rst): Tutorials, Themen, praxisnahe Anleitungen, Referenz und Interna.
- [API-Referenz](docs/api/index.rst): von Guidedog selbst aus dem Rust-Quellcode erzeugt und als Anhang im PDF-Buch des Handbuchs enthalten. `cargo doc --workspace --no-deps` baut zusätzlich die rustdoc-Seiten.
- [Engine-Leitfaden](crates/guidedoc/README.md): Workspaces, Anfragen, Adapter und Durchläufe; die [Beispiele](examples/README.md) sind kleine Programme, die sich mit `cargo run` ausführen lassen.
- [Vorlagen](docs/manual/templates.rst): HTML mit Jinja und Bücher mit Typst anpassen.
- [GDS](docs/gds/README.md): nummerierte Entwurfsaufzeichnungen und Implementierungsentscheidungen.
- [Diskussionsbibliothek](crates/gd-discussion/README.md): die Kataloge, die `guidedog dsc` verwaltet.
- [Plugin-Bibliothek](crates/gd-plugins/README.md): typisierte Build-Dienste und die Publisher-Schnittstelle, die `guidedog publish` und das eingebundene Pages-Plugin verwenden.
- [Wiederverwendbare Crates](docs/manual/internals/crates.rst): die Engine, Bibliotheken, API-Generatoren und das Plugin-SDK, die andere Rust-Projekte über crates.io nutzen können, sowie ihre Abhängigkeiten untereinander.

Jedes Crate unter `crates/` enthält ein kurzes README zu seiner API und seinen Entwurfsentscheidungen. Guidedoc verwaltet seine Dokumenttabellen selbst und verwendet sie zwischen Konvertierungen wieder, sodass im eingeschwungenen Zustand kein neuer Speicher reserviert wird. Es liefert strukturierte Diagnosen samt Lösungshinweis zurück. Der Host übernimmt Dateien, Budgets, Caching, Veröffentlichung und die eingebetteten Compiler. `--untrusted` schränkt Fähigkeiten ein, ist jedoch keine Prozess-Sandbox und begrenzt nicht sämtliche nativen Ressourcen. Der Funktionsumfang der Beta richtet sich an vertrauenswürdige Dokumentationsprojekte unter Linux, macOS und Windows.

## Danksagung und Lizenz

Erstellt von **Vikrant Rathore**, unter Mitwirkung von **Ronak Rathore** und **Kanak Rathore**. Beiträge über Issues und Pull Requests sind willkommen. Namensnennungen in der Dokumentation verwenden Namen oder Handles, keine persönlichen E-Mail-Adressen; Git-Identitätseinstellungen werden separat verwaltet.

Dank gilt Sphinx und Georg Brandl, Docutils und David Goodger, Typst, Tree-sitter und Max Brunsfeld, Graphviz sowie dem Rust-Projekt und seiner Community.

Guidedog wird unter der [GNU Lesser General Public License v3.0 oder neuer](LICENSE) (LGPL-3.0-or-later) veröffentlicht, die die [GNU General Public License v3.0](COPYING) um zusätzliche Berechtigungen ergänzt. Für andere Lizenzbedingungen wenden Sie sich bitte an die Autoren. [NOTICE](NOTICE) beschreibt, wie Sie die Autoren erreichen, und führt die Drittanbieterkomponenten auf – etwa die Rust-Crates und die mitgelieferten Schriftarten –, die unter ihren eigenen Lizenzen verbleiben. Eine zusätzliche Genehmigung gestattet es, Guidedog zusammen mit Graphviz zu linken und weiterzugeben; siehe [NOTICE](NOTICE).
