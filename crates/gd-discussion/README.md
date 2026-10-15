# gd-discussion cheat sheet

Discussion catalogs (GDS 0007): proposals and decisions kept as reStructuredText records
under a prefix (`gds`, `sds`, `eds`), ported from `guidedog_old/lib/discussion`. Depends
on `gd-report`, `gd-toml` and `gd-json`. Design notes:
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
| `build(&c, &BuildOptions, &dyn Builder)`, `Builder`, `BuildRequest`, `GuidedogProgram` | publication |
| `command::run(&Program, &args)`, `Program::default()` | the `dsc` command line |
| `Problem { code, title, path, line, excerpt, message, hint, suggestion, exit }` | `report()` gives a `gd_report::Report` |

## Builders

`Builder::build(&self, &BuildRequest) -> Result<PathBuf>` makes one target (HTML site or
PDF books) of an already checked catalog. `GuidedogProgram` runs the installed
`guidedog` (`--guidedog`, `$DSC_GUIDEDOG`, `PATH`) with `std::process::Command`, no
shell, books passed as a quoted `pdf_documents` setting. `dscc` will implement `Builder`
over gd-project's in-process builder and pass it as `Program::make_builder`; until
gd-project lands, `dscc` uses `GuidedogProgram` too. `build_typst_catalog` (older Typst
catalogs) defaults to a `legacy-build` problem.

## Redesigns

- No allocator watch or `discussion.memory`: Rust aborts on allocation failure, so the
  Odin refusal sweeps became interruption sweeps over `apply`.
- `apply` and `recover` take the `Lock`, so a change cannot be written unlocked; the lock
  uses `File::try_lock` (flock) and retries for 200 ms (forked children briefly hold
  copies of descriptors).
- `Change { before: Option, after: Option }` replaces `existed`/`present` (the journal
  keeps the Odin JSON form); `Catalog` carries its folder; `Record` keeps `Sections`, no
  text; one `Options` split into `NewDraft`, `Decision`, `BuildOptions`, `Init`.
- `replace_field` takes a typed `FieldValue`, so invalid literals cannot be passed.
- Problems carry a line, an excerpt with caret (configuration mistakes) and a
  "Did you mean" suggestion (commands, options, states, labels, settings, records).
