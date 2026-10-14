# gd-sha cheat sheet

SHA-256 (content digests), SHA-1 (Git object ids only) and FNV-1a 64 (cache keys), which
the Odin code took from `core:crypto` and `core:hash`. Standard library only.

| Item | Does |
|---|---|
| `sha256(&[u8]) -> [u8; 32]`, `sha1(&[u8]) -> [u8; 20]` | one call |
| `Sha256::new()`, `.update(bytes)`, `.finish()`; same for `Sha1` | streaming; also `io::Write`, so `io::copy(&mut file, &mut h)` hashes a file without loading it |
| `to_hex`, `write_hex`, `from_hex` | lower-case hex |
| `fnv1a_64(&[u8]) -> u64` | non-cryptographic |
