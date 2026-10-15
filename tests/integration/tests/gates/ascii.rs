//! Guidedog's documentation is plain ASCII wherever ASCII can say it: no Unicode dashes,
//! curly quotes, ellipses, arrows or no-break spaces in the prose of `docs/` and the
//! READMEs. Unicode stays where it is content: translations (`locales/`,
//! `README.<lang>.md`), fixture data, and lines marked as names or Unicode examples.
//!
//! A line is marked by an allow comment on the line above it: `.. ascii-allow` in
//! reStructuredText, `<!-- ascii-allow -->` in Markdown and `// ascii-allow` in Typst.

use std::fs;
use std::path::{Path, PathBuf};

/// The characters ASCII replaces in documentation.
fn is_forbidden(c: char) -> bool {
    matches!(c, '\u{2013}' | '\u{2014}' | '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}')
        || matches!(c, '\u{2026}' | '\u{00A0}' | '\u{2190}'..='\u{21FF}')
}

/// What to write instead of `c`.
fn ascii_for(c: char) -> &'static str {
    match c {
        '\u{2013}' | '\u{2014}' => "`--`, a comma, a colon or parentheses",
        '\u{2018}' | '\u{2019}' => "a straight quote `'`",
        '\u{201C}' | '\u{201D}' => "a straight quote `\"`",
        '\u{2026}' => "three dots `...`",
        '\u{00A0}' => "a plain space",
        _ => "`->`, `<-` or a word",
    }
}

/// The allow comment for a file, by its extension.
fn allow_marker(path: &str) -> &'static str {
    if path.ends_with(".md") {
        "<!-- ascii-allow -->"
    } else if path.ends_with(".typ") {
        "// ascii-allow"
    } else {
        ".. ascii-allow"
    }
}

/// Whether a translation of a README (`README.de.md`, `README.zh-CN.md`).
fn is_translated_readme(name: &str) -> bool {
    name.strip_prefix("README.").is_some_and(|rest| rest != "md" && rest.ends_with(".md"))
}

/// The documentation sources below `dir`, skipping build output, translations and data.
fn collect_docs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !matches!(name.as_str(), "_build" | "locales" | "data" | "target" | ".git") {
                collect_docs(&path, out);
            }
        } else if is_doc_name(&name) {
            out.push(path);
        }
    }
}

fn is_doc_name(name: &str) -> bool {
    let doc = name.ends_with(".rst") || name.ends_with(".md") || name.ends_with(".typ");
    doc && !is_translated_readme(name)
}

/// Every checked source, relative to the repository root with `/` separators: all of
/// `docs/`, the root, crate, command and tool READMEs, and those of `examples` and
/// `plugins`.
fn checked_sources(root: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    collect_docs(&root.join("docs"), &mut paths);
    paths.push(root.join("README.md"));
    for top in ["crates", "cmd", "tools"] {
        let Ok(entries) = fs::read_dir(root.join(top)) else { continue };
        paths.extend(entries.flatten().map(|e| e.path().join("README.md")));
    }
    paths.push(root.join("examples/README.md"));
    paths.push(root.join("plugins/README.md"));
    let mut rel: Vec<String> = paths
        .iter()
        .filter(|p| p.is_file())
        .map(|p| p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/"))
        .collect();
    rel.sort();
    rel
}

/// The Elm-style reports for the forbidden characters in `text`, the source at `path`.
fn offences(path: &str, text: &str) -> Vec<String> {
    let marker = allow_marker(path);
    let mut reports = Vec::new();
    let mut offset = 0;
    let mut previous = "";
    for line in text.split_inclusive('\n') {
        if previous.trim() != marker {
            for (i, c) in line.char_indices().filter(|&(_, c)| is_forbidden(c)) {
                let start = offset + i;
                reports.push(report(path, text, start, start + c.len_utf8(), c, marker));
            }
        }
        previous = line;
        offset += line.len();
    }
    reports
}

fn report(path: &str, text: &str, start: usize, end: usize, c: char, marker: &str) -> String {
    let message = format!("This line uses U+{:04X}, which ASCII can say.", c as u32);
    let hint = format!(
        "Write {} instead. If the character is content (a name or an example about \
         Unicode), put `{marker}` on the line above.",
        ascii_for(c)
    );
    let report = gd_report::Report::error("docs.ascii", "UNICODE PUNCTUATION", &message)
        .hint(&hint)
        .at(path, text, start, end);
    gd_report::render_text(&report, &gd_report::Style::default())
}

#[test]
fn documentation_prose_is_ascii() {
    let root = guidedog_tests::repository_root();
    let mut problems = Vec::new();
    for rel in checked_sources(&root) {
        let text = fs::read_to_string(root.join(&rel)).unwrap_or_default();
        problems.extend(offences(&rel, &text));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n\n"));
}

#[test]
fn the_scan_reports_the_place_and_honours_allow_markers() {
    let text = "Plain -- text.\nA dash \u{2014} here.\n.. ascii-allow\nA name: \u{2014}\n";
    let found = offences("docs/a.rst", text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("docs/a.rst:2:8"), "{}", found[0]);
    assert!(found[0].contains("Hint: Write `--`"), "{}", found[0]);
    let md = "<!-- ascii-allow -->\nCaf\u{e9} \u{2192} ok\n\u{201C}quoted\u{201D}\n";
    assert_eq!(offences("README.md", md).len(), 2);
    assert!(is_translated_readme("README.zh-CN.md"));
    assert!(!is_translated_readme("README.md"));
}
