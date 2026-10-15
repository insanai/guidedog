# gd-toml cheat sheet

TOML 1.0 reader and writer, ported from `guidedog_old/lib/toml` (reader) and the writing
helpers of `lib/project/migrate_toml.odin`. Depends only on `gd-report`.

## Use

```toml
[dependencies]
gd-toml = "0.7"
```

We read a setting and write the table back:

```rust
fn main() {
    let text = "[site]\ntitle = \"Guide\"\n";
    match gd_toml::parse(text) {
        Ok(table) => {
            let title = table.path(&["site", "title"]).and_then(|v| v.as_str());
            assert_eq!(title, Some("Guide"));
            print!("{}", gd_toml::to_string(&table));
        }
        Err(err) => eprintln!("{err}"),
    }
}
```

The API documentation is on [docs.rs](https://docs.rs/gd-toml). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

```rust
let table = gd_toml::parse(&text).map_err(|e| e.report("conf.toml", &text))?;
let title = table.path(&["project", "title"]).and_then(|v| v.as_str());
let line = table.get("html").map(|v| v.line);   // every value knows where it was written
let text = gd_toml::to_string(&table);          // a document parse reads back equal
```

| Item | Does |
|---|---|
| `parse(&str) -> Result<Table, Error>`, `"..".parse::<Table>()` | the full TOML 1.0 grammar, nesting at most `MAX_DEPTH` (64) deep |
| `Value { data: Data, line, offset }` | `Data::{String, Integer, Float, Boolean, Datetime, Array, Table}`; equality ignores position |
| `v.as_str/as_integer/as_float/as_bool/as_datetime/as_array/as_table/get/kind_name` | typed access |
| `Table::{new, get, get_mut, path, insert, remove, iter, keys, len, contains_key}` | ordered; hashed above 16 keys |
| `Datetime { date, time, offset }`, `Datetime::parse`, `Display` | the four TOML date-time kinds |
| `Error { kind, message, hint, offset, end, line, column }`, `e.report(path, source)` | `ErrorKind::{Syntax, Duplicate, String, Number, Datetime, Depth}` |
| `to_string(&Table)`, `write_table`, `write_value`, `write_string`, `write_key` | writer |

Redesigns: a `Value` is an enum-backed struct instead of one struct with a field per kind;
date-times are parsed and validated rather than kept as text; `inf`/`nan` are real
infinities and NaN (Odin used `max(f64)` and 0); errors carry byte spans, columns and
hints; table-extension rules (dotted keys versus headers) follow the TOML 1.0 spec
strictly, which Odin only approximated. The Odin `\e` escape is still accepted.
