# gd-gettext cheat sheet

GNU gettext message catalogs: PO/POT text, compiled MO files (both byte orders),
Plural-Forms expressions, msgmerge-style updates, and statistics. Standard library only;
independent of every other workspace crate. Ported from `guidedog_old/lib/gettext`
(behaviour kept, API redesigned for Rust; see *Redesigns* below). For large catalogs,
see [docs/manual/internals/gettext.rst](../../docs/manual/internals/gettext.rst): research, design, and measurements.

```rust
use gd_gettext::{LoadOptions, Lookup, MoCatalog};
let mo = MoCatalog::load("locales/de/LC_MESSAGES/index.mo", &LoadOptions::default())?;
println!("{}", mo.text("Hello"));                          // gettext
println!("{}", mo.plural("%d file", "%d files", 3));       // ngettext
println!("{}", mo.with_context("menu", "Open"));           // pgettext
// On failure: eprint!("{err}") renders an Elm-style report with a hint.
```

Ownership: a `Catalog` owns its strings; a `MoCatalog` owns one buffer (the file) and
serves `&str` borrowed from it. Lookups return borrowed strings (or the `id` you
passed). Parsers borrow their input and copy only what an editable catalog keeps.

## Which catalog

| Type | Made by | For |
|---|---|---|
| `MoCatalog` | `MoCatalog::load(path, &opts)`, `MoCatalog::from_bytes(vec, name)` | serving lookups: zero-copy, O(1) through the file's hash table |
| `Catalog` | `load_po(path, &opts)`, `parse_po(text, name)`, `parse_po_bytes`, `load_mo`, `parse_mo`, `Catalog::new(name)` | editing, merging, writing |
| `PoReader` | `PoReader::new(buf_read, name)`, `PoReader::open(path, &opts)` | streaming a PO file entry by entry |

`LoadOptions { max_bytes: 1 GiB, allow_fuzzy: false }` is the default; a larger file is
an `ErrorKind::TooLarge` error. `Catalog::from(&mo)` copies a `MoCatalog`.

## Lookups: the `Lookup` trait (`Catalog`, `MoCatalog`, your own)

| Method | C | Returns |
|---|---|---|
| `text(id)` | gettext | the translation, or `id` |
| `plural(id, id_plural, n)` | ngettext | the form for n, or `id`/`id_plural` |
| `with_context(ctx, id)` | pgettext | |
| `plural_with_context(ctx: Option, id, id_plural, n)` | npgettext | |
| `find(id, ctx) -> Option<Forms>` | | all forms (`len`, `get(k)`, `iter`) |
| `translation(id, ctx) -> Option<&str>` | | the singular, when translated |
| `plural_rule() -> &PluralRule` | | the compiled Plural-Forms |

Untranslated and fuzzy messages fall back to the msgid unless `cat.allow_fuzzy`; an
empty plural form falls back to the English one. `Catalog` only: `pool()` (the text
every translation is a slice of) and `build_index()` (required after editing `messages`
directly). `MoCatalog` only: `header()`, `header_field(name)`, `entries()` (borrowed
`MoEntry { ctx, id, id_plural, translation }`), `entry_count()`, `search()`
(`MoSearch::Hash`/`Sorted`/`Permuted`).

## Catalog data

`Catalog { name, header: Option<Message>, messages, plural, allow_fuzzy }`; methods
`add_message`, `message_count`, `header_field`, `set_header_field`, `charset`. A
`Message` has `ctx: Option<String>` (msgctxt "" is `Some("")`), `id`,
`id_plural: Option<String>`, `strs`, `comments` (`# `), `notes` (`#.`), `references`
(`#:`), `flags` (`#,` except fuzzy), `fuzzy`, `obsolete` (`#~`),
`previous: Option<Previous>` (`#|`), `line`. Helpers: `Message::new(id)`,
`is_translated`, `is_plural`, `is_header`, `has_flag`, `same_key`.

## Domains: `Translator`

```rust
let mut tr = gd_gettext::Translator::new("pt_BR");
tr.bind_text_domain("guidedog", Path::new("locales"), &LoadOptions::default())?;
tr.add_catalog("guidedog", builtin);        // any Lookup + Send + Sync: a fallback
tr.text_domain("guidedog");
tr.text("Next");                            // also plural, with_context, plural_with_context
tr.domain_text("sphinx", "Next");           // dgettext; domain_plural, domain_with_context, ...
```

`bind_text_domain` tries `pt_BR`, then `pt` (`language_candidates`), `.po` then `.mo`
(`find_catalog_file`); a `.mo` is bound as a zero-copy `MoCatalog`.

## Writing

| Item | Does |
|---|---|
| `write_po(&cat, &WriteOptions) -> String`, `write_po_into(&mut String, ..)` | PO text, laid out as msgmerge does |
| `write_po_to(&mut impl Write, ..)`, `save_po(&cat, path, ..)` | streamed, one entry at a time |
| `write_mo(&cat, include_fuzzy) -> Result<Vec<u8>, Error>` | the bytes msgfmt writes (sorted, hashed) |
| `write_mo_to(&mut impl Write, ..)`, `save_mo(&cat, path, ..)` | streamed from the catalog's strings |
| `escape_string(s) -> Cow<str>` | a C string body; borrows when nothing changes |

`WriteOptions { width: 76, references: ReferenceStyle::Wrapped, previous: true,
obsolete: true }` is the default (sphinx-intl's layout); `OnePerLine` is Sphinx's
builder, `ReferenceStyle::None` drops locations; `width: 0` never wraps.

## Plurals

`parse_plural_forms("nplurals=3; plural=...;") -> Result<PluralRule, Error>`,
`compile_plural(expr, nplurals) -> Result<PluralRule, PluralProblem>`,
`plural_index(&rule, n) -> usize` (out of range gives 0), `plural_forms_for("ru")`.
C's subset: `n`, numbers, `( )`, `!`, `* / %`, `+ -`, `< > <= >=`, `== !=`, `&&`, `||`,
`?:`. Arithmetic is unsigned 64-bit, and division by zero gives 0. A rule has at most
128 nodes and 100 levels. `PluralRule::default()` is `n != 1`.

## Merging and statistics

| Item | Does |
|---|---|
| `merge(&def, &template, &MergeOptions) -> Catalog` | msgmerge / sphinx-intl update |
| `merge_counted(..) -> (Catalog, MergeStats)` | also what the fuzzy search did |
| `similarity(a, b) -> f64` | 2·LCS / total length, the fuzzy measure |
| `stats(&cat)`, `Stats::add(&message)`, `format_stats(&s)` | sphinx-intl stat (streamable) |
| `sort_by_reference(&mut cat)` | by first location (file, then line number) |

New messages within `FUZZY_THRESHOLD` (0.6) of an old translated one get its translation,
marked fuzzy, with the old msgid as `#|`. The search finds the same match comparing
every pair would, using a length-sorted, trigram-ranked candidate walk. It is bounded
by `FUZZY_WORK_PER_BYTE` (256) bytes per byte of msgids (at least `FUZZY_MIN_WORK`).

## Errors

`Error { kind, file, line, column, excerpt, message, hint }`; kinds `Syntax`,
`Duplicate`, `Charset`, `Encoding`, `Plural`, `MoFormat`, `Io`, `Memory`, `TooLarge`.
`Display`:

```
-- PO SYNTAX ERROR ------------------------------------------------------ de.po:2

This string has no closing quote.

2| msgstr "b
          ^

Hint: End the string with " on the same line; continue on the next line with a new "...".
```

Hostile input costs linear time (n log n for MO checks). Continued PO fields are
gathered once. MO files whose strings share bytes or repeat keys are refused. A hash
table is trusted only after every key is found within 64 probes, so lookups stay
bounded. The fuzzy search is bounded, and wrapping looks only at the next piece.
`src/tests/cost.rs`, `mo_compat.rs` and `fuzz.rs` check these.

## Redesigns from the Odin package

- The Odin package had arenas, `destroy*`, `catalog_refused` and `catalog_bytes`. Here
  values own their data and `Drop` frees them; refused allocations become
  `ErrorKind::Memory` through `try_reserve`.
- `Option` replaces sentinels and flags: `has_ctx`/`has_plural`/`has_header`/
  `has_previous`, the `-1` best match, and the plural parser's `-1` node (its rules now
  return `Result` and use `?`).
- Lookups are one trait, `Lookup`, instead of procedure groups. `Translator` binds any
  `Lookup + Send + Sync`, and catalogs are `Sync` (`OnceLock` index).
- Options structs replace bare `bool`s: `LoadOptions` for loading;
  `parse_po(text, name)` drops `allow_fuzzy` (set the field instead).
- MO reading is zero-copy (`MoCatalog`) and uses the file's hash table. MO writing
  hashes the singular msgid as GNU does (the Odin writer hashed the plural too) and
  matches msgfmt's output byte for byte.
- PO reading streams (`PoReader`); writers take any `io::Write`.
- Not ported: the allocator-refusal tests (`memory_test.odin`, `refusal_test.odin`),
  because Rust aborts on allocation failure where `try_reserve` is not used. Also not
  ported: MO revision-1 system-dependent strings (glibc-only `<PRIu64>` segments; such
  entries are ignored, as is a hash table that points at them).
