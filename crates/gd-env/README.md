# gd-env cheat sheet

The foundation of a Guidedog build: the vocabulary every builder crate shares, the
project's documents, the environment reading keeps between builds, the object store,
lookups and traces, the settings digests, and the build plugins one build runs.

```rust
let sources = gd_env::discover(&config)?;                 // sorted by docname
let env = gd_env::load_environment(&gd_env::environment_path(&config),
    settings, options.fresh, &budget)?;                    // empty when stale
let store = gd_env::ObjectStore::new(gd_env::doctrees_dir(&config));
let object = store.save_object("guide/intro", document)?;  // objects/guide/intro.<16>.gdo
let doc = store.load_object(&mut ws, &object, &limits)?;   // Err(ObjectError { stale, .. })
let run = gd_env::PluginRun::start(gd_env::PluginStart {
    config: &config, linked: &registry, kind, untrusted })?; // stops on Drop
let text = run.process_source(&source, &config.language, &text)?;
```

| Item | Does |
|---|---|
| `BuilderKind` (`name`, `from_name`, `is_html`, `format`), `OutputFormat`, `BuildOptions`, `Jobs`, `BuildResult`, `BuildStage`, `BUILD_BUDGET` | build vocabulary |
| `SourceFile { docname, path, relative, kind, generated: Option<Generated> }`, `SourceKind`, `discover`, `glob_match`, `is_excluded`, `contents_root`, `with_generated` | discovery |
| `Environment { settings, docs }`, `DocInfo` and its infos, `DocId(u32)`, `ENVIRONMENT_VERSION` (27), `load_environment`, `dependencies_changed` | the environment |
| `SourceLocation { domain, module, defined_in, fullname, anchor, path, line, end_line }` (`DocInfo::locations`) | where a described Python object is defined (viewcode, linkcode) |
| `normalize_name`, `option_key` | the keys labels, terms and options are filed under |
| `ObjectStore` (`save_object`, `load_object`, `save_search`, `load_bytes`, `sweep`), `CachedFile` | content-addressed cache |
| `Lookup { kind: LookupKind, key }`, `Trace` | verifying traces |
| `Asset`, `Rendered { body, assets, trace, reports, failed }`, `UiText`, `EnglishUi` | rendering |
| `reading_settings`, `build_settings`, `output_settings`, `executable_stamp`, `file_digest` | digests |
| `PluginRun`, `PluginStart`, `host_problem`, `assets_url` | build plugins |
| `ObjectBuilder`, `ObjectReader`, `read_record`, `report_to_json`, `report_from_json` | JSON records |

## Design notes

- **Option instead of sentinels**: `parent: Option<u32>`, `object:
  Option<CachedFile>`, `Option<DocId>`; no `-1`, `""` or `NO_DOC` stands for none.
- **Digests are values** (`gd_conf::Digest`, 32 bytes with hex `Display`), not hex
  strings; `CachedFile` pairs a cache path with its digest.
- **Lookups are typed**: `Lookup { kind, key }`, not `"L" + key` strings;
  `encode`/`decode` write one letter per kind in the manifest.
- **Explicit JSON**: the environment is written and read field by field with gd-json
  (`ObjectBuilder`/`ObjectReader`); a record missing a field is refused whole.
  `written` is not saved: it describes one build only.
- **Plugins**: one `Mutex<PluginSession>` per plugin per build, stopped on `Drop`;
  gd-plugins problems become host problems with a known code (`host_problem`).
  `process_source`/`process_html` return `Cow`, borrowing unchanged text.
- **Ownership, not arenas**: records are owned values; the build budget is checked
  before work, so JSON is parsed without a meter.
- `BuildOptions` takes `Option`s for its limits and `Jobs` for `-j`.
- `BuildResult::steps` holds extra summary lines as (step, text), such as
  sphinx.ext.duration's reading times, so the command line prints them without knowing
  each extension.
