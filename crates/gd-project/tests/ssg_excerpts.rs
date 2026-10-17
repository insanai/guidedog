//! Rich excerpts are built from the article AST, never fetched in the browser.

#![allow(clippy::unwrap_used)]
mod common;

use common::{TempDir, assert_built, load};
use gd_project::{BuildOptions, BuilderKind};

fn options() -> BuildOptions {
    BuildOptions {
        now: Some(gd_env::publication_time("2026-10-17").unwrap()),
        ..Default::default()
    }
}

fn fixture(name: &str) -> TempDir {
    let dir = TempDir::new(name);
    dir.put(
        "conf.toml",
        concat!(
            "project = \"Excerpts\"\ntemplates_path = [\"_templates\"]\n",
            "[site]\nbase_url = \"https://example.org\"\ntaxonomies = [\"tags\", \"authors\"]\n",
            "page_size = 1\n[[site.collections]]\noutput = \"blog\"\npage_size = 1\nfeeds = true\n"
        ),
    );
    dir.put(
        "index.rst",
        "Home\n====\n\n.. postlist::\n   :excerpts:\n\n.. postlist::\n   :excerpts:\n",
    );
    dir.put("posts/one.rst", &article("**Rich lead**", "Tail text."));
    dir.put(
        "posts/two.rst",
        concat!(
            ":date: 2026-10-16\n:tags: rust\n:author: Ada\n:summary: Plain <fallback>.\n\n",
            "Second\n======\n\nA whole body that must not become an excerpt.\n"
        ),
    );
    dir.put(
        "posts/lead.svg",
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"/>",
    );
    dir.put("posts/example.txt", "Downloadable example.");
    dir.put("reference.rst", ":orphan:\n\nReference title\n===============\n");
    dir
}

fn article(lead: &str, tail: &str) -> String {
    format!(
        concat!(
            ":date: 2026-10-17\n:tags: rust\n:author: Ada\n:summary: Metadata only.\n\n",
            "First\n=====\n\n.. _lead:\n\n{lead} with *emphasis* and `lead`_.\n\n",
            "A note [1]_ and `later`_. See First_, :doc:`two`, ",
            ":doc:`/reference` and :download:`example.txt`.\n\n",
            "* A list item.\n\n.. image:: lead.svg\n   :alt: Lead image\n\n",
            ".. more::\n\n{tail}\n\n.. _later:\n\nLater target.\n\n.. [1] A late note.\n"
        ),
        lead = lead,
        tail = tail
    )
}

#[test]
fn rich_excerpts_relocate_resources_and_keep_full_articles() {
    for kind in [BuilderKind::Html, BuilderKind::Dirhtml] {
        let dir = fixture(&format!("rich-{kind}"));
        assert_built(&gd_project::build(&load(&dir.0), kind, &options()));
        let root = format!("_build/{}", kind.name());
        let blog = dir.read(&format!("{root}/blog/index.html"));
        for rich in
            ["<strong>Rich lead</strong>", "<em>emphasis</em>", "A list item.", "Lead image"]
        {
            assert!(blog.contains(rich), "{rich}: {blog}");
        }
        assert!(!blog.contains("Tail text."));
        assert!(!blog.contains("Metadata only."));
        assert!(blog.contains("../_images/"), "{blog}");
        assert!(blog.contains("../_downloads/"), "{blog}");
        let article =
            if kind == BuilderKind::Html { "posts/one.html" } else { "posts/one/index.html" };
        let full = dir.read(&format!("{root}/{article}"));
        assert!(full.contains("Tail text."));
        assert!(!full.contains("more::"));
        assert!(blog.contains(&format!("../{article}#later")), "{blog}");
        let second = dir.read(&format!("{root}/blog/page/2/index.html"));
        assert!(second.contains("Plain &lt;fallback&gt;."));
        assert!(!second.contains("A whole body"));
        let taxonomy = dir.read(&format!("{root}/authors/~41da/index.html"));
        assert!(taxonomy.contains("../../_images/"));
        let feed = dir.read(&format!("{root}/blog/atom.xml"));
        assert!(feed.contains("Metadata only."));
        assert!(!feed.contains("Rich lead"));
        for file in
            ["blog/index.html", "blog/page/2/index.html", "index.html", "authors/~41da/index.html"]
        {
            check_links(&dir.path(&root), file);
        }
    }
}

#[test]
fn templates_and_native_lists_share_excerpts_with_isolated_ids() {
    let dir = fixture("rich-templates");
    let conf = format!("{}\ntemplate = \"cards.html\"\n", dir.read("conf.toml"));
    dir.put("conf.toml", &conf);
    dir.put(
        "_templates/cards.html",
        concat!(
            "{% for post in collection.posts %}<article>{{ post.excerpt_html }}</article>",
            "<aside>{{ post.summary }}</aside>{% endfor %}"
        ),
    );
    assert_built(&gd_project::build(&load(&dir.0), BuilderKind::Html, &options()));
    let html = dir.read("_build/html/blog/index.html");
    assert!(html.contains("<strong>Rich lead</strong>"));
    assert!(html.contains("<aside>Metadata only.</aside>"));
    let native = dir.read("_build/html/index.html");
    assert_eq!(native.matches("<strong>Rich lead</strong>").count(), 2);
    let mut ids = std::collections::HashSet::new();
    for rest in native.split(" id=\"").skip(1) {
        let id = rest.split('"').next().unwrap();
        assert!(ids.insert(id), "Duplicate id: {id}");
    }
}

#[test]
fn singlehtml_namespaces_excerpts_separately_from_the_full_articles() {
    let dir = fixture("excerpt-singlehtml");
    let index = format!(
        "{}\n.. toctree::\n   :hidden:\n\n   posts/one\n   posts/two\n   reference\n",
        dir.read("index.rst")
    );
    dir.put("index.rst", &index);
    let result = gd_project::build(&load(&dir.0), BuilderKind::Singlehtml, &options());
    assert_built(&result);
    assert!(!result.reports.iter().any(|r| r.code == "build.singlehtml.id"));
    let html = dir.read("_build/singlehtml/index.html");
    assert_eq!(html.matches("<strong>Rich lead</strong>").count(), 3);
    check_links(&dir.path("_build/singlehtml"), "index.html");
}

#[test]
fn invalid_boundaries_fail_and_literal_examples_stay_literal() {
    for (name, body) in [
        ("duplicate", "Lead.\n\n.. more::\n\nTail.\n\n.. more::\n"),
        ("nested", ".. note::\n\n   Lead.\n\n   .. more::\n"),
        ("subsection", "Lead.\n\nDetails\n-------\n\n.. more::\n"),
    ] {
        let dir = fixture(name);
        dir.put("posts/one.rst", &format!("First\n=====\n\n{body}"));
        let result = gd_project::build(&load(&dir.0), BuilderKind::Html, &options());
        assert!(!result.published);
        assert!(result.reports.iter().any(|r| r.code == "page.excerpt"), "{:?}", result.reports);
    }
    let dir = fixture("literal-more");
    dir.put("posts/one.rst", ":date: 2026-10-17\n\nFirst\n=====\n\nExample::\n\n   .. more::\n");
    assert_built(&gd_project::build(&load(&dir.0), BuilderKind::Html, &options()));
    assert!(dir.read("_build/html/posts/one.html").contains("more::"));
    assert!(!dir.read("_build/html/blog/index.html").contains("more::"));
}

#[test]
fn excerpt_changes_invalidate_listings_without_exposing_withheld_posts() {
    let dir = fixture("excerpt-incremental");
    let project = load(&dir.0);
    assert_built(&gd_project::build(&project, BuilderKind::Html, &options()));
    dir.put("posts/one.rst", &article("**Changed introduction**", "Tail text."));
    assert_built(&gd_project::build(&project, BuilderKind::Html, &options()));
    for file in ["blog/index.html", "tags/rust/index.html", "index.html"] {
        assert!(dir.read(&format!("_build/html/{file}")).contains("Changed introduction"));
    }
    dir.put("reference.rst", ":orphan:\n\nChanged reference\n=================\n");
    assert_built(&gd_project::build(&project, BuilderKind::Html, &options()));
    for file in ["blog/index.html", "index.html"] {
        assert!(dir.read(&format!("_build/html/{file}")).contains("Changed reference"));
    }
    for (name, guard) in [
        ("draft", ":draft: true"),
        ("scheduled", ":publish-at: 2027-01-01T00:00:00Z"),
        ("expired", ":expires-at: 2026-01-01T00:00:00Z"),
    ] {
        dir.put(
            &format!("posts/{name}.rst"),
            &format!("{guard}\n{}", article("Hidden introduction", "Secret tail.")),
        );
    }
    assert_built(&gd_project::build(&project, BuilderKind::Html, &options()));
    assert!(!dir.read("_build/html/blog/index.html").contains("Hidden introduction"));
    assert!(!dir.read("_build/html/index.html").contains("Hidden introduction"));
}

fn check_links(root: &std::path::Path, file: &str) {
    let path = root.join(file);
    let html = std::fs::read_to_string(&path).unwrap();
    for attr in ["href", "src"] {
        for rest in html.split(&format!(" {attr}=\"")).skip(1) {
            let url = rest.split('"').next().unwrap();
            if url.contains(':') || url.starts_with('/') || url.is_empty() {
                continue;
            }
            let (file, anchor) = url.split_once('#').unwrap_or((url, ""));
            let mut target =
                if file.is_empty() { path.clone() } else { path.parent().unwrap().join(file) };
            if target.is_dir() {
                target = target.join("index.html");
            }
            assert!(target.is_file(), "{}: {url}", path.display());
            if !anchor.is_empty() {
                let target_html = std::fs::read_to_string(&target).unwrap();
                assert!(
                    target_html.contains(&format!("id=\"{anchor}\"")),
                    "{}: {url}",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn included_leads_and_local_notes_survive_excerpt_copying() {
    let dir = fixture("included-excerpt");
    dir.put(
        "posts/lead.rst.inc",
        concat!(
            "An *included* lead with [1]_.\n\n.. [1] A local note.\n\n",
            ".. code-block:: rst\n\n   .. more::\n"
        ),
    );
    dir.put(
        "posts/one.rst",
        concat!(
            ":date: 2026-10-17\n\nFirst\n=====\n\n.. include:: lead.rst.inc\n\n",
            ".. more::\n\nFull tail.\n"
        ),
    );
    let project = load(&dir.0);
    assert_built(&gd_project::build(&project, BuilderKind::Html, &options()));
    check_links(&dir.path("_build/html"), "index.html");
    check_links(&dir.path("_build/html"), "blog/index.html");
    let blog = dir.read("_build/html/blog/index.html");
    assert!(blog.contains("A local note."));
    assert!(blog.contains("class=\"backref\""));
    assert!(blog.contains("more")); // Syntax highlighting may split the punctuation.
    assert!(!blog.contains("Full tail."));
    assert_built(&gd_project::build(&project, BuilderKind::Pdf, &options()));
    assert!(dir.read("_build/pdf/book-en.typ").contains("A local note"));
    dir.put("posts/lead.rst.inc", "A **changed include**.\n");
    assert_built(&gd_project::build(&project, BuilderKind::Html, &options()));
    assert!(dir.read("_build/html/blog/index.html").contains("<strong>changed include</strong>"));
    assert!(dir.read("_build/html/index.html").contains("<strong>changed include</strong>"));
}

#[test]
fn article_pdfs_keep_the_tail_and_books_render_native_excerpts() {
    let dir = fixture("excerpt-pdfs");
    dir.put(
        "posts/one.rst",
        &format!(":pdf: article\n{}", article("**Printed lead**", "Printed tail.")),
    );
    let project = load(&dir.0);
    assert_built(&gd_project::build(&project, BuilderKind::Html, &options()));
    let pdf = std::fs::read(dir.path("_build/html/posts/one.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    let source = dir.read("_build/html/sources/posts/one.pdf.typ");
    assert!(source.contains("Printed tail"));
    assert!(!source.contains("more::"));
    assert_built(&gd_project::build(&project, BuilderKind::Pdf, &options()));
    let source = dir.read("_build/pdf/book-en.typ");
    assert!(source.contains("Printed lead"));
    assert!(!source.contains("Printed tail"));
}
