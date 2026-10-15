//! The manual's tutorials build as written: the reStructuredText examples of "Your first
//! project" and "A small book" through every builder, and the theme tutorial's files
//! (which the manual also offers for download) across the HTML address schemes and PDF.

use guidedog_tests::{Scratch, has, lacks, repository_root, repository_text, rst_code_blocks};

use crate::support::{build, cli};

/// The tutorials' RST examples produce the promised mathematics, graph,
/// cross-references, numbering, HTML and PDF, and every documented builder runs.
#[test]
fn authored_manual_tutorials_build_as_written() {
    let dir = Scratch::new("tutorial");
    cli(&dir.path, 0, &["quickstart", "notes", "-q", "-p", "Field Notes", "-a", "Your name"]);
    let notes = dir.join("notes");
    let first = rst_code_blocks(&repository_text("docs/manual/tutorial/first-project.rst"), "rst");
    assert_eq!(first.len(), 1, "first-project.rst has one rst example");
    dir.write("notes/index.rst", &first[0]);
    for builder in ["html", "pdf"] {
        build(&notes, 0, &[builder, "-n", "-W"]);
    }
    has(&dir.read("notes/_build/html/index.html"), "math notranslate");
    lacks(&dir.read("notes/_build/pdf/book-en.typ"), "#set heading(outlined: false)");
    let book = rst_code_blocks(&repository_text("docs/manual/tutorial/a-small-book.rst"), "rst");
    assert_eq!(book.len(), 3, "a-small-book.rst has three rst examples");
    dir.write("notes/measurement.rst", &format!("{}\n{}", book[0], book[2]));
    dir.write("notes/index.rst", &book[1]);
    let conf = dir.read("notes/conf.toml");
    dir.write("notes/conf.toml", &format!("{conf}\nnumfig = true\n"));
    for builder in ["html", "dirhtml", "singlehtml", "pdf", "text", "gettext", "dummy"] {
        build(&notes, 0, &[builder, "-n", "-W"]);
    }
    let html = dir.read("notes/_build/html/measurement.html");
    has(&html, "graphviz");
    has(&html, "caption-number");
    has(&dir.read("notes/_build/html/index.html"), "measurement.html#measurement-units");
    let source = dir.read("notes/_build/pdf/book-en.typ");
    has(&source, "#figure(");
    has(&source, "image(\"");
    has(&source, "measurement-units");
    assert!(dir.exists("notes/_build/pdf/field-notes-en.pdf"));
}

/// The theme tutorial's files, copied into a fresh quickstart, build with every HTML
/// address scheme and PDF; the downloadable copy is the file exercised.
#[test]
fn theme_tutorial_builds_as_written() {
    let dir = Scratch::new("theme-tutorial");
    let args =
        ["quickstart", "orbit", "-q", "-p", "Orbit Field Notes", "-a", "Orbit contributors"];
    cli(&dir.path, 0, &[&args[..], &["-v", "1.0"]].concat());
    let project = dir.join("orbit");
    let base = dir.read("orbit/_templates/book.typ");
    dir.write("orbit/_templates/base-book.typ", &base);
    let root = repository_root();
    for path in [
        "conf.toml",
        "index.rst",
        "chapters/reading.rst",
        "_templates/layout.html",
        "_templates/macros/ui.html",
        "_templates/partials/edition.html",
        "_templates/book.typ",
        "_templates/palette.typ",
        "_static/theme.css",
        "README.md",
    ] {
        let example = guidedog_tests::read(&root.join("docs/manual/template/theme").join(path));
        let copy = root.join("docs/manual/_static/repository/theme").join(path);
        assert_eq!(guidedog_tests::read(&copy), example, "{path}: the download differs");
        dir.write(&format!("orbit/{path}"), &example);
    }
    for builder in ["html", "dirhtml", "singlehtml", "pdf"] {
        build(&project, 0, &[builder, "-n", "-W"]);
    }
    let chapter = dir.read("orbit/_build/html/chapters/reading.html");
    has(&chapter, "Field edition");
    has(&chapter, "../_static/theme.css");
    has(&chapter, "href=\"../chapters/reading.html\"");
    has(&chapter, "graphviz-adaptive");
    has(&dir.read("orbit/_build/dirhtml/chapters/reading/index.html"), "../../_static/theme.css");
    has(&dir.read("orbit/_build/singlehtml/index.html"), "#document-chapters-reading");
    has(&dir.read("orbit/_build/html/search.html"), "orbit-edition");
    assert!(dir.exists("orbit/_build/pdf/orbit-notes-en.pdf"));
    has(&dir.read("orbit/_build/pdf/book-en.typ"), "_templates/book.typ");
    check_graph_cache(&dir);
    dir.write(
        "orbit/_templates/partials/edition.html",
        "<p class=\"orbit-edition\">Revised edition</p>\n",
    );
    build(&project, 0, &["html", "-W"]);
    has(&dir.read("orbit/_build/html/chapters/reading.html"), "Revised edition");
    has(&dir.read("orbit/_build/html/search.html"), "Revised edition");
}

/// HTML and PDF do not share styled SVG cache entries: a PDF image is light.
fn check_graph_cache(dir: &Scratch) {
    let (mut styled, mut plain) = (0, 0);
    for entry in std::fs::read_dir(dir.join("orbit/_build/.doctrees/graphs")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "svg") {
            continue;
        }
        let svg = guidedog_tests::read(&path);
        if svg.contains("@media screen and (prefers-color-scheme: dark)") {
            styled += 1;
        } else {
            plain += 1;
        }
        has(&svg, "gd&#45;input");
        lacks(&svg, "<script");
    }
    assert_eq!((styled, plain), (1, 1), "one styled (HTML) and one plain (PDF) graph");
}
