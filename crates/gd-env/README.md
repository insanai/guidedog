# gd-env cheat sheet

The foundation of a Guidedog build: the vocabulary every builder crate shares, the
project's documents, the environment reading keeps between builds, the object store,
lookups and traces, the settings digests, and the build plugins one build runs. Ported
from `guidedog_old/lib/project` (environment.odin, discover.odin, bookkeeping.odin,
plugins.odin, and the types of build.odin, link.odin and render_page.odin).

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
| `Environment { settings, docs }`, `DocInfo` and its infos, `DocId(u32)`, `ENVIRONMENT_VERSION` (26), `load_environment`, `dependencies_changed` | the environment |
| `ObjectStore` (`save_object`, `load_object`, `save_search`, `load_bytes`, `sweep`), `CachedFile` | content-addressed cache |
| `Lookup { kind: LookupKind, key }`, `Trace` | verifying traces |
| `Asset`, `Rendered { body, assets, trace, reports, failed }`, `UiText`, `EnglishUi` | rendering |
| `reading_settings`, `build_settings`, `output_settings`, `executable_stamp`, `file_digest` | digests |
| `PluginRun`, `PluginStart`, `host_problem`, `assets_url` | build plugins |
| `ObjectBuilder`, `ObjectReader`, `report_to_json`, `report_from_json` | JSON records |

## Redesign notes

- **Option instead of sentinels**: `parent: Option<u32>` (was -1), `object:
  Option<CachedFile>` (was two strings, `""` for none), `Option<DocId>` (was `NO_DOC`).
- **Digests are values** (`gd_conf::Digest`, 32 bytes with hex `Display`), not hex
  strings; `CachedFile` pairs a cache path with its digest.
- **Lookups are typed**: `Lookup { kind, key }` replaces `"L" + key` strings;
  `encode`/`decode` keep the old letters for the manifest.
- **Explicit JSON**: the environment is written and read field by field with gd-json
  (`ObjectBuilder`/`ObjectReader`); a record missing a field is refused whole.
  `written` is not saved (Odin's `json:"-"`).
- **Plugins**: one `Mutex<PluginSession>` per plugin per build, stopped on `Drop`;
  gd-plugins problems become host problems with a known code (`host_problem`).
  `process_source`/`process_html` return `Cow`, borrowing unchanged text.
- **Dropped**: arenas, scratch allocators and metered JSON parsing (`load_json`,
  `save_json` with a meter); `BuildOptions.heap`, `arena_block` and `year`.
- `BuildOptions` takes `Option`s for its limits and `Jobs` for `-j`.
