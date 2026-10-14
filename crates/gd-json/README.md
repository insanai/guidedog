# gd-json cheat sheet

JSON (RFC 8259) for the plugin protocol, caches, search indexes and diagnostics: a thin
API over `serde_json` with a validating parser, an HTML-safe writer and Elm-style errors.
New in the Rust port (Odin used `core:encoding/json`).

```rust
let v = gd_json::parse(line)?;                       // duplicate keys refused
let id = v.get("id").and_then(|n| n.as_i64());
let mut o = gd_json::Object::new();
o.insert("status".into(), "ok".into());
println!("{}", gd_json::to_string(&o.into()));        // to_string_pretty for 2-space
let cfg: MyConfig = gd_json::from_value(v)?;          // any serde Deserialize type
```

| Item | Does |
|---|---|
| `Value` (= `serde_json::Value`): `Null, Bool, Number, String, Array, Object` | `as_i64`, `as_u64`, `as_f64`, `as_str`, `as_array`, `as_object`, `get`, `is_*` |
| `Object` (= `serde_json::Map<String, Value>`) | insertion-ordered; `insert(String, Value)`, `get`, `shift_remove` (keeps order), `iter`, `keys` |
| `kind_name(&value)` | `"an integer"`, `"a string"`, ... for messages |
| `parse(text)`, `parse_with(text, &Limits { max_depth, allow_duplicate_keys })` | refuses duplicates and nesting > `MAX_DEPTH` (127); skips a BOM |
| `to_string`, `to_string_pretty`, `write_value`, `write_pretty(out, v, level)`, `write_string` | `<`, `>`, `&`, U+2028/9 escaped so output embeds in `<script>` |
| `from_value::<T>(v)`, `to_value(&t)` | serde conversions; failures are `ErrorKind::Data` without position |
| `Error { kind, message, hint, offset, end, line, column }`, `e.report(path, source)` | Elm-style; kinds `Syntax`, `Eof`, `String`, `Number`, `Duplicate`, `Depth`, `Data` |

## Design notes

**Built on serde_json** because the embedded Typst compiler already compiles it into
every build (same version, with `preserve_order`), so an in-house parser and writer was
~600 lines of duplicate, less-tested code. serde_json is mature, fast and fuzzed.

- `Value` and `Object` are re-exports, not newtypes: values flow to and from any
  serde-based code (Typst, plugin payloads, `#[derive(Deserialize)]` structs) at no cost.
  The old `Value::Integer`/`Value::Float` became `Value::Number`; `Value::kind_name()`
  became the free function `kind_name`.
- `parse_with` drives serde_json with a small `DeserializeSeed` that builds the value
  and checks duplicate keys and depth in the same pass; the error keeps serde_json's
  line and column but carries this crate's kind, message and hint.
- The writers run serde_json's serializer with a formatter that splits string fragments
  at `<`, `>`, `&`, U+2028 and U+2029 and writes them as `\uXXXX`.
- `Value`'s own `Display` and `FromStr` are serde_json's: no HTML escaping and no
  duplicate-key check. Use `to_string` and `parse`.

Behaviour differences from the in-house version:

- Depth is capped at 127 by serde_json's stack guard (was 128); larger `max_depth`
  values act as 127.
- Large floats are written with an explicit sign: `1e+300` (was `1e300`); `\b`/`\f`
  are written short (were `\u0008`/`\u000c`).
- Integers above `i64::MAX` stay exact as `u64` (were floats); numbers beyond the f64
  range are refused (`ErrorKind::Number`).
- A text that ends early (`[1,`, `"open`, ``) is `ErrorKind::Eof` (was `Syntax`/`String`).
