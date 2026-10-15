//! gd-serve: the `guidedog serve` preview. A std-only HTTP/1.1 static file server for a
//! built site (GET and HEAD, `index.html` for folders, paths confined to the site root,
//! bounded requests, a 404 page) that rebuilds before serving a page when the watched
//! sources changed. Change detection polls metadata (`Watch::stamp`); the rebuild is
//! the caller's closure, run on the caller's thread.
//!
//! ```no_run
//! use gd_serve::{ServeOptions, Server, Watch};
//! let options = ServeOptions { site_dir: "_build/html".into(), ..Default::default() };
//! let server = Server::bind(options)?;
//! println!("Serving at {} (Ctrl+C to stop)", server.url());
//! let watch = Watch { roots: vec![".".into()], skip: vec!["./_build".into()] };
//! server.run(watch, |_watch| {
//!     println!("Sources changed; rebuilding...");
//!     true // the result of the build
//! })?;
//! # Ok::<(), gd_host::Problem>(())
//! ```
//!
//! Failures are `gd_host::Problem`s; nothing here prints.

mod http;
mod server;
mod watch;

pub use server::{ServeOptions, Server, StopHandle};
pub use watch::Watch;

use gd_host::Problem;

/// The one-call path: binds with `options` and serves until Ctrl+C (see
/// [`Server::run`]). Fails with `serve.listen` when the port cannot be used.
pub fn serve(
    options: ServeOptions,
    watch: Watch,
    rebuild: impl FnMut(&mut Watch) -> bool,
) -> Result<(), Problem> {
    Server::bind(options)?.run(watch, rebuild)
}
