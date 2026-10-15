//! A build publishes its output, its manifest and its environment together, or none of
//! them: a failed build (-W included) leaves what the last good build published, the next
//! build reports the same diagnostics, and the summary says what happened to the output.

use std::fs;

use guidedog_tests::{Scratch, has, lacks};

use crate::support::{build, project};

/// A quickstarted project with a one-line configuration.
fn demo(name: &str) -> Scratch {
    let dir = project(name);
    dir.write("conf.toml", "project = \"Demo\"\n");
    dir
}

#[test]
fn warnings_as_errors_publish_nothing() {
    let dir = demo("pub-w");
    dir.write("index.rst", "Demo\n====\n\n.. toctree::\n\n   old\n\nFirst.\n");
    dir.write("old.rst", "Old\n===\n\nOld page.\n");
    build(&dir.path, 0, &["html", "-W"]);
    let environment = dir.read("_build/.doctrees/environment.json");
    // The page changes and gains a warning: -W fails the build, and nothing moves.
    dir.write("index.rst", "Demo\n====\n\nSecond, see :ref:`missing`.\n");
    fs::remove_file(dir.join("old.rst")).unwrap();
    let r = build(&dir.path, 1, &["html", "-W"]);
    has(&r.err, "WARNINGS TREATED AS ERRORS");
    has(&dir.read("_build/html/index.html"), "First.");
    assert!(dir.exists("_build/html/old.html"));
    assert_eq!(dir.read("_build/.doctrees/environment.json"), environment);
    assert!(!dir.exists("_build/.doctrees/html.commit.json"));
    // The next build reads the document again and reports the same warning.
    let r = build(&dir.path, 1, &["html", "-W"]);
    has(&r.err, "missing");
    has(&r.out, "1 document(s) parsed");
    has(&dir.read("_build/html/index.html"), "First.");
    // Without -W the warning is reported and the build publishes.
    let r = build(&dir.path, 0, &["html"]);
    has(&r.err, "missing");
    has(&dir.read("_build/html/index.html"), "Second");
    assert!(!dir.exists("_build/html/old.html"));
}

/// A page changes while an extra file becomes a folder: the build publishes all of it,
/// the next build has nothing to do, and the folder can become a file again.
#[test]
fn a_file_that_becomes_a_folder_is_published() {
    let dir = demo("pub-folder");
    dir.write("conf.toml", "project = \"Demo\"\nhtml_extra_path = [\"extras\"]\n");
    dir.write("index.rst", "Demo\n====\n\nFirst.\n");
    dir.write("extras/z", "a file\n");
    build(&dir.path, 0, &["html"]);
    dir.write("index.rst", "Demo\n====\n\nSecond.\n");
    fs::remove_file(dir.join("extras/z")).unwrap();
    dir.write("extras/z/nested", "now a folder\n");
    let r = build(&dir.path, 0, &["html"]);
    has(&r.out, "Output in _build/html");
    assert_eq!(dir.read("_build/html/z/nested"), "now a folder\n");
    has(&dir.read("_build/html/index.html"), "Second.");
    for left in [".html.old", ".doctrees/html.commit.json"] {
        assert!(!dir.exists(&format!("_build/{left}")), "{left} is left");
    }
    let r = build(&dir.path, 0, &["html"]);
    has(&r.out, "0 output(s) written");
    fs::remove_dir_all(dir.join("extras/z")).unwrap();
    dir.write("extras/z", "a file again\n");
    build(&dir.path, 0, &["html"]);
    assert_eq!(dir.read("_build/html/z"), "a file again\n");
}

/// A build folder that cannot be written stops the build before anything is published,
/// and the summary says the previous output is unchanged; a folder the user made
/// read-only inside the output does not stop later builds.
#[cfg(unix)]
#[test]
fn publication_failures_are_reported_truthfully() {
    use std::os::unix::fs::PermissionsExt;
    let mode = |dir: &Scratch, rel: &str, m: u32| {
        fs::set_permissions(dir.join(rel), fs::Permissions::from_mode(m)).unwrap();
    };
    let dir = demo("pub-fail");
    dir.write("index.rst", "Demo\n====\n\nFirst.\n");
    build(&dir.path, 0, &["html"]);
    mode(&dir, "_build", 0o555);
    if fs::create_dir(dir.join("_build/probe")).is_ok() {
        mode(&dir, "_build", 0o755); // an administrator, whom a folder's mode does not stop.
        return;
    }
    dir.write("index.rst", "Demo\n====\n\nSecond.\n");
    let r = build(&dir.path, 4, &["html"]);
    mode(&dir, "_build", 0o755);
    has(&r.out, "The previous output in _build/html is unchanged.");
    has(&dir.read("_build/html/index.html"), "First.");
    mode(&dir, "_build/html/_static", 0o555);
    let r = build(&dir.path, 0, &["html"]);
    has(&r.out, "Output in _build/html");
    has(&dir.read("_build/html/index.html"), "Second.");
    for (text, word) in [("Third.", "Third."), ("Fourth.", "Fourth.")] {
        dir.write("index.rst", &format!("Demo\n====\n\n{text}\n"));
        build(&dir.path, 0, &["html"]);
        has(&dir.read("_build/html/index.html"), word);
    }
    assert!(!dir.exists("_build/.html.old"));
    assert!(!dir.exists("_build/.html.old-2"));
}

/// A cached object whose digest matches but which the engine cannot load is a stale
/// cache: the document is read again, with a warning, and the build publishes it.
#[test]
fn unusable_cached_documents_are_read_again() {
    let dir = demo("pub-cache");
    dir.write("index.rst", "Demo\n====\n\nFirst.\n");
    build(&dir.path, 0, &["html"]);
    let object = fs::read_dir(dir.join("_build/.doctrees/objects"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.file_name().unwrap().to_string_lossy().starts_with("index."))
        .expect("an object for index");
    let old_sum = digest(&fs::read(&object).unwrap());
    let damaged = b"not a Guidedoc object";
    fs::write(&object, damaged).unwrap();
    let env = dir.read("_build/.doctrees/environment.json");
    assert!(env.contains(&old_sum));
    dir.write("_build/.doctrees/environment.json", &env.replace(&old_sum, &digest(damaged)));
    fs::remove_file(dir.join("_build/html/index.html")).unwrap();
    let r = build(&dir.path, 0, &["html"]);
    has(&r.err, "CACHED DOCUMENT READ AGAIN");
    has(&r.err, "CACHED DOCUMENT IS UNUSABLE");
    lacks(&r.err, "DOES NOT MATCH");
    has(&dir.read("_build/html/index.html"), "First.");
    let r = build(&dir.path, 0, &["html", "-a", "-W"]);
    lacks(&r.err, "CACHED DOCUMENT");
}

/// The digest the environment records for a cached file: SHA-256 of its bytes and a NUL.
fn digest(bytes: &[u8]) -> String {
    let mut data = bytes.to_vec();
    data.push(0);
    gd_sha::to_hex(&gd_sha::sha256(&data))
}

/// A problem that stops the build is an error and counts in the summary, and the summary
/// says what happened to the output: written, prepared but not published, or nothing.
#[cfg(unix)]
#[test]
fn fatal_problems_count_in_the_summary() {
    let dir = demo("pub-fatal");
    let outside = Scratch::new("pub-outside");
    outside.write("away.rst", "Away\n====\n");
    dir.write("index.rst", "Demo\n====\n\nFirst.\n");
    std::os::unix::fs::symlink(outside.join("away.rst"), dir.join("away.rst")).unwrap();
    let r = build(&dir.path, 1, &["html"]);
    has(&r.err, "DOCUMENT OUTSIDE THE PROJECT");
    has(&r.out, "build failed, 1 error(s), 0 warning(s). Nothing was published.");
    has(&r.out, "nothing read: the build stopped before reading the documents");
    for absent in ["document(s) parsed", "output(s)", "Output in"] {
        lacks(&r.out, absent);
    }
    assert!(!dir.exists("_build/html"));
    fs::remove_file(dir.join("away.rst")).unwrap();
    dir.write("index.rst", "Demo\n====\n\nSee :ref:`missing`.\n");
    let r = build(&dir.path, 4, &["pdf", "--typst=/nonexistent/typst"]);
    has(&r.out, "1 document(s) parsed, 0 unchanged");
    has(&r.out, "nothing prepared: the build stopped before writing its output");
    lacks(&r.out, "output(s)");
    let r = build(&dir.path, 1, &["html", "-W"]);
    has(&r.out, "output(s) prepared, not published");
    lacks(&r.out, "output(s) written");
    has(&r.out, "build failed, 1 error(s), 1 warning(s). Nothing was published.");
    let r = build(&dir.path, 0, &["html"]);
    has(&r.out, "output(s) written");
    has(
        &r.out,
        "build succeeded with 0 error(s) and 1 warning(s) in the documents. Output in _build/html",
    );
    dir.write("index.rst", "Demo\n====\n\nSee :ref:`absent`.\n");
    let r = build(&dir.path, 1, &["html", "-W"]);
    has(&r.out, "The previous output in _build/html is unchanged.");
}
