# gd-plugins cheat sheet

Typed build plugins (GDS 0010: source text, complete HTML pages, generated static
assets) and publishers (GDS 0009: send a finished site somewhere), linked into an
application or run as installed programs behind a bounded JSON Lines protocol. A
`Plugin` is build hooks **or** a publisher, never both. Protocol and design details:
[docs/manual/internals/plugins.rst](../../docs/manual/internals/plugins.rst).

```rust
// A plugin program: implement the trait, serve it with one call.
#[derive(Clone, Default)]
struct Badge { product: String }

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
fn main() -> ExitCode { gd_plugins::serve(Badge::default()) }

// A host: link it, or run a registered program.
let registry = Registry::new(vec![Plugin::build(Badge::default())])?;
let mut s = load_plugin(&registry, "product-badge", &options)?;   // Drop unloads
let text = s.process_source(&input)?;      // Cow: borrowed when unchanged
let entry = read_machine(&machine_path()?)?.find("pages").cloned();
let pages = process_publisher(&entry)?;    // after verify_entry(&entry)
let mut p = PublisherSession::load(&pages, &LoadOptions { config, ..Default::default() })?;
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
| Registry | `Plugin::{build, publisher}`, `Registry::{new, find}`, `is_valid_id` |
| Programs | `serve`, `serve_publisher`, `serve_streams`, `serve_publisher_streams`, `process_plugin`, `process_publisher`, `inspect_program`, `Handshake`, `select_version`, `Cancel` |
| Machine registry | `config_dir`, `machine_path`, `read_machine`, `Machine`, `MachineEntry`, `inspect_entry`, `verify_entry`, `register`, `unregister` |
| Errors | `Problem` (boxed `ProblemData { code, title, path, message, hint, exit }`) (+ `report()` for gd-report), `EXIT_ACCEPTED` (6), `EXIT_UNKNOWN` (7) |

Limits: text 8 MiB, configuration 1 MiB, 256 assets / 16 MiB, 256 dependencies, paths
1 KiB; request lines 1 MiB (publisher), responses 64 MiB; build requests time out after
60 s, publisher requests after 5 min, handshakes after 10 s.

Examples: `examples/build_plugin.rs` (the product badge), `examples/publisher.rs` (a
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
- **Served plugins load once.** `serve` takes the plugin by value (no `Clone` needed),
  so after a failed `load` the program refuses another load; hosts never retry anyway.
- **Dropped:** Windows-specific pipe and handle code (std covers it), the exported
  `try_lock`/`unlock` (use `std::fs::File::try_lock`), and the exported line-reader and
  JSON field helpers (use gd-json).
- **`Problem` is one pointer wide:** its fields live in a boxed `ProblemData`, reached
  through `Deref` (`p.code`, `p.exit = 2`), so `Result<T, Problem>` stays small and no
  author needs to allow `clippy::result_large_err`. Build with `Problem::new(..).hint(..)`
  or `ProblemData { .. }.into()`.
