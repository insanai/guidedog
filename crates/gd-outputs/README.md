# gd-outputs cheat sheet

How a build writes and publishes its output: one `Generation` per build and builder,
units written to a spare folder beside the output, reused when unchanged, and published
whole by a journaled commit. Ported from `guidedog_old/lib/project` (generation.odin,
commit.odin, publish.odin, build_paths.odin, remove.odin, `clean` of build.odin).

**Wave 0: signatures only.** The types and functions below are final; bodies are
`todo!("wave 1")`.

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
| `Generation` (`begin`, `reuse`, `unit`, `commit`, `abandon`, `failed_count`, `is_unsettled`, `taken_path`) | one generation; `Drop` abandons |
| `Unit` (`emit`, `emit_text`, `emit_copy`, `emit_tree`, `stage_path`, `keep_file`, `finish`, `fail`), `UnitRecord` | one output unit |
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
- `remove.odin` becomes `gd_host::remove_path`; stage/rename/sync use `gd_host`.
- Allocation-refusal paths (`OUT_OF_MEMORY` before a commit is decided) are gone: Rust
  aborts on allocation failure.
