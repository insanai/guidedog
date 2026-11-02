//! The built-in theme's scripts in a real headless browser. Chromium or Chrome loads a
//! built page and prints its document once the scripts have run. A script that loops or
//! blocks never lets the page finish, so the browser is stopped after a time limit and
//! the test fails.
//!
//! The browser is `GUIDEDOG_BROWSER`, or the first Chromium or Chrome found. Without one
//! the tests are skipped, unless `GUIDEDOG_REQUIRE_BROWSER` is set, as CI sets it.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use guidedog_tests::Scratch;

use crate::support::{build, project};

/// The longest a page may take to load and run its scripts.
const LIMIT: Duration = Duration::from_secs(45);

/// The browser to run, or None when there is none and none is required.
fn browser() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("GUIDEDOG_BROWSER") {
        return Some(PathBuf::from(path));
    }
    let names =
        ["chromium", "chromium-browser", "google-chrome", "google-chrome-stable", "chrome"];
    let on_path = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
            .find(|path| path.is_file())
    });
    let mac = PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
    let found = on_path.or_else(|| mac.is_file().then_some(mac));
    if found.is_none() {
        assert!(
            std::env::var_os("GUIDEDOG_REQUIRE_BROWSER").is_none(),
            "GUIDEDOG_REQUIRE_BROWSER is set, but no Chromium or Chrome was found; \
             set GUIDEDOG_BROWSER to its path"
        );
        eprintln!("skipped: no Chromium or Chrome found (set GUIDEDOG_BROWSER)");
    }
    found
}

/// Loads `page` at desktop width and returns its document after the scripts ran, with
/// the time it took. Panics when the page does not finish within [`LIMIT`].
fn load(browser: &Path, page: &Path, profile: &Path) -> (String, Duration) {
    let start = Instant::now();
    let mut child = Command::new(browser)
        .args([
            "--headless=new",
            "--disable-gpu",
            // The page is our own build; the sandbox needs user namespaces that some
            // CI machines do not grant.
            "--no-sandbox",
            "--no-first-run",
            "--no-default-browser-check",
            "--window-size=1400,900",
            "--virtual-time-budget=10000",
            "--dump-dom",
        ])
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg(format!("file://{}", page.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut dom = String::new();
        let _ = stdout.read_to_string(&mut dom);
        dom
    });
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if start.elapsed() > LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{} did not finish within {LIMIT:?}: a script loops or blocks", page.display());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    (reader.join().unwrap(), start.elapsed())
}

/// A quickstarted project whose index holds `body`, built to HTML.
fn site(name: &str, body: &str) -> Scratch {
    let dir = project(name);
    dir.write("index.rst", &format!("Demo\n====\n\n{body}"));
    build(&dir.path, 0, &["html", "-W"]);
    dir
}

/// Paragraphs long enough to break into several lines at desktop width.
fn paragraphs(count: usize, sentence: &str) -> String {
    let paragraph = sentence.repeat(6);
    (0..count).map(|_| format!("{paragraph}\n\n")).collect()
}

#[test]
fn typography_typesets_repeated_hyphenation_exceptions_without_looping() {
    let Some(browser) = browser() else { return };
    // "project" is a hyphenation exception. The hyphenator once handed out its stored
    // exception arrays, and each occurrence appended its punctuation to them, until a
    // syllable outgrew the text and the line breaker looped forever.
    let dir = site(
        "browser-exceptions",
        &paragraphs(
            3,
            "Every project starts small. A project, once begun, needs care: the project \
             grows, and the project's readers follow the project. ",
        ),
    );
    let (dom, _) = load(&browser, &dir.join("_build/html/index.html"), &dir.join(".chrome"));
    assert!(dom.contains("data-layout=\"knuth-plass\""), "no paragraph was typeset:\n{dom}");
}

#[test]
fn typography_keeps_long_pages_responsive() {
    let Some(browser) = browser() else { return };
    // Measuring each paragraph right after rewriting the previous one made the browser lay
    // out the whole page once per paragraph; a page this long then blocked for minutes.
    let dir = site(
        "browser-long",
        &paragraphs(
            1200,
            "We measure every paragraph first and rewrite them afterwards, so the browser \
             lays out the page a fixed number of times. ",
        ),
    );
    let (dom, took) = load(&browser, &dir.join("_build/html/index.html"), &dir.join(".chrome"));
    let typeset = dom.matches("data-layout=\"knuth-plass\"").count();
    assert!(typeset >= 1000, "only {typeset} of 1200 paragraphs were typeset in {took:?}");
}
