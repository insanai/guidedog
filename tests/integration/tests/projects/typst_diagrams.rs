//! Real HTML, PDF and incremental builds with embedded Typst drawings, all offline.

use guidedog_tests::{Scratch, has, lacks};

use crate::support::{both, build, project};

fn setup(name: &str) -> Scratch {
    let dir = project(name);
    dir.write(
        "conf.toml",
        &format!(
            "{}\npdf_packages = 'offline'\npdf_fonts = 'embedded'\nnumfig = true\n",
            dir.read("conf.toml")
                .replace("source_suffix = [\".rst\"]", "source_suffix = [\".rst\", \".md\"]"),
        ),
    );
    dir
}

fn diagram_url(html: &str, extension: &str) -> String {
    let start = html.find("_images/typst-").expect("generated image");
    let end = html[start..].find(&format!(".{extension}")).unwrap() + extension.len() + 1;
    html[start..start + end].to_owned()
}

#[test]
fn rst_and_myst_diagrams_are_figures_in_html_and_real_pdf() {
    let dir = setup("typst-both");
    dir.write(
        "index.rst",
        concat!(
            "Drawings\n========\n\n.. toctree::\n\n   sub/other\n\n",
            ".. typst-diagram::\n   :caption: A *circle*\n   :alt: Round\n",
            "   :name: circle\n   :class: drawing\n   :align: center\n\n",
            "   #circle(radius: 12pt)\n\nSee :numref:`circle`.\n",
        ),
    );
    dir.write(
        "sub/other.md",
        concat!(
            "# Other\n\n```{typst-diagram}\n:caption: A square\n:alt: Square\n",
            ":format: png\n\n#rect(width: 20pt, height: 20pt)\n```\n",
        ),
    );
    for builder in ["html", "pdf"] {
        build(&dir.path, 0, &[builder, "-n", "-W"]);
    }
    let html = dir.read("_build/html/index.html");
    has(&html, "alt=\"Round\"");
    has(&html, "id=\"circle\"");
    has(&html, "<em>circle</em>");
    has(&html, "class=\"caption-number\">Fig. ");
    has(&html, "href=\"#circle\">Fig. ");
    let svg = diagram_url(&html, "svg");
    has(&dir.read(&format!("_build/html/{svg}")), "<svg");
    let other = dir.read("_build/html/sub/other.html");
    let png = diagram_url(&other, "png");
    let bytes = std::fs::read(dir.path.join("_build/html").join(png)).unwrap();
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    has(&dir.read("_build/pdf/book-en.typ"), "typst-");
    let pdf = std::fs::read(dir.path.join("_build/pdf/demo-en.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    let pdf_path = dir.path.join("_build/pdf/demo-en.pdf");
    let before = std::fs::metadata(&pdf_path).unwrap().modified().unwrap();
    let result = build(&dir.path, 0, &["pdf", "-n", "-W"]);
    has(&both(&result), "1 unchanged");
    assert_eq!(std::fs::metadata(&pdf_path).unwrap().modified().unwrap(), before);
}

#[test]
fn helper_changes_and_missing_data_invalidate_cached_pages() {
    let dir = setup("typst-incremental");
    dir.write("index.rst", "Drawings\n========\n\n.. include:: sub/part.rst\n");
    dir.write(
        "sub/part.rst",
        concat!(
            ".. typst-diagram::\n   :alt: Local helper\n\n",
            "   #import \"helper.typ\": draw\n   #draw\n",
        ),
    );
    dir.write("sub/helper.typ", "#let draw = circle(radius: 12pt)\n");
    build(&dir.path, 0, &["html", "-W"]);
    let first = diagram_url(&dir.read("_build/html/index.html"), "svg");
    build(&dir.path, 0, &["html", "-W"]);
    assert_eq!(first, diagram_url(&dir.read("_build/html/index.html"), "svg"));
    dir.write("sub/helper.typ", "#let draw = circle(radius: 24pt)\n");
    build(&dir.path, 0, &["html", "-W"]);
    let second = diagram_url(&dir.read("_build/html/index.html"), "svg");
    assert_ne!(first, second);
    dir.write("sub/helper.typ", "#let draw = read(\"data.txt\")\n");
    let failed = build(&dir.path, 1, &["html", "-W"]);
    has(&both(&failed), "TYPST DIAGRAM NOT DRAWN");
    assert_eq!(second, diagram_url(&dir.read("_build/html/index.html"), "svg"));
    build(&dir.path, 0, &["html"]);
    lacks(&dir.read("_build/html/index.html"), "_images/typst-");
    dir.write("sub/data.txt", "Now available");
    build(&dir.path, 0, &["html", "-W"]);
    has(&dir.read("_build/html/index.html"), "_images/typst-");
}

#[test]
fn file_sources_resolve_beside_the_file_and_text_never_compiles() {
    let dir = setup("typst-file");
    dir.write("index.rst", "Drawing\n=======\n\n.. typst-diagram:: figures/main.typ\n");
    dir.write("figures/main.typ", "#import \"helper.typ\": draw\n#draw\n");
    dir.write("figures/helper.typ", "#let draw = circle(radius: 12pt)\n");
    build(&dir.path, 0, &["html", "-W"]);
    has(&dir.read("_build/html/index.html"), "_images/typst-");
    dir.write("figures/main.typ", "#unknown()\n");
    build(&dir.path, 0, &["text", "-W"]);
    let failure = build(&dir.path, 1, &["html", "-W"]);
    has(&both(&failure), "diagram:1:");
}

#[test]
fn untrusted_sources_are_retained_without_compilation() {
    let dir = setup("typst-untrusted");
    dir.write("index.rst", "Drawing\n=======\n\n.. typst-diagram::\n\n   #unknown()\n");
    let output = build(&dir.path, 0, &["html", "--untrusted"]);
    has(&both(&output), "disabled in --untrusted");
    let html = dir.read("_build/html/index.html");
    has(&html, "unknown");
    lacks(&html, "_images/typst-");
}

#[cfg(unix)]
#[test]
fn source_symlinks_cannot_escape_the_project() {
    let dir = setup("typst-confined");
    let outside = Scratch::new("typst-outside");
    outside.write("secret.typ", "#let secret = [private]\n");
    std::os::unix::fs::symlink(outside.path.join("secret.typ"), dir.path.join("secret.typ"))
        .unwrap();
    dir.write(
        "index.rst",
        concat!(
            "Drawing\n=======\n\n.. typst-diagram::\n\n",
            "   #import \"secret.typ\": secret\n   #secret\n",
        ),
    );
    let failure = build(&dir.path, 1, &["html", "-W"]);
    has(&both(&failure), "denied");
}
