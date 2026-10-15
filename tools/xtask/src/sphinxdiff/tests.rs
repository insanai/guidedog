use std::path::Path;

use super::html::{self, Side, attribute_values, counted, link_texts, numbers, numref_texts};
use super::warnings::{Warning, located, per_document, relative_to, split_location};
use super::*;
use crate::fsutil::{TempDir, create_dir, write};

const SPHINX_BODY: &str = concat!(
    "<section id=\"bar\"><h1><span class=\"section-number\">2. </span>Bar</h1>\n",
    "<figure id=\"id1\"><figcaption><span class=\"caption-number\">Fig. 2.1 </span>x",
    "</figcaption>\n<p><a class=\"reference internal\" href=\"baz.html#fig22\">",
    "<span class=\"std std-numref\">Fig. 2.2</span></a> and ",
    "<a class=\"reference internal\" href=\"#bar\"><span class=\"std std-ref\">Bar</span>",
    "</a></p></section>",
);

const GUIDEDOG_BODY: &str = concat!(
    "<section id=\"bar\"><h1><span class=\"section-number\">2. </span>Bar</h1>\n",
    "<figure id=\"id1\"><figcaption><span class=\"caption-number\">Fig. 2.3 </span>x",
    "</figcaption>\n<p><a href=\"baz.html#fig22\">Fig. 2.2</a> and <a href=\"#bar\">Bar</a>",
    "</p></section>",
);

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn numbers_and_numref_texts_are_compared() {
    let (s, g) = (numbers(SPHINX_BODY), numbers(GUIDEDOG_BODY));
    assert_eq!(s.join("|"), "caption Fig. 2.1|section 2.");
    assert_eq!(html::only(&s, &g).join("|"), "caption Fig. 2.1");
    let refs = numref_texts(SPHINX_BODY, "sub/bar.html");
    assert_eq!(refs.join("|"), "sub/baz.html#fig22 Fig. 2.2");
    assert!(html::only(&refs, &link_texts(GUIDEDOG_BODY, "sub/bar.html")).is_empty());
    assert_eq!(counted(strings(&["a", "b", "a"])).join("|"), "a|a (2)|b");
}

#[test]
fn links_resolve_from_the_site_root() {
    let page = "sub/bar.html";
    assert_eq!(html::internal_target("../a.html#x", page).as_deref(), Some("a.html#x"));
    assert_eq!(html::internal_target("#top", page).as_deref(), Some("sub/bar.html#top"));
    assert_eq!(html::internal_target("https://x.org/", page), None);
    assert_eq!(html::internal_target("../_images/a.png", page), None);
    let body = "<a href=\"a.html\">1</a><a href=\"a.html\">2</a>";
    assert_eq!(html::link_targets(body, "index.html").join("|"), "a.html|a.html (2)");
}

#[test]
fn warnings_match_absolute_and_working_directory_relative_paths() {
    let cwd = std::env::current_dir().unwrap_or_default();
    let source = cwd.join("comparison").join("src");
    let (doc, line) = split_location("comparison/src/sub/a.rst:12", &source);
    assert_eq!((doc.as_str(), line), ("sub/a.rst", 12));
    let absolute = source.join("sub").join("a.rst");
    let relative = Path::new("comparison/src");
    assert_eq!(relative_to(&absolute.to_string_lossy(), relative), "sub/a.rst");
    assert_eq!(relative_to("sub/a.rst", &source), "sub/a.rst");
    let outside = "comparison/src-other/a.rst";
    assert_eq!(relative_to(outside, &source), outside);
}

#[test]
fn warnings_are_placed_by_document() {
    let (doc, line) = split_location("/w/src/sub/a.rst:12", Path::new("/w/src/"));
    assert_eq!((doc.as_str(), line), ("sub/a.rst", 12));
    let (doc, line) = split_location("/w/src/a.py:docstring of f", Path::new("/w/src"));
    assert_eq!((doc.as_str(), line), ("a.py:docstring of f", 0));
    assert_eq!(relative_to("/w/src", Path::new("/w/src")), "(project)");
    let warning = |document: &str, severity, line| Warning {
        document: document.to_string(),
        severity,
        line,
    };
    let found = [
        warning("a.rst", "warning", 3),
        warning("a.rst", "warning", 9),
        warning("b.rst", "error", 0),
    ];
    assert_eq!(per_document(&found).join("|"), "a.rst warning|a.rst warning (2)|b.rst error");
    assert_eq!(located(&found).join("|"), "a.rst:3 warning|a.rst:9 warning|b.rst error");
}

#[test]
fn both_logs_are_read() {
    let source = Path::new("/w/src");
    let log = "/w/src/a.rst:4: WARNING: x\nWARNING: y\n/w/src/b.rst:1: ERROR: z\nother\n";
    let found = warnings::sphinx_warnings(log, source);
    assert_eq!(located(&found).join("|"), "(no location) warning|a.rst:4 warning|b.rst:1 error");
    let log = concat!(
        "{\"code\":\"rst.x\",\"severity\":\"warning\",\"path\":\"/w/src/a.rst\",\"line\":4}\n",
        "{\"code\":\"config.y\",\"severity\":\"warning\",\"path\":\"\",\"line\":0}\n",
        "{\"code\":\"rst.z\",\"severity\":\"note\",\"path\":\"/w/src/a.rst\",\"line\":5}\n",
        "not json\n",
    );
    let found = warnings::guidedog_warnings(log, source);
    assert_eq!(located(&found).join("|"), "a.rst:4 warning");
}

// A Sphinx build that failed wrote no pages; the comparison says so instead of
// comparing nothing.
#[test]
fn a_missing_output_has_no_pages() {
    assert!(list_pages(Path::new("/nonexistent/sphinxdiff")).is_empty());
    let options = Options { sphinx: "/nonexistent/sphinxdiff".into(), ..Default::default() };
    let comparison = compare(&options).unwrap_or_default();
    assert_eq!(comparison.summary(), "sphinx=failed");
}

#[test]
fn relative_output_paths_compare_real_page_bodies() {
    let Ok(temp) = TempDir::new("sphinxdiff") else { panic!("no temporary folder") };
    let body = "<article class=\"gd-article\"><p id=\"real\">Actual text.</p></article>";
    assert!(write(&temp.path().join("index.html"), body).is_ok());
    assert!(create_dir(&temp.path().join("_static")).is_ok());
    assert!(write(&temp.path().join("_static/x.html"), body).is_ok());
    let pages = list_pages(temp.path());
    assert_eq!(pages.join("|"), "index.html");
    let ids = attribute_values(html::body(body, Side::Guidedog), "id");
    assert_eq!(ids.join("|"), "real");
}

#[test]
fn two_builds_are_compared_page_by_page() {
    let Ok(temp) = TempDir::new("sphinxdiff") else { panic!("no temporary folder") };
    let (sphinx, guidedog) = (temp.path().join("s"), temp.path().join("g"));
    assert!(create_dir(&sphinx.join("sub")).is_ok() && create_dir(&guidedog.join("sub")).is_ok());
    let sphinx_page =
        format!("<div class=\"body\" role=\"main\">{SPHINX_BODY}<div class=\"footer\">");
    let guidedog_page = format!("<article class=\"gd-article\">{GUIDEDOG_BODY}</article>");
    assert!(write(&sphinx.join("sub/bar.html"), sphinx_page).is_ok());
    assert!(write(&guidedog.join("sub/bar.html"), guidedog_page).is_ok());
    assert!(write(&sphinx.join("only.html"), "").is_ok());
    let options = Options { sphinx, guidedog, verbose: true, logs: None };
    let comparison = compare(&options).unwrap_or_default();
    assert!(comparison.report.contains("  only Sphinx:   only.html\n"));
    assert!(comparison.report.contains("  only Guidedog: caption Fig. 2.3\n"));
    let summary = "pages=1/1 ids=2/0 links=2/0 numbers=2/2 numrefs=1/0 inventory=0/0 \
                   warnings=0/0 identical=0";
    assert_eq!(comparison.summary(), summary);
}
