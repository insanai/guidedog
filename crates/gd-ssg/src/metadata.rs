//! Rewrite only leading metadata, retaining the body and its line endings byte for byte.

use std::collections::BTreeSet;

use gd_host::Problem;

pub(crate) fn leading_metadata(source: &str, markdown: bool) -> &str {
    let text = source.strip_prefix('\u{feff}').unwrap_or(source);
    let text = if markdown { text } else { &text[blank_prefix(text)..] };
    let mut lines = text.split_inclusive('\n');
    if markdown {
        let Some(first) = lines.next() else { return "" };
        if first.trim_end() != "---" {
            return "";
        }
        let mut end = first.len();
        for line in lines {
            end += line.len();
            if matches!(line.trim_end(), "---" | "...") {
                return &text[..end.min(text.len())];
            }
        }
        return text;
    }
    if !text.starts_with(':') {
        return "";
    }
    let mut end = 0;
    for line in text.split_inclusive('\n') {
        if !line.trim().is_empty() && !line.starts_with([':', ' ', '\t']) {
            break;
        }
        end += line.len();
    }
    &text[..end]
}

pub(crate) fn replace_fields(
    source: &str,
    markdown: bool,
    changes: &[(&str, Option<&str>)],
) -> Result<String, Problem> {
    let (bom, text) = source.strip_prefix('\u{feff}').map_or(("", source), |s| ("\u{feff}", s));
    let prefix = if markdown { 0 } else { blank_prefix(text) };
    let (padding, text) = text.split_at(prefix);
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let lines: Vec<_> = text.split_inclusive('\n').collect();
    let yaml = markdown && lines.first().is_some_and(|l| l.trim_end() == "---");
    let end = if yaml {
        lines
            .iter()
            .skip(1)
            .position(|l| matches!(l.trim_end(), "---" | "..."))
            .map(|i| i + 1)
            .ok_or_else(|| invalid("The YAML front matter is not closed."))?
    } else {
        rst_end(&lines)
    };
    let start = usize::from(yaml);
    let end = if !yaml && lines.first().is_none_or(|l| !l.starts_with(':')) { 0 } else { end };
    let mut out = String::from(bom);
    out.push_str(padding);
    if markdown {
        out.push_str("---");
        out.push_str(newline);
    }
    let mut seen = BTreeSet::new();
    let mut replacing = false;
    for line in &lines[start..end] {
        let key = field_key(line, yaml);
        if let Some(key) = key {
            replacing = changes.iter().any(|(name, _)| *name == key);
            if replacing && !seen.insert(key.to_owned()) {
                return Err(invalid("A publication field occurs more than once."));
            }
        } else if !line.starts_with([' ', '\t']) && !line.trim().is_empty() {
            replacing = false;
        }
        if !replacing {
            out.push_str(line);
        }
    }
    if !out.is_empty() && !out.ends_with('\n') && out != bom {
        out.push_str(newline);
    }
    for &(key, value) in changes {
        if let Some(value) = value {
            if yaml || markdown {
                out.push_str(&format!("{key}: {value}{newline}"));
            } else {
                out.push_str(&format!(":{key}: {value}{newline}"));
            }
        }
    }
    if markdown {
        out.push_str("---");
        out.push_str(newline);
    }
    if end == 0 && !text.is_empty() {
        out.push_str(newline);
    }
    let body = if yaml { end + 1 } else { end };
    for line in &lines[body..] {
        out.push_str(line);
    }
    Ok(out)
}

fn blank_prefix(text: &str) -> usize {
    text.split_inclusive('\n').take_while(|line| line.trim().is_empty()).map(str::len).sum()
}

fn rst_end(lines: &[&str]) -> usize {
    let mut end = lines
        .iter()
        .position(|line| !line.trim().is_empty() && !line.starts_with([':', ' ', '\t']))
        .unwrap_or(lines.len());
    while end > 0 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    end
}

fn field_key(line: &str, yaml: bool) -> Option<&str> {
    let line = if yaml { line } else { line.strip_prefix(':')? };
    let (key, _) = line.split_once(':')?;
    if key.is_empty() || key.contains(char::is_whitespace) { None } else { Some(key) }
}

fn invalid(message: &str) -> Problem {
    Problem::new(
        "ssg.metadata",
        "CANNOT EDIT PUBLICATION METADATA",
        "",
        message,
        "Use one unambiguous leading metadata block, then retry.",
        2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_prefixes_and_multiline_metadata_survive_edits() {
        let text = "\n\n:summary: First.\n\n   Second.\n:draft: true\n\nTitle\n=====\n";
        let edited = replace_fields(text, false, &[("draft", Some("false"))]).unwrap();
        assert_eq!(edited, text.replace(":draft: true", ":draft: false"));
        assert!(leading_metadata(text, false).contains(":draft: true"));
        assert_eq!(
            leading_metadata("---\r\ndraft: true\r\n---", true),
            "---\r\ndraft: true\r\n---"
        );
    }

    #[test]
    fn preserves_bom_crlf_and_body_metadata_examples() {
        let text = concat!(
            "\u{feff}:draft: true\r\n:summary: Keep this.\r\n",
            "\r\nTitle\r\n=====\r\n\r\n:draft: example\r\n"
        );
        let edited = replace_fields(text, false, &[("draft", Some("false"))]).unwrap();
        assert!(edited.starts_with("\u{feff}:summary: Keep this.\r\n:draft: false\r\n"));
        assert!(edited.ends_with("\r\nTitle\r\n=====\r\n\r\n:draft: example\r\n"));
    }

    #[test]
    fn yaml_nested_values_and_body_are_preserved() {
        let text = "---\nauthors:\n  - Ada\ndraft: true\n---\n# Title\n\nBody\n";
        let edited = replace_fields(text, true, &[("draft", Some("false"))]).unwrap();
        assert_eq!(edited, "---\nauthors:\n  - Ada\ndraft: false\n---\n# Title\n\nBody\n");
        assert!(replace_fields("---\ndraft: true\n", true, &[]).is_err());
        assert!(
            replace_fields(":draft: true\n:draft: false\n\n", false, &[("draft", Some("false"))])
                .is_err()
        );
    }
}
