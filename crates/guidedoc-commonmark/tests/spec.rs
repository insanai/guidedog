//! All 652 examples of the CommonMark 0.31.2 specification, read with the
//! "commonmark" reader and written by the HTML renderer in the reference profile.

mod common;

use guidedoc::{Workspace, convert};

const SPEC: &str = include_str!("data/commonmark-spec-0.31.2.txt");
const FENCE: &str = "````````````````````````````````";

struct Example {
    number: usize,
    section: String,
    markdown: String,
    html: String,
}

/// Extracts every example; the spec writes tabs as U+2192.
fn spec_examples() -> Vec<Example> {
    let mut examples = Vec::new();
    let mut section = String::new();
    let (mut state, mut markdown, mut html) = (0, String::new(), String::new());
    for line in SPEC.split('\n') {
        match state {
            0 if line == format!("{FENCE} example") => state = 1,
            0 if line.starts_with('#') => section = line.trim_start_matches(['#', ' ']).into(),
            1 if line == "." => state = 2,
            1 => markdown.push_str(&format!("{line}\n")),
            2 if line == FENCE => {
                examples.push(Example {
                    number: examples.len() + 1,
                    section: section.clone(),
                    markdown: std::mem::take(&mut markdown).replace('\u{2192}', "\t"),
                    html: std::mem::take(&mut html).replace('\u{2192}', "\t"),
                });
                state = 0;
            }
            2 => html.push_str(&format!("{line}\n")),
            _ => {}
        }
    }
    examples
}

#[test]
fn commonmark_spec_examples() {
    let examples = spec_examples();
    assert_eq!(examples.len(), 652);
    let registry = common::registry();
    let mut ws = Workspace::new();
    let mut failed = Vec::new();
    for ex in &examples {
        let request = common::reference_request(&ex.markdown);
        let got = match convert(&mut ws, &registry, &request) {
            Ok(html) => html.to_owned(),
            Err(fail) => format!("<failed {}>", fail.code()),
        };
        if got != ex.html {
            eprintln!(
                "FAIL example {} ({})\n--- markdown\n{:?}\n--- expected\n{:?}\n--- got\n{:?}\n",
                ex.number, ex.section, ex.markdown, ex.html, got
            );
            failed.push(ex.number);
        }
    }
    eprintln!("CommonMark 0.31.2: {} / {} examples pass", examples.len() - failed.len(), 652);
    assert!(failed.is_empty(), "failing examples: {failed:?}");
}
