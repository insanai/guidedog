//! The pages a Sphinx project's configuration shapes, through the command line: theme
//! templates, html_context, additional pages, the module and general indexes,
//! include_roots, section labels, wildcard images, intersphinx, MathJax, canonical names
//! and module targets.

use guidedog_tests::{has, lacks};

use crate::support::{both, build, in_order, project};

/// A theme's template extends Guidedog's layout as it extends Sphinx's: blocks with
/// super(), toctree() with its arguments, related links, parents and meta tags.
#[test]
fn sphinx_templates_extend_the_layout() {
    let dir = project("templates");
    dir.write(
        "_templates/layout.html",
        "{% extends \"!layout.html\" %}\n{% block rootrellink %}{{ super() }}<li id=\"home\">\
         <a href=\"{{ pathto('index') }}\">{{ shorttitle }}</a>{{ reldelim1 }}</li>\
         {% endblock %}\n{% block extrahead %}{{ metatags }}{% endblock %}\n\
         {% block sidebartoc %}<nav id=\"flat\">{{ toctree(maxdepth=1, titles_only=true) }}\
         </nav>{% endblock %}\n{% block footer %}{% for r in rellinks %}<a class=\"rel\" \
         href=\"{{ pathto(r[0]) }}\" accesskey=\"{{ r[2] }}\">{{ r[3] }}</a>{% endfor %}\
         {% for p in parents %}<span class=\"parent\">{{ p.title }}</span>{% endfor %}\
         {% endblock %}\n",
    );
    dir.write("index.rst", "Demo\n====\n\n.. toctree::\n\n   a\n");
    dir.write("a.rst", "A\n=\n\n.. toctree::\n\n   b\n\nA1\n--\n\nText.\n");
    dir.write("b.rst", ".. meta::\n   :description: About B\n\nB\n=\n\nText.\n");
    build(&dir.path, 0, &["html", "-W"]);
    let page = dir.read("_build/html/b.html");
    has(&page, "<li id=\"home\"><a href=\"index.html\">Demo documentation</a> &#187;</li>");
    has(&page, "<meta name=\"description\" content=\"About B\" />");
    has(&page, "accesskey=\"I\">index</a>");
    has(&page, "accesskey=\"P\">previous</a>");
    has(&page, "<span class=\"parent\">A</span>");
    // maxdepth=1 with titles_only: documents at the first level only, no sections.
    has(&page, "<nav id=\"flat\"><ul><li class=\"toctree-l1 current\">");
    lacks(&page, "toctree-l2");
    has(
        &dir.read("_build/html/a.html"),
        "<li class=\"toctree-l1 current\"><a class=\"reference internal current\"",
    );
}

/// html_context's values reach templates with their types.
#[test]
fn html_context_values_keep_their_types() {
    let dir = project("context");
    dir.write(
        "conf.toml",
        "project = \"Demo\"\ntemplates_path = [\"_templates\"]\n\n[html_context]\n\
         preview = false\nshown = true\ncount = 3\nlinks = [{name = \"Donate\", url = \
         \"https://example.org/donate\"}, {name = \"Chat\", url = \
         \"https://example.org/chat\"}]\n",
    );
    dir.write(
        "_templates/layout.html",
        "{% extends \"!layout.html\" %}\n{% block extrahead %}{% if preview %}PREVIEW{% endif %}\
         {% if shown %}SHOWN{% endif %}<meta name=\"n\" content=\"{{ count + 1 }}\">\
         {% for l in links %}<link rel=\"x\" title=\"{{ l.name }}\" href=\"{{ l.url }}\">\
         {% endfor %}{% endblock %}\n",
    );
    build(&dir.path, 0, &["html", "-W"]);
    let page = dir.read("_build/html/index.html");
    lacks(&page, "PREVIEW");
    has(
        &page,
        "SHOWN<meta name=\"n\" content=\"4\"><link rel=\"x\" title=\"Donate\" \
         href=\"https://example.org/donate\"><link rel=\"x\" title=\"Chat\" \
         href=\"https://example.org/chat\">",
    );
}

const INDEX_CONTENT: &str = "{% extends \"layout.html\" %}\n\
    {% block htmltitle %}<title>{{ shorttitle }}</title>{% endblock %}\n\
    {% block body %}<h1>{{ docstitle|e }}</h1>\n\
    <a class=\"biglink\" href=\"{{ pathto('usage/install') }}\">Install</a>\n\
    <a id=\"gi\" href=\"{{ pathto('genindex') }}\">Index</a> page={{ pagename }}\n\
    {% if preview %}PREVIEW{% endif %}{% for l in links %}<a class=\"pl\" \
    href=\"{{ l.url }}\">{{ l.name }}</a>{% endfor %}{% endblock %}\n";

/// html_additional_pages renders pages from templates alone, as CPython makes its
/// index.html from indexcontent.html, in every HTML address scheme.
#[test]
fn additional_pages_are_rendered_from_templates() {
    let dir = project("additional");
    std::fs::remove_file(dir.join("index.rst")).unwrap();
    dir.write(
        "conf.toml",
        "project = \"Demo\"\nroot_doc = \"contents\"\ntemplates_path = [\"_templates\"]\n\n\
         [html_additional_pages]\nindex = \"indexcontent.html\"\n\n[html_context]\n\
         preview = false\nlinks = [{name = \"Donate\", url = \"https://example.org/donate\"}]\n",
    );
    dir.write(
        "_templates/layout.html",
        "{% extends \"!layout.html\" %}\n{% block rootrellink %}<li><a id=\"home\" \
         href=\"{{ pathto('index') }}\">Home</a></li>{% endblock %}\n",
    );
    dir.write("_templates/indexcontent.html", INDEX_CONTENT);
    dir.write("contents.rst", "Demo\n====\n\n.. toctree::\n\n   usage/install\n");
    dir.write("usage/install.rst", "Install\n=======\n\nText.\n");
    build(&dir.path, 0, &["html", "-W"]);
    let index = dir.read("_build/html/index.html");
    has(&index, "<title>Demo documentation</title>");
    has(&index, "<h1>Demo documentation</h1>");
    has(&index, "<a class=\"biglink\" href=\"usage/install.html\">Install</a>");
    has(&index, "<a id=\"gi\" href=\"genindex.html\">Index</a> page=index");
    has(&index, "<a class=\"pl\" href=\"https://example.org/donate\">Donate</a>");
    lacks(&index, "PREVIEW");
    has(
        &dir.read("_build/html/usage/install.html"),
        "<a id=\"home\" href=\"../index.html\">Home</a>",
    );
    // The page is written again when its template changes.
    let changed = "{% block extrahead %}<meta name=\"changed\">{% endblock %}\n";
    dir.write("_templates/indexcontent.html", &format!("{INDEX_CONTENT}{changed}"));
    build(&dir.path, 0, &["html", "-W"]);
    has(&dir.read("_build/html/index.html"), "<meta name=\"changed\">");
    // dirhtml writes it at the root, where pathto finds it and the generated pages.
    build(&dir.path, 0, &["dirhtml", "-W"]);
    has(&dir.read("_build/dirhtml/index.html"), "<a id=\"gi\" href=\"genindex.html\">Index</a>");
    assert!(dir.exists("_build/dirhtml/genindex.html"));
    has(
        &dir.read("_build/dirhtml/usage/install/index.html"),
        "<a id=\"home\" href=\"../../index.html\">Home</a>",
    );
    // singlehtml writes it beside the single page, as Sphinx's does.
    build(&dir.path, 0, &["singlehtml", "-W"]);
    has(
        &dir.read("_build/singlehtml/index.html"),
        "href=\"contents.html#document-usage-install\">Install</a>",
    );
}

/// An additional page named as a document is written in its place; one that would
/// replace a generated page or leave the output root is refused with a warning.
#[test]
fn additional_pages_replace_documents_but_not_generated_pages() {
    let dir = project("additional-replace");
    let conf =
        "project = \"Demo\"\ntemplates_path = [\"_templates\"]\n\n[html_additional_pages]\n";
    dir.write("conf.toml", &format!("{conf}index = \"landing.html\"\n"));
    dir.write(
        "_templates/landing.html",
        "{% extends \"layout.html\" %}{% block body %}LANDING{% endblock %}",
    );
    dir.write("index.rst", "Demo\n====\n\nThe document.\n");
    build(&dir.path, 0, &["html", "-W"]);
    let index = dir.read("_build/html/index.html");
    has(&index, "LANDING");
    lacks(&index, "The document.");
    let r = build(&dir.path, 0, &["html"]);
    has(&r.out, "0 output(s) written");
    has(&dir.read("_build/html/index.html"), "LANDING");
    dir.write(
        "conf.toml",
        &format!("{conf}genindex = \"landing.html\"\n\"sub/page\" = \"landing.html\"\n"),
    );
    let r = build(&dir.path, 0, &["html"]);
    has(&r.err, "html_additional_pages.genindex is not written: the site already has");
    has(&r.err, "genindex.html.");
    has(&r.err, "html_additional_pages.sub/page is not written");
    has(&dir.read("_build/html/index.html"), "The document.");
    assert!(!dir.exists("_build/html/sub/page.html"));
}

/// Sphinx's Python module index: modules by letter, packages heading their submodules,
/// placeholders for undocumented packages, and the generated pages' labels.
#[test]
fn python_modules_have_an_index() {
    let dir = project("modindex");
    dir.write(
        "index.rst",
        "Mods\n====\n\n.. toctree::\n\n   os\n   mime\n\nSee :ref:`genindex`, :ref:`modindex`, \
         :ref:`search`, and :py:mod:`os.path`.\n",
    );
    dir.write(
        "os.rst",
        "os\n==\n\n.. py:module:: os\n   :synopsis: OS.\n\n.. py:module:: os.path\n   \
         :platform: Unix\n   :deprecated:\n",
    );
    dir.write("mime.rst", "MIME\n====\n\n.. py:module:: email.mime\n");
    build(&dir.path, 0, &["html", "-W"]);
    let index = dir.read("_build/html/index.html");
    has(&index, "<a href=\"py-modindex.html\">Module Index</a>");
    has(&index, "<a href=\"search.html\">Search Page</a>");
    has(&index, "<a href=\"os.html#module-os.path\"><code>os.path</code></a>");
    let mods = dir.read("_build/html/py-modindex.html");
    has(&mods, "<tr><td></td><td><code class=\"xref\">email</code></td>");
    has(
        &mods,
        "<tr class=\"cg-2\"><td></td><td>&#160;&#160;&#160;<a href=\"os.html#module-os.path\">",
    );
    has(&mods, "<em>(Unix)</em></td><td><strong>Deprecated:</strong>");
}

/// The general index has Sphinx's terms, order, groups, anchors and folding, and splits
/// into a page per letter with html_split_index.
#[test]
fn general_index_is_built_as_in_sphinx() {
    let dir = project("genindex");
    dir.write(
        "index.rst",
        "Demo\n====\n\n.. index::\n   pair: loop; statement\n   triple: a; b; c\n   \
         see: alias; loop\n   seealso: loop; iteration\n   !Zeta, _private, 42\n\n\
         .. py:function:: getcwd()\n   :module: posix\n\n.. py:function:: getcwd()\n   \
         :module: os\n\n.. glossary::\n\n   API : iface\n      Words.\n",
    );
    build(&dir.path, 0, &["html"]);
    let page = dir.read("_build/html/genindex.html");
    in_order(
        &page,
        &[
            "<h2 id=\"Symbols\">",
            ">42</a>",
            "<h2 id=\"_\">",
            ">_private</a>",
            "<h2 id=\"A\">",
            "<li>a\n",
            "<li>alias\n",
            "<li>see loop</li>",
            "<h2 id=\"B\">",
            "<h2 id=\"C\">",
            "<h2 id=\"G\">",
            "<h2 id=\"iface\">",
            "<strong>API</strong>",
            "<h2 id=\"L\">",
            "<li>loop\n",
            "<li>see also iteration</li>",
            "<li>statement\n",
            "<h2 id=\"Z\">",
        ],
    );
    in_order(
        &page,
        &[
            "<a href=\"index.html#os.getcwd\">getcwd() (in module os)</a>",
            "<li><a href=\"index.html#posix.getcwd\">(in module posix)</a></li>",
        ],
    );
    has(&page, "<a href=\"index.html#term-API\"><strong>API</strong></a>");
    has(&page, "<strong>Zeta</strong>");
    has(&page, "<a href=\"#Symbols\"><strong>Symbols</strong></a>");
    dir.write("conf.toml", "project = \"Demo\"\nhtml_split_index = true\n");
    build(&dir.path, 0, &["html"]);
    has(&dir.read("_build/html/genindex.html"), "href=\"genindex-L.html\"");
    has(&dir.read("_build/html/genindex-L.html"), "Index &ndash; L");
    has(&dir.read("_build/html/genindex-all.html"), "<h2 id=\"Z\">");
}

/// Files outside the source folder are read only from the include_roots listed.
#[test]
fn include_roots_opt_into_files_outside_the_source() {
    let top = guidedog_tests::Scratch::new("include-roots");
    top.write("Lib/grammar.txt", "start: expr\n");
    top.write("Lib/logo.svg", "<svg xmlns=\"http://www.w3.org/2000/svg\"/>\n");
    std::fs::create_dir(top.join("Doc")).unwrap();
    let doc = top.join("Doc");
    crate::support::cli(&doc, 0, &["quickstart", "-q", "-p", "Demo", "-a", "Ada"]);
    top.write(
        "Doc/index.rst",
        "Demo\n====\n\n.. literalinclude:: ../Lib/grammar.txt\n\n.. image:: ../Lib/logo.svg\n",
    );
    let output = both(&build(&doc, 0, &["html"]));
    has(&output, "core.include.refused");
    has(&output, "build.image.refused");
    top.write("Doc/conf.toml", "project = \"Demo\"\ninclude_roots = [\"../Lib\"]\n");
    build(&doc, 0, &["html", "-W"]);
    let page = top.read("Doc/_build/html/index.html");
    has(&page, "<span class=\"n\">start</span>");
    has(&page, "src=\"_images/logo.svg\"");
    assert!(top.exists("Doc/_build/html/_images/logo.svg"));
    top.write("Doc/conf.toml", "project = \"Demo\"\ninclude_roots = [\"../Nope\"]\n");
    let r = build(&doc, 2, &["html"]);
    has(&r.err, "include_roots names ../Nope");
}

/// autosectionlabel and suppress_warnings behave as in Sphinx.
#[test]
fn section_labels_and_suppressed_warnings_follow_sphinx() {
    let dir = project("autosectionlabel");
    dir.write(
        "index.rst",
        "Demo\n====\n\n.. toctree::\n\n   guide\n\nExamples\n--------\n\nSee :ref:`Install`, \
         :ref:`It's *here*`, and :ref:`missing-label`.\n",
    );
    dir.write(
        "guide.rst",
        "Guide\n=====\n\nInstall\n-------\n\nExamples\n~~~~~~~~\n\nText.\n\nIt's *here*\n\
         -----------\n\nText.\n",
    );
    let conf = "project = \"Demo\"\nextensions = [\"sphinx.ext.autosectionlabel\"]\n";
    dir.write("conf.toml", conf);
    let r = build(&dir.path, 0, &["html"]);
    let index = dir.read("_build/html/index.html");
    has(&index, "href=\"guide.html#install\"");
    has(&index, "href=\"guide.html#it-s-here\"");
    has(&r.err, "The section label examples is also defined in");
    has(&r.err, "missing-label");
    let suppress = "suppress_warnings = [\"autosectionlabel.*\", \"ref.ref\"]\n";
    dir.write("conf.toml", &format!("{conf}{suppress}"));
    build(&dir.path, 0, &["html", "-a", "-W"]);
    dir.write(
        "index.rst",
        "Demo\n====\n\n.. toctree::\n\n   guide\n\nSee :ref:`guide:Install` and \
         :ref:`guide:Examples`.\n",
    );
    let prefixed = "autosectionlabel_prefix_document = true\nautosectionlabel_maxdepth = 2\n";
    dir.write("conf.toml", &format!("{conf}{prefixed}"));
    let r = build(&dir.path, 0, &["html"]);
    has(&dir.read("_build/html/index.html"), "href=\"guide.html#install\"");
    has(&r.err, "guide:examples");
}

/// "name.*" picks the builder's preferred file, and only a titled label is a duplicate.
#[test]
fn wildcard_images_and_anonymous_labels_follow_sphinx() {
    let dir = project("wildcards");
    dir.write("img/flow.pdf", "%PDF-1.4\n");
    dir.write("img/flow.svg", "<svg xmlns=\"http://www.w3.org/2000/svg\"/>\n");
    dir.write(
        "index.rst",
        "Demo\n====\n\n.. toctree::\n\n   other\n\n.. image:: img/flow.*\n\n.. _models:\n\n\
         .. code-block:: python\n\n   pass\n",
    );
    dir.write(
        "other.rst",
        "Other\n=====\n\n.. _models:\n\n.. code-block:: python\n\n   pass\n\n\
         See :ref:`the models <models>`.\n",
    );
    build(&dir.path, 0, &["html", "-W"]);
    has(&dir.read("_build/html/index.html"), "src=\"_images/img/flow.svg\"");
    assert!(dir.exists("_build/html/_images/img/flow.svg"));
    dir.write("other.rst", "Other\n=====\n\n.. _models:\n\nModels\n------\n\nText.\n");
    dir.write(
        "index.rst",
        "Demo\n====\n\n.. toctree::\n\n   other\n\n.. _models:\n\nMore models\n-----------\n\n\
         Text.\n",
    );
    let r = build(&dir.path, 0, &["html"]);
    has(&r.err, "The label models is also defined in");
}

/// Math is written and loaded as sphinx.ext.mathjax does it; :eq: finds equation
/// labels, which :ref: does not.
#[test]
fn math_follows_sphinx_mathjax() {
    let dir = project("mathjax");
    dir.write(
        "index.rst",
        "Demo\n====\n\n.. toctree::\n\n   other\n\nInline :math:`a^2`.\n\n.. math::\n   \
         :label: pyth\n\n   c^2 = a^2 + b^2\n\nSee :eq:`pyth` and :ref:`pyth`.\n",
    );
    dir.write("other.rst", "Other\n=====\n\nNo math.\n");
    dir.write(
        "conf.toml",
        "project = \"Demo\"\nmathjax_options = {async = \"async\"}\n\n[mathjax3_config]\n\
         tex = {tags = \"ams\"}\n",
    );
    let r = build(&dir.path, 0, &["html"]);
    let page = dir.read("_build/html/index.html");
    has(&page, "<span class=\"math notranslate nohighlight\">\\(a^2\\)</span>");
    has(&page, "See <a href=\"#equation-pyth\">(1)</a>");
    has(&r.err, "The :ref: reference to `pyth` has no target in this project.");
    has(&page, "<script>window.MathJax = {\"tex\": {\"tags\": \"ams\"}}</script>");
    has(
        &page,
        "<script async=\"async\" src=\"https://cdn.jsdelivr.net/npm/mathjax@4/tex-mml-chtml.js\">\
         </script>",
    );
    lacks(&dir.read("_build/html/other.html"), "mathjax");
}

/// A module declared first thing in a section gives the section its id, as Sphinx's
/// MoveModuleTargets does; the title's id stays as a second one.
#[test]
fn module_targets_name_their_section() {
    let dir = project("module-targets");
    dir.write(
        "index.rst",
        "Demo\n====\n\nText.\n\nTools\n-----\n\n.. py:module:: tools\n\nSee :py:mod:`tools`.\n",
    );
    build(&dir.path, 0, &["html", "-W"]);
    let page = dir.read("_build/html/index.html");
    has(&page, "<section id=\"module-tools\"><span id=\"tools\"></span>");
    has(&page, "href=\"#module-tools\" title=\"Link to this heading\"");
    has(&page, "<a href=\"#module-tools\"><code>tools</code></a>");
}
