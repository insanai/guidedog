# gd-project cheat sheet

The Guidedog project builder: load a project, build it with one builder, clean it.
It starts and stops the build plugins and applies the trust policy and the build
limits.

```rust
use gd_project::{BuildOptions, BuilderKind, LoadOptions};

let project = gd_project::load("docs".as_ref(), &LoadOptions {
    plugins: linked,                       // gd_plugins::Registry, before the machine's
    ..Default::default()                   // registry: None = guidedoc_defaults::registry()
})?;                                       // Err: core.registry.invalid, config.invalid, ...
let result = gd_project::build(&project, BuilderKind::Html,
    &BuildOptions { warnings_as_errors: true, ..Default::default() });
println!("{} parsed, {} written", result.read, result.written);
if let Some(problem) = &result.problem { /* exit = result.exit */ }
gd_project::clean(&project)?;
let roots = gd_project::watch_roots(&project.config); // for gd_serve::Watch
```

| Item | Does |
|---|---|
| `LoadOptions { config, plugins, registry }`, `Project { config, warnings, settings, registry, plugins }`, `load` | a loaded project |
| `build(&Project, BuilderKind, &BuildOptions) -> BuildResult` | discover, translate, read, resolve, write, publish |
| `clean(&Project)` | empties the build folder |
| `watch_roots(&Config)`, `Project::watch_skip` | what `guidedog serve` watches |
| `build_limits`, `trusted_policy`, `untrusted_policy` | limits and content policy of a build |
| `BuildOptions`, `BuildResult`, `BuildStage`, `BuilderKind`, `Jobs` | re-exported from gd-env |

Problems: `core.registry.invalid` (load), `build.stack`, `build.reader`, `build.read`
(a document could not be read), `build.warnings` (`-W`), `plugins.untrusted`, and
those of the crates below (`host.budget`, `build.output`, `typst.*`, ...).

The pipeline and each step's crate: `docs/manual/internals/project.rst` (S8).

## Design notes

- **No arenas, memory guards or `release`.** A `Project` and a `BuildResult` are
  owned values, released on drop; there is no `host.memory` problem (Rust aborts on
  allocation failure). Only the coarse `gd_host::Budget` counts memory, with `BuildOptions::on_budget`
  deciding at the budget.
- **A build borrows its project.** `build(&Project, ..)` clones the configuration, so
  the type system enforces that a build never changes the project (an untrusted build
  confines only its copy).
- **No `Build` god object.** The build is a few stages over small structs (`Setup`,
  `Parts`, `Log`); builders get a borrowed `gd_resolve::BuildView` and are chosen in one
  `match` (`write.rs`).
- **Counters from the generation.** `OutputBuilder::write` returns nothing, so
  `written`/`unchanged` are the generation's units written and kept besides the site
  unit (`Generation::written_count`, `reused_count`, `is_site_written`); the site
  and search units are not pages, singlehtml and gettext count their one output.
- **The registry is a load option** (`LoadOptions::registry`, default
  `guidedoc_defaults::registry()`), so an embedder chooses the readers and renderers;
  the linked plugins are `LoadOptions::plugins`.
- **Stack limit**: documents are read and pages rendered on `gd_host::WORKER_STACK_BYTES`
  threads (the CLI runs on one too), so `build_limits` checks `--stack-kib` against that
  thread size instead of measuring the main thread.
- **What the budget does not charge**: search words, page layouts and the Odin package
  listing are small reads that gd-site and gd-read do not charge, so the budget tests
  cover the saved state, the catalogs and the budget policy.
