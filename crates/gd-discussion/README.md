# gd-discussion cheat sheet

Discussion catalogs (GDS 0007): proposals and decisions kept as reStructuredText records
under a prefix (`gds`, `sds`, `eds`). Depends
on `gd-report`, `gd-toml`, `gd-json`, `gd-clock`, `gd-plugins` (the plugin interface) and
`gd-typst` (older Typst catalogs). Design notes:
`docs/manual/internals/discussion.rst`.

```rust
use gd_discussion as ds;
let prefix = ds::Prefix::new("eds")?;
let lock = ds::lock_catalog(dir, &prefix)?;          // released on drop
let catalog = ds::load_catalog(dir, &prefix)?;       // metadata only, never record text
let plan = ds::state_plan(&catalog, &ds::Decision {
    identifier: "0003".into(), to: "accepted".into(), reason: "Converged".into(),
    ..Default::default() })?;                        // changes nothing yet
ds::apply(&lock, &plan)?;                            // journaled; ds::recover rolls back
```

| Item | Does |
|---|---|
| `Prefix::new`, `is_valid_prefix` | 2 to 16 lowercase letters; names fields, lock, journal, codes |
| `init_plan(&Init)`, `initialize(&Init)` | plan, or make whole (staged, checked, renamed), a new catalog |
| `definition(dir)`, `discover(from)` | `discussion.toml` (or 0.2.1 `.discussion.json`); the catalog a folder is in |
| `read_config`, `render_config`, `Definition`, `Process`, `State`, `Numbering` | configuration; `Process::default()` is GDS 0001's |
| `load_catalog(dir, &prefix) -> Catalog` | records sorted by number, reserved numbers, labels, process |
| `resolve(&catalog, id)` | the one record a number, slug, title, or file name names |
| `new_plan(&c, &NewDraft)`, `promote_plan`, `state_plan(&c, &Decision)` | record changes, index included |
| `index_plan(&c)`, `check_index(&c)`, `check_sources(&c)`, `next_number` | the generated index; required sections |
| `lock_catalog`, `apply(&Lock, &Plan)`, `recover(&Lock)`, `check_recovery` | transactions |
| `read_record`, `replace_field(src, &prefix, key, FieldValue)` | metadata; one line changed, every other byte kept |
| `build(&c, &BuildOptions, &mut dyn Builder)`, `Builder`, `BuildRequest` | publication |
| `plugin()`, `ID` | the `guidedog dsc` commands, as a plugin for guidedog to link |
| `Problem` (boxed `ProblemData { code, title, path, line, excerpt, message, hint, suggestion, exit }`, one pointer wide) | `report()` gives a `gd_report::Report`; `with_path`, `with_title`, `with_line`, `exit`, `suggest`, `at_line` chain |

## The plugin

`plugin()` is the discussion plugin (ID `dsc`), which guidedog links: it adds
`guidedog dsc init`, `new`, `list`, `show`, `promote`, `state`, `index`, `check`,
`build`, `recover` and `config`, run on the catalog the working folder is in (or
`--dir`) with what the host's `CommandContext` offers. It adds commands only, no build
hooks. Every catalog is its own: any prefix, its own workflow, numbering and labels in
`discussion.toml`; Guidedog's `docs/gds` is one of them.

## Builders

`build(&catalog, &BuildOptions, &mut dyn Builder)` checks the catalog and makes its
targets. `Builder::build(&mut self, &BuildRequest) -> Result<PathBuf>` makes one target
(HTML site or PDF books) into `BuildRequest::output()`. The plugin's builder asks the
host to build the catalog as a Guidedog project (`CommandContext::build_project`),
strict, with the books as a quoted `pdf_documents` setting. An older Typst catalog
compiles record by record with gd-typst (`legacy.rs`: the embedded compiler, or the
program `--typst` names), staged and themed like the HTML and PDF of a current catalog.

## Design notes

- No allocator watch or `discussion.memory`: Rust aborts on allocation failure, so the
  tests sweep interruptions over `apply` instead of allocation refusals.
- `apply` and `recover` take the `Lock`, so a change cannot be written unlocked; the lock
  uses `File::try_lock` (flock) and retries for 200 ms (forked children briefly hold
  copies of descriptors).
- `Change { before: Option, after: Option }` says whether a file existed before and
  after (the journal keeps the JSON form older releases wrote); `Catalog` carries its folder; `Record` keeps `Sections`, no
  text; one `Options` split into `NewDraft`, `Decision`, `BuildOptions`, `Init`.
- `replace_field` takes a typed `FieldValue`, so invalid literals cannot be passed.
- The command line is a plugin of guidedog (`guidedog dsc`): one command for every
  catalog, builds by the host.
- Problems carry a line, an excerpt with caret (configuration mistakes) and a
  "Did you mean" suggestion (commands, options, states, labels, settings, records).

Catalog initialization writes `_templates/discussion.typ` and `_templates/theme.typ` from the embedded Guidedog PDF theme. The discussion layout keeps its own metadata panel and contents; ordinary books use a separate layout.
