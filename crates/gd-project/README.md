# gd-project cheat sheet

The Guidedog project builder: load a project, build it with one builder, clean it.
Ported from `guidedog_old/lib/project` (the rest of project.odin and build.odin:
`load`, `build`/`run_build`, `build_config`, `untrusted_policy`, `build_limits`,
`finish_build`, `publication_problem`, `unsuppressed`, `clean`; the start/stop of
plugins.odin) and the Odin tests registry_test, trust_test and budget_test.

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

## Redesign notes

- **No arenas, memory guards or `release`.** A `Project` and a `BuildResult` are
  owned values; `unload`/`release`/`open_memory`, `Build_Options.heap`/`arena_block`,
  step allocators and the `host.memory` problem are gone (Rust aborts on allocation
  failure). Only the coarse `gd_host::Budget` stays, with `BuildOptions::on_budget`
  deciding at the budget.
- **A build borrows its project.** `build(&Project, ..)` clones the configuration, so
  the type system enforces that a build never changes the project (an untrusted build
  confines only its copy). Odin's `build_config` copy of two fields is not needed.
- **No `Build` god object.** The build is a few stages over small structs (`Setup`,
  `Parts`, `Log`); builders get a borrowed `gd_resolve::BuildView` and are chosen in one
  `match` (`write.rs`).
- **Counters from the generation.** `OutputBuilder::write` returns nothing, so
  `written`/`unchanged` are the generation's units written and kept besides the site
  unit (new `Generation::written_count`, `reused_count`, `is_site_written`); the site
  and search units are not pages, singlehtml and gettext count their one output.
- **The registry is a load option** (`LoadOptions::registry`, default
  `guidedoc_defaults::registry()`), as Odin's `load` took one; the linked plugins are
  `LoadOptions::plugins`.
- **Stack limit**: documents are read and pages rendered on `gd_host::WORKER_STACK_BYTES`
  threads (the CLI runs on one too), so `build_limits` checks `--stack-kib` against that
  thread size instead of measuring the main thread.
- **Dropped with their machinery**: `refusal_location` (the template line a budget
  question named), `memory_problem`, `scratch_refusal`. The budget tests of search
  words, page layouts and the Odin package listing are not ported: those reads are no
  longer charged (gd-site, gd-read).
