//! End-to-end builds of a small project with the html, dirhtml and singlehtml builders:
//! pages, navigation, the search index, the indexes, static files, templates, and
//! downloads (download_test.odin).

#![allow(clippy::unwrap_used)]

mod common;

use common::{Project, problems, section, toctree};
use gd_env::{BuilderKind, IndexInfo, ModuleInfo, ObjectInfo};

const INDEX: &str = "Demo\n====\n\nWelcome to the demo.\n\n\
.. toctree::\n   :caption: Contents\n\n   intro\n   api\n";
const INTRO: &str = "Introduction\n============\n\nSome words about widgets.\n\n\
Getting started\n---------------\n\nInstall it, then compute :math:`a^2`.\n";
const API: &str = "API\n===\n\n.. py:module:: demo.api\n\n.. py:function:: run(x)\n\n   Runs.\n";

/// index -> intro, api; intro has a subsection, an index entry and math; api declares
/// a Python module and a function.
fn demo(name: &str) -> Project {
    let mut p = Project::new(name);
    let index = p.add("index.rst", "Demo", INDEX);
    index.sections = vec![section(1, "Demo", "demo", None)];
    let mut tree = toctree(Some(0), 1, &["intro", "api"]);
    tree.caption = "Contents".into();
    index.toctrees = vec![tree];
    let intro = p.add("intro.rst", "Introduction", INTRO);
    intro.sections = vec![
        section(1, "Introduction", "introduction", None),
        section(2, "Getting started", "getting-started", Some(0)),
    ];
    intro.index = vec![IndexInfo {
        entries: "single: widget".into(),
        anchor: "index-0".into(),
        ..IndexInfo::default()
    }];
    let api = p.add("api.rst", "API", API);
    api.sections = vec![section(1, "API", "api", None)];
    api.modules = vec![ModuleInfo {
        name: "demo.api".into(),
        domain: "py".into(),
        anchor: "module-demo.api".into(),
        synopsis: "The API.".into(),
        ..ModuleInfo::default()
    }];
    api.objects = vec![ObjectInfo {
        domain: "py".into(),
        objtype: "function".into(),
        fullname: "demo.api.run".into(),
        anchor: "demo.api.run".into(),
        ..ObjectInfo::default()
    }];
    p.words(
        "intro",
        "introduction\tIntroduction\tSome words about widgets.\n\
         getting-started\tGetting started\tInstall it, then compute.\n",
    );
    p.words("index", "demo\tDemo\tWelcome to the demo.\n");
    p.resolve();
    assert!(p.reports.is_empty(), "{:?}", p.reports);
    p
}

#[test]
fn html_writes_pages_with_navigation_and_math() {
    let p = demo("html-pages");
    let reports = p.build(BuilderKind::Html).unwrap();
    assert!(problems(&reports).is_empty(), "{reports:?}");
    let intro = p.read(BuilderKind::Html, "intro.html");
    assert!(intro.contains("<title>Introduction · Demo documentation</title>"), "{intro}");
    assert!(intro.contains("Some words about widgets."));
    // The sidebar marks the current page; the caption names the toctree.
    assert!(intro.contains("<span class=\"caption-text\">Contents</span>"));
    assert!(intro.contains("class=\"reference internal current\" aria-current=\"page\""));
    assert!(intro.contains("<li class=\"toctree-l1 current\">"));
    // The outline lists the subsection; prev and next follow the reading order.
    assert!(intro.contains("<li class=\"level-2\"><a href=\"#getting-started\">"));
    assert!(intro.contains("rel=\"prev\" href=\"index.html\""));
    assert!(intro.contains("rel=\"next\" href=\"api.html\""));
    // The page has math, so it loads MathJax; the root page does not.
    assert!(intro.contains("mathjax"), "{intro}");
    let index = p.read(BuilderKind::Html, "index.html");
    assert!(!index.contains("mathjax"));
    assert!(index.contains("href=\"intro.html#getting-started\""), "{index}");
    assert!(intro.contains("href=\"_sources/intro.rst.txt\""));
    assert_eq!(p.read(BuilderKind::Html, "_sources/intro.rst.txt"), INTRO);
}

#[test]
fn html_writes_the_theme_the_indexes_and_the_search_index() {
    let p = demo("html-site");
    p.build(BuilderKind::Html).unwrap();
    let kind = BuilderKind::Html;
    assert_eq!(p.read(kind, "_static/guidedog.css"), gd_theme::GUIDEDOG_CSS);
    assert_eq!(p.read(kind, "_static/guidedog.js"), gd_theme::GUIDEDOG_JS);
    assert!(p.read(kind, "_static/pygments.css").contains(".highlight"));
    let genindex = p.read(kind, "genindex.html");
    assert!(genindex.contains("<a href=\"intro.html#index-0\">widget</a>"), "{genindex}");
    let modindex = p.read(kind, "py-modindex.html");
    assert!(modindex.contains("api.html#module-demo.api"), "{modindex}");
    assert!(modindex.contains("<em>The API.</em>"));
    let search = p.read(kind, "search.html");
    assert!(search.contains("id=\"search-page-input\""));
    // Special pages link from the output root and have no outline.
    assert!(search.contains("href=\"intro.html\""));
    assert!(p.out(kind).join("objects.inv").is_file());
    let index = p.read(kind, "_static/searchindex.js");
    assert!(index.starts_with("window.GUIDEDOG_INDEX = {\"pages\":[[\"API\",\"api.html\"],"));
    assert!(index.contains("\"widgets\":["), "{index}");
    assert!(
        index
            .contains("[2,\"Getting started\",\"getting-started\",\"Install it, then compute.\"]")
    );
    // Objects are searched as titles; stop words are listed for the script.
    assert!(index.contains("\"demo.api.run\""));
    assert!(index.contains("\"stopwords\":[\"a\",\"and\""));
}

#[test]
fn dirhtml_writes_a_folder_per_document() {
    let p = demo("dirhtml");
    let reports = p.build(BuilderKind::Dirhtml).unwrap();
    assert!(problems(&reports).is_empty(), "{reports:?}");
    let intro = p.read(BuilderKind::Dirhtml, "intro/index.html");
    assert!(intro.contains("href=\"../_static/guidedog.css\""));
    assert!(intro.contains("rel=\"next\" href=\"../api/\""), "{intro}");
    let index = p.read(BuilderKind::Dirhtml, "index.html");
    assert!(index.contains("href=\"intro/#getting-started\""), "{index}");
    assert!(p.out(BuilderKind::Dirhtml).join("genindex.html").is_file());
}

#[test]
fn singlehtml_puts_every_document_on_the_root_page() {
    let p = demo("singlehtml");
    let reports = p.build(BuilderKind::Singlehtml).unwrap();
    assert!(problems(&reports).is_empty(), "{reports:?}");
    let page = p.read(BuilderKind::Singlehtml, "index.html");
    assert!(page.contains("<section class=\"singlehtml-document\" id=\"document-intro\">"));
    assert!(page.contains("Some words about widgets."));
    assert!(page.contains("Runs."));
    assert!(page.contains("href=\"#intro-getting-started\""), "{page}");
    assert!(!p.out(BuilderKind::Singlehtml).join("intro.html").exists());
    let index = p.read(BuilderKind::Singlehtml, "_static/searchindex.js");
    assert!(index.contains("\"intro-getting-started\""));
}

#[test]
fn project_templates_extend_the_built_in_layout() {
    let mut p = demo("templates");
    p.config.templates_path = vec!["_templates".into()];
    p.config.html.additional_pages.insert("download".into(), "download.html".into());
    p.write(
        "_templates/layout.html",
        "{% extends \"!layout.html\" %}{% block extrahead %}<meta name=\"t\" content=\"\
         {{ pathto('api') }}|{{ pathto('_static/x.png', 1) }}|{{ _('Index') }}|\
         {{ hasdoc('api') }}|{{ hasdoc('nope') }}|{{ ngettext('page', 'pages', 2) }}\">\
         <nav class=\"t\">{{ toctree(maxdepth=1) }}</nav>{% endblock %}",
    );
    p.write(
        "_templates/download.html",
        "{% extends \"layout.html\" %}{% block body %}<p>Get {{ pagename }} \
         from {{ pathto('index') }}.</p>{% endblock %}",
    );
    p.resolve();
    p.build(BuilderKind::Html).unwrap();
    let intro = p.read(BuilderKind::Html, "intro.html");
    assert!(
        intro.contains("content=\"api.html|_static/x.png|Index|True|False|pages\""),
        "{intro}"
    );
    // toctree() answers for the page: one level, collapsed.
    let nav = &intro[intro.find("<nav class=\"t\">").unwrap()..];
    let first =
        "<span class=\"caption-text\">Contents</span></p><ul><li class=\"toctree-l1 current\">";
    assert!(nav.contains(first), "{nav}");
    assert!(!nav[..nav.find("</nav>").unwrap()].contains("getting-started"));
    let download = p.read(BuilderKind::Html, "download.html");
    assert!(download.contains("<p>Get download from index.html.</p>"), "{download}");
}

#[test]
fn a_template_error_stops_the_build_at_its_line() {
    let mut p = demo("template-error");
    p.config.templates_path = vec!["_templates".into()];
    p.write("_templates/layout.html", "<p>\n{{ missing_function() }}</p>");
    p.resolve();
    let problem = p.build(BuilderKind::Html).unwrap_err();
    assert_eq!(problem.code, "template.error");
    assert_eq!(problem.path, "_templates/layout.html:2");
    assert!(!problem.hint.is_empty());
    // Nothing was published.
    assert!(!p.out(BuilderKind::Html).join("index.html").exists());
}

#[test]
fn additional_pages_replace_documents_and_refuse_taken_names() {
    let mut p = demo("additional");
    p.config.templates_path = vec!["_templates".into()];
    p.config.html.additional_pages.insert("intro".into(), "landing.html".into());
    p.config.html.additional_pages.insert("search".into(), "landing.html".into());
    p.write("_templates/landing.html", "<html>Landing {{ pagename }}</html>");
    p.resolve();
    let reports = p.build(BuilderKind::Html).unwrap();
    assert_eq!(p.read(BuilderKind::Html, "intro.html"), "<html>Landing intro</html>");
    let refused: Vec<_> = reports.iter().filter(|r| r.code == "build.additional_page").collect();
    assert_eq!(refused.len(), 1, "{reports:?}");
    assert!(refused[0].message.contains("the site already has search.html"));
    assert!(p.read(BuilderKind::Html, "search.html").contains("search-page-input"));
}

#[test]
fn static_and_extra_files_are_copied_and_github_pages_get_a_cname() {
    let mut p = demo("static");
    p.config.html.static_path = vec!["_static".into()];
    p.config.html.extra_path = vec!["extra".into()];
    p.config.html.logo = "img/logo.svg".into();
    p.config.html.baseurl = "https://docs.example.org/".into();
    p.config.extensions.push("sphinx.ext.githubpages".into());
    p.write("_static/custom.css", "p{}");
    p.write("_static/guidedog.css", "/* mine */");
    p.write("extra/robots.txt", "User-agent: *");
    p.write("img/logo.svg", "<svg/>");
    p.resolve();
    p.build(BuilderKind::Html).unwrap();
    let kind = BuilderKind::Html;
    assert_eq!(p.read(kind, "_static/custom.css"), "p{}");
    assert_eq!(p.read(kind, "_static/guidedog.css"), "/* mine */");
    assert_eq!(p.read(kind, "robots.txt"), "User-agent: *");
    assert_eq!(p.read(kind, "_static/logo.svg"), "<svg/>");
    assert!(p.read(kind, "index.html").contains("src=\"_static/logo.svg\""));
    assert_eq!(p.read(kind, ".nojekyll"), "");
    assert_eq!(p.read(kind, "CNAME"), "docs.example.org");
}

/// download_test.odin: downloads keep their names and suffixes in reStructuredText and
/// Markdown, are copied below `_downloads`, and are saved rather than opened.
#[test]
fn source_downloads_preserve_extensions_in_rst_and_markdown() {
    let notes = "# Original Markdown\n\nUnchanged source.\n";
    let rst = "Original RST\n============\n\nUnchanged source.\n";
    let mut p = Project::new("downloads");
    p.write("downloads/notes.md", notes);
    p.write("downloads/notes.rst", rst);
    p.write("downloads/layout.html", "<a href=\"{{ pathto('guide') }}\">Template source</a>");
    let index = p.add(
        "index.rst",
        "Downloads",
        "Downloads\n=========\n\n:download:`Markdown <downloads/notes.md>`\n\n\
         :download:`RST <downloads/notes.rst>`\n\n\
         :download:`Template <downloads/layout.html>`\n\n\
         :download:`External <https://example.org/manual.pdf>`\n\n\
         :doc:`references.md.rst`\n\n.. toctree::\n\n   guide\n   references.md\n",
    );
    index.sections = vec![section(1, "Downloads", "downloads", None)];
    index.toctrees = vec![toctree(Some(0), 1, &["guide", "references.md"])];
    let guide = p.add(
        "guide.md",
        "Guide",
        "# Guide\n\n{download}`Markdown <downloads/notes.md>`\n\n\
         {download}`RST </downloads/notes.rst>`\n\n\
         {download}`Template <downloads/layout.html>`\n\n[Reference](references.md.rst)\n",
    );
    guide.sections = vec![section(1, "Guide", "guide", None)];
    let refs = p.add(
        "references.md.rst",
        "References",
        "References\n==========\n\nThe document name ends in .md.\n",
    );
    refs.sections = vec![section(1, "References", "references", None)];
    p.options.nitpicky = true;
    p.resolve();
    assert!(p.reports.is_empty(), "{:?}", p.reports);
    let reports = p.build(BuilderKind::Html).unwrap();
    assert!(problems(&reports).is_empty(), "{reports:?}");
    for page in ["index.html", "guide.html"] {
        let html = p.read(BuilderKind::Html, page);
        assert!(html.contains("notes.md") && html.contains("notes.rst"), "{html}");
        assert!(html.contains("references.md.html"), "{html}");
        assert_eq!(html.matches(" download=\"\"").count(), 3, "{html}");
        assert!(!html.contains("href=\"https://example.org/manual.pdf\" download="));
    }
    let mut found = 0;
    for folder in std::fs::read_dir(p.out(BuilderKind::Html).join("_downloads")).unwrap() {
        let folder = folder.unwrap().path();
        for (name, text) in [("notes.md", notes), ("notes.rst", rst)] {
            if let Ok(data) = std::fs::read_to_string(folder.join(name)) {
                assert_eq!(data, text);
                found += 1;
            }
        }
    }
    assert_eq!(found, 2);
}
