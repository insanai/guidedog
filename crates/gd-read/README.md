# gd-read cheat sheet

Incremental, parallel reading of a Guidedog project's documents, and the hosts of the
generating directives and API pages: reading and collecting symbols, search text,
Python autodoc, the odin domain's directives and pages, translation while reading, the
sources build plugins add, and the Rust API pages.

```rust
let mut reports = Vec::new();
let found = gd_env::discover(&config)?;
let pages = gd_read::api_sources(&config, &found, &mut reports);   // odin + rust pages
let sources = gd_env::with_generated(found, pages);
let ctx = gd_read::ReadContext { config: &config, registry: &registry, store: &store,
    intl: &intl, plugins: &plugins, budget: &budget, policy, limits,
    settings, today: &dates.today, date: &dates.date, jobs: 0 };
let outcome = gd_read::read_documents(&ctx, previous, &sources)?; // Err only build.reader
println!("{} parsed, {} unchanged, {} failed", outcome.read, outcome.reused, outcome.unread);
// A writer met a stale object (gd_env::ObjectError { stale: true, .. }):
reports.push(gd_read::stale_warning(&error.problem, doc));
let fresh = gd_read::reread_document(&ctx, source, doc)?;          // new object, same digest
```

| Item | Does |
|---|---|
| `ReadContext` (`thread_count`) | what reading borrows from the build: config, registry, object store, `gd_intl::Intl`, `gd_env::PluginRun`, budget, policy, limits, settings digest, dates, `-j` |
| `read_documents(&ctx, previous, &sources) -> Result<ReadOutcome, Problem>` | the new `Environment`, the counts `read`, `reused`, `unread`, and `reading_times: Vec<ReadingTime { docname, time }>` of the documents parsed |
| `reread_document`, `stale_warning` | reading a document again when its cached object is stale (gd-intl's `Reread`, the page writers) |
| `api_sources(&config, &taken, &mut reports)` | the generated Odin (`<odin_autoapi_root>/...`) and Rust (`<rust_autoapi_root>/...`) pages |
| `sphinx_config`, `myst_settings` | conf.toml as the `sphinx` and `myst` readers' settings |

Reports: `build.reader` (problem), `build.read`, `build.object`, `host.utf8`, `host.io`,
plugin codes, every reader and autodoc code (`autodoc.*` with Sphinx's categories
`autodoc` / `autodoc.import_object`, `odin.autodoc.*`, `autodoc.paths`,
`odin.autodoc.paths`), `inheritance.class`, `inheritance.limit`, `viewcode.source`,
`viewcode.limit`, `rust.autodoc.*` and `odin.autoapi.clash` /
`rust.autoapi.clash` from `api_sources`, `build.object.stale`.

## Design notes

- **Each source is read once.** The thread that takes a document reads it, digests
  it, compares with the previous `DocInfo` and parses only when it changed. Results
  are placed by the source's position, so the environment is the same whatever the
  scheduling (`threads_do_not_change_the_environment`).
- **Workers own their workspace** (`gd_host::Session`), taken from `gd_host::run_workers`;
  each joins the budget while it reads.
- **Reused documents are moved**, not copied, out of the previous environment, with
  `written` cleared (only this build's objects are flushed at commit). Reuse also checks
  that the markup, path and generated flag are unchanged.
- **Dependencies hash file bytes**, as `gd_env::dependencies_changed` does: an included
  file's digest is its bytes' (not the decoded text's), so a file with a byte order mark
  is not read again every build.
- **Providers are traits.** The Python and Odin source hosts implement
  `guidedoc_sphinx::AutodocProvider`. Each host keeps one gd-autodoc session (Python) or
  gd-odindoc project (Odin) per document behind a `Mutex`, made at the first directive:
  a module is parsed once per document, not once per directive, and the session's merged
  annotations carry over between directives as in Sphinx. The document's files are read
  from disk once and recorded in a shared, confined log (`files.rs`) that becomes its
  dependencies.
- **Source scan for the GDS 0011 extensions.** With sphinx.ext.viewcode or linkcode
  listed, each document parsed records `DocInfo::locations`: for every `py`
  description, the module and lines gd-pyscan finds it defined at (`sources.rs`), its
  Python files becoming dependencies. sphinx.ext.inheritance_diagram's directive is
  served by a provider registered as `guidedoc_sphinx::INHERITANCE_DIAGRAM`
  (`inheritance.rs`), which walks the bases in the scan and answers with a `graphviz`
  directive whose node URLs are `gd-xref:py:class:<name>` references for the link
  pass, and a list of the classes for text.
- **Reading times** are measured around each document's read, on the thread that
  reads it, with `std::time::Instant` (sphinx.ext.duration).
- **Static messages.** The engine reports `&'static` messages; gd-autodoc and gd-odindoc
  messages are reported with their code and title from a table (checked against those
  crates' sources by a test) and their text and hint as arguments, so braces in them are
  never read as a template.
- **Rust API pages**: `rust_autoapi_crates` are read by gd-rustdoc into generated
  pages `<root>/<crate>/<module>` and `<root>/index` (one crate's index, or every crate's
  under "Rust API reference"); each page depends on every file its crate was made from
  (`gd_rustdoc::Crate::inputs`, digests of their bytes), so editing the crate reads its
  pages again. The crates are documented in parallel, and `<doctrees>/rust-api.json`
  (`rustcache.rs`) keeps each crate's pages and reports for the next build, reused only
  when the executable, path, options, file listing below the crate folder (without
  `target/` and hidden folders) and every input's bytes are unchanged: a no-op build
  parses no crate. Load and page warnings are reported every build. The object
  collector joins a rust signature's module and name with `::`, so targets are full
  paths.
- **One digest per file per build**: the reading threads share a memo of file digests,
  seeded with those the generated pages were made with, so a crate file every API page
  depends on is hashed once, not once per page.
- **Content errors are warnings** (Sphinx's rule): a reader error in a document that was
  read anyway is reported as a warning, so the build succeeds and `-W` fails it.
- **Translation** is gd-i18n's pass on the request (`Request::passes`), with gd-intl's
  catalog and catalog files as dependencies; localized figures check the source folder.
- **Search words** are written only when a document has some (`DocInfo::search` is
  `None` otherwise).
- Odin packages are listed with owned memory, outside the budget, and the collector
  builds its anchor table in one call (`build_anchors` cannot fail).
