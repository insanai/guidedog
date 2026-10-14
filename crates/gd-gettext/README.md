# gd-gettext cheat sheet

GNU gettext message catalogs: PO/POT text, compiled MO files (both byte orders),
Plural-Forms expressions, msgmerge-style updates, and statistics. Standard library only;
independent of every other workspace crate. Ported from `guidedog_old/lib/gettext`.

```rust
let cat = gd_gettext::load_po("locales/de/LC_MESSAGES/index.po", false)?; // or load_mo
println!("{}", cat.text("Hello"));                          // gettext
println!("{}", cat.plural("%d file", "%d files", 3));       // ngettext
println!("{}", cat.with_context("menu", "Open"));           // pgettext
// On failure: eprint!("{err}") renders an Elm-style report with a hint.
```

Ownership: a `Catalog` owns its strings; lookups return `&str` borrowed from it (or the
`id` you passed). Parsers borrow their input and copy what they keep.

## Catalogs

| Item | Does |
|---|---|
| `load_po(path, allow_fuzzy) -> Result<Catalog, Error>` | read a .po or .pot file |
| `parse_po(text, name, allow_fuzzy)`, `parse_po_bytes(bytes, ..)` | the same from memory |
| `load_mo(path)`, `parse_mo(bytes, name)` | read a compiled catalog |
| `Catalog::new(name)`, `cat.add_message(m)`, `cat.message_count()` | build one |
| `cat.header_field("Language")`, `cat.set_header_field(name, value)` | the msgid "" header |
| `cat.charset()` | the header's charset; only UTF-8 (and ASCII) is accepted |

`Catalog { name, header: Option<Message>, messages, plural, allow_fuzzy }`. A `Message`
has `ctx: Option<String>` (msgctxt "" is `Some("")`), `id`, `id_plural: Option<String>`,
`strs`, `comments` (`# `), `notes` (`#.`), `references` (`#:`), `flags` (`#,` except
fuzzy), `fuzzy`, `obsolete` (`#~`), `previous: Option<Previous>` (`#|`), `line`.
`Message::new(id)`, `m.is_translated()`, `m.is_plural()`, `m.has_flag(f)`, `m.same_key(o)`.

## Lookups (methods on `Catalog`)

| Method | C | Returns |
|---|---|---|
| `text(id)` | gettext | the translation, or `id` |
| `plural(id, id_plural, n)` | ngettext | the form for n, or `id`/`id_plural` |
| `with_context(ctx, id)` | pgettext | |
| `plural_with_context(ctx, id, id_plural, n)` | npgettext | |
| `find(id, ctx) -> Option<Forms>` | | all forms (`len`, `get(k)`, `iter`) |
| `translation(id, ctx) -> Option<&str>` | | the singular, when translated |
| `pool() -> &str` | | the text every translation is a slice of |
| `build_index()` | | build now; required after editing `messages` directly |

Untranslated and fuzzy messages fall back to the msgid unless `allow_fuzzy`; an empty
plural form falls back to the English one.

## Domains: `Translator`

```rust
let mut tr = gd_gettext::Translator::new("pt_BR");
tr.bind_text_domain("guidedog", Path::new("locales"), false)?; // pt_BR/.., then pt/..
tr.add_catalog("guidedog", builtin);                            // a fallback
tr.text_domain("guidedog");
tr.text("Next");                    // also plural, with_context, plural_with_context
tr.domain_text("sphinx", "Next");   // dgettext; domain_plural, domain_with_context, ...
```

`language_candidates("pt-BR")` gives `pt-BR`, `pt_BR`, `pt`;
`find_catalog_file(dir, language, domain)` finds `dir/<lang>/LC_MESSAGES/<domain>.po`
(or `.mo`).

## Writing

| Item | Does |
|---|---|
| `write_po(&cat, &WriteOptions) -> String` | PO text, laid out as msgmerge does |
| `write_po_into(&mut String, &cat, &options)` | the same into a reused buffer |
| `save_po(&cat, path, &options) -> Result<(), Error>` | into a file |
| `write_mo(&cat, include_fuzzy) -> Result<Vec<u8>, Error>` | as msgfmt writes it |
| `escape_string(s) -> Cow<str>` | a C string body; borrows when nothing changes |

`WriteOptions { width: 76, references: ReferenceStyle::Wrapped, previous: true,
obsolete: true }` is the default (sphinx-intl's layout); `ReferenceStyle::OnePerLine`
is Sphinx's builder, `ReferenceStyle::None` drops locations; `width: 0` never wraps.

## Plurals

| Item | Does |
|---|---|
| `parse_plural_forms("nplurals=3; plural=...;") -> Result<PluralRule, Error>` | the header |
| `compile_plural(expr, nplurals) -> Result<PluralRule, PluralProblem>` | one expression |
| `plural_index(&rule, n) -> usize` | the form for n; out of range gives 0 |
| `plural_forms_for("ru") -> &'static str` | a rule for a new catalog |

C's subset: `n`, numbers, `( )`, `!`, `* / %`, `+ -`, `< > <= >=`, `== !=`, `&&`, `||`,
`?:`; unsigned 64-bit, division by zero gives 0. At most `MAX_PLURAL_NODES` (128) nodes
and `MAX_PLURAL_DEPTH` (100) levels. `PluralRule::default()` is `n != 1`.

## Merging and statistics

| Item | Does |
|---|---|
| `merge(&def, &template, &MergeOptions) -> Catalog` | msgmerge / sphinx-intl update |
| `merge_counted(..) -> (Catalog, MergeStats)` | also what the fuzzy search did |
| `similarity(a, b) -> f64` | 2·LCS / total length, the fuzzy measure |
| `stats(&cat) -> Stats`, `format_stats(&s)` | sphinx-intl stat |
| `sort_by_reference(&mut cat)` | by first location (file, then line number) |

New messages within `FUZZY_THRESHOLD` (0.6) of an old translated one get its translation,
marked fuzzy with the old msgid as `#|`. The search gives the every-pair answer via a
length-sorted, trigram-ranked candidate walk, bounded by `FUZZY_WORK_PER_BYTE` (256)
bytes per byte of msgids (at least `FUZZY_MIN_WORK`); `MergeStats` counts searches,
bounds, comparisons, work, and `cut_short`.

## Errors

`Error { kind, file, line, column, excerpt, message, hint }`; kinds `Syntax`,
`Duplicate`, `Charset`, `Encoding`, `Plural`, `MoFormat`, `Io`, `Memory`. `Display`:

```
-- PO SYNTAX ERROR ------------------------------------------------------ de.po:2

This string has no closing quote.

2| msgstr "b
          ^

Hint: End the string with " on the same line; continue on the next line with a new "...".
```

Hostile input costs linear time (n log n for MO): continued fields are gathered once, MO
files whose strings share bytes are refused, the fuzzy search is bounded, and wrapping
looks only at the next piece; `src/tests/cost.rs` checks each.
