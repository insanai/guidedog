# gd-host cheat sheet

Where the Guidedoc engine meets the operating system: sessions with Ctrl+C
cancellation, sources read within limits, includes confined to approved roots, atomic
publication, diagnostics as `gd_report::Report`s, a coarse memory budget, worker
threads with known stacks, a crash reporter, and `convert_artifact`, the route
`guidedog convert` takes. Spec: GDS 0003 "Host surface", GDS 0004.

```rust
use gd_host::{ArtifactRequest, Format, Target, catch_crash, catch_interrupts, convert_artifact};

fn main() {
    catch_interrupts();
    gd_host::install_crash_reporter(&std::env::args().collect::<Vec<_>>(), "guidedog 0.7.0");
    let status = catch_crash(|| {
        let registry = guidedoc_defaults::registry();
        let request = ArtifactRequest { input: "intro.rst", output: "intro.pdf",
            from: Format::Rst, to: Target::Pdf, ..Default::default() };
        let result = convert_artifact(&registry, &request);
        let style = gd_report::Style::default();
        for r in &result.reports { eprint!("{}", gd_report::render_text(r, &style)); }
        if let Some(p) = &result.problem { eprint!("{}", gd_report::render_text(&p.report(), &style)); }
        result.exit
    });
    std::process::exit(status);
}
```

| Area | Names |
|---|---|
| Sessions | `Session` (`new`, `run`, `read`, `document`, `reports`, `workspace`, `workspace_mut`) |
| Diagnostics | `describe(&ws, &d)`, `describe_all(&ws)`, `exit_code`, `display_path`, `Problem` (`report()`), `io_problem`, `interrupted_problem`; render with `gd_report::render_text`/`render_json` |
| Files | `read_source` (path or `"-"`; UTF-8 checked, BOM dropped) -> `SourceFile`, `read_file` (charged), `read_stream`, `same_contents`, `each_chunk`, `MAX_SOURCE_BYTES` |
| Confinement | `Files` (`new`, `with_ledger`, `with_project`; a `guidedoc::ResourceProvider`), `canonical_roots`, `confined`, `confinement` -> `Confinement`, `containing_root` |
| Publication | `publish`, `Stage` (`beside`, `path`, `commit`; removed on drop), `write_unsynced`, `sync_all`, `sync_each`, `folders_of`, `remove_stages`, `is_stage_name`, `STAGE_PREFIX` |
| Folders | `exchange`, `link_file`, `identity` -> `Identity`, `path_type` -> `PathKind`, `list_folder` -> `FolderEntry`, `remove_path` |
| Budget | `Budget` (`new`, `with_policy`, `join` -> `Unit`, `limit`, `set_limit`, `used`, `peak`, `last_refusal`), `Ledger` (`charge`, `discharge`, `settle`; settles on drop), `Refusal`, `RefusalChoice`, `OnRefusal`, `budget_problem`, `budget_hint`, `budget_advice`, `suggest_budget`, `fitting_budget`, `available_memory`, `AVAILABLE_OVERRIDE`, `limits_within`, `DEFAULT_BUDGET` |
| Interrupts | `catch_interrupts`, `was_interrupted`, `cancel_probe`, `request_interrupt`, `clear_interrupt` |
| Threads | `run_workers(count, \|i\| ..)`, `run_with_stack(bytes, \|\| ..)`, `WORKER_STACK_BYTES`, `stack_budget_limit`, `default_stack_budget` |
| Crashes | `install_crash_reporter`, `catch_crash`, `crash_report`, `CRASH_EXIT` (70), `ISSUE_TRACKER` (`GUIDEDOG_ISSUE_TRACKER` at build time) |
| Artifacts | `convert_artifact(&registry, &ArtifactRequest)` -> `ArtifactResult { exit, reports, problem, delivered: Delivered }`, `Format`, `Target`, `backend_problem` |

Exit statuses: 0 ok, 1 invalid input/policy, 2 usage or file type (and an existing
output), 3 limits, budget and memory, 4 I/O and Typst, 5 internal, 70 crash, 130
cancelled.

## Design notes (redesigns from the Odin package)

- **No sizing, no retries.** Odin sessions planned slab sizes, grew the exhausted region
  and retried on `Capacity` (`plan_session`, `grow_session`, `prepare_session`,
  `resize_output`, `resize_source_map`, `sizes_for`, `make_storage`, `MAX_RETRIES`). The
  Rust `Workspace` grows reusable buffers itself, bounded by `Limits` with `try_reserve`,
  so a `Capacity` result is a configured limit reached, which a retry would reach again.
  `Session` is now a thin owner of a workspace that adds the Ctrl+C probe and described
  reports. `read_kept`/`read_owned` are gone: callers own their bytes (`Vec<u8>`).
- **The budget is coarse.** It counts what the host holds that it can size before
  allocating: sources, included files (`Files` charges its ledger, a refusal is
  `ResourceStatus::Limit`), and the PDF the embedded Typst returns (`Options::reserve`).
  A conversion's own memory is bounded by `Limits`, which `limits_within` lowers to the
  budget. Kept: units that wait for each other without deadlock (`join` -> `Unit` guard),
  the raise-or-stop policy, and the `--budget=N` hints. `Ledger` takes `&self` (atomics)
  so a shared provider can charge it, and releases its charges on drop.
- **Dropped allocator machinery.** `Meter`, `Guard`, `Arena`, `Disk` (spilling working
  memory to mapped files), `meter_prepay`, and the `--memory=disk` advice existed because
  Odin allocators fail per allocation and its libraries had to survive a refusal; Rust's
  global allocator aborts instead, and nothing in the Rust port runs behind a per-
  allocation meter, so there is no concrete need. cgroup scanning is dropped too:
  `available_memory` reads the override or `/proc/meminfo` only, for hints.
- **No fixed-buffer file operations.** `rename_path`, `sync_each`, `identity` and friends
  avoided allocation so a decided publication could not stop for want of memory. Rust
  aborts rather than fails on allocation, so they are plain `std::fs` calls;
  `rename_path` is just `std::fs::rename`. `exchange` keeps its foreign call
  (`renameat2`/`renamex_np`), and `Ctrl+C` uses C's `signal`: both in `src/sys.rs`, the
  only module with `unsafe`.
- **RAII stages.** `Stage` replaces `stage_file`/`commit`/`os.remove`: dropping an
  uncommitted stage removes it; `commit` flushes the file, renames, and flushes the
  folder. Stage names (`.guidedog-stage-` + ten digits + extension) are unchanged, so
  `remove_stages` still clears what a killed process left.
- **Threads.** `run_workers` starts scoped threads with an 8 MiB stack, so a conversion's
  stack budget is known to fit; `run_with_stack` does the same for the main command. The
  per-thread alternate signal stacks and `main_stack_bytes`/`thread_stack_bytes`
  (getrlimit) are gone. A worker's panic is re-raised on the caller with its payload.
- **Crashes are panics.** A panic hook records the cause; `catch_crash` prints the
  report and returns 70. The hook does not print or exit, so panics a library catches
  itself (the embedded Typst compiler) stay internal. Rust reports stack overflows
  itself.
- **Reports come from `gd-report`.** `Report`, the text and JSON renderers, excerpts and
  `excerpt_at` (`gd_report::set_line_column`) live there; `describe` fills templates with
  the core's `Diagnostic::format_into` (span arguments quoted in backticks, cut at 60
  characters) and adds a note when the report store overflowed.
- **Sources.** `read_source` checks UTF-8 (`host.utf8` with line and column) and drops a
  BOM, so the engine only ever sees `&str`. An included file that is not UTF-8 is
  refused (`ResourceStatus` has no status for it).
- **Not ported:** `dump` (use `guidedoc::dump_to_string`), `budget_join`/`budget_leave`
  (now `Budget::join`), `render_problem_*` (`Problem::report` + `gd-report`),
  `crash_on_assertion`, Windows unhandled-exception filters.
