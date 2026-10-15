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
| `run`, `run_with(Registry)`, `run_args(&[String], Registry)` -> `ExitCode`; `run_status` -> `i32` | the process's or given words, on the real streams |
| `run_in(&args, &Registry, Streams)` -> `i32` | any `Write`/`BufRead` streams (tests) |
| `parse_command`, `parse_command_with(&args, &[Extension])` -> `Parsed { output: Output, command: Result<Command, Problem> }` | every command, typed: `ConvertCommand`, `BuildCommand` (holds `gd_conf::LoadOptions` and `gd_env::BuildOptions`), `ServeCommand`, `CleanCommand`, `QuickstartCommand`, `MigrateCommand`, `IntlCommand`, `PluginCommand`, `PublishCommand`, ... |
| `extensions(&Registry, machine)` -> `Vec<Extension>` | the commands plugins add (linked, then registered), for `parse_command_with` |
| `discussion_builder(typst)` | `gd_discussion::Builder` with this engine (for `guidedog gds`, `dsc`) |
| `version_text()`, `full_version()`, `VERSION`, `VERSION_SUFFIX` | `--version` and the crash report |

Usage problems are `cli.usage` (exit 2), Elm-style through `gd-report`, with "Did you
mean" for commands (guidedog's and the plugins'), options, builders, intl/plugin
subcommands and values.

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
- **Plugins add commands; gd-cli knows none by name.** Odin hard-wired `publish pages`
  and `pages workflow`. Here a plugin declares commands (`gd_plugins::CommandInfo`) and
  runs them with a `CommandContext` that gd-cli lends (`extension_run.rs`: its console,
  questions, and builds with this engine). A word guidedog does not know, or a
  `publish` target, is matched against `extensions()` (linked first, then registered
  programs, longest name wins, option values skipped); `--help` on one prints help from
  its declaration. gd-cli depends only on gd-plugins; `cmd/guidedog` links the Pages
  plugin (the dependency gate checks it). See docs/manual/internals/plugins.rst.
- **Ctrl+C** is caught by the commands that must settle (convert, build, publish, plugin
  check, plugin commands), as in Odin, not globally in `main`: `serve` and `gds` stop at once.
- **Stdin is not locked** for the run (`StdinLines` reads on demand): `convert -` reads
  stdin through the standard library, whose lock is not reentrant.
- **`--version`** names Guidedog, Guidedoc and its schema, the tree-sitter crates (a
  test keeps them equal to Cargo.lock), the linked Graphviz, and the Typst backend; the
  Odin `odin` line is gone. `--licenses` prints NOTICE's third-party section.
- **`plugin list`/`inspect`** show the linked plugins beside the registered programs.
- **Older Typst discussion catalogs** (`legacy.rs`) compile with `gd_typst`, staged
  and themed as Odin's `discussion/embedded` did.
- `convert` and `formats` use `guidedoc_defaults::registry()` (readers rst, sphinx,
  commonmark, myst).

## The project builder (`src/ops.rs`)

`build`, `serve`, `clean`, `intl`, the project build of `publish`, and
`discussion_builder` reach gd-project only through `ops`: `ops::load` (the linked
plugins go into `gd_project::LoadOptions`), `ops::build`, `ops::clean`, `ops::intl`
(gd-intl's `intl_update`/`intl_stat`, counts formatted by gd-gettext), and
`ops::serve`, which serves `<build_dir>/html` with gd-serve, watching
`gd_project::watch_roots` and skipping the build folder; the command implements
`ops::ServeHost` (prints the "Serving" line, reloads and rebuilds on change). The
project's conf.toml warnings are reported by the build (they count for `-W`), not again
by the command. `cmd/dsc` passes `discussion_builder` as its `make_builder`.
