//! Every docstring conversion of Sphinx 9.1's napoleon tests
//! (`tests/test_ext_napoleon/test_ext_napoleon_docstring.py`), with the settings, the
//! object and the output Sphinx gave, compared line for line.
//!
//! `tests/data/napoleon-cases.txt` holds one case per `@@@@ case N STYLE` header:
//! `@@@@ set NAME = VALUE` lines change a setting, `@@@@ what`, `@@@@ name`,
//! `@@@@ no-index` and `@@@@ annotation NAME = TYPE` describe the object, and
//! `@@@@ input N` and `@@@@ output N` are followed by N lines each.

use gd_autodoc::{DocstringStyle, Napoleon, NapoleonContext, NapoleonSection, convert_docstring};

/// One recorded conversion.
#[derive(Debug, Default)]
struct Case {
    title: String,
    style: DocstringStyle,
    settings: Napoleon,
    strip_signature_backslash: bool,
    what: String,
    name: String,
    no_index: bool,
    annotations: Vec<(String, String)>,
    input: Vec<String>,
    output: Vec<String>,
}

fn flag(value: &str) -> bool {
    value == "true"
}

/// `{a=b, c=d}`.
fn pairs(value: &str) -> Vec<(String, String)> {
    let inner = value.trim_start_matches('{').trim_end_matches('}');
    inner
        .split(", ")
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

/// `[name; (name, like); ...]`.
fn sections(value: &str) -> Vec<NapoleonSection> {
    let inner = value.trim_start_matches('[').trim_end_matches(']');
    inner
        .split("; ")
        .map(|entry| match entry.strip_prefix('(').and_then(|e| e.strip_suffix(')')) {
            Some(pair) => {
                let (name, like) = pair.split_once(", ").unwrap_or((pair, ""));
                NapoleonSection { name: name.into(), like: like.into() }
            }
            None => NapoleonSection { name: entry.into(), like: String::new() },
        })
        .collect()
}

fn set(case: &mut Case, name: &str, value: &str) {
    let s = &mut case.settings;
    match name {
        "use_param" => s.use_param = flag(value),
        "use_rtype" => s.use_rtype = flag(value),
        "use_keyword" => s.use_keyword = flag(value),
        "use_ivar" => s.use_ivar = flag(value),
        "preprocess_types" => s.preprocess_types = flag(value),
        "attr_annotations" => s.attr_annotations = flag(value),
        "use_admonition_for_examples" => s.use_admonition_for_examples = flag(value),
        "use_admonition_for_notes" => s.use_admonition_for_notes = flag(value),
        "use_admonition_for_references" => s.use_admonition_for_references = flag(value),
        "type_aliases" => s.type_aliases = pairs(value),
        "custom_sections" => s.custom_sections = sections(value),
        "strip_signature_backslash" => case.strip_signature_backslash = flag(value),
        other => panic!("unknown setting {other}"),
    }
}

fn load_cases() -> Vec<Case> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/napoleon-cases.txt");
    let text = std::fs::read_to_string(path).expect("the napoleon cases are readable");
    let mut lines = text.lines();
    let mut cases = Vec::new();
    while let Some(line) = lines.next() {
        let Some(header) = line.strip_prefix("@@@@ ") else { panic!("unexpected {line:?}") };
        let (key, rest) = header.split_once(' ').unwrap_or((header, ""));
        if key == "case" {
            let style = if rest.ends_with("numpy") {
                DocstringStyle::Numpy
            } else {
                DocstringStyle::Google
            };
            cases.push(Case { title: rest.to_owned(), style, ..Case::default() });
            continue;
        }
        let case = cases.last_mut().expect("a case header comes first");
        match key {
            "set" => {
                let (name, value) = rest.split_once(" = ").expect("NAME = VALUE");
                set(case, name, value);
            }
            "what" => case.what = rest.to_owned(),
            "name" => case.name = rest.to_owned(),
            "no-index" => case.no_index = true,
            "annotation" => {
                let (name, ty) = rest.split_once(" = ").expect("NAME = TYPE");
                case.annotations.push((name.to_owned(), ty.to_owned()));
            }
            "input" | "output" => {
                let n: usize = rest.parse().expect("a line count");
                let block: Vec<String> =
                    (0..n).map(|_| lines.next().unwrap_or("").to_owned()).collect();
                if key == "input" { case.input = block } else { case.output = block }
            }
            other => panic!("unknown key {other}"),
        }
    }
    cases
}

#[test]
fn sphinx_napoleon_conversions() {
    let cases = load_cases();
    assert!(cases.len() > 100, "{} cases", cases.len());
    let mut failures = Vec::new();
    for case in &cases {
        let what = if case.what.is_empty() { "object" } else { case.what.as_str() };
        let ctx = NapoleonContext {
            what,
            name: &case.name,
            annotations: (what == "class" || what == "module").then_some(&case.annotations[..]),
            no_index: case.no_index,
            strip_signature_backslash: case.strip_signature_backslash,
        };
        let out = convert_docstring(case.style, &case.input, &case.settings, &ctx);
        let (got, want) = (out.lines.join("\n"), case.output.join("\n"));
        if got != want {
            failures.push(format!("case {}:\n--- want\n{want}\n--- got\n{got}\n", case.title));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}
