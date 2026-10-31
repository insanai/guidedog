//! Directive semantics are identical through the RST and MyST readers.

use crate::tests::*;

#[test]
fn diagrams_preserve_source_and_figure_options() {
    let read = read_text(
        concat!(
            ".. typst-diagram::\n   :caption: A *circle*\n   :alt: Round\n",
            "   :name: round\n   :class: drawing\n   :align: center\n   :format: png\n\n",
            "   #circle(radius: 12pt)\n",
        ),
        None,
    );
    assert!(read.ok && read.codes.is_empty(), "{}", read.codes);
    for part in [
        "code-block",
        "#circle(radius: 12pt)",
        "@language=\"typst\"",
        "@typst-diagram=\"1\"",
        "@format=\"png\"",
        "@alt=\"Round\"",
        "@name=\"round\"",
        "@class=\"drawing\"",
        "caption",
        "emphasis",
    ] {
        assert!(read.dump.contains(part), "missing {part}: {}", read.dump);
    }
    md_expect(
        "```{typst-diagram}\n:alt: Round\n:format: png\n\n#circle(radius: 12pt)\n```\n",
        &["#circle(radius: 12pt)", "@typst-diagram=\"1\"", "@format=\"png\""],
    );
}

#[test]
fn a_file_is_included_but_ambiguous_sources_are_refused() {
    let read = read_text(".. typst-diagram:: old.txt\n", None);
    assert!(read.ok && read.codes.is_empty(), "{}", read.codes);
    assert!(read.dump.contains("@typst-source=\"old.txt\""), "{}", read.dump);
    assert!(read.dump.contains("a\\nb\\nc\\nd\\n"), "{}", read.dump);
    for source in [".. typst-diagram::\n", ".. typst-diagram:: old.txt\n\n   extra\n"] {
        expect_codes(source, "sphinx.typst.source", None);
    }
    expect_codes(".. typst-diagram:: missing.typ\n", "core.include.missing", None);
}

#[test]
fn ordinary_typst_code_is_never_marked_for_execution() {
    let read = read_text(".. code-block:: typst\n\n   #circle(radius: 12pt)\n", None);
    assert!(!read.dump.contains("@typst-diagram"));
    let read = md_read(
        "```typst\n#circle(radius: 12pt)\n```\n",
        guidedoc_commonmark::Settings::default(),
    );
    assert!(!read.dump.contains("@typst-diagram"));
}
