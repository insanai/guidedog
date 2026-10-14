# gd-json cheat sheet

A small JSON (RFC 8259) value type, parser and writer for the plugin protocol, caches,
search indexes and diagnostics. New in the Rust port (Odin used `core:encoding/json`).

```rust
let v = gd_json::parse(line)?;                       // or line.parse::<gd_json::Value>()
let id = v.get("id").and_then(|n| n.as_i64());
let mut o = gd_json::Object::new();
o.insert("status", "ok");
println!("{}", gd_json::Value::from(o));             // compact; to_string_pretty for 2-space
```

| Item | Does |
|---|---|
| `Value::{Null, Bool, Integer(i64), Float(f64), String, Array, Object}` | integers stay integers |
| `parse(text)`, `parse_with(text, &Limits { max_depth, allow_duplicate_keys })` | refuses duplicates and nesting > 128 by default |
| `Object` | insertion-ordered; `get`, `insert`, `remove`, `iter`, `keys`, `FromIterator` |
| `to_string`, `to_string_pretty`, `write_value`, `write_pretty`, `write_string` | `<`, `>`, `&`, U+2028/9 escaped so output embeds in `<script>`; NaN/inf become `null` |
| `Error { kind, message, hint, offset, end, line, column }`, `e.report(path, source)` | Elm-style |
