# gd-i18n cheat sheet

Translates Guidedoc documents as Sphinx does with gettext: extract the translatable
messages of a document into a POT template (Sphinx's `gettext` builder), and replace
each message with its translation in a pass (Sphinx's Locale transform). Also
Guidedog's own words (`guidedog` text domain, catalogs built in), CLDR-named dates,
and `figure_language_filename`. Depends on `guidedoc` and `gd-gettext` only; the
reader that parses translations is handed in (`Reader::read_inline`). Ported from
`guidedog_old/lib/i18n`.

```rust
use gd_i18n::*;
// Extraction: every document of a text domain into one template.
let mut pot = Template::new(domain_for("guide/usage", &Compact::PerDirectory)); // "guide"
pot.extract(doc, &ExtractOptions { location: true, path: "../../guide/usage.rst",
    docname: "guide/usage", ..Default::default() });
pot.set_header(&PotHeader { project: "Demo", version: "1.0",
    creation_date: &format_date("%Y-%m-%d %H:%M%z", &now, "en"), ..Default::default() });
let text: String = pot.write();

// Translation: a pass over the workspace's current document.
let catalog = gd_gettext::load_po("locales/de/LC_MESSAGES/guide.po", &Default::default())?;
let t = Translation { catalog: Some(&catalog), read_inline: guidedoc_rst::reader().read_inline,
    targets: Targets::of(&[Target::LiteralBlock]), ..Default::default() };
let (doc, counts) = translate_document(&mut ws, t)?;      // or TranslationPass::run
```

| Item | Does |
|---|---|
| `unit_of(doc, id, targets) -> Unit` | Sphinx's `is_translatable`: Inline, Literal, Parsed, Alt, Graphviz, Index, Caption, Entry, Meta, Contents, None |
| `write_message(doc, id, &mut String)` | a node's msgid: its source text, lines joined with one space |
| `index_parts(line)` | the translatable pieces of an index entry (`single: a; !b` gives a, b) |
| `parse_target(name)`, `Targets` | `gettext_additional_targets` (`literal-block`, `doctest-block`, `raw`, `image`, `index`) |
| `Template::{new, add, extract, set_header, write, to_catalog, message_ids}` | POT collection in Sphinx's layout (one `#:` per location, sorted, then uuids) |
| `ExtractOptions { targets, location, uuid, path, docname, base }` | `path` names the main source; included files relative to `base` |
| `domain_for(docname, &Compact)` | `gettext_compact` |
| `Translation { catalog, read_inline, read, policy, limits, targets, source_name, progress, images }` | the pass's settings |
| `TranslationPass::{new, run, counts}`, `translate_document(&mut ws, t)` | the pass (`TRANSLATION_PASS`), staged or one call; `TranslationCounts { translated, kept }` |
| `ui_translator(lang, &dirs, allow_fuzzy) -> (Translator, Vec<UiProblem>)` | project `guidedog.po`, then `sphinx.po`, then built in |
| `ui_text(Some(&tr), msgid)`, `fill_labels(&tr) -> Labels`, `embedded_catalog(lang)` | Guidedog's words; `UI_DOMAIN`, `UI_TEMPLATE` |
| `format_date(fmt, &Moment, lang)` | strftime as Sphinx maps it to Babel; `Moment::{utc, from_unix_seconds, from_unix_seconds_at}` |
| `figure_filename`, `write_figure_filename`, `figure_for_language` | `{root}.{language}{ext}` and friends; `ImageLocalization`, `ImageExists` |

Pass behaviour: references in a translation are matched with the original's (named
references by name, footnotes and citations by label, substitutions by name,
cross-references by role and target) and take the original's resolution and context.
A reference the original lacks keeps the original text (`i18n.references.unknown`);
one the translation drops warns (`i18n.references.inconsistent`); markup the reader
refuses keeps the original (`i18n.translation.markup`). A section whose title is
translated gets an explicit id from the original title. Equal or fuzzy translations are
not applied. An untouched document costs one scan (`Unchanged`). `progress` adds the
`translated`/`untranslated` class; `images` localizes images without a catalog.

## Built-in catalogs

`locale/<language>/LC_MESSAGES/guidedog.po` (de, es, fr, ja, pt_BR, ru, zh_CN) are
compiled in; `locale/guidedog.pot` lists every msgid Guidedog looks up (the test
`embedded_catalogs_are_complete` checks each catalog translates all of them and keeps
each `%s`). The translations were written for Guidedog by its authors, not copied from
Sphinx's catalogs, although many msgids are Sphinx's English strings so a project's own
`sphinx.po` overrides them. To add a language, copy the template, fill in each msgstr,
set `Language` and `Plural-Forms`, and add it to `EMBEDDED` in `src/ui.rs`. A project
can override any word without rebuilding: `locales/<language>/LC_MESSAGES/guidedog.po`.

## Redesigns from the Odin package

- Translations are parsed in a reader workspace the pass owns and copied into the
  pass's builder, because a `guidedoc` pass cannot add sources or make a
  `ReadContext` on its own document. New nodes take the original message's span
  (Odin registered the catalog's pool as a source for exact spans); diagnostics quote
  the translation as text. Odin's builder marks and rollback became truncating the
  snapshot's `Vec` tables; reference matching pairs first and adopts only on success.
- `Translation` is plain `Copy` data with `Option<&dyn Lookup>` (any `Catalog` or
  `MoCatalog`); counts come back as `TranslationCounts` instead of fields written by
  the pass. `TranslationPass` keeps its buffers in a `RefCell` because passes are `Fn`.
- `Template` keeps locations and uuids itself and builds a `gd_gettext::Catalog` to
  write, instead of shuffling uuids in and out of the references; no allocators.
- Bit sets became structs of `bool`s (`Targets`, `Progress`); `Compact` is an enum with
  `Single(String)`; `index_parts` is an iterator (no 8-part cap); `Image_Exists`'s
  procedure+user pair is the `ImageExists` trait (closures implement it).
- Dates use the crate's own `Moment` (wall-clock fields, UTC offset, zone abbreviation)
  instead of `core:time/datetime` and a time-zone database: the host converts.
- `figure_filename` returns `Result<String, FigureFormatError>` (with a hint) instead of
  `(string, ok, why)`.
- `fill_labels` returns `Labels`; `ui_text` takes `Option<&Translator>`.
- Not ported: tests that need the Sphinx reader (`:ref:`/`:py:func:` roles, MyST with
  Sphinx roles, info fields read from source) are in `tests/sphinx.rs`, ignored until
  `guidedoc-sphinx` exists; `tests/shapes.rs` covers the same shapes with Builder-made
  documents and a toy inline reader.
