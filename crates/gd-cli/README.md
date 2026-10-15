# gd-cli cheat sheet

The `guidedog` command line as a library. `cmd/guidedog` is `run_status` inside the
crash reporter and a `WORKER_STACK_BYTES` thread; a custom binary passes its own plugins.
Reference: `docs/manual/reference/cli.rst`.

```rust
fn main() -> std::process::ExitCode {
    match gd_plugins::Registry::new(vec![badge::plugin()]) {
        Ok(linked) => gd_cli::run_with(linked),   // linked first, then plugins.toml
        Err(problem) => { eprintln!("{problem}"); std::process::ExitCode::from(2) }
    }
}

// Staged: words -> typed command (plain data), then run with any streams.
let parsed = gd_cli::parse_command(&args);           // Parsed { output, command: Result }
let status = gd_cli::run_in(&args, &linked, gd_cli::Streams { out, err, input, terminals });
```

| Item | Does |
|---|---|
| `run`, `run_with(Registry)`, `run_args(&[String], Registry)` → `ExitCode`; `run_status` → `i32` | the process's or given words, on the real streams |
| `run_in(&args, &Registry, Streams)` → `i32` | any `Write`/`BufRead` streams (tests) |
| `parse_command` → `Parsed { output: Output, command: Result<Command, Problem> }` | every command, typed: `ConvertCommand`, `BuildCommand` (holds `gd_conf::LoadOptions` and `gd_env::BuildOptions`), `ServeCommand`, `CleanCommand`, `QuickstartCommand`, `MigrateCommand`, `IntlCommand`, `PluginCommand`, `PublishCommand`, ... |
| `default_registry()` | the linked `pages` publisher |
| `engine_registry()` | readers rst/commonmark/myst, renderers html/typst/text |
| `discussion_builder(typst)` | `gd_discussion::Builder` with this engine (for `guidedog gds`, `dscc`) |
| `version_text()`, `full_version()`, `VERSION`, `VERSION_SUFFIX` | `--version` and the crash report |

Usage problems are `cli.usage` (exit 2), Elm-style through `gd-report`, with "Did you
mean" for commands, options, builders, intl/plugin/pages subcommands and values.

## Decisions and redesigns (from Odin `lib/cli`)

- **Parse, then run.** Odin parsed into one flat `Options`/`Project_Args` and checked
  meaning while running. Here each command has a typed struct, built while parsing, so
  every usage mistake (builder names, the sphinx-build forms, `intl` actions, formats
  from suffixes) is found before anything runs, and tests check meaning without I/O.
- **Each command takes only its own options.** Odin accepted every project option for
  every project command (`guidedog clean -W` was silently ignored); now it is
  `UNKNOWN OPTION` with a suggestion. `intl -w` with a bad number is an error (Odin fell
  back to 76). `serve` honours `-c`/`-C` like build (Odin passed only `-D`).
- **Global option values before the command word** (`guidedog --color never build`)
  are skipped when finding the command (Odin took `never` for the command).
- **No disk memory mode.** gd-host dropped `Disk`; `--memory=disk` and `--disk-budget`
  are `cli.memory-disk` errors whose hint says to raise `--budget` or use
  `--memory=ram`. The budget questions remain: before a build (continue, lower, stop)
  and at the budget (raise, stop), terminal only, scripted in tests (`memory::Ask`).
- **No arena, guard or `host.memory` report.** Rust aborts on allocation failure.
- **gd-pages is a dependency** (the plan said never): `guidedog publish pages` (the
  guided flow) and `guidedog pages workflow` are gd-pages' own command line
  (`gd_pages::command::run_with`), which the `Publisher` trait cannot express (setup,
  sign-in, workflow files). gd-pages links no engine, so the cost is small, and it makes
  `pages` the bundled default publisher: `default_registry()` links it, so `guidedog
  publish --publisher=pages` works without registering `guidedog-pages`. The guided
  flow builds with this very program (`--guidedog=<current exe>` unless given).
- **Ctrl+C** is caught by the commands that must settle (convert, build, publish, plugin
  check, pages), as in Odin, not globally in `main`: `serve` and `gds` stop at once.
- **Stdin is not locked** for the run (`StdinLines` reads on demand): `convert -` reads
  stdin through the standard library, whose lock is not reentrant.
- **`--version`** names Guidedog, Guidedoc and its schema, the tree-sitter crates (a
  test keeps them equal to Cargo.lock), the linked Graphviz, and the Typst backend; the
  Odin `odin` line is gone. `--licenses` prints NOTICE's third-party section.
- **`plugin list`/`inspect`** show the linked plugins beside the registered programs.
- **Older Typst discussion catalogs** (`legacy.rs`) compile with `gd_typst`, staged
  and themed as Odin's `discussion/embedded` did.
- `engine_registry()` stands in for `guidedoc_defaults::registry()` until wave 2.

## The project-builder seam (`src/ops.rs`) — wave 4 fills in

Everything above the seam is complete: options, prompts, budget questions, reports,
summaries (`summary.rs`), verdicts. `ops::load` already reads conf.toml with gd-conf.
Wave 4 (gd-project) replaces these bodies, keeping their signatures where it can:

| Function | Today | Wave 4 |
|---|---|---|
| `ops::Project`, `ops::load(dir, &LoadOptions, &Registry)` | `gd_conf::load_config` | `gd_project::load` (owns the linked registry) |
| `ops::build(&Project, BuilderKind, &BuildOptions) -> BuildResult` | `cli.not-available` | `gd_project::build` |
| `ops::clean(&Project)` | `cli.not-available` | `gd_project::clean` |
| `ops::serve(&Project, port, &mut dyn FnMut() -> bool)` | `cli.not-available` | gd-serve with the project's watch set; `rebuild` reloads and builds |
| `ops::intl(&Project, &IntlCommand) -> (Vec<IntlFile>, Option<Problem>)` | `cli.not-available` | gd-intl update/stat (remove the `expect(dead_code)` on `IntlOutcome`) |

Through them, `build`, `serve`, `clean`, `intl`, the project build of `publish`, and
`discussion_builder` (RST catalogs) start working; `cmd/dscc` can then pass
`gd_cli::discussion_builder` as `make_builder` with `tool: "typst"`. `quickstart` and
`migrate` already run gd-migrate.
