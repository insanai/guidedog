# gd-inventory cheat sheet

Sphinx object inventories (`objects.inv`, versions 1 and 2), compressed with `gd-flate`.

## Use

```toml
[dependencies]
gd-inventory = "0.7"
```

We write an inventory and read it back:

```rust
use gd_inventory::{Item, load_inventory, write_inventory};

fn main() -> Result<(), gd_inventory::Error> {
    let items = [Item {
        name: "len".into(),
        kind: "py:function".into(),
        priority: 1,
        uri: "library/functions.html#len".into(),
        display: "-".into(),
    }];
    let data = write_inventory("Python", "3.15", &items);
    let inventory = load_inventory(&data, "https://docs.python.org/3/")?;
    let len = inventory.find("py:function", "len").map(|item| item.uri.as_str());
    assert_eq!(len, Some("https://docs.python.org/3/library/functions.html#len"));
    Ok(())
}
```

The API documentation is on [docs.rs](https://docs.rs/gd-inventory). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

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

Design: items do not copy the project and version (read them from the `Inventory`); the
type field is `kind`; written inventories are compressed with DEFLATE; the
`std:label`/`std:term` case-folded index is built on `add`.
