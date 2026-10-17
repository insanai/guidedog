//! Templates depend on their actual navigation reads, including dynamic native calls.
#![allow(clippy::unwrap_used)]
mod common;

use common::{TempDir, assert_built, load};
use gd_project::{BuildOptions, BuilderKind};

#[test]
fn unrelated_orphans_leave_templates_without_navigation_cached() {
    let dir = TempDir::new("ssg-nav-local");
    dir.put("conf.toml", "project=\"Site\"\ntemplates_path=[\"_templates\"]\n");
    dir.put("_templates/layout.html", "{{ body }}");
    dir.put("index.rst", "Home\n====\n");
    let project = load(&dir.0);
    let run = || gd_project::build(&project, BuilderKind::Html, &BuildOptions::default());
    assert_built(&run());
    dir.put("unrelated.rst", ":orphan:\n\nUnrelated\n=========\n");
    let added = run();
    assert_built(&added);
    assert_eq!(added.read, 1);
    assert_eq!(added.written, 2, "only the new page and search index: {added:?}");
    assert_eq!(run().written, 0);
}

#[test]
fn dynamic_hasdoc_and_pathto_track_creation_removal_and_dirhtml_changes() {
    let dir = TempDir::new("ssg-nav-dynamic");
    dir.put("conf.toml", "project=\"Site\"\ntemplates_path=[\"_templates\"]\n");
    dir.put(
        "_templates/layout.html",
        "{% set target = 'other' %}{{ hasdoc(target) }}|{{ pathto(target) }}|{{ body }}",
    );
    dir.put("index.rst", "Home\n====\n");
    let project = load(&dir.0);
    let run = || gd_project::build(&project, BuilderKind::Dirhtml, &BuildOptions::default());
    assert_built(&run());
    assert!(dir.read("_build/dirhtml/index.html").starts_with("False|other.html|"));
    dir.put("other.rst", ":orphan:\n\nOther\n=====\n");
    assert_built(&run());
    assert!(dir.read("_build/dirhtml/index.html").starts_with("True|other/index.html|"));
    std::fs::remove_file(dir.path("other.rst")).unwrap();
    assert_built(&run());
    assert!(dir.read("_build/dirhtml/index.html").starts_with("False|other.html|"));
}

#[test]
fn body_toctrees_and_parameterized_template_trees_track_target_titles() {
    let dir = TempDir::new("ssg-nav-trees");
    dir.put("conf.toml", "project=\"Site\"\ntemplates_path=[\"_templates\"]\n");
    dir.put("_templates/layout.html", "{{ toctree(maxdepth=1, collapse=False) }}|{{ body }}");
    dir.put("index.rst", "Home\n====\n\n.. toctree::\n\n   other\n");
    dir.put("other.rst", "First title\n===========\n");
    let project = load(&dir.0);
    let run = || gd_project::build(&project, BuilderKind::Html, &BuildOptions::default());
    assert_built(&run());
    dir.put("other.rst", "Second title\n============\n");
    assert_built(&run());
    let html = dir.read("_build/html/index.html");
    assert_eq!(html.matches("Second title").count(), 2, "template tree and body tree: {html}");
    assert!(!html.contains("First title"));
    assert_eq!(run().written, 0);
}

#[test]
fn sections_beyond_a_traced_depth_do_not_invalidate_the_listing_page() {
    let dir = TempDir::new("ssg-nav-depth");
    dir.put("conf.toml", "project=\"Site\"\ntemplates_path=[\"_templates\"]\n");
    dir.put("_templates/layout.html", "{{ toctree(maxdepth=1) }}|{{ body }}");
    dir.put("index.rst", "Home\n====\n\n.. toctree::\n   :maxdepth: 1\n\n   other\n");
    dir.put("other.rst", "Other\n=====\n\nFirst section\n-------------\n\nText.\n");
    let project = load(&dir.0);
    let run = || gd_project::build(&project, BuilderKind::Html, &BuildOptions::default());
    assert_built(&run());
    dir.put("other.rst", "Other\n=====\n\nChanged section\n---------------\n\nText.\n");
    let result = run();
    assert_built(&result);
    assert_eq!(result.written, 2, "changed document and search index: {result:?}");
}

#[test]
fn article_templates_track_only_the_neighbour_keys_they_read() {
    for (name, template, written) in [
        ("meta", "{{ page.meta.date }}|{{ body }}", 2),
        ("alias", "{% set article = page %}{{ article.meta.date }}|{{ body }}", 2),
        ("older", "{% if page.older %}{{ page.older.title }}{% endif %}|{{ body }}", 2),
        ("newer", "{% if page.newer %}{{ page.newer.title }}{% endif %}|{{ body }}", 3),
        ("whole", "{{ page }}|{{ body }}", 3),
    ] {
        let dir = TempDir::new(&format!("ssg-page-{name}"));
        dir.put("conf.toml", "project=\"Site\"\ntemplates_path=[\"_templates\"]\n");
        dir.put("_templates/layout.html", template);
        dir.put("index.rst", "Home\n====\n");
        dir.put("first.rst", ":date: 2026-10-14\n\nFirst\n=====\n");
        dir.put("second.rst", ":date: 2026-10-15\n\nSecond\n======\n");
        dir.put(
            "third.rst",
            ":date: 2026-10-16\n:publish-at: 2026-10-16T00:00:00Z\n\nThird\n=====\n",
        );
        let project = load(&dir.0);
        let run = |at| {
            let options = BuildOptions {
                now: Some(gd_env::publication_time(at).unwrap()),
                ..Default::default()
            };
            gd_project::build(&project, BuilderKind::Html, &options)
        };
        assert_built(&run("2026-10-15"));
        let old_page = dir.read("_build/html/second.html");
        let changed = run("2026-10-16");
        assert_built(&changed);
        assert_eq!(changed.read, 0, "cached scheduled document: {changed:?}");
        assert_eq!(changed.written, written, "{name}: {changed:?}");
        assert_eq!(old_page == dir.read("_build/html/second.html"), written == 2);
        assert_eq!(run("2026-10-16").written, 0);
    }
}
