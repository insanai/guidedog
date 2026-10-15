//! Source pages, source links and class diagrams end to end, through the built
//! `guidedog` program: sphinx.ext.viewcode, linkcode, inheritance_diagram, duration and
//! imgconverter over a small Python package. Nothing runs the package: the source scan
//! reads it.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

use std::path::{Path, PathBuf};

use guidedog_tests::{Run, Scratch, guidedog, has, lacks};

const SHAPES_INIT: &str = "\"\"\"Shapes.\"\"\"\n\nfrom .base import Shape\n";

const SHAPES_BASE: &str = r#""""Base shapes."""


class Shape:
    """A shape."""

    def area(self):
        """The area."""
        return 0


class Named:
    """Something with a name."""
"#;

const SHAPES_SQUARE: &str = r#""""Squares."""
import functools

from .base import Named, Shape


class Square(Shape, Named):
    """A square."""

    @functools.cache
    def area(self):
        """The area of the square."""
        return 1


class Cube(Square):
    """A cube."""
"#;

/// A project in `docs/` documenting `src/shapes` with `extensions` listed and `more`
/// settings, and `index` as its root document.
fn shapes_project(name: &str, extensions: &[&str], more: &str, index: &str) -> (Scratch, PathBuf) {
    let root = Scratch::new(name);
    guidedog(&root.path, 0, &["quickstart", "docs", "-q", "-p", "Shapes", "-a", "Ada"]);
    let listed: Vec<String> = extensions.iter().map(|e| format!("\"sphinx.ext.{e}\"")).collect();
    let conf = format!(
        "project = \"Shapes\"\nextensions = [\"sphinx.ext.autodoc\", {}]\n\
         autodoc_source_paths = [\"../src\"]\n{more}",
        listed.join(", ")
    );
    root.write("docs/conf.toml", &conf);
    root.write("docs/index.rst", index);
    root.write("src/shapes/__init__.py", SHAPES_INIT);
    root.write("src/shapes/base.py", SHAPES_BASE);
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

const API: &str = "Shapes\n======\n\n.. automodule:: shapes.square\n   :members:\n\n\
                   .. autoclass:: shapes.Shape\n   :members:\n";

#[test]
fn viewcode_writes_source_pages_linked_both_ways() {
    let (root, docs) = shapes_project("viewcode-pages", &["viewcode"], "", API);
    build(&docs, 0, &["html", "-W"]);
    let page = root.read("docs/_build/html/index.html");
    has(&page, "href=\"_modules/shapes/square.html#Square\" class=\"viewcode-link\">[source]");
    has(&page, "href=\"_modules/shapes/square.html#Square.area\"");
    // A re-export links to the module that defines the class.
    has(&page, "href=\"_modules/shapes/base.html#Shape\"");
    let source = root.read("docs/_build/html/_modules/shapes/square.html");
    has(&source, "<h1>Source code for shapes.square</h1>");
    has(
        &source,
        "<div class=\"viewcode-block\" id=\"Square\">\n\
         <a class=\"viewcode-back\" href=\"../../index.html#shapes.square.Square\">[docs]</a>",
    );
    // The block of a decorated method starts at its decorator.
    has(&source, "[docs]</a>\n    <span class=\"nd\">@functools.cache</span>");
    lacks(&source, "class=\"linenos\"");
    let index = root.read("docs/_build/html/_modules/index.html");
    has(&index, "<a href=\"shapes/base.html\">shapes.base</a>");
    has(&index, "<a href=\"shapes/square.html\">shapes.square</a>");
}

#[test]
fn viewcode_numbers_lines_and_skips_singlehtml_and_the_book() {
    let more = "viewcode_line_numbers = true\n";
    let (root, docs) = shapes_project("viewcode-other", &["viewcode"], more, API);
    build(&docs, 0, &["html"]);
    has(
        &root.read("docs/_build/html/_modules/shapes/square.html"),
        "<span class=\"linenos\"> 1</span>",
    );
    build(&docs, 0, &["singlehtml"]);
    assert!(!root.exists("docs/_build/singlehtml/_modules"));
    lacks(&root.read("docs/_build/singlehtml/index.html"), "viewcode-link");
    build(&docs, 0, &["text"]);
    lacks(&root.read("docs/_build/text/index.txt"), "[source]");
}

#[test]
fn viewcode_follows_edits_to_the_source() {
    let (root, docs) = shapes_project("viewcode-edits", &["viewcode"], "", API);
    build(&docs, 0, &["html"]);
    let r = build(&docs, 0, &["html"]);
    has(&r.out, "0 document(s) parsed, 1 unchanged");
    root.write("src/shapes/square.py", &SHAPES_SQUARE.replace("return 1", "return 2"));
    let r = build(&docs, 0, &["html"]);
    has(&r.out, "1 document(s) parsed");
    has(&root.read("docs/_build/html/_modules/shapes/square.html"), "<span class=\"mi\">2</span>");
}

#[test]
fn linkcode_fills_the_url_template() {
    let more = "linkcode_url = \"https://example.org/{branch}/{path}#L{line}-L{end_line}\"\n\
                linkcode_branch = \"main\"\nlinkcode_root = \"..\"\n";
    let index = format!("{API}\n.. py:function:: shapes.missing()\n\n   Not in the source.\n");
    let (root, docs) = shapes_project("linkcode", &["linkcode"], more, &index);
    build(&docs, 0, &["html", "-W"]);
    let page = root.read("docs/_build/html/index.html");
    has(&page, "href=\"https://example.org/main/src/shapes/square.py#L7-L13\"");
    has(&page, "href=\"https://example.org/main/src/shapes/square.py#L10-L13\"");
    has(&page, "href=\"https://example.org/main/src/shapes/square.py#L16-L17\"");
    has(&page, "href=\"https://example.org/main/src/shapes/base.py#L4-L9\"");
    // An object the scan does not know has no file, so the template cannot link it.
    assert_eq!(page.matches("class=\"viewcode-link\"").count(), 5);
}

#[test]
fn linkcode_without_a_branch_warns_once() {
    let more = "linkcode_url = \"https://example.org/{branch}/{path}\"\n";
    let (_root, docs) = shapes_project("linkcode-branch", &["linkcode"], more, API);
    let r = build(&docs, 0, &["html"]);
    let all = format!("{}{}", r.out, r.err);
    assert_eq!(all.matches("[linkcode.branch]").count(), 1, "{all}");
}

const DIAGRAM: &str = "Shapes\n======\n\n.. autoclass:: shapes.square.Square\n\n\
                       .. inheritance-diagram:: shapes.square\n   :caption: The squares.\n";

#[test]
fn inheritance_diagrams_are_drawn_and_linked_in_html() {
    let (root, docs) = shapes_project("inheritance-html", &["inheritance_diagram"], "", DIAGRAM);
    build(&docs, 0, &["html", "-W"]);
    let page = root.read("docs/_build/html/index.html");
    has(&page, "<object data=\"_images/graphviz-");
    has(&page, "type=\"image/svg+xml\"");
    has(&page, "The squares.");
    let start = page.find("<object data=\"").unwrap() + "<object data=\"".len();
    let file = &page[start..start + page[start..].find('"').unwrap()];
    let svg = root.read(&format!("docs/_build/html/{file}"));
    // The described class links to its description; the others are drawn plain.
    has(&svg, "xlink:href=\"../index.html#shapes.square.Square\"");
    has(&svg, "target=\"_top\"");
    for name in ["shapes.square.Cube", "shapes.base.Shape", "shapes.base.Named"] {
        has(&svg, name);
    }
    lacks(&svg, "gd-xref:");
}

#[test]
fn inheritance_diagrams_reach_the_book_and_text() {
    let index = format!(
        "{DIAGRAM}\n.. inheritance-diagram:: Cube\n   :parts: 1\n   \
         :top-classes: shapes.square.Square\n"
    );
    let index = index
        .replace("Shapes\n======\n", "Shapes\n======\n\n.. py:currentmodule:: shapes.square\n");
    let (root, docs) = shapes_project("inheritance-book", &["inheritance_diagram"], "", &index);
    build(&docs, 0, &["pdf"]);
    let book = root.read("docs/_build/pdf/book-en.typ");
    has(&book, "graphviz-");
    build(&docs, 0, &["text"]);
    let text = root.read("docs/_build/text/index.txt");
    has(&text, "\"shapes.square.Square\": \"shapes.base.Shape\", \"shapes.base.Named\"");
    // :parts: 1 and :top-classes: stop at Square.
    has(&text, "\"shapes.square.Cube\": \"Square\"");
}

#[test]
fn inheritance_reports_a_name_the_scan_does_not_know() {
    let index = "Shapes\n======\n\n.. inheritance-diagram:: shapes.nothing\n";
    let (_root, docs) = shapes_project("inheritance-missing", &["inheritance_diagram"], "", index);
    let r = build(&docs, 0, &["html"]);
    has(&format!("{}{}", r.out, r.err), "[inheritance.class]");
}

#[test]
fn duration_reports_the_slowest_documents() {
    let more = "duration_n_slowest = 1\n";
    let (root, docs) = shapes_project("duration", &["duration"], more, API);
    root.write("docs/other.rst", ":orphan:\n\nOther\n=====\n");
    let r = build(&docs, 0, &["html"]);
    has(&r.out, "s reading 2 document(s)");
    assert_eq!(r.out.matches("slowest").count(), 1, "{}", r.out);
    let json = root.read("docs/_build/html/sphinx-reading-durations.json");
    has(&json, "\"index\": ");
    has(&json, "\"other\": ");
    let r = build(&docs, 0, &["html"]);
    has(&r.out, "s reading 0 document(s)");
    assert_eq!(root.read("docs/_build/html/sphinx-reading-durations.json"), "{}");
}

#[test]
fn imgconverter_reports_an_image_the_book_cannot_show() {
    let index = "Shapes\n======\n\n.. image:: scan.tiff\n   :alt: A scanned page\n\n\
                 .. image:: logo.svg\n";
    let (root, docs) = shapes_project("imgconverter", &["imgconverter"], "", index);
    root.write("docs/scan.tiff", "II*\0");
    root.write(
        "docs/logo.svg",
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"/>\n",
    );
    let r = build(&docs, 0, &["pdf"]);
    has(&format!("{}{}", r.out, r.err), "[imgconverter.format]");
    let book = root.read("docs/_build/pdf/book-en.typ");
    has(&book, "A scanned page");
    has(&book, "logo.svg");
    let r = build(&docs, 0, &["html", "-W"]);
    lacks(&format!("{}{}", r.out, r.err), "imgconverter");
}
