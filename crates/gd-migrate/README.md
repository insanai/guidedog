# gd-migrate cheat sheet

`guidedog migrate` (Sphinx conf.py to conf.toml, **without running Python**) and
`guidedog quickstart`: a small Python-literal evaluator for conf.py, extension
harvesting, the conf.toml writer, and the quickstart files.

```rust
let done = gd_migrate::migrate(&gd_migrate::MigrateOptions {
    input: "docs".into(),                 // conf.py, source/conf.py or guidedog.toml
    output: gd_migrate::Destination::Nowhere, // -o -; Beside (default) or File(path)
    ..Default::default()
});
match done {
    Ok(m) => print!("{}", m.conversion.toml),   // + m.conversion.notes, converted, attention
    Err(f) => { /* f.notes (shown anyway), f.problem */ }
}
let created = gd_migrate::quickstart(&gd_migrate::QuickstartOptions {
    dir: "docs".into(), project: "Demo".into(), author: "Ada".into(),
    ..Default::default()
})?;                                           // Err: quickstart.exists, host.io
```

| Item | Does |
|---|---|
| `migrate(&MigrateOptions) -> Result<Migrated, Box<MigrateFailure>>` | find, read, convert, check it loads, write |
| `convert_conf_py(text, path, year) -> Conversion` | the staged path: never fails; notes explain the rest |
| `convert_guidedog_toml(text, path) -> Result<(Conversion, PathBuf), Problem>` | old `guidedog init` projects; conf.toml goes in the first source root |
| `check_conf_toml(&str)` | loads text with gd-conf as a build would (`migrate.internal` on failure) |
| `quickstart(&QuickstartOptions) -> Result<Vec<PathBuf>, Problem>` | Sphinx layout, `separate` for `source/` + `build/` |

Notes are `gd_report::Report`s (codes `migrate.setup`, `migrate.code`,
`migrate.computed`, `migrate.path`, `migrate.extension`, `migrate.extension.read`,
`migrate.form`, `migrate.value` (a value gd-conf refuses, kept as a comment),
`migrate.linkcode` (a `linkcode_resolve` function; use `linkcode_url`), `migrate.unknown` with a "Did you mean" hint from gd-conf's settings
table, `migrate.theme`, `migrate.sphinx-only`, ...) with the conf.py line as excerpt.

## Design notes

- **Typed Python values.** `enum Value { None, Bool, Int(i64), Float(String), Str, List,
  Dict(Vec<(k, v)>) }`; evaluation returns `Result<Value, String>` (the reason Python
  is needed), so a failure cannot be overlooked. Integer sums are checked.
- **Bounded evaluation.** Brackets and f-string fields nest at most 64 deep, so a
  hostile conf.py cannot overflow the stack; extension modules over 1 MiB are skipped
  and module folders are searched in sorted order (the same module on every system).
- **Characters, not bytes.** String indexing (`'héllo'[1]`) and escapes work on
  Unicode scalars, as in Python.
- **Failures keep their notes.** `MigrateFailure { problem, notes }` carries the notes
  gathered before the problem; `Destination` names the output instead of `""` / `"-"`
  sentinels; `year`/`date` are `Option`s defaulting to the system clock (UTC), so the
  caller need not compute them.
- **TOML writing comes from gd-toml** (`write_key`, `write_string`); the conf.toml text
  has a fixed layout (keys, then tables, then commented Sphinx lines).
- **Quickstart defaults live in `Default`** (`language = "en"`, `suffix = ".rst"`,
  `root_doc = "index"`); empty strings still take them, as the CLI passes them.
- **Theme files live in `gd-theme`**, a dependency-free crate shared with gd-site and
  gd-book, so the builders' built-in copies and quickstart's copies are the same texts.
