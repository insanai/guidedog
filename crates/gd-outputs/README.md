# gd-outputs cheat sheet

How a build writes and publishes its output: one `Generation` per build and builder,
units written to a spare folder beside the output, reused when unchanged, and published
whole by a journaled commit. Ported from `guidedog_old/lib/project` (generation.odin,
commit.odin, publish.odin, build_paths.odin, remove.odin, `clean` of build.odin).

```rust
let paths = gd_outputs::output_paths(&config, kind, options.output_dir.as_deref())?;
let mut generation = Generation::begin(paths, GenerationOptions { all, budget: &budget },
    &mut reports)?;                                   // settles interrupted commits
if let Some(old) = generation.reuse(docname, inputs, |l| lookup_answer(&view, docname, l)) {
    reports.extend(old.reports.iter().cloned());     // kept: same inputs, same answers
} else {
    let mut unit = generation.unit(docname);          // dropped unfinished = failed
    unit.emit_text("guide/intro.html", &html)?;
    unit.finish(UnitRecord { inputs, assets, lookups, reports: page_reports });
}
let outcome = generation.commit(CommitRecords { environment: &env, store: &store });
```

| Item | Does |
|---|---|
| `OutputPaths { out_dir, next, old, legacy, build_dir, doctrees, manifest, journal }`, `output_paths`, `output_absolute`, `contained` | where things go |
| `Generation` (`begin`, `reuse`, `unit`, `commit`, `abandon`, `failed_count`, `written_count`, `reused_count`, `is_site_written`, `taken_path`) | one generation; `Drop` abandons |
| `Unit` (`emit`, `emit_text`, `emit_copy`, `emit_tree`, `stage_path`, `keep_file`, `files`, `taken_path`, `finish`, `fail`), `UnitRecord` | one output unit |
| `Manifest`, `OutputRecord { inputs, files, assets, lookups: Vec<(Lookup, Digest)>, reports }`, `MANIFEST_VERSION` (2), `SITE_UNIT` | the manifest |
| `CommitRecords`, `CommitOutcome { state, problem, reports }`, `CommitState` | publication |
| `clean(&Config)` | `guidedog clean` |

## Redesign notes

- **RAII**: a `Unit` dropped unfinished counts as failed (Odin's `unit_failed`); a
  `Generation` dropped uncommitted is abandoned (Odin's `abandon_generation`).
- **Lookups store their answers' digests** (`OutputRecord::lookups`), so `reuse`
  compares each answer instead of one digest of all of them: the verifying-trace check
  is explicit and a changed answer is known exactly (manifest version 2).
- **Paths are relative to the output folder** (`unit.emit_text("a/b.html", ..)`), not
  absolute paths turned relative.
- `remove.odin` becomes a private `remove_tree` (a work list over `gd_host::path_type`,
  `list_folder` and `remove_path`; never follows a link); stage/exchange/sync/link use
  `gd_host`.
- Allocation-refusal paths (`OUT_OF_MEMORY`, `Commit_Plan`'s preallocated buffers, the
  "a decided commit allocates nothing" test) are gone: Rust aborts on allocation
  failure, so a commit step can never fail for want of memory.
- The journal names its records with `Option` instead of `""` sentinels
  (`previous: null`, `next: null`, `next_id: null`) and keeps `JOURNAL_VERSION` 2.
- `discard_journal` marks an untrusted manifest by zeroing each unit's `inputs`
  digest; `reuse` never keeps a unit with a zero digest.
- A commit refuses to publish a generation with failed units (`build.output.failed`):
  the caller no longer has to remember not to commit.
- `emit_tree` walks with a work list (no recursion) and follows links only after
  `gd_host::confinement` accepts them.
- Unit paths are normalized (`a//./b` is `a/b`) before they reach the manifest.
- **Spare and previous outputs live in the build folder** (`<build_dir>/.<name>.next`,
  `.old`): unchanged for default outputs, inside `.<name>.guidedog` for custom ones, so a
  custom output leaves one hidden sibling. `Generation::begin` moves or removes old-layout
  siblings that are clearly Guidedog's (`migrate.rs`), with a note.
- Test hooks (`commit_hook`, `commit_renames_only`) are thread-local and exist only
  under `cfg(test)`; the commit tests run a small simulated builder (`src/tests/
  support.rs`) through the real `Generation` instead of a whole project build.
