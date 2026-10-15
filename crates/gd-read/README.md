# gd-read cheat sheet

Incremental, parallel reading of a Guidedog project's documents, and the hosts of the
generating directives and API pages. Ported from `guidedog_old/lib/project` (read.odin,
collect.odin, the search-text half of search.odin, autodoc.odin, odin.odin, the
`Translating` hook of intl.odin, and the `plugin_sources` call of plugins.odin), plus the
Rust API pages (new).

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
| `read_documents(&ctx, previous, &sources) -> Result<ReadOutcome, Problem>` | the new `Environment` and the counts `read`, `reused`, `unread` |
| `reread_document`, `stale_warning` | reading a document again when its cached object is stale (gd-intl's `Reread`, the page writers) |
| `api_sources(&config, &taken, &mut reports)` | the generated Odin (`<odin_autoapi_root>/...`) and Rust (`<rust_autoapi_root>/...`) pages |
| `sphinx_config`, `myst_settings` | conf.toml as the `sphinx` and `myst` readers' settings |

Reports: `build.reader` (problem), `build.read`, `build.object`, `host.utf8`, `host.io`,
plugin codes, every reader and autodoc code (`autodoc.*` with Sphinx's categories
`autodoc` / `autodoc.import_object`, `odin.autodoc.*`, `autodoc.paths`,
`odin.autodoc.paths`), `rust.autodoc.*` and `odin.autoapi.clash` /
`rust.autoapi.clash` from `api_sources`, `build.object.stale`.

## Redesign notes

- **Each source is read once.** Odin read every source for its digest on the main
  thread, then again on the thread that parsed it. Here the thread that takes a document
  reads it, digests it, compares with the previous `DocInfo` and parses only when it
  changed. Results are placed by the source's position, so the environment is the same
  whatever the scheduling (`threads_do_not_change_the_environment`).
- **Workers own their workspace** (`gd_host::Session`), taken from `gd_host::run_workers`;
  each joins the budget while it reads. No arenas, mutex allocators or meters.
- **Reused documents are moved**, not copied, out of the previous environment, with
  `written` cleared (only this build's objects are flushed at commit). Reuse also checks
  that the markup, path and generated flag are unchanged.
- **Dependencies hash file bytes**, as `gd_env::dependencies_changed` does: an included
  file's digest is its bytes' (not the decoded text's), so a file with a byte order mark
  is not read again every build.
- **Providers are traits.** The Python and Odin hosts implement
  `guidedoc_sphinx::AutodocProvider`. gd-pyscan and gd-odindoc projects are not `Send`
  (`Rc` modules, boxed readers), and providers must be, so each directive analyses in a
  project of its own while the document's files are read from disk once and cached in a
  shared, confined log (`files.rs`) that becomes its dependencies. A per-document session
  across directives (Odin) would need the session on one thread; the cost is reparsing
  per directive, never rereading.
- **Static messages.** The engine reports `&'static` messages; gd-autodoc and gd-odindoc
  messages are reported with their code and title from a table (checked against those
  crates' sources by a test) and their text and hint as arguments, so braces in them are
  never read as a template (Odin's `safe_braces`).
- **Rust API pages** (new): `rust_autoapi_crates` are read by gd-rustdoc into generated
  pages `<root>/<crate>/<module>` and `<root>/index` (one crate's index, or every crate's
  under "Rust API reference"); each page depends on every file of its crate (digests of
  their bytes), so editing the crate reads its pages again. Load and page warnings are
  reported every build, as the pages are generated every build. The object collector
  joins a rust signature's module and name with `::`, so targets are full paths.
- **Translation** is gd-i18n's pass on the request (`Request::passes`), with gd-intl's
  catalog and catalog files as dependencies; localized figures check the source folder.
- **Search words** are written only when a document has some (`DocInfo::search` is
  `None` otherwise).
- Not ported: the budget metering of the Odin package listing (packages are listed with
  owned memory), the collector's anchor-table retries (`build_anchors` cannot fail).
