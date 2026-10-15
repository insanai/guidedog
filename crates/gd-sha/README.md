# gd-sha cheat sheet

SHA-256 (content digests), SHA-1 (Git object ids only) and FNV-1a 64 (cache keys).
Standard library only.

## Use

```toml
[dependencies]
gd-sha = "0.7"
```

We hash bytes in one call, or stream them through `io::Write` (a file goes through `io::copy` without being loaded whole):

```rust
use std::io::Write;

fn main() -> std::io::Result<()> {
    let digest = gd_sha::sha256(b"abc");
    println!("{}", gd_sha::to_hex(&digest));

    let mut hasher = gd_sha::Sha256::new();
    hasher.write_all(b"abc")?; // or std::io::copy(&mut file, &mut hasher)
    assert_eq!(hasher.finish(), digest);
    Ok(())
}
```

The API documentation is on [docs.rs](https://docs.rs/gd-sha). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

| Item | Does |
|---|---|
| `sha256(&[u8]) -> [u8; 32]`, `sha1(&[u8]) -> [u8; 20]` | one call |
| `Sha256::new()`, `.update(bytes)`, `.finish()`; same for `Sha1` | streaming; also `io::Write`, so `io::copy(&mut file, &mut h)` hashes a file without loading it |
| `to_hex`, `write_hex`, `from_hex` | lower-case hex |
| `fnv1a_64(&[u8]) -> u64` | non-cryptographic |
