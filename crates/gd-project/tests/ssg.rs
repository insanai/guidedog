//! Publication boundaries exercise cached reads, real templates and transactional output.

#![allow(clippy::unwrap_used)]
mod common;

use common::{TempDir, assert_built, load};
use gd_project::{BuildOptions, BuilderKind};

fn options(at: &str) -> BuildOptions {
    BuildOptions { now: Some(gd_env::publication_time(at).unwrap()), ..Default::default() }
}

fn project(name: &str) -> TempDir {
    let dir = TempDir::new(name);
    dir.put(
        "conf.toml",
        concat!(
            "project = \"Publication\"\nauthor = \"Ada\"\n",
            "extensions = [\"myst_parser\"]\nsource_suffix = [\".rst\", \".md\"]\n",
            "templates_path = [\"_templates\"]\n[site]\n",
            "base_url = \"https://example.org/project\"\nsitemap = true\nfeeds = true\n"
        ),
    );
    dir.put("index.rst", "Publication\n===========\n\n.. postlist:: 10\n   :excerpts:\n");
    dir
}

#[test]
fn clock_transitions_hide_every_public_artifact_and_reuse_private_objects() {
    let dir = project("ssg-clock");
    dir.put(
        "post.md",
        concat!(
            "---\ndate: 2026-10-16\npublish-at: 2026-10-16T09:00:00Z\n",
            "authors: [Ada, Grace]\ntags: [rust, publishing]\nsummary: Secret summary.\n---\n",
            "# Scheduled article\n\nSecret body.\n"
        ),
    );
    let project = load(&dir.0);
    let run = |clock| gd_project::build(&project, BuilderKind::Html, &options(clock));
    let first = run("2026-10-16T08:59:59Z");
    assert_built(&first);
    assert!(!dir.exists("_build/html/post.html"));
    for file in ["index.html", "rss.xml", "atom.xml", "sitemap.xml", "_static/searchindex.js"] {
        assert!(!dir.read(&format!("_build/html/{file}")).contains("Scheduled article"));
    }
    let released = run("2026-10-16T09:00:00Z");
    assert_built(&released);
    assert_eq!(released.read, 0, "the clock changes eligibility without reparsing");
    let html = dir.read("_build/html/post.html");
    assert!(html.contains("Grace"), "{html}");
    assert!(dir.read("_build/html/index.html").contains("Scheduled article"));
    assert!(dir.read("_build/html/atom.xml").contains("2026-10-16T00:00:00Z"));
    let hidden = run("2026-10-16T08:59:59Z");
    assert_built(&hidden);
    assert_eq!(hidden.read, 0);
    assert!(!dir.exists("_build/html/post.html"));
    assert!(!dir.exists("_build/html/_sources/post.md.txt"));
    let preview = BuildOptions { preview: true, ..options("2026-10-16T08:59:59Z") };
    assert_built(&gd_project::build(&project, BuilderKind::Html, &preview));
    assert!(dir.exists("_build/html/post.html"));
    assert_built(&run("2026-10-16T08:59:59Z"));
    assert!(!dir.exists("_build/html/post.html"));
}

#[test]
fn body_edits_and_unused_templates_leave_other_pages_cached() {
    let dir = project("ssg-inputs");
    dir.put("post.rst", ":date: 2026-10-15\n:summary: Summary.\n\nArticle\n=======\n\nBody.\n");
    dir.put("_templates/unused.html", "Unused template");
    let project = load(&dir.0);
    let run = || gd_project::build(&project, BuilderKind::Html, &options("2026-10-16"));
    assert_built(&run());
    dir.put(
        "post.rst",
        ":date: 2026-10-15\n:summary: Summary.\n\nArticle\n=======\n\nNew body.\n",
    );
    let changed = run();
    assert_built(&changed);
    assert_eq!(changed.read, 1);
    assert_eq!(changed.written, 2, "article HTML and search index: {changed:?}");
    dir.put("_templates/unused.html", "Changed unused template");
    let unchanged = run();
    assert_built(&unchanged);
    assert_eq!(unchanged.written, 0);
    dir.put(
        "_templates/article.html",
        "{% extends 'layout.html' %}{% block body %}Custom{{ body }}{% endblock %}",
    );
    let changed = run();
    assert_built(&changed);
    assert_eq!(changed.written, 1);
    assert!(dir.read("_build/html/post.html").contains("Custom"));
}

#[test]
fn article_pdf_uses_metadata_and_is_removed_when_the_article_is_withheld() {
    let dir = project("ssg-pdf");
    dir.put(
        "post.rst",
        concat!(
            ":date: 2026-10-15\n:pdf: article\n:author: Ada\n",
            ":summary: Abstract content.\n\nPaper\n=====\n\nArticle body.\n"
        ),
    );
    let project = load(&dir.0);
    let run = || gd_project::build(&project, BuilderKind::Html, &options("2026-10-16"));
    assert_built(&run());
    let pdf = std::fs::read(dir.path("_build/html/post.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    let source = dir.read("_build/html/sources/post.pdf.typ");
    assert!(source.contains("authors: (\"Ada\",)"));
    assert!(source.contains("Abstract content."));
    dir.put("post.rst", ":draft: true\n:date: 2026-10-15\n:pdf: article\n\nPaper\n=====\n");
    assert_built(&run());
    assert!(!dir.exists("_build/html/post.pdf"));
}

#[test]
fn invalid_publication_metadata_prevents_output_commit() {
    let dir = project("ssg-invalid");
    dir.put("post.rst", ":draft: perhaps\n\nPrivate\n=======\n");
    let result = gd_project::build(&load(&dir.0), BuilderKind::Html, &options("2026-10-16"));
    assert!(!result.published);
    assert!(result.reports.iter().any(|r| r.code == "page.metadata"));
    assert!(!dir.exists("_build/html/post.html"));
}

#[test]
fn dynamic_optional_templates_track_creation_edits_and_removal() {
    let dir = project("ssg-dynamic-templates");
    dir.put("post.rst", ":date: 2026-10-15\n\nArticle\n=======\n\nText.\n");
    dir.put(
        "_templates/article.html",
        concat!(
            "{% extends 'layout.html' %}",
            "{% block body %}{% set partial = 'optional.html' %}",
            "{% include partial ignore missing %}{{ body }}{% endblock %}"
        ),
    );
    let project = load(&dir.0);
    let run = || gd_project::build(&project, BuilderKind::Html, &options("2026-10-16"));
    assert_built(&run());
    assert_eq!(run().written, 0);
    for text in ["First optional content", "Changed optional content"] {
        dir.put("_templates/optional.html", text);
        let changed = run();
        assert_built(&changed);
        assert_eq!(changed.written, 1);
        assert!(dir.read("_build/html/post.html").contains(text));
    }
    std::fs::remove_file(dir.path("_templates/optional.html")).unwrap();
    let changed = run();
    assert_built(&changed);
    assert_eq!(changed.written, 1);
    assert!(!dir.read("_build/html/post.html").contains("Changed optional content"));
}

#[test]
fn malformed_markdown_publication_blocks_fail_closed() {
    let dir = project("ssg-invalid-yaml");
    for source in ["---\ndraft: true\n# Private\n", "---\ndraft: [unterminated\n---\n# Private\n"]
    {
        dir.put("post.md", source);
        let result = gd_project::build(&load(&dir.0), BuilderKind::Html, &options("2026-10-16"));
        assert!(!result.published, "{result:?}");
        assert!(!dir.exists("_build/html/post.html"));
    }
}
