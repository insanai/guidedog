//! Older Typst catalogs publish record by record, with the embedded compiler or an
//! installed `typst`; a failing compiler keeps the previous artifacts.

use std::path::PathBuf;
use std::process::Command;

use super::{Legacy, exists};
use guidedog_tests::{Scratch, has, lacks, program, run_command, write};

/// An installed `typst` on this test's PATH, if any.
fn installed_typst() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(format!("typst{}", std::env::consts::EXE_SUFFIX)))
        .find(|candidate| candidate.is_file())
}

#[test]
fn typst_builds_themed_html_and_pdf() {
    let c = Legacy::new("gds-typst");
    // An installed compiler named by a path with spaces; without one, the embedded one.
    let alias = c.scratch.join("typst with spaces.exe");
    let mut args = vec!["build", "1"];
    if let Some(typst) = installed_typst() {
        std::fs::copy(typst, &alias).unwrap();
        args.extend(["--typst", alias.to_str().unwrap()]);
    }
    c.dsc(0, &args);
    let html = c.scratch.read("docs/build/gds-0001-gds-process.html");
    let pdf = std::fs::read(c.scratch.join("docs/build/gds-0001-gds-process.pdf")).unwrap();
    for needle in [
        "class=\"sidebar\"",
        "class=\"record-card\"",
        "gds-section-0",
        "<svg",
        "@media (max-width: 760px)",
        "Download PDF",
    ] {
        has(&html, needle);
    }
    assert!(pdf.starts_with(b"%PDF-"));
    lacks(&html, "https://fonts");
}

#[test]
fn compiler_failure_preserves_previous_artifact() {
    let c = Legacy::new("gds-typst-failure");
    c.dsc(0, &["build", "--number=1", "--target=pdf"]);
    assert!(!c.scratch.exists("docs/build/index.html"));
    let path = c.scratch.join("docs/build/gds-0001-gds-process.pdf");
    let before = std::fs::read(&path).unwrap();
    // guidedog refuses Typst's compile arguments: a portable compiler that fails.
    let guidedog = format!("--typst={}", program("guidedog").display());
    c.dsc(4, &["build", "1", "--target=pdf", &guidedog]);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    has(&c.dsc(4, &["build", "1", "--typst=/not/a/typst"]).err, "--typst=");
}

#[test]
fn full_title_selects_unnumbered_draft() {
    let c = Legacy::new("gds-typst-draft");
    c.dsc(0, &["new", "Cache policy", "--author=Ada"]);
    c.dsc(0, &["build", "Cache policy", "--target=html"]);
    has(&c.scratch.read("docs/build/gds-XXXXX-cache-policy.html"), "Cache policy");
    let partial = c.scratch.read("docs/build/index.html");
    has(&partial, "gds-XXXXX-cache-policy.html");
    has(&partial, "This record has not been built yet.");
    lacks(&partial, "href=\"gds-0001-gds-process.html\"");
    c.dsc(0, &["build", "--target=both"]);
    let index = c.scratch.read("docs/build/index.html");
    has(&index, "gds-XXXXX-cache-policy.html");
    has(&index, "gds-0001-gds-process.pdf");
    assert!(c.scratch.exists("docs/build/gds-XXXXX-cache-policy.pdf"));
    assert!(!c.scratch.exists("docs/build/guidedog-discussions.pdf"));
    assert!(!c.scratch.exists("docs/build/index.pdf"));
}

/// A record that is renamed or removed leaves no page behind: the build folder holds only
/// what the catalog builds, and files that are not the catalog's stay.
#[test]
fn builds_remove_pages_of_records_that_are_gone() {
    let c = Legacy::new("gds-typst-gone");
    c.dsc(0, &["build", "1", "--target=html"]);
    let build = c.scratch.join("docs/build");
    let (gone, stage, other) = (
        build.join("gds-XXXXX-gone.html"),
        build.join(".gds-stage-1.pdf"),
        build.join("notes.html"),
    );
    for path in [&gone, &stage, &other] {
        write(path, "old");
    }
    c.dsc(0, &["build", "1", "--target=html"]);
    assert!(!exists(&gone));
    assert!(!exists(&stage));
    assert!(exists(&other));
    assert!(exists(&build.join("gds-0001-gds-process.html")));
}

/// A reproducible compiler date and explicit offsets agree; an offset that overflows is
/// an error, not a wrapped date.
#[test]
fn compiler_dates_follow_epoch_and_offsets() {
    let dir = Scratch::new("typst-clock");
    dir.write(
        "clock.typ",
        "#assert(datetime.today() == datetime(year: 2000, month: 1, day: 1))\n\
         #assert(datetime.today(offset: -12) == datetime(year: 1999, month: 12, day: 31))\n\
         #assert(datetime.today(offset: duration(hours: 14)) == \
         datetime(year: 2000, month: 1, day: 2))\nClock.\n",
    );
    let convert = |output: &str| {
        let mut command = Command::new(program("guidedog"));
        command.current_dir(&dir.path).args(["convert", "clock.typ", "--to=pdf", "--output"]);
        command.arg(output).env("SOURCE_DATE_EPOCH", "946724400").env("NO_COLOR", "1");
        command
    };
    run_command(convert("clock.pdf").env("TZ", "Etc/GMT-14")).expect(0);
    assert!(dir.exists("clock.pdf"));
    dir.write("clock.typ", "#datetime.today(offset: duration(seconds: 9223372036854775807))\n");
    let r = run_command(&mut convert("overflow.pdf"));
    assert_eq!(r.status, 4, "{}", r.err);
    assert!(!dir.exists("overflow.pdf"));
}
