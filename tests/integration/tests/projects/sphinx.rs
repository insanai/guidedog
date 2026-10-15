//! A Sphinx-style project through the command line: quickstart, build, rebuild and
//! clean; configuration mistakes; Sphinx's constructs across documents (numbered
//! toctrees, labels, domains, graphs, todos, MyST, options, keywords, dates, includes);
//! and parallel reading matching sequential reading.

use guidedog_tests::{Scratch, has, lacks};

use crate::support::{both, build, cli, guidedog_env, project};

#[test]
fn quickstart_build_rebuild_and_clean() {
    let dir = Scratch::new("quickstart");
    cli(&dir.path, 0, &["quickstart", "docs", "-q", "-p", "Demo", "-a", "Ada"]);
    for f in [
        "conf.toml",
        "index.rst",
        ".gitignore",
        "_static/guidedog.css",
        "_templates/layout.html",
        "_templates/book.typ",
        "assets/images/README.txt",
        "assets/video/README.txt",
        "assets/audio/README.txt",
    ] {
        assert!(dir.exists(&format!("docs/{f}")), "quickstart did not write {f}");
    }
    cli(&dir.path, 2, &["quickstart", "docs", "-q", "-p", "Demo", "-a", "Ada"]);
    let docs = dir.join("docs");
    dir.write("docs/index.rst", "Demo\n====\n\nSee `Details`_.\n\nDetails\n-------\n\nText.\n");
    let r = build(&docs, 0, &["html"]);
    has(&r.out, "1 document(s) parsed");
    let page = dir.read("docs/_build/html/index.html");
    has(&page, "<title>Demo · Demo documentation</title>");
    has(&page, "href=\"#details\"");
    assert!(dir.exists("docs/_build/html/_static/guidedog.css"));
    assert!(dir.exists("docs/_build/html/objects.inv"));
    let r = build(&docs, 0, &[]);
    has(&r.out, "0 document(s) parsed, 1 unchanged");
    has(&r.out, "0 output(s) written");
    let r = build(&docs, 0, &["html", "-E"]);
    has(&r.out, "1 document(s) parsed");
    cli(&docs, 0, &["clean", "--color=never"]);
    assert!(!dir.exists("docs/_build/html"));
    let r = build(&dir.path, 2, &["html"]);
    has(&r.err, "quickstart");
}

#[test]
fn configuration_errors_are_explained() {
    let dir = project("conf-errors");
    dir.write("conf.toml", "projct = \"Demo\"\n");
    let r = build(&dir.path, 2, &[]);
    has(&r.err, "conf.toml:1");
    has(&r.err, "Did you mean project?");
    dir.write("conf.toml", "project = \"Demo\"\n");
    let r = build(&dir.path, 2, &["epub"]);
    has(&r.err, "UNKNOWN BUILDER");
    has(&r.err, "dirhtml");
}

/// A numbered toctree, labels and :ref:, domain objects, a captioned graph with
/// :numref:, and a todolist of every todo.
#[test]
fn sphinx_project_links_numbers_and_draws() {
    let dir = project("sphinx-links");
    dir.write(
        "conf.toml",
        "project = \"Demo\"\nnumfig = true\nextensions = [\"sphinx.ext.graphviz\", \
         \"sphinx.ext.todo\"]\ntodo_include_todos = true\n",
    );
    dir.write(
        "index.rst",
        "Demo\n====\n\n.. toctree::\n   :numbered:\n\n   usage\n   api\n\nSee :ref:`install`, \
         :py:func:`demo.run`, and :numref:`flow`.\n\nAlso :numref:`install`.\n",
    );
    dir.write(
        "usage.rst",
        ".. _install:\n\nInstalling\n==========\n\n.. digraph:: flow\n   :caption: The flow\n   \
         :name: flow\n\n   a -> b\n\n.. todo:: Write more.\n",
    );
    dir.write(
        "api.rst",
        "API\n===\n\n.. py:module:: demo\n\n.. py:function:: run(x)\n\n   Runs.\n\n\
         Todos\n-----\n\n\
         .. todolist::\n",
    );
    build(&dir.path, 0, &["html", "-W"]);
    let index = dir.read("_build/html/index.html");
    has(&index, "<a href=\"usage.html#install\">Installing</a>");
    has(&index, "<a href=\"api.html#demo.run\"><code>demo.run()</code></a>");
    has(&index, "<a href=\"usage.html#flow\">Fig. 1.1</a>"); // numbered by chapter
    has(&index, "<a href=\"usage.html#install\">Section 1</a>");
    let usage = dir.read("_build/html/usage.html");
    has(&usage, "<span class=\"section-number\">1. </span>Installing");
    has(&usage, "class=\"graphviz\"");
    has(&usage, "<span class=\"caption-number\">Fig. 1.1 </span>");
    let api = dir.read("_build/html/api.html");
    has(&api, "<span class=\"section-number\">2.1. </span>Todos");
    has(
        &api,
        "<a href=\"usage.html#id1\"><em>original entry</em></a> is located in usage.rst, line 12.",
    );
}

/// Reading on several threads gives the same pages and caches as reading on one.
#[test]
fn parallel_reading_matches_sequential() {
    let dir = project("parallel");
    dir.write("index.rst", "Demo\n====\n\n.. toctree::\n   :glob:\n\n   d/*\n");
    for i in 0..24 {
        let next = (i + 1) % 24;
        dir.write(
            &format!("d/doc{i:02}.rst"),
            &format!(
                ".. _l{i}:\n\nDoc {i}\n======\n\nSee :ref:`l{next}`.\n\n.. py:function:: f{i}(x)\n"
            ),
        );
    }
    build(&dir.path, 0, &["html", "-E", "-j", "1"]);
    let page = |i: usize| dir.read(&format!("_build/html/d/doc{i:02}.html"));
    let pages: Vec<String> = (0..24).map(page).collect();
    let environment = dir.read("_build/.doctrees/environment.json");
    let r = build(&dir.path, 0, &["html", "-E", "-j", "4"]);
    has(&r.out, "25 document(s) parsed");
    for (i, before) in pages.iter().enumerate() {
        assert!(page(i) == *before, "doc{i:02} differs between -j 1 and -j 4");
    }
    assert!(dir.read("_build/.doctrees/environment.json") == environment);
}

/// Markdown documents with myst-parser's syntax share a project with reStructuredText.
#[test]
fn markdown_documents_use_myst_syntax() {
    let dir = project("myst");
    dir.write(
        "conf.toml",
        "project = \"Demo\"\nextensions = [\"myst_parser\"]\nmyst_enable_extensions = \
         [\"colon_fence\", \"dollarmath\", \"substitution\"]\nmyst_substitutions = {product = \
         \"Widget\"}\nmyst_heading_anchors = 2\n",
    );
    dir.write("index.rst", "Demo\n====\n\n.. toctree::\n\n   guide.md\n\nSee :ref:`install`.\n");
    dir.write(
        "guide.md",
        "# Guide\n\n(install)=\n## Installing {{ product }}\n\n```{note}\nInstall *first*.\n\
         ```\n\n:::{warning}\nCareful.\n:::\n\nMath $x^2$ and {ref}`install`.\n\n## Usage\n\n\
         See [usage](index.rst) and [here](guide.md#usage).\n",
    );
    build(&dir.path, 0, &["html", "-W"]);
    let guide = dir.read("_build/html/guide.html");
    has(&guide, "Installing Widget");
    has(&guide, "<aside class=\"admonition note\">");
    has(&guide, "<aside class=\"admonition warning\">");
    has(&guide, "<span class=\"math notranslate nohighlight\">\\(x^2\\)</span>");
    has(&guide, "<a href=\"#install\">Installing Widget</a>");
    has(&guide, "<a href=\"#usage\">here</a>");
    has(
        &dir.read("_build/html/index.html"),
        "<a href=\"guide.html#install\">Installing Widget</a>",
    );
}

/// An unknown directive or role is reported and the page still built; -W fails.
#[test]
fn unknown_constructs_are_reported_and_pages_still_built() {
    let dir = project("unknown");
    dir.write(
        "index.rst",
        "Demo\n====\n\n.. availability:: Unix.\n\nFixed in :gh:`123`. Still *here*.\n\n\
         .. doctest::\n\n   >>> 1 + 1  # doctest: +SKIP\n   2\n",
    );
    let r = build(&dir.path, 0, &["html"]);
    has(&r.err, "rst.directive.unknown");
    has(&r.err, "rst.role.unknown");
    let page = dir.read("_build/html/index.html");
    has(&page, "Still <em>here</em>.");
    has(&page, "highlight-pycon");
    lacks(&page, "doctest: +SKIP");
    build(&dir.path, 1, &["html", "-W"]);
}

/// A page made of many includes, as CPython's deprecations page is, fits.
#[test]
fn pages_made_of_includes_fit() {
    let dir = project("includes");
    let part: String =
        (0..60).map(|i| format!("* Item {i} with :ref:`genindex` and *emphasis*.\n")).collect();
    let mut index = String::from("Demo\n====\n\n");
    for i in 0..40 {
        dir.write(&format!("parts/p{i:02}.rst"), &part);
        index.push_str(&format!(".. include:: parts/p{i:02}.rst\n\n"));
    }
    dir.write("index.rst", &index);
    dir.write("conf.toml", "project = \"Demo\"\nexclude_patterns = [\"parts\"]\n");
    build(&dir.path, 0, &["html", "-W"]);
    has(&dir.read("_build/html/index.html"), "Item 59 with");
}

/// Options, grammar tokens and environment variables resolve and warn as in Sphinx.
#[test]
fn options_tokens_and_envvars_resolve_as_in_sphinx() {
    let dir = project("options");
    dir.write(
        "index.rst",
        "Demo\n====\n\n.. option:: -X\n\n   Options.\n\n.. program:: unittest\n\n\
         .. option:: -c, --catch\n\n   Catch.\n\n.. program:: None\n\n\
         .. productionlist:: python-grammar\n   expr: `atom`\n   atom: NAME\n\n\
         See :option:`-X dev`, :option:`unittest --catch`, :token:`python-grammar:expr`, \
         and :envvar:`PATH`.\n",
    );
    build(&dir.path, 0, &["html", "-W"]);
    let page = dir.read("_build/html/index.html");
    has(&page, "<a href=\"#cmdoption-X\"><code>-X dev</code></a>");
    has(&page, "<a href=\"#cmdoption-unittest-catch\"><code>unittest --catch</code></a>");
    has(&page, "<a href=\"#grammar-token-python-grammar-expr\">");
    // envvar warns only when nitpicky, and nitpick_ignore silences it.
    let r = build(&dir.path, 1, &["html", "-W", "-n", "-a"]);
    has(&r.err, "PATH");
    let ignore = |rule: &str| dir.write("conf.toml", &format!("project = \"Demo\"\n{rule}\n"));
    ignore("nitpick_ignore = [[\"std:envvar\", \"PATH\"]]");
    build(&dir.path, 0, &["html", "-W", "-n", "-a"]);
    // nitpick_ignore_regex matches the whole type and target, as re.fullmatch does.
    ignore("nitpick_ignore_regex = [[\"std:env.*\", \"PAT\"]]");
    build(&dir.path, 1, &["html", "-W", "-n", "-a"]);
    ignore("nitpick_ignore_regex = [[\"std:env.*\", \"P[A-Z]+\"]]");
    build(&dir.path, 0, &["html", "-W", "-n", "-a"]);
    ignore("nitpick_ignore_regex = [[\"std:envvar\", \"P[A\"]]");
    let r = build(&dir.path, 2, &["html"]);
    has(&r.err, "not a regular expression");
}

/// Sphinx resolves :keyword: through the label of the same name, keeping the keyword.
#[test]
fn keywords_resolve_through_labels() {
    let dir = project("keywords");
    dir.write(
        "index.rst",
        "Demo\n====\n\n.. _if:\n\nThe if statement\n----------------\n\nUse :keyword:`if`.\n",
    );
    build(&dir.path, 0, &["html", "-W"]);
    has(&dir.read("_build/html/index.html"), "Use <a href=\"#if\"><code>if</code></a>.");
}

/// Dates follow today_fmt and html_last_updated_fmt in the document language, and
/// SOURCE_DATE_EPOCH fixes them, as in Sphinx.
#[test]
fn dates_are_formatted_as_in_sphinx() {
    let dir = project("dates");
    dir.write(
        "index.rst",
        "Demo\n====\n\nBuilt |today|.\n\n.. |iso| date::\n\nOn |iso|.\n\n\
         .. toctree::\n\n   notes\n",
    );
    dir.write("notes.md", "# Notes\n\nOn {sub-ref}`today`.\n");
    dir.write(
        "conf.toml",
        "project = \"Demo\"\nlanguage = \"de\"\nsource_suffix = [\".rst\", \".md\"]\n\
         today_fmt = \"%A, %-d. %B %Y\"\nhtml_last_updated_fmt = \"\"\n",
    );
    guidedog_env(&dir.path, 0, &["build", "html", "-q"], &[("SOURCE_DATE_EPOCH", "1790000000")]);
    let page = dir.read("_build/html/index.html");
    has(&page, "Built Montag, 21. September 2026.");
    has(&page, "On 2026-09-21.");
    has(&page, "Sept. 21, 2026"); // the translated default format, unless a catalog says.
    has(&dir.read("_build/html/notes.html"), "Montag, 21. September 2026");
}

/// A document another one includes is not an orphan; one only quoted by
/// literalinclude still is.
#[test]
fn included_documents_are_not_orphans() {
    let dir = project("orphans");
    dir.write(
        "index.rst",
        "Demo\n====\n\n.. include:: parts/a.rst\n\n.. literalinclude:: parts/b.rst\n",
    );
    dir.write("parts/a.rst", "A\n-\n\nText a.\n");
    dir.write("parts/b.rst", "B\n-\n\nText b.\n");
    let output = both(&build(&dir.path, 0, &["html"]));
    has(&output, "parts/b.rst");
    lacks(&output, "parts/a.rst");
}
