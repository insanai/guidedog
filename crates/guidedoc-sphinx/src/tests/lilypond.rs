use crate::tests::*;

#[test]
fn music_options_and_captions_survive_both_readers() {
    let source = concat!(
        ".. lilypond::\n   :caption: A *melody*\n   :alt: Four notes\n",
        "   :name: tune\n   :class: score\n   :align: center\n   :format: png\n",
        "   :staff-size: 16.5\n   :width: 80%\n   :justify: all\n\n   c'4 d' e' f'\n",
    );
    let read = read_text(source, None);
    assert!(read.ok && read.codes.is_empty(), "{}", read.codes);
    for part in [
        "@lilypond=\"1\"",
        "@language=\"lilypond\"",
        "@staff-size=\"16.5\"",
        "@width=\"80%\"",
        "@justify=\"all\"",
        "@format=\"png\"",
        "emphasis",
        "c'4 d' e' f'\\n",
    ] {
        assert!(read.dump.contains(part), "missing {part}: {}", read.dump);
    }
    md_expect(
        "```{lilypond}\n:caption: A *melody*\n:width: 12cm\n\nc'4 d' e' f'\n```\n",
        &["c'4 d' e' f'\\n", "@lilypond=\"1\"", "@width=\"12cm\""],
    );
}

#[test]
fn raw_body_bytes_and_per_line_offsets_are_exact() {
    let source = ".. lilypond::\n\n\tc'4\t d'4\n\te'4 ^\"café\" f'4\n";
    let mut request = default_request("test.rst", source, "sphinx", "html");
    let config = Config::default();
    request.read.extension = Some(read_extension(&config));
    let mut workspace = Workspace::new();
    let doc = read_document(&mut workspace, &crate::READER, &request).unwrap();
    let id = (0..doc.node_count())
        .map(guidedoc::NodeId::from_index)
        .find(|id| doc.custom_attribute(*id, "lilypond").is_some())
        .unwrap();
    assert_eq!(doc.node_text(id), "c'4\t d'4\ne'4 ^\"café\" f'4\n");
    let map: Vec<usize> = doc
        .custom_attribute(id, "lilypond-map")
        .unwrap()
        .split(',')
        .map(|n| n.parse().unwrap())
        .collect();
    assert_eq!(map, [source.find("c'4").unwrap(), source.find("e'4").unwrap()]);
    assert_eq!(doc.custom_attribute(id, "lilypond-line"), Some("3"));
}

#[test]
fn file_requests_are_retained_without_reading_and_ambiguous_sources_warn() {
    let read = read_text(".. lilypond:: never-opened.ly\n", None);
    assert!(read.ok && read.codes.is_empty(), "{}", read.codes);
    assert!(read.dump.contains("@lilypond-file=\"never-opened.ly\""));
    for source in [".. lilypond::\n", ".. lilypond:: score.ly\n\n   c4\n"] {
        expect_codes(source, "sphinx.lilypond.source", None);
    }
    let read = read_text(".. code-block:: lilypond\n\n   c4 d e f\n", None);
    assert!(!read.dump.contains("@lilypond="));
    let read = md_read("```lilypond\nc4 d e f\n```\n", guidedoc_commonmark::Settings::default());
    assert!(!read.dump.contains("@lilypond="));
}
