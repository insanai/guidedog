//! `watch`: publish once, then again whenever the sources change and have been quiet
//! for `debounce_ms` (or `max_wait_ms` after the first change), until Ctrl+C. A failed
//! build or publication is reported and the watch goes on; the site stays as it was.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, UNIX_EPOCH};

#[cfg(feature = "preview")]
use gd_plugins::Cancel;
#[cfg(feature = "preview")]
use gd_serve::{ServeOptions, Server, Watch};

use super::Context;
use super::display::Console;
use super::publish::publish_once;
#[cfg(feature = "preview")]
use crate::build_dir;
#[cfg(feature = "preview")]
use crate::config::SiteSource;
use crate::config::{Config, read_config};

const MAX_FOLDERS: usize = 100_000;

/// Why a round of publication stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Round {
    Published,
    Cancelled,
    /// The token does not work; the watch goes on, paused until it does.
    Credential,
    /// GitHub could not be reached; worth retrying soon.
    Network,
    Failed,
}

pub(crate) fn watch(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    let c = match read_config(&ctx.config_path()) {
        Ok(c) => c,
        Err(p) => return console.report(&p),
    };
    match ctx.o.serve {
        Some(port) => watch_with_preview(ctx, console, &c, port),
        None => watch_loop(ctx, console, &c),
    }
}

/// Without the `preview` feature there is no server to start.
#[cfg(not(feature = "preview"))]
fn watch_with_preview(
    _ctx: &Context<'_>,
    console: &mut Console<'_>,
    _c: &Config,
    _port: u16,
) -> i32 {
    console.report(
        &crate::problem::problem(
            "pages.preview",
            "This build has no preview",
            "--serve",
            "This program was built without the preview server (the preview feature).",
            "Run guidedog-pages watch without --serve, or use a guidedog-pages built with it.",
        )
        .exit(2),
    )
}

/// Serves the published site's folder (the build folder of a project) on `port` while
/// the watch publishes. One Ctrl+C stops both: the watch stops on the run's cancel and
/// then stops the server; a server that ends stops the watch.
#[cfg(feature = "preview")]
fn watch_with_preview(ctx: &Context<'_>, console: &mut Console<'_>, c: &Config, port: u16) -> i32 {
    let site_dir = match &c.source {
        SiteSource::Built(site) => site.clone(),
        SiteSource::Project(_) => match build_dir(ctx.settings, c, false) {
            Ok(dir) => dir,
            Err(p) => return console.report(&p),
        },
    };
    let options =
        ServeOptions { site_dir, port, stop_on_interrupt: false, ..ServeOptions::default() };
    let server = match Server::bind(options) {
        Ok(server) => server,
        Err(p) => return console.report(&host_problem(&p)),
    };
    console.say(&format!("Previewing at {}", server.url()));
    let stop = server.stop_handle();
    let (outer, ended) = (ctx.cancel.clone(), stop.clone());
    let cancel = Cancel::from_fn(move || outer.is_cancelled() || ended.is_stopped());
    let inner =
        Context { o: ctx.o, settings: ctx.settings, cancel: &cancel, dir: ctx.dir.clone() };
    std::thread::scope(|scope| {
        let ender = stop.clone();
        let serving = scope.spawn(move || {
            // The preview only serves what the watch builds; it never rebuilds itself.
            let served = server.run(Watch::default(), |_| true);
            ender.stop();
            served
        });
        let status = watch_loop(&inner, console, c);
        stop.stop();
        match serving.join() {
            Ok(Err(p)) => console.report(&host_problem(&p)),
            _ => status,
        }
    })
}

/// A preview server's problem as this crate reports it.
#[cfg(feature = "preview")]
fn host_problem(p: &gd_host::Problem) -> gd_plugins::Problem {
    crate::problem::problem(p.code, p.title, &p.path, &p.message, &p.hint).exit(p.exit)
}

fn watch_loop(ctx: &Context<'_>, console: &mut Console<'_>, c: &Config) -> i32 {
    let folder = c.source.folder().to_path_buf();
    console.say(&format!(
        "Watching {}; publishing to {} (Ctrl+C stops)",
        folder.display(),
        c.site_url
    ));
    let mut stamp = source_stamp(&folder);
    while !ctx.cancel.is_cancelled() {
        let round = publish_round(ctx, console);
        if round == Round::Cancelled {
            break;
        }
        if round != Round::Published {
            console.say("Will publish again when sources change.");
        }
        match wait_for_change(ctx, &folder, stamp, c) {
            Some(next) => stamp = next,
            None => break,
        }
    }
    console.say("Stopped watching.");
    0
}

/// Publishes once, retrying a publication that failed for want of the network after 2,
/// 4, 8... seconds, at most five times.
fn publish_round(ctx: &Context<'_>, console: &mut Console<'_>) -> Round {
    for attempt in 0..6u32 {
        let round = round(ctx, console);
        if round == Round::Network && attempt < 5 {
            let pause = Duration::from_secs(2 << attempt);
            console.say(&format!("Trying again in {} seconds...", pause.as_secs()));
            if !sleep(ctx, pause) {
                return Round::Cancelled;
            }
            continue;
        }
        if round == Round::Credential {
            console.say("Publication is paused until the token works; the watch goes on.");
        }
        return round;
    }
    Round::Network
}

fn round(ctx: &Context<'_>, console: &mut Console<'_>) -> Round {
    let (status, code) = publish_once(ctx, console, false);
    match code.as_str() {
        _ if status == 0 || status == gd_plugins::EXIT_ACCEPTED => Round::Published,
        "pages.cancelled" => Round::Cancelled,
        "pages.credential" | "pages.no-token" => Round::Credential,
        "pages.network" | "pages.github" | "pages.rate-limit" => Round::Network,
        _ => Round::Failed,
    }
}

/// Waits until the folder's stamp changes and settles; `None` when cancelled.
fn wait_for_change(ctx: &Context<'_>, folder: &Path, stamp: u64, c: &Config) -> Option<u64> {
    let mut current = stamp;
    while current == stamp {
        if !sleep(ctx, Duration::from_millis(500)) {
            return None;
        }
        current = source_stamp(folder);
    }
    let debounce = Duration::from_millis(u64::from(c.debounce_ms));
    let max_wait = Duration::from_millis(u64::from(c.max_wait_ms));
    let first = Instant::now();
    let mut quiet = Instant::now();
    loop {
        if !sleep(ctx, debounce.min(Duration::from_millis(250))) {
            return None;
        }
        let next = source_stamp(folder);
        if next != current {
            current = next;
            quiet = Instant::now();
        }
        if quiet.elapsed() >= debounce || first.elapsed() >= max_wait {
            return Some(current);
        }
    }
}

/// Sleeps for `d` in short steps; `false` when cancelled.
fn sleep(ctx: &Context<'_>, d: Duration) -> bool {
    let end = Instant::now() + d;
    while Instant::now() < end {
        if ctx.cancel.is_cancelled() {
            return false;
        }
        std::thread::sleep((end - Instant::now()).min(Duration::from_millis(100)));
    }
    !ctx.cancel.is_cancelled()
}

/// Sums what can change in `folder`: every file's path, size, and time, except build
/// output, caches, and hidden folders such as `.git`. It is never 0.
fn source_stamp(folder: &Path) -> u64 {
    let mut hash = Fnv::new();
    let mut pending: Vec<PathBuf> = vec![folder.to_path_buf()];
    let mut seen = 0;
    while let Some(dir) = pending.pop() {
        seen += 1;
        if seen > MAX_FOLDERS {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || name == "_build" || name == "build" {
                continue;
            }
            let Ok(meta) = e.metadata() else {
                continue;
            };
            if meta.is_dir() {
                pending.push(e.path());
                continue;
            }
            let modified = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok());
            hash.add(e.path().to_string_lossy().as_bytes());
            hash.add(&meta.len().to_le_bytes());
            hash.add(&modified.map_or(0, |d| d.as_nanos()).to_le_bytes());
        }
    }
    hash.0.max(1)
}

/// FNV-1a, 64-bit: a quick, stable hash of what a folder holds.
struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    fn add(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdir::TestDir;

    #[test]
    fn stamps_change_with_sources_but_not_build_output() {
        let dir = TestDir::new("stamp");
        std::fs::write(dir.path().join("index.rst"), "one").unwrap();
        let first = source_stamp(dir.path());
        assert_eq!(first, source_stamp(dir.path()));
        std::fs::create_dir(dir.path().join("_build")).unwrap();
        std::fs::write(dir.path().join("_build").join("x.html"), "x").unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".git").join("HEAD"), "x").unwrap();
        assert_eq!(first, source_stamp(dir.path()));
        std::fs::write(dir.path().join("more.rst"), "two").unwrap();
        assert_ne!(first, source_stamp(dir.path()));
    }
}
