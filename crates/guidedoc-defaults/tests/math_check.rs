//! The math check runs once in the read stage of every reader, and a host can give it
//! its macros or turn it off.

use guidedoc::{ReadOptions, Request, Source, Workspace, read_document};

/// The math findings of reading `text` with `reader`: code, hint, and the source text
/// under the caret.
fn findings(reader: &str, text: &str, read: ReadOptions<'_>) -> Vec<(String, String, String)> {
    let registry = guidedoc_defaults::registry();
    let reader = registry.find_reader(reader).expect("a built-in reader");
    let request = Request { source: Source { name: "page", text }, read, ..Request::default() };
    let mut ws = Workspace::new();
    read_document(&mut ws, reader, &request).expect("reads");
    let math = ws.reports().items().iter().filter(|d| d.code.starts_with("math."));
    math.map(|d| {
        let under = text[d.span.start as usize..d.span.end as usize].to_string();
        (d.code.to_string(), d.formatted_hint(), under)
    })
    .collect()
}

fn options<'a>(reader: &str, settings: &'a guidedoc_defaults::ReaderSettings) -> ReadOptions<'a> {
    settings.options(reader)
}

#[test]
fn every_reader_checks_its_formulas_once() {
    let settings = guidedoc_defaults::ReaderSettings::default();
    let rst = "A :math:`\\fraction{a}{b}` here.\n\n.. math::\n\n   \\left( a\n";
    for reader in ["rst", "sphinx"] {
        let found = findings(reader, rst, options(reader, &settings));
        let expected = [
            ("math.command.unknown", "Did you mean `\\frac`?", "\\fraction"),
            (
                "math.delimiter.unpaired",
                "Add `\\right` with a delimiter, or `\\right.` for none.",
                "\\left",
            ),
        ];
        let found: Vec<_> =
            found.iter().map(|(c, h, u)| (c.as_str(), h.as_str(), u.as_str())).collect();
        assert_eq!(found, expected, "{reader}");
    }
    let found = findings("myst", "Now $\\aplha$.\n", options("myst", &settings));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1, "Did you mean `\\alpha`?");
    assert_eq!(found[0].2, "\\aplha");
}

#[test]
fn a_host_names_its_macros_or_turns_the_check_off() {
    let settings = guidedoc_defaults::ReaderSettings::default();
    let text = ":math:`\\RR^n`\n";
    let base = options("sphinx", &settings);
    assert_eq!(findings("sphinx", text, base).len(), 1);
    let known = ReadOptions { math_macros: &["RR"], ..options("sphinx", &settings) };
    assert!(findings("sphinx", text, known).is_empty());
    let off = ReadOptions { check_math: false, ..options("sphinx", &settings) };
    assert!(findings("sphinx", text, off).is_empty());
}

#[test]
fn mathml_written_by_the_author_is_not_checked_as_latex() {
    let settings = guidedoc_defaults::ReaderSettings::default();
    let text = ".. math::\n   :notation: mathml\n\n   <math><mi>x</mi></math>\n";
    assert!(findings("sphinx", text, options("sphinx", &settings)).is_empty());
}
