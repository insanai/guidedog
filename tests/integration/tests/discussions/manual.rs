//! The reStructuredText catalog the manual describes, managed and published through the
//! command line with the repository's own record template.

use guidedog_tests::{Scratch, guidedog, has, lacks, repository_text, run_program};

/// A targeted PDF preview leaves the complete catalog's published books intact.
#[test]
fn rst_gds_cli_manages_and_publishes_the_authored_project() {
    let root = Scratch::new("gds-rst");
    guidedog(&root.path, 0, &["quickstart", "docs", "-q", "-p", "Discussions", "-a", "Ada"]);
    root.write(
        "docs/conf.toml",
        "project = \"Discussions\"\nexclude_patterns = [\"_build\", \"template/**\"]\n",
    );
    root.write(
        "docs/index.rst",
        "Discussions\n===========\n\n.. toctree::\n\n   registry\n   bundle\n",
    );
    root.write(
        "docs/template/rfc-template.rst",
        &repository_text("docs/gds/template/rfc-template.rst"),
    );
    std::fs::create_dir_all(root.join("docs/records")).unwrap();
    let dir = root.join("docs");
    let dir_arg = format!("--dir={}", dir.display());
    let gds = |status: i32, args: &[&str]| {
        let mut all = vec!["gds", dir_arg.as_str()];
        all.extend_from_slice(args);
        run_program("guidedog", &root.path, &all).expect(status)
    };
    gds(0, &["new", "Cache policy", "--author=Ada", "--labels=build"]);
    gds(0, &["show", "cache-policy"]);
    gds(0, &["promote", "cache-policy", "--dry-run"]);
    gds(0, &["promote", "cache-policy"]);
    gds(0, &["state", "1", "--to=accepted", "--reason=Reviewed"]);
    gds(
        0,
        &["state", "1", "--to=committed", "--reason=Implemented", "--evidence=Tests and review"],
    );
    gds(0, &["new", "Storage policy", "--author=Ada"]);
    gds(0, &["index", "--check"]);
    gds(0, &["check", "--render"]);
    let complete = root.join("docs/_build/pdf/gds-0001-cache-policy-en.pdf");
    let before = std::fs::read(&complete).unwrap();
    gds(0, &["build", "cache-policy", "--target=pdf"]);
    assert_eq!(std::fs::read(&complete).unwrap(), before);
    assert!(
        root.exists("docs/_build/selected/gds-0001-cache-policy/pdf/gds-0001-cache-policy-en.pdf")
    );
    has(&root.read("docs/records/0001-cache-policy.rst"), ":gds-state: committed");
    let html = root.read("docs/_build/html/records/0001-cache-policy.html");
    for needle in ["Cache policy", "Number", "Authors"] {
        has(&html, needle);
    }
    lacks(&html, "gds-number");
    lacks(&html, "gds-authors");
    let book = root.read("docs/_build/pdf/sources/gds-0001-cache-policy-en.pdf.typ");
    has(&book, "Authors");
    lacks(&book, "gds-number");
}
