# gd-serve cheat sheet

The `guidedog serve` preview: a std-only HTTP/1.1 static file server for a built site
that rebuilds before serving a page when the watched sources changed. Port of
`lib/project/serve.odin` and `serve_watch.odin`.

```rust
use gd_serve::{ServeOptions, Server, Watch};

let server = Server::bind(ServeOptions { site_dir: build.join("html"), port, ..Default::default() })?;
println!("  Serving {} at {} (Ctrl+C to stop)", site, server.url());
let watch = Watch { roots: source_roots, skip: vec![build.clone()] };
server.run(watch, |watch| {
    println!("  Sources changed; rebuilding...");
    rebuild_and_update(watch) // false when the build failed
})?;
```

| Area | Names |
|---|---|
| One call | `serve(options, watch, rebuild)` |
| Staged | `Server::bind`, `address`, `url`, `stop_handle`, `run` |
| Options | `ServeOptions { site_dir, address, port, workers, max_request_bytes, timeout, stop_on_interrupt }` |
| Stopping | `StopHandle::stop`, `is_stopped`; Ctrl+C via `gd_host::was_interrupted` |
| Watching | `Watch { roots, skip }`, `Watch::stamp` |
| Errors | `gd_host::Problem` `serve.listen` (exit 4) |

Behaviour kept: loopback by default, GET and HEAD only (405 otherwise), query and
fragment ignored, percent-decoding, `..` and backslashes refused (400), `index.html`
for folder paths, the 404 page, `Cache-Control: no-store`, `nosniff`,
`Connection: close`, 2 s client timeouts, the Odin content types, and a rebuild only
before HTML pages when the metadata stamp changed (hidden entries, folder times and the
skipped build folder ignored; a missing root has its own marker).

## Redesign notes

- **No project dependency.** The Odin server read the project config to find its
  roots; here the caller passes a `Watch` (roots + skip paths) and a rebuild closure
  `FnMut(&mut Watch) -> bool`, which may replace the roots after a configuration
  reload. gd-project / gd-cli compute the roots (source dir, Odin roots, static,
  extra, template, include, autodoc and locale paths outside the source dir).
- **Concurrency.** A fixed pool of worker threads answers connections; the listener
  runs on the caller's thread, which also owns the rebuild (so it need not be `Send`).
  Workers ask that thread to refresh before an HTML page; pending asks share one check.
- **Stopping.** Ctrl+C (or a `StopHandle`) ends `run` with `Ok(())`; the Odin server
  only stopped with the process.
- **No printing.** The caller prints the "Serving" and "rebuilding" lines.
- **Safer paths.** Only plain segments are kept (so `//etc/passwd` cannot become an
  absolute join), NUL and invalid UTF-8 are refused, and symbolic links that resolve
  outside the site root give 404. A folder without its trailing slash is redirected
  (301) instead of 404. Request heads are read up to `max_request_bytes` (431 past
  it) rather than in one `recv`. Files are streamed, not read whole. A few content
  types were added (`txt`, `xml`, `ico`, `woff`, `ttf`, `mjs`, `htm`).
- **Stamp.** std's `DefaultHasher` instead of SHA-256 (stamps are only compared within
  one process); recursion is bounded at 64 levels and symbolic links to folders are
  not followed.
- No live reload: the Odin preview had none.
