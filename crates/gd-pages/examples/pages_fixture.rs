//! A test fixture, not a tool: the pages publisher program pointed at a fake GitHub.
//!
//! A released `guidedog-pages` cannot be pointed away from GitHub (its token goes nowhere
//! else), so the process-protocol tests run this program instead. It reads its settings
//! from the file beside it named like it with `.fixture` appended, which a test writes:
//! one `key = value` per line for `root` (the fake server), `state`, `cache` (folders),
//! and `token`.

use std::path::PathBuf;
use std::process::ExitCode;

use gd_pages::{Credential, Pages, Roots, Secret, Settings};

fn main() -> ExitCode {
    let Some(settings) = fixture_settings() else {
        eprintln!("pages_fixture: no readable fixture file beside the program.");
        return ExitCode::from(2);
    };
    gd_plugins::serve_publisher(Pages::new(settings))
}

fn fixture_settings() -> Option<Settings> {
    let exe = std::env::current_exe().ok()?;
    let mut path = exe.into_os_string();
    path.push(".fixture");
    let text = std::fs::read_to_string(PathBuf::from(path)).ok()?;
    let value = |key: &str| {
        text.lines().find_map(|line| {
            let (k, v) = line.split_once(" = ")?;
            (k == key).then(|| v.to_string())
        })
    };
    Some(Settings {
        roots: Roots::at(&value("root")?),
        credential: Credential::Token(Secret::new(&value("token")?)),
        state_dir: Some(PathBuf::from(value("state")?)),
        cache_dir: Some(PathBuf::from(value("cache")?)),
        ..Settings::default()
    })
}
