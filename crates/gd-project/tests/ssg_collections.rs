//! Generated archives use the same publication gate and transaction as source pages.

#![allow(clippy::unwrap_used)]
mod common;

use common::{TempDir, assert_built, load};
use gd_project::{BuildOptions, BuilderKind};

fn fixture(name: &str) -> TempDir {
    let dir = TempDir::new(name);
    dir.put(
        "conf.toml",
        concat!(
            "project = \"Journal\"\ntemplates_path = [\"_templates\"]\n",
            "[site]\nbase_url = \"https://example.org/project\"\n",
            "taxonomies = [\"tags\", \"category\", \"year\"]\npage_size = 1\nsitemap = true\n",
            "[[site.collections]]\noutput = \"journal\"\ntitle = \"Journal\"\n",
            "tags = [\"rust\"]\npage_size = 1\nfeeds = true\n"
        ),
    );
    dir.put("index.rst", "Home\n====\n");
    dir.put("one.rst", ":date: 2026-10-01\n:tags: rust\n:category: code\n\nFirst\n=====\n");
    dir.put("two.rst", ":date: 2026-10-02\n:tags: rust\n:category: code\n\nSecond\n======\n");
    dir.put("secret.rst", ":draft: true\n:date: 2026-10-03\n:tags: private\n\nSecret\n======\n");
    dir
}

fn options() -> BuildOptions {
    BuildOptions {
        now: Some(gd_env::publication_time("2026-10-16").unwrap()),
        ..Default::default()
    }
}

#[test]
fn archives_paginate_filter_link_and_remove_old_outputs() {
    for kind in [BuilderKind::Html, BuilderKind::Dirhtml] {
        let dir = fixture(&format!("archives-{kind}"));
        let project = load(&dir.0);
        assert_built(&gd_project::build(&project, kind, &options()));
        let root = format!("_build/{}", kind.name());
        let first = dir.read(&format!("{root}/journal/index.html"));
        assert!(first.contains("Second"));
        assert!(!first.contains(">First</a>"));
        assert!(first.contains("page/2/index.html"));
        let second = dir.read(&format!("{root}/journal/page/2/index.html"));
        assert!(second.contains(">First</a>"));
        assert!(second.contains("../../index.html"));
        assert!(dir.exists(&format!("{root}/tags/rust/page/2/index.html")));
        assert!(dir.exists(&format!("{root}/category/code/index.html")));
        assert!(dir.exists(&format!("{root}/year/2026/index.html")));
        assert!(!dir.exists(&format!("{root}/tags/private/index.html")));
        let feed = dir.read(&format!("{root}/journal/atom.xml"));
        assert!(feed.contains("https://example.org/project/journal/atom.xml"));
        assert!(feed.contains("Second"));
        assert!(!feed.contains("Secret"));
        let map = dir.read(&format!("{root}/sitemap.xml"));
        assert!(map.contains("https://example.org/project/journal/page/2/index.html"));
        dir.put("two.rst", ":draft: true\n:date: 2026-10-02\n\nSecond\n======\n");
        assert_built(&gd_project::build(&project, kind, &options()));
        assert!(!dir.exists(&format!("{root}/journal/page/2/index.html")));
        assert!(!dir.exists(&format!("{root}/tags/rust/page/2/index.html")));
        assert!(!dir.read(&format!("{root}/journal/atom.xml")).contains("Second"));
    }
}

#[test]
fn generated_pages_cannot_replace_sources_or_additional_pages() {
    let dir = fixture("archive-collision");
    let project = load(&dir.0);
    assert_built(&gd_project::build(&project, BuilderKind::Html, &options()));
    let old = dir.read("_build/html/journal/index.html");
    dir.put("journal/index.rst", "Handwritten\n===========\n");
    let result = gd_project::build(&project, BuilderKind::Html, &options());
    assert!(!result.published);
    assert_eq!(result.problem.as_ref().unwrap().code, "site.collision");
    assert_eq!(dir.read("_build/html/journal/index.html"), old);
}

#[test]
fn taxonomy_names_are_escaped_without_case_or_path_collisions() {
    let dir = fixture("archive-labels");
    dir.put(
        "labels.rst",
        ":date: 2026-10-04\n:tags: Rust, ../private, con, rust\n\nLabels\n======\n",
    );
    assert_built(&gd_project::build(&load(&dir.0), BuilderKind::Html, &options()));
    for slug in ["~52ust", "~2e~2e~2fprivate", "~r-con", "rust"] {
        assert!(dir.exists(&format!("_build/html/tags/{slug}/index.html")), "{slug}");
    }
    assert!(!dir.exists("_build/html/private/index.html"));
}

#[test]
fn article_context_tracks_chronology_taxonomy_and_reading_estimates() {
    let dir = fixture("article-context");
    dir.put(
        "_templates/article.html",
        concat!(
            "{{ page.older.title }}|{{ page.newer.title }}|{{ page.reading_minutes }}|",
            "{% for tag in page.taxonomies.tags %}{{ tag.name }}={{ tag.url }}{% endfor %}"
        ),
    );
    dir.put(
        "one.rst",
        &format!(":date: 2026-10-01\n:tags: rust\n\nFirst\n=====\n\n{}\n", "word ".repeat(400)),
    );
    let project = load(&dir.0);
    assert_built(&gd_project::build(&project, BuilderKind::Html, &options()));
    let first = dir.read("_build/html/one.html");
    assert!(first.contains("|Second|3|rust=tags/rust/index.html"), "{first}");
    dir.put("two.rst", ":date: 2026-09-30\n:tags: rust\n\nSecond\n======\n");
    assert_built(&gd_project::build(&project, BuilderKind::Html, &options()));
    assert!(dir.read("_build/html/one.html").starts_with("Second||3|"));
}

#[test]
fn collection_templates_receive_typed_metadata_and_invalidate_when_it_changes() {
    let dir = fixture("collection-context");
    let config = format!("{}\ntemplate=\"cards.html\"\n", dir.read("conf.toml"));
    dir.put("conf.toml", &config);
    dir.put(
        "_templates/cards.html",
        concat!(
            "{{ collection.page }}/{{ collection.pages }}|{{ collection.next_url }}|",
            "{% for post in collection.posts %}{{ post.date }}:",
            "{% for name in post.meta.authors %}{{ name }}{% endfor %}{% endfor %}"
        ),
    );
    let project = load(&dir.0);
    assert_built(&gd_project::build(&project, BuilderKind::Html, &options()));
    assert!(
        dir.read("_build/html/journal/index.html")
            .starts_with("1/2|page/2/index.html|2026-10-02:")
    );
    dir.put("two.rst", ":date: 2026-10-02\n:tags: rust\n:author: Grace\n\nSecond\n======\n");
    assert_built(&gd_project::build(&project, BuilderKind::Html, &options()));
    assert!(dir.read("_build/html/journal/index.html").ends_with(":Grace"));
}
