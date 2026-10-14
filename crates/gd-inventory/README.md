# gd-inventory cheat sheet

Sphinx object inventories (`objects.inv`, versions 1 and 2), ported from
`guidedog_old/lib/inventory` onto `gd-flate`.

```rust
let inv = gd_inventory::load_inventory(&bytes, "https://docs.python.org/3/")?;
let len = inv.find("py:function", "len");            // Option<&Item>
let term = inv.find_folded("std:term", "With Statement");
let data = gd_inventory::write_inventory("Guide", "1.0", &items);
```

| Item | Does |
|---|---|
| `load_inventory(data, base) -> Result<Inventory, Error>` | `base` joins relative locations; output capped at `MAX_BODY` |
| `Inventory { project, version }`, `find`, `find_folded`, `add`, `items`, `has_type` | a later item of the same type and name wins |
| `Item { name, kind, priority, uri, display }` | `kind` is `domain:objtype` |
| `write_inventory(project, version, &items)`, `sort_items(&mut items)` | `$` and `-` shortening as Sphinx writes |
| `Error { kind }`, `message`, `hint`, `report(path)` | `ErrorKind::{UnknownVersion, Truncated, Compression(_)}` |

Redesigns: no arena or out-of-memory error (Rust aborts on allocation failure); items no
longer copy the project and version (read them from the `Inventory`); the type field is
`kind`; written inventories are compressed with DEFLATE instead of stored blocks; the
`std:label`/`std:term` case-folded index is built on `add`.
