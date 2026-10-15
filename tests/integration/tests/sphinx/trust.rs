//! What a build may read: only the source folder and the include roots, however a file
//! is reached (document, include, image, template, static file, book, graph); an
//! `--untrusted` build cannot widen that, and runs none of the project's Typst.

use guidedog_tests::{Scratch, guidedog, has, lacks};

use crate::build;

/// A 1x1 PNG image.
const PIXEL_PNG: [u8; 70] = [
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 218, 99, 100, 96, 248, 95, 15, 0, 2,
    135, 1, 128, 235, 71, 186, 146, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

/// A scratch folder holding a quickstart project in `rel`.
fn root_with_project(name: &str, rel: &str) -> Scratch {
    let root = Scratch::new(name);
    std::fs::create_dir_all(root.join(rel)).unwrap();
    guidedog(&root.join(rel), 0, &["quickstart", "-q", "-p", "Demo", "-a", "Ada"]);
    root
}

/// A link that leads out of the project is refused as a path that leads out is, until
/// conf.toml names the folder it leads to as an include root.
#[cfg(unix)]
#[test]
fn links_do_not_lead_builds_out_of_the_project() {
    use std::os::unix::fs::symlink;
    let root = root_with_project("trust-links", "docs");
    let dir = root.join("docs");
    root.write("private/secret.rst", "Secret\n======\n\nPrivate text.\n");
    root.write("private/logo.svg", "<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
    root.write("docs/index.rst", "Demo\n====\n\n.. image:: logo.svg\n");
    symlink(root.join("private/secret.rst"), dir.join("secret.rst")).unwrap();
    symlink(root.join("private/logo.svg"), dir.join("logo.svg")).unwrap();
    let r = build(&dir, 1, &["html"]);
    has(&r.err, "DOCUMENT OUTSIDE THE PROJECT");
    root.write("docs/conf.toml", "project = \"Demo\"\ninclude_roots = [\"../private\"]\n");
    build(&dir, 0, &["html"]);
    has(&root.read("docs/_build/html/secret.html"), "Private text.");
    assert!(root.exists("docs/_build/html/_images/logo.svg"));
    // Without it, the image is not read either.
    std::fs::remove_file(dir.join("secret.rst")).unwrap();
    root.write("docs/conf.toml", "project = \"Demo\"\n");
    let r = build(&dir, 0, &["html"]);
    assert!(!root.exists("docs/_build/html/_images/logo.svg"));
    has(&r.err, "logo.svg");
}

/// A project cannot widen what an --untrusted build reads: include_roots and every other
/// setting naming a file or folder outside the source folder are left out with a warning.
#[test]
fn untrusted_builds_read_only_the_source_folder() {
    let root = root_with_project("trust-untrusted", "project/docs");
    let dir = root.join("project/docs");
    root.write("secret.rst", "Secret text.\n");
    root.write(
        "project/docs/conf.toml",
        "project = \"Demo\"\ninclude_roots = [\"../..\"]\n\
        html_static_path = [\"../..\"]\n",
    );
    root.write("project/docs/index.rst", "Demo\n====\n\n.. include:: ../../secret.rst\n");
    build(&dir, 0, &["html"]);
    let page = "project/docs/_build/html/index.html";
    has(&root.read(page), "Secret text.");
    let r = build(&dir, 0, &["html", "-a", "--untrusted"]);
    for part in ["SETTING NOT USED", "include_roots", "html_static_path", "core.include.refused"] {
        has(&r.err, part);
    }
    lacks(&root.read(page), "Secret text.");
    assert!(!root.exists("project/docs/_build/html/_static/secret.rst"));
}

/// Typst reads only the folders the build may read: a book template that reads a file
/// beside the project fails, and the failed book is kept in the cache, not published.
/// An --untrusted build leaves the project's template and preamble out.
#[test]
fn books_read_only_what_the_build_may_read() {
    let root = root_with_project("trust-books", "docs");
    let dir = root.join("docs");
    std::fs::create_dir_all(root.join("shared")).unwrap();
    root.write("private/secret.txt", "Secret\n");
    root.write(
        "docs/conf.toml",
        "project = \"Demo\"\ninclude_roots = [\"../shared\"]\n\
        pdf_preamble = \"pre.typ\"\n",
    );
    root.write("docs/pre.typ", "#let also = read(\"/private/secret.txt\")\n");
    let template = format!(
        "#let secret = read(\"/private/secret.txt\")\n{}",
        root.read("docs/_templates/book.typ")
    );
    root.write("docs/_templates/book.typ", &template);
    let r = build(&dir, 4, &["pdf"]);
    // Typst refuses the read: the file lies outside every folder the build may read.
    has(&r.err, "failed to load file (access denied)");
    assert!(!root.exists("docs/_build/pdf/demo-en.pdf"));
    assert!(!root.exists("docs/_build/pdf/book-en.typ"));
    let kept = "docs/_build/.doctrees/failed/book-en.typ";
    assert!(root.exists(kept));
    has(&r.err, &root.join(kept).to_string_lossy());
    let r = build(&dir, 0, &["pdf", "--untrusted"]);
    has(&r.err, "BOOK TEMPLATE NOT USED");
    has(&r.err, "pdf_preamble");
    assert!(root.exists("docs/_build/pdf/demo-en.pdf"));
    assert!(!root.exists(kept));
}

/// A graph reads the images it names as a document does, confined and relative to its
/// document, and embeds them, so no path reaches the output; --untrusted reads none.
#[test]
fn graphs_read_only_confined_files() {
    let root = root_with_project("trust-graphs", "docs");
    let dir = root.join("docs");
    root.write("docs/conf.toml", "project = \"Demo\"\nextensions = [\"sphinx.ext.graphviz\"]\n");
    std::fs::create_dir_all(root.join("outside")).unwrap();
    std::fs::write(root.join("outside/real.png"), PIXEL_PNG).unwrap();
    std::fs::write(root.join("docs/inside.png"), PIXEL_PNG).unwrap();
    let real = root.join("outside/real.png");
    root.write(
        "docs/index.rst",
        &format!(
            "Demo\n====\n\n.. graphviz::\n\n   \
        digraph {{ a [image=\"{}\", label=\"\"] }}\n\n.. graphviz::\n\n   \
        digraph {{ b [label=<<TABLE><TR><TD><IMG SRC=\"inside.png\"/></TD></TR></TABLE>>] }}\n",
            real.display()
        ),
    );
    let r = build(&dir, 0, &["html"]);
    has(&r.err, "GRAPH NOT DRAWN");
    has(&r.err, "real.png");
    let images: Vec<_> =
        std::fs::read_dir(root.join("docs/_build/html/_images")).unwrap().flatten().collect();
    assert_eq!(images.len(), 1);
    let svg = std::fs::read_to_string(images[0].path()).unwrap();
    has(&svg, "xlink:href=\"data:image/png;base64,");
    lacks(&svg, &root.path.to_string_lossy());
    let r = build(&dir, 0, &["html", "-a", "--untrusted"]);
    has(&r.err, "--untrusted build reads no file a graph names");
    assert!(!root.exists("docs/_build/html/_images"));
}
