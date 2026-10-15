//! The autodoc family of Sphinx's built-in extensions end to end, through the built
//! `guidedog` program: sphinx.ext.napoleon, autosummary, apidoc and coverage over a
//! small Python package. Nothing runs the package: the source scan reads it.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

use std::path::{Path, PathBuf};

use guidedog_tests::{Run, Scratch, guidedog, has, lacks};

const SHAPES_INIT: &str = r#""""Shapes and their areas."""

from .square import Square

#: The number of corners of a square.
CORNERS = 4


def area(width, height):
    """Compute an area.

    Args:
        width (float): The width.
        height (float): The height.

    Returns:
        float: The product.
    """
    return width * height


def perimeter(width, height):
    return 2 * (width + height)
"#;

const SHAPES_SQUARE: &str = r#""""Squares."""


class Square:
    """A square. It has four sides.

    Attributes:
        side (float): The length of a side.
    """

    def __init__(self, side):
        self.side = side

    def area(self):
        """The area of the square.

        Returns
        -------
        float
            The side squared.
        """
        return self.side ** 2

    def scale(self, factor):
        return Square(self.side * factor)
"#;

/// A project in `docs/` reading `src/shapes`, with `conf` as its conf.toml and `index`
/// as its root document.
fn project(name: &str, conf: &str, index: &str) -> (Scratch, PathBuf) {
    let root = Scratch::new(name);
    guidedog(&root.path, 0, &["quickstart", "docs", "-q", "-p", "Shapes", "-a", "Ada"]);
    root.write("docs/conf.toml", &format!("project = \"Shapes\"\n{conf}"));
    root.write("docs/index.rst", index);
    root.write("src/shapes/__init__.py", SHAPES_INIT);
    root.write("src/shapes/square.py", SHAPES_SQUARE);
    let docs = root.join("docs");
    (root, docs)
}

/// `guidedog build <args> --color=never` in `dir`, expecting `status`.
fn build(dir: &Path, status: i32, args: &[&str]) -> Run {
    let mut all = vec!["build"];
    all.extend_from_slice(args);
    all.push("--color=never");
    guidedog(dir, status, &all)
}

#[test]
fn napoleon_turns_sections_into_fields() {
    let conf = "extensions = [\"sphinx.ext.napoleon\"]\nautodoc_source_paths = [\"../src\"]\n";
    let index = "Shapes\n======\n\n.. autofunction:: shapes.area\n\n\
                 .. autoclass:: shapes.Square\n   :members:\n";
    let (root, docs) = project("napoleon", conf, index);
    build(&docs, 0, &["html", "-W"]);
    let page = root.read("docs/_build/html/index.html");
    has(&page, "Parameters");
    has(&page, "The width.");
    has(&page, "Return type");
    has(&page, "The side squared.");
    lacks(&page, "Args:");
    lacks(&page, "-------");
    build(&docs, 0, &["text", "-W"]);
    let text = root.read("docs/_build/text/index.txt");
    has(&text, "* **width** (*float*) – The width.");
    has(&text, "Return type:\n      float");
    has(&text, "The length of a side.\n\n      Type:\n         float");
}

#[test]
fn napoleon_reports_a_section_it_cannot_read() {
    let conf = "extensions = [\"sphinx.ext.napoleon\"]\nautodoc_source_paths = [\"../src\"]\n";
    let (root, docs) =
        project("napoleon-section", conf, "Shapes\n======\n\n.. automodule:: shapes.bad\n");
    root.write("src/shapes/bad.py", "\"\"\"Bad.\n\nReturns:\nthe value\n\"\"\"\n");
    let r = build(&docs, 0, &["text"]);
    has(&r.err, "napoleon.section");
    has(&root.read("docs/_build/text/index.txt"), "Returns: the value");
}

const SUMMARY: &str =
    "Shapes\n======\n\n.. autosummary::\n   :toctree: generated\n   :recursive:\n\n   shapes\n";

#[test]
fn autosummary_writes_tables_and_stub_pages() {
    let conf = "extensions = [\"sphinx.ext.autosummary\", \"sphinx.ext.napoleon\"]\n\
                autodoc_source_paths = [\"../src\"]\n";
    let (root, docs) = project("autosummary", conf, SUMMARY);
    build(&docs, 0, &["html", "-W"]);
    let page = root.read("docs/_build/html/index.html");
    has(&page, "<table class=\"autosummary longtable");
    has(&page, "href=\"generated/shapes.html");
    has(&page, "Shapes and their areas.");
    let stub = root.read("docs/_build/html/generated/shapes.html");
    has(&stub, "Module Attributes");
    has(&stub, "Compute an area.");
    has(&stub, "perimeter");
    assert!(root.exists("docs/_build/html/generated/shapes.square.html"));
    // The stub pages are never written to the source folder.
    assert!(!root.exists("docs/generated"));
    build(&docs, 0, &["text", "-W"]);
    has(&root.read("docs/_build/text/generated/shapes.square.txt"), "A square.");
}

#[test]
fn autosummary_stub_pages_follow_the_project_templates_and_the_book() {
    let conf = "extensions = [\"sphinx.ext.autosummary\"]\nautodoc_source_paths = [\"../src\"]\n\
                templates_path = [\"_templates\"]\n";
    let (root, docs) = project("autosummary-template", conf, SUMMARY);
    root.write(
        "docs/_templates/autosummary/module.rst",
        "{{ fullname | escape | underline }}\n\nCustom page of {{ name }}.\n",
    );
    build(&docs, 0, &["text"]);
    has(&root.read("docs/_build/text/generated/shapes.txt"), "Custom page of shapes.");
    build(&docs, 0, &["pdf"]);
    let pdf = std::fs::read_dir(root.join("docs/_build/pdf")).unwrap();
    assert!(pdf.flatten().any(|e| e.path().extension().is_some_and(|x| x == "pdf")));
}

#[test]
fn autosummary_reports_a_name_it_cannot_find() {
    let conf = "extensions = [\"sphinx.ext.autosummary\"]\nautodoc_source_paths = [\"../src\"]\n";
    let index = "Shapes\n======\n\n.. autosummary::\n\n   shapes.nothing\n";
    let (_root, docs) = project("autosummary-missing", conf, index);
    let r = build(&docs, 0, &["text"]);
    has(&r.err, "autosummary.import");
}

#[test]
fn apidoc_generates_package_pages() {
    let conf = "extensions = [\"sphinx.ext.apidoc\"]\n\n[[apidoc_modules]]\n\
                path = \"../src/shapes\"\ndestination = \"api\"\nseparate_modules = true\n";
    let index = "Shapes\n======\n\n.. toctree::\n\n   api/modules\n";
    let (root, docs) = project("apidoc", conf, index);
    build(&docs, 0, &["html", "-W"]);
    let page = root.read("docs/_build/html/api/shapes.html");
    has(&page, "shapes package");
    has(&page, "Compute an area.");
    has(&root.read("docs/_build/html/api/shapes.square.html"), "A square.");
    assert!(!root.exists("docs/api"));
    build(&docs, 0, &["text", "-W"]);
    has(&root.read("docs/_build/text/api/shapes.square.txt"), "shapes.square module");
}

#[test]
fn coverage_lists_what_the_documents_do_not_describe() {
    let conf = "extensions = [\"sphinx.ext.autodoc\", \"sphinx.ext.coverage\"]\n\
                autodoc_source_paths = [\"../src\"]\ncoverage_modules = [\"shapes\"]\n";
    let index = "Shapes\n======\n\n.. automodule:: shapes\n\n.. autofunction:: shapes.area\n\n\
                 .. autoclass:: shapes.square.Square\n\n   .. automethod:: area\n";
    let (root, docs) = project("coverage", conf, index);
    let r = build(&docs, 0, &["coverage"]);
    has(&r.out, "| TOTAL");
    let report = root.read("docs/_build/coverage/python.txt");
    has(&report, "Undocumented Python objects\n===========================\n");
    has(&report, "shapes\n------\n\nFunctions:\n * perimeter\n");
    has(&report, " * Square -- missing methods:\n\n   - scale\n");
    assert!(root.exists("docs/_build/coverage/c.txt"));
}

#[test]
fn autolink_refers_to_python_objects() {
    let conf = "extensions = [\"sphinx.ext.autosummary\"]\nautodoc_source_paths = [\"../src\"]\n";
    let index =
        "Shapes\n======\n\nSee :autolink:`shapes.area`.\n\n.. autofunction:: shapes.area\n";
    let (root, docs) = project("autolink", conf, index);
    build(&docs, 0, &["html", "-W"]);
    has(&root.read("docs/_build/html/index.html"), "href=\"#shapes.area\"");
}
