# gd-fetch cheat sheet

Reads URLs and files, and sends HTTP requests, in-process: `ureq` for HTTP and `rustls`
for TLS, with Mozilla's root certificates built in (`webpki-roots`). No `curl` program,
OpenSSL or system certificate store is needed. Used for remote intersphinx inventories
and the Pages publisher.

| Item | Does |
|---|---|
| `get(location, &Options) -> Result<Fetched, Error>` | http(s) with redirects, `file://`, plain paths |
| `send(&Request, &Options) -> Response` | one request, no redirects; failures in `Response::error` with body and headers kept |
| `Request { method: Method, url, headers, body: Body }` | `Method::{Get, Head, Post, Put, Patch, Delete}`, `Body::{None, Bytes, File}` |
| `Options { timeout, max_bytes, user_agent, cancel }` | `Default` is 30 s, 64 MiB, `"Guidedog"` |
| `Error { kind, location, status, detail }`, `.report()` | Elm-style via `gd-report`; codes `fetch.*` |
| `testserver::start(handler)` | loopback HTTP fixture for tests; stops when dropped |

## Behaviour

- `get` follows up to 20 redirects; `send` follows none. ureq speaks only http and https,
  so a redirect cannot lead anywhere else.
- Bodies are read in 64 KiB chunks and stop at `Options::max_bytes` (`TooLarge`); the
  timeout covers the whole transfer (`Timeout`).
- `cancel`: while waiting for an answer the transfer runs on a worker thread and the
  caller polls the flag every 10 ms; body reads check it between chunks. A worker left
  behind by a cancel ends with its transfer (bounded by the timeout).
- Requests say `Accept-Encoding: identity` (unless the caller sets it), so bodies arrive
  as they are.
- Proxies follow curl's variables: `https_proxy`/`HTTPS_PROXY` for https, `http_proxy`
  (lowercase only) for http, then `all_proxy`/`ALL_PROXY`; `no_proxy`/`NO_PROXY` lists
  hosts and domains that go direct (`*` for all). The proxy chosen for the first URL of a
  `get` serves its redirects too.
- A body file is sent with its length as `Content-Length`, read as it is sent.

## Design notes

- `Method` and `Body` enums replace a method string (empty meant GET) and the
  `body`/`body_path` pair that had to be checked for "not both".
- `Options::max_bytes` and `user_agent` hold their real defaults (`Default`), instead of
  0 and "" standing for them; `user_agent` is a `Cow<'static, str>`.
- HTTP statuses are `Option<u16>` (`None`: no response, or a file) instead of 0.
- `testserver::Server::stop` is gone: dropping the server stops it.
- `Error` renders through `gd-report`, so URLs with control characters cannot reach the
  terminal.

## Breaking change: no curl (0.7.0)

The curl backend is gone. `Options::curl` and `ErrorKind::CurlMissing`
(`fetch.curl-missing`) are removed; callers that matched on them drop those arms. Headers
no longer travel through a temporary file, because they never reach a command line.
