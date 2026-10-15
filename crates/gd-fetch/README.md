# gd-fetch cheat sheet

Reads URLs and files, and sends HTTP requests, through the `curl` executable (argument
vector, never a shell). Used for remote intersphinx inventories and the Pages publisher.

| Item | Does |
|---|---|
| `get(location, &Options) -> Result<Fetched, Error>` | http(s) with redirects, `file://`, plain paths |
| `send(&Request, &Options) -> Response` | one request, no redirects; failures in `Response::error` with body and headers kept |
| `Request { method: Method, url, headers, body: Body }` | `Method::{Get, Head, Post, Put, Patch, Delete}`, `Body::{None, Bytes, File}` |
| `Options { timeout, max_bytes, user_agent, cancel, curl }` | `Default` is 30 s, 64 MiB, `"Guidedog"` |
| `Error { kind, location, status, detail }`, `.report()` | Elm-style via `gd-report`; codes `fetch.*` |
| `testserver::start(handler)` | loopback HTTP fixture for tests; stops when dropped |

## Design notes

- `Method` and `Body` enums replace a method string (empty meant GET) and the
  `body`/`body_path` pair that had to be checked for "not both"; HEAD uses `--head`.
- `Options::max_bytes` and `user_agent` hold their real defaults (`Default`), instead of
  0 and "" standing for them; `user_agent` is a `Cow<'static, str>`.
- HTTP statuses are `Option<u16>` (`None`: no response, or a file) instead of 0.
- curl's stdout reader reports a `Drained` enum instead of two flags.
- `testserver::Server::stop` is gone: dropping the server stops it.
- `Error` renders through `gd-report`, so URLs with control characters cannot reach the
  terminal.

## Credentials

Request headers often carry tokens, and a process's command line is visible to other
local users, so `send` never puts headers on curl's command line: it writes them to a new
file readable only by the current user and passes `--header @file`; the file is removed
when the request ends. The body travels on curl's stdin.
