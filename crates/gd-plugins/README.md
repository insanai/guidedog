# gd-plugins cheat sheet

Typed build plugins (GDS 0010: source text, complete HTML pages, generated static
assets) and publishers (GDS 0009: send a finished site somewhere), linked into an
application or run as installed programs behind a bounded JSON Lines protocol. A
`Plugin` is build hooks **or** a publisher, never both. Protocol and design details:
[docs/manual/internals/plugins.rst](../../docs/manual/internals/plugins.rst).

## Use

```toml
[dependencies]
gd-plugins = "0.7"
```

We write a build plugin that runs as its own program and upper-cases every source:

```rust
use std::borrow::Cow;

use gd_plugins::{BuildPlugin, Hooks, PluginInfo, Problem, SourceInput};

#[derive(Clone, Default)]
struct Shout;

impl BuildPlugin for Shout {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("shout", "Shout", "1.0.0", Hooks { source: true, ..Hooks::NONE })
    }

    fn process_source<'a>(&mut self, input: &SourceInput<'a>) -> Result<Cow<'a, str>, Problem> {
        Ok(Cow::Owned(input.text.to_uppercase()))
    }
}

fn main() -> std::process::ExitCode {
    gd_plugins::serve(Shout)
}
```

The API documentation is on [docs.rs](https://docs.rs/gd-plugins). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## One implementation, two deployments

A plugin crate implements `BuildPlugin` (or `Publisher`) once and exposes
`pub fn plugin() -> Plugin`. The same value is then:

- **linked** into a custom `guidedog` binary (Rust has no stable ABI, so this
  replaces Odin's linked procedure tables; no dylib loading):
  `gd_cli::run_with(Registry::new(vec![badge::plugin()])?)` (planned gd-cli entry,
  `fn run_with(linked: Registry) -> ExitCode`);
- **served** as an installed program by the crate's tiny `main`:
  `fn main() -> ExitCode { gd_plugins::serve_plugin(&badge::plugin()) }`, then
  `guidedog plugin register ID /path/to/program`.

A build looks up each selected ID in the linked registry first, then in the machine
registry (`select_build_plugin`), and folds `plugin.identity(host_build)` into its cache
key. See `examples/build_plugin/{badge.rs, main.rs}`.

```rust
#[derive(Clone, Default)]
pub struct Badge { product: String }

impl BuildPlugin for Badge {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("product-badge", "Product badge", "1.0.0", Hooks::ALL)
    }
    fn load(&mut self, ctx: &mut LoadContext) -> Result<(), Problem> {
        let toml = ctx.read_config()?;          // declared + fingerprinted
        /* ... */ Ok(())
    }
    fn process_source<'a>(&mut self, i: &SourceInput<'a>) -> Result<Cow<'a, str>, Problem> {
        Ok(Cow::Owned(i.text.replace("{{product}}", &self.product)))
    }
}
pub fn plugin() -> Plugin { Plugin::build(Badge::default()) }

// A host (gd-project): one session per plugin per build, Mutex-shared by workers.
let plugin = select_build_plugin(&linked, "product-badge", &machine_path()?)?;
let options = PluginOptions::new(&config, &project, "html", "en");
let session = Mutex::new(PluginSession::load(&plugin, &options)?);   // Drop unloads
let key = (plugin.identity(HOST_BUILD), session.lock()?.cache_key().to_owned());
let text = session.lock()?.process_source(&SourceInput::new(path, doc, fmt, "en", src))?;

// Publishing.
let pages = process_publisher(&entry)?;    // after verify_entry(&entry)
let mut p = PublisherSession::load(&pages, &LoadOptions::new("pages.toml"))?;
let snap = snapshot(&p.source().site, &SnapshotOptions::default())?;
let plan = p.plan(&snap)?;
let outcome = p.publish(&snap, &plan);     // outcome.exit_status(): 0, 1-5, 6, 7
```

| Area | Items |
|---|---|
| Build hooks | `BuildPlugin` (`info`, `load`, `process_source`, `process_html`, `collect_assets`; defaults = service absent), `PluginInfo`, `Hooks`, `SourceInput`, `SourceFormat`, `HtmlInput`, `Asset`, `PluginOptions`, `LoadContext` (`read_config`, `read_file`, `declare_dependency`, `add_cache_key`) |
| Build sessions | `load_plugin`, `PluginSession::{load, process_source, process_html, collect_assets, dependencies, cache_key}`, `check_plugin` |
| Publishers | `Publisher` (`info`, `load`, `plan`, `publish`, `status`, `recover`), `PublisherInfo`, `Capabilities`, `LoadOptions`, `Loaded`, `Source`, `Plan` (+ `data_as::<T>()`), `Change`, `ChangeKind`, `Outcome`, `State` |
| Publisher sessions | `load_publisher`, `PublisherSession::{load, plan, publish, status, recover}` |
| Snapshots | `snapshot(dir, &SnapshotOptions)`, `Snapshot::{entries, digest, verify}`, `SnapshotMode::{Copy, Borrow, Adopt}`, `digest_hex` |
| Registry | `Plugin::{build, publisher, identity}`, `Registry::{new, find}`, `select_build_plugin`, `MAX_BUILD_PLUGINS` (64), `is_valid_id` |
| Programs | `serve_plugin`, `serve`, `serve_publisher`, `serve_streams`, `serve_publisher_streams`, `process_plugin`, `process_publisher`, `inspect_program`, `Handshake`, `select_version`, `Cancel` |
| Private files | `files::{create_private_dir, make_temp_dir, create_private, write_atomically}` (folders `0700`, files `0600`; a file replaced in one step) |
| Machine registry | `config_dir`, `machine_path`, `read_machine`, `Machine`, `MachineEntry`, `inspect_entry`, `verify_entry`, `register`, `unregister` |
| Errors | `Problem` (boxed `ProblemData { code, title, path, message, hint, exit }`) (+ `report()` for gd-report), `EXIT_ACCEPTED` (6), `EXIT_UNKNOWN` (7) |

Limits: text 8 MiB, configuration 1 MiB, 256 assets / 16 MiB, 256 dependencies, paths
1 KiB; request lines 1 MiB (publisher), responses 64 MiB; build requests time out after
60 s, publisher requests after 5 min, handshakes after 10 s.

Examples: `examples/build_plugin/` (the product badge: `badge.rs` library, `main.rs` server), `examples/publisher.rs` (a
local-folder publisher), `examples/protocol_fixture.rs` (faulty programs for tests).

## Redesign notes (from Odin `lib/plugins`)

- **Traits instead of procedure tables.** A plugin value *is* its session state: no
  `rawptr data`, no `unload` callback (use `Drop`), no context structs carrying
  allocators. A `Plugin` holds a prototype and clones it per session, so sessions never
  share state. Hooks declare themselves in `PluginInfo::hooks` / `Capabilities`.
- **No owners to release.** `Text_Result`, `Assets_Result`, `Result`, `Plan` and
  `Snapshot` are ordinary values; `release_*`, arenas and allocation-refusal
  machinery (`memory_refused`, the per-allocation refusal tests) are gone. Hook text is
  `Cow`, so unchanged text is never copied.
- **Types forbid misuse.** `Registry::new` validates, so an invalid registry cannot
  exist; `SourceFormat` is an enum; `Duration` cannot be negative; the SDK keeps the
  session's identity private, so a callback cannot replace it; a plan's generation is
  read-only, and its session is checked by serial number. `LoadContext` only appends
  to dependencies and the cache key, so helper fingerprints cannot be erased.
- **Process transport.** A writer and a reader thread own the child's pipes; the host
  waits on channels with one monotonic deadline per exchange and polls the `Cancel`
  probe (replacing non-blocking pipes and SIGPIPE masking). Wire format unchanged.
- **Evolution.** Host-made inputs (`SourceInput`, `HtmlInput`, `PluginOptions`,
  `LoadOptions`, `SnapshotOptions`, `Handshake`, `MachineEntry`) are `#[non_exhaustive]`
  with constructors (`::new`, or `Default` plus field assignment), so new fields are not
  breaking; new hooks arrive as default trait methods; `PLUGIN_API_VERSION`/`API_VERSION`
  version the traits.
- **Served plugins load once.** `serve` takes the plugin by value (no `Clone` needed),
  so after a failed `load` the program refuses another load; hosts never retry anyway.
- **Dropped:** Windows-specific pipe and handle code (std covers it), the exported
  `try_lock`/`unlock` (use `std::fs::File::try_lock`), and the exported line-reader and
  JSON field helpers (use gd-json).
- **`Problem` is one pointer wide:** its fields live in a boxed `ProblemData`, reached
  through `Deref` (`p.code`, `p.exit = 2`), so `Result<T, Problem>` stays small and no
  author needs to allow `clippy::result_large_err`. Build with `Problem::new(..).hint(..)`
  or `ProblemData { .. }.into()`.
