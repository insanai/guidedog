//! Source documented from code in a project: `sphinx.ext.autodoc` over Python and the
//! Odin domain's API pages. The source is data: nothing runs or compiles it. The
//! extraction itself is covered by gd-autodoc and gd-odindoc; these check the build:
//! pages, references, dependencies on the source files, and the problems' wording.

use guidedog_tests::{Scratch, guidedog, has, lacks};

use crate::build;

const PACKAGE_INIT: &str = r#""""The package."""

from .core import Thing

__all__ = ["Thing", "helper"]


def helper(x, y=2):
    """Helps with x."""
"#;

const PACKAGE_CORE: &str = r#"class Thing:
    """A thing."""

    #: How many there are.
    count = 3

    def run(self, fast: bool = False) -> None:
        """Runs."""
"#;

/// A project in `docs/` documenting the package in `src/pkg`.
fn autodoc_project(name: &str) -> (Scratch, std::path::PathBuf) {
    let root = Scratch::new(name);
    let docs = project_in(
        &root,
        "project = \"Demo\"\nextensions = [\"sphinx.ext.autodoc\"]\n\
        autodoc_source_paths = [\"../src\"]\nautodoc_member_order = \"bysource\"\n",
    );
    root.write(
        "docs/index.rst",
        "Demo\n====\n\n.. automodule:: pkg\n   :members:\n\n\
         See :py:class:`pkg.Thing`, :py:meth:`pkg.Thing.run`, and :py:attr:`.count`.\n",
    );
    root.write("src/pkg/__init__.py", PACKAGE_INIT);
    root.write("src/pkg/core.py", PACKAGE_CORE);
    (root, docs)
}

/// A quickstart project in `root/docs` with `conf` as its configuration.
fn project_in(root: &Scratch, conf: &str) -> std::path::PathBuf {
    guidedog(&root.path, 0, &["quickstart", "docs", "-q", "-p", "Demo", "-a", "Ada"]);
    root.write("docs/conf.toml", conf);
    root.join("docs")
}

#[test]
fn autodoc_documents_python_source() {
    let (root, docs) = autodoc_project("autodoc-pages");
    build(&docs, 0, &["html", "-W"]);
    let page = root.read("docs/_build/html/index.html");
    for id in ["module-pkg", "pkg.Thing", "pkg.Thing.count", "pkg.Thing.run", "pkg.helper"] {
        has(&page, &format!("id=\"{id}\""));
    }
    for text in ["A thing.", "Helps with x.", "How many there are."] {
        has(&page, text);
    }
    for target in ["#pkg.Thing\"", "#pkg.Thing.run\"", "#pkg.Thing.count\""] {
        has(&page, &format!("href=\"{target}"));
    }
    assert!(!std::fs::read(root.join("docs/_build/html/objects.inv")).unwrap().is_empty());
}

#[test]
fn autodoc_sources_are_dependencies() {
    let (root, docs) = autodoc_project("autodoc-deps");
    build(&docs, 0, &["html"]);
    let r = build(&docs, 0, &["html"]);
    has(&r.out, "0 document(s) parsed, 1 unchanged");
    root.write("src/pkg/core.py", &PACKAGE_CORE.replace("\"\"\"Runs.", "\"\"\"Runs quickly."));
    let r = build(&docs, 0, &["html"]);
    has(&r.out, "1 document(s) parsed, 0 unchanged");
    has(&root.read("docs/_build/html/index.html"), "Runs quickly.");
}

#[test]
fn autodoc_reports_what_it_cannot_find() {
    let (root, docs) = autodoc_project("autodoc-missing");
    root.write("docs/index.rst", "Demo\n====\n\n.. autoclass:: pkg.Missing\n");
    let r = build(&docs, 1, &["html", "-W"]);
    let all = format!("{}{}", r.out, r.err);
    has(&all, "PYTHON OBJECT NOT FOUND");
    has(&all, "Missing");
    // Without the extension the directive is unknown, as in Sphinx.
    root.write("docs/conf.toml", "project = \"Demo\"\n");
    let r = build(&docs, 1, &["html", "-W"]);
    has(&format!("{}{}", r.out, r.err), "UNKNOWN DIRECTIVE");
}

const SHAPES: &str = r#"// Package shapes measures plane figures.
package shapes

import "core:math"

// Kind names a figure.
Kind :: enum u8 {
    Circle, // round.
    Square,
}

// Shape is a figure and its size.
Shape :: struct {
    kind: Kind,
    size: f32, // the radius or the side.
}

/*
Measures a shape.

Inputs:
- s: The shape

Returns:
- a: Its area
- ok: Whether the kind is known
*/
@(require_results)
area :: proc(s: Shape) -> (a: f32, ok: bool) {
    switch s.kind {
    case .Circle: return math.PI * s.size * s.size, true
    case .Square: return s.size * s.size, true
    }
    return 0, false
}

@(private)
helper :: proc() {}
"#;

const SHAPES_WINDOWS: &str = "package shapes\n\n// Draws on a Windows window.\n\
    draw :: proc(s: Shape) {}\n";

/// A project in `docs/` whose API pages document the Odin package in `src/shapes`.
fn odin_project(name: &str) -> (Scratch, std::path::PathBuf) {
    let root = Scratch::new(name);
    let docs = project_in(
        &root,
        "project = \"Demo\"\nodin_autoapi_dirs = [\"../src\"]\n\
        nitpick_ignore_regex = [[\"odin:type\", \"core:.*\"]]\n",
    );
    root.write(
        "docs/index.rst",
        "Demo\n====\n\n.. toctree::\n\nSee :odin:proc:`shapes.area`, \
         :odin:type:`~shapes.Shape`, :odin:field:`shapes.Shape.size`, and \
         :odin:pkg:`shapes`.\n",
    );
    root.write("src/shapes/shapes.odin", SHAPES);
    root.write("src/shapes/shapes_windows.odin", SHAPES_WINDOWS);
    (root, docs)
}

#[test]
fn odin_api_pages_document_packages() {
    let (root, docs) = odin_project("odin-pages");
    build(&docs, 0, &["html", "-W", "-n"]);
    let page = root.read("docs/_build/html/api/shapes.html");
    for id in [
        "package-shapes",
        "shapes.Kind",
        "shapes.Kind.Circle",
        "shapes.Shape.size",
        "shapes.area",
        "shapes.draw",
    ] {
        has(&page, &format!("id=\"odin-{id}\""));
    }
    for text in [
        "&#64;(require_results) ",
        "Measures a shape.",
        "the radius or the side.",
        "Availability",
        "Windows",
        "href=\"#odin-shapes.Shape\"",
    ] {
        has(&page, text);
    }
    lacks(&page, "odin-shapes.helper");
    let index = root.read("docs/_build/html/api/index.html");
    has(&index, "href=\"shapes.html#odin-package-shapes\"");
    has(&index, "measures plane figures");
    let root_page = root.read("docs/_build/html/index.html");
    has(&root_page, "href=\"api/index.html\"");
    has(&root_page, "href=\"api/shapes.html#odin-shapes.area\"");
    has(&root_page, "href=\"api/shapes.html#odin-shapes.Shape.size\"");
    has(
        &root.read("docs/_build/html/_sources/api/shapes.rst.txt"),
        ".. odin:autopackage:: shapes",
    );
    assert!(!root.exists("docs/api"), "a generated page was written into the project");
    build(&docs, 0, &["singlehtml", "-W", "-n"]);
    has(&root.read("docs/_build/singlehtml/index.html"), "Measures a shape.");
}

#[test]
fn odin_sources_are_dependencies() {
    let (root, docs) = odin_project("odin-deps");
    build(&docs, 0, &["html"]);
    let r = build(&docs, 0, &["html"]);
    has(&r.out, "0 document(s) parsed, 3 unchanged");
    root.write("src/shapes/shapes.odin", &SHAPES.replace("a shape.", "a shape quickly."));
    let r = build(&docs, 0, &["html"]);
    has(&r.out, "1 document(s) parsed, 2 unchanged"); // the package's page alone.
    has(&root.read("docs/_build/html/api/shapes.html"), "Measures a shape quickly.");
}

#[test]
fn odin_directives_report_what_they_cannot_find() {
    let (root, docs) = odin_project("odin-missing");
    root.write("docs/index.rst", "Demo\n====\n\n.. odin:autopackage:: missing\n");
    let r = build(&docs, 1, &["html", "-W"]);
    has(&format!("{}{}", r.out, r.err), "ODIN PACKAGE NOT FOUND");
    // Without source folders, the directives say where to set them.
    root.write("docs/conf.toml", "project = \"Demo\"\n");
    let r = build(&docs, 1, &["html", "-W"]);
    has(&format!("{}{}", r.out, r.err), "NO ODIN SOURCE FOLDERS");
    root.write("docs/conf.toml", "project = \"Demo\"\nodin_autoapi_member_order = \"random\"\n");
    let r = build(&docs, 2, &["html"]);
    has(&format!("{}{}", r.out, r.err), "odin_autoapi_member_order");
}
