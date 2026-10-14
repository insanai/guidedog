# gd-git cheat sheet

Git objects and the receive-pack push protocol without a git program, for the Pages
publisher. SHA-1 repositories only; no networking (send the request with `gd-fetch`).

| Item | Does |
|---|---|
| `object_id(kind, data)`, `file_id(path) -> (Id, size)` | object IDs; files stream through the hasher |
| `build_trees(&[File]) -> Vec<Tree>` | one tree per folder, root last; Git's path rules and order |
| `commit_bytes(&Commit)`, `write_commit(&mut Vec<u8>, &Commit)` | commit objects |
| `PackWriter::begin(path, count)`, `.add`, `.add_file`, `.end()` | pack files; dropping an unfinished writer removes the file |
| `parse_advertisement`, `write_request(path, &Update)`, `parse_report` | receive-pack messages |
| `Error { kind, path, message, hint }`, `.report()` | Elm-style via `gd-report`; codes `git.*` |

## Design notes

- SHA-1 comes from `gd-sha` and zlib from `gd-flate` (`zlib_compress_into` at
  `Level::Default`); the private SHA-1 and fixed-Huffman deflater are gone. Packs are now
  smaller (dynamic Huffman, lazy matching). Each object is compressed whole, so memory
  holds the largest single file (reserved with `try_reserve`, `TooLarge` on failure),
  never the pack.
- `PackWriter` has no `abort`: dropping it removes an unfinished pack. A `Pending` guard
  declared after the file closes the file before removing it (Windows).
- `write_request` takes an `Update { reference, old, new, pack, agent }` instead of six
  arguments.
- `Report::unpack_error: Option<String>` and `RefStatus::refusal: Option<String>` replace
  the `unpacked`/`ok` flags paired with empty-string reasons; `is_unpacked()`/`is_ok()`
  ask.
- `Error` renders through `gd-report` and carries a stable code (`ErrorKind::code`).
