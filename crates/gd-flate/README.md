# gd-flate cheat sheet

DEFLATE and zlib, replacing Odin's `core:compress/zlib` (inflate) and `vendor:zlib`
(deflate for Git packs). Standard library only. Gzip is not provided: nothing used it.

| Item | Does |
|---|---|
| `zlib_decompress(data, limit)`, `zlib_decompress_into(&mut out, data, limit) -> used` | header, Adler-32 check; `used` lets consecutive streams be read |
| `inflate(data, limit)`, `inflate_into` | raw DEFLATE: stored, fixed and dynamic blocks |
| `zlib_compress(data, Level)`, `zlib_compress_into`, `deflate`, `deflate_into` | LZ77 hash chains, lazy matching; per block the smallest of stored/fixed/dynamic |
| `Level::{Store, Fast, Default, Best}` | `Default` matches zlib level 6 in size on text |
| `adler32`, `adler32_update` | checksum |
| `Error { kind, offset }`, `message()`, `hint()` | `ErrorKind::{Truncated, Header, Block, Code, Distance, Checksum, Limit}` |

Every decompression takes a byte `limit`, so hostile input cannot exhaust memory.
