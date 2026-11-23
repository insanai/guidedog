use guidedoc::{Pass, RenderOptions, Request, Source, Workspace};

use super::*;

/// The HTML fragment of `text` (reStructuredText with the Sphinx layer) with its
/// pictures embedded under `options`, and the codes of the findings.
fn convert(text: &str, options: &EmbedOptions) -> (String, Vec<&'static str>) {
    let registry = guidedoc_defaults::registry();
    let settings = guidedoc_defaults::ReaderSettings::default();
    let embedder = Embedder::new();
    let embed = |doc: Document<'_>, b: &mut Builder<'_>| embedder.embed(doc, b, options);
    let request = Request {
        source: Source { name: "page.rst", text },
        reader: "sphinx",
        renderer: "html",
        read: settings.options("sphinx"),
        passes: &[Pass { name: "embed-pictures", run: &embed }],
        render: RenderOptions { standalone: false, ..RenderOptions::default() },
        ..Request::default()
    };
    let mut ws = Workspace::new();
    let html = guidedoc::convert(&mut ws, &registry, &request).expect("converts").to_string();
    let codes = ws.reports().items().iter().map(|d| d.code).collect();
    (html, codes)
}

fn offline() -> EmbedOptions {
    EmbedOptions { offline: true, embedded_fonts_only: true, ..EmbedOptions::default() }
}

const GRAPHS: &str = "\
.. graphviz::
   :caption: One.
   :alt: a to b

   digraph { a -> b }

.. graphviz::

   digraph { c -> d }
";

#[test]
fn a_graph_becomes_an_image_with_its_alt_text_and_caption() {
    let (html, codes) = convert(GRAPHS, &offline());
    assert!(codes.is_empty(), "{codes:?}");
    assert!(html.contains("<figure"), "{html}");
    assert!(html.contains("<img src=\"data:image/svg+xml;base64,"), "{html}");
    assert!(html.contains("alt=\"a to b\" class=\"graphviz\" />"), "{html}");
    assert!(html.contains("<figcaption>One.</figcaption>"), "{html}");
    assert_eq!(html.matches("<img").count(), 2);
}

#[test]
fn a_typst_diagram_is_drawn_and_reads_no_file_without_a_root() {
    let text = ".. typst-diagram::\n   :alt: A circle\n\n   #circle(radius: 10pt)\n\n\
                .. typst-diagram::\n\n   #read(\"/etc/hostname\")\n";
    let (html, codes) = convert(text, &offline());
    assert!(html.contains("alt=\"A circle\" class=\"typst-diagram\" />"), "{html}");
    assert_eq!(html.matches("<img").count(), 1, "{html}");
    assert!(html.contains("#read("), "the refused source stays as code: {html}");
    assert_eq!(codes, ["embed.typst-diagram"]);
}

#[test]
fn a_broken_picture_stays_code_with_a_finding() {
    let (html, codes) = convert(".. graphviz::\n\n   digraph { broken\n", &offline());
    assert!(!html.contains("<img"), "{html}");
    assert!(html.contains("digraph { broken"), "{html}");
    assert_eq!(codes, ["embed.graphviz"]);
}

#[test]
fn the_count_and_size_bounds_leave_code_and_a_finding() {
    let one = EmbedOptions { max_pictures: 1, ..offline() };
    let (html, codes) = convert(GRAPHS, &one);
    assert_eq!(html.matches("<img").count(), 1, "{html}");
    assert!(html.contains("digraph { c -&gt; d }"), "{html}");
    assert_eq!(codes, ["embed.limit"]);
    let small = EmbedOptions { max_bytes: 1024, ..offline() };
    let (html, codes) = convert(GRAPHS, &small);
    assert!(!html.contains("<img"), "{html}");
    assert_eq!(codes, ["embed.limit", "embed.limit"]);
    let none = EmbedOptions { max_pictures: 0, ..offline() };
    let (html, codes) = convert("Text only.\n", &none);
    assert!(html.contains("Text only.") && codes.is_empty());
}
