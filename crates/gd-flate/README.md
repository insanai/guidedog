# gd-flate cheat sheet

DEFLATE and zlib: inflate and deflate for Sphinx inventories, deflate for Git packs.
Standard library only. Gzip is not provided: nothing uses it.

## Use

```toml
[dependencies]
gd-flate = "0.7"
```

We compress, then decompress with a limit on the output, so a hostile input cannot exhaust memory:

```rust
fn main() -> Result<(), gd_flate::Error> {
    let packed = gd_flate::zlib_compress(b"hello hello hello", gd_flate::Level::Default);
    let unpacked = gd_flate::zlib_decompress(&packed, 1 << 20)?;
    assert_eq!(unpacked, b"hello hello hello");
    Ok(())
}
```

The API documentation is on [docs.rs](https://docs.rs/gd-flate). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

| Item | Does |
|---|---|
| `zlib_decompress(data, limit)`, `zlib_decompress_into(&mut out, data, limit) -> used` | header, Adler-32 check; `used` lets consecutive streams be read |
| `inflate(data, limit)`, `inflate_into` | raw DEFLATE: stored, fixed and dynamic blocks |
| `zlib_compress(data, Level)`, `zlib_compress_into`, `deflate`, `deflate_into` | LZ77 hash chains, lazy matching; per block the smallest of stored/fixed/dynamic |
| `Level::{Store, Fast, Default, Best}` | `Default` matches zlib level 6 in size on text |
| `adler32`, `adler32_update` | checksum |
| `Error { kind, offset }`, `message()`, `hint()` | `ErrorKind::{Truncated, Header, Block, Code, Distance, Checksum, Limit}` |

Every decompression takes a byte `limit`, so hostile input cannot exhaust memory.
