//! `extract_summary`: the first sentence of a docstring, by Sphinx's rule for where a
//! sentence ends. reStructuredText is not parsed here; the few shapes the rule asks
//! about (a heading, a block that is not a paragraph, inline markup left open) are
//! recognised from the text.

/// Abbreviations a sentence does not end at.
const WELL_KNOWN_ABBREVIATIONS: &[&str] = &["et al.", "e.g.", "i.e.", "vs."];

/// The summary of prepared docstring lines.
pub(crate) fn extract_summary(doc: &[String]) -> String {
    let stanza: Vec<&str> = doc
        .iter()
        .map(String::as_str)
        .skip_while(|l| l.trim().is_empty())
        .take_while(|l| !l.trim().is_empty())
        .collect();
    let Some(first) = stanza.first() else { return String::new() };
    let summary = if let Some(title) = heading(&stanza) {
        title.trim().to_owned()
    } else if !is_paragraph(&stanza) {
        first.trim().to_owned()
    } else {
        first_sentence(&stanza)
    };
    match summary.trim_end().strip_suffix("::") {
        Some(head) => format!("{head}."),
        None => summary,
    }
}

/// The sentences of a paragraph, joined until the first that ends the summary.
fn first_sentence(stanza: &[&str]) -> String {
    let text = stanza.join(" ");
    let sentences = split_sentences(&text);
    if sentences.len() == 1 {
        return sentences[0].trim().to_owned();
    }
    let broken = has_open_markup(&text);
    let mut summary = String::new();
    for i in 0..sentences.len() {
        summary = format!("{}.", sentences[..=i].join(". ").trim_end_matches('.'));
        if WELL_KNOWN_ABBREVIATIONS.iter().any(|a| summary.ends_with(a)) {
            continue;
        }
        if !broken {
            break;
        }
    }
    summary
}

/// `periods_re.split`: the text split at each `.` followed by whitespace.
fn split_sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c != '.' || !chars.peek().is_some_and(|(_, n)| n.is_whitespace()) {
            continue;
        }
        out.push(&text[start..i]);
        let mut end = i + 1;
        while let Some(&(j, n)) = chars.peek().filter(|(_, n)| n.is_whitespace()) {
            end = j + n.len_utf8();
            chars.next();
        }
        start = end;
    }
    out.push(&text[start..]);
    out
}

/// A section title: `Title` over an underline, or between an overline and an
/// underline.
fn heading<'a>(stanza: &[&'a str]) -> Option<&'a str> {
    match stanza {
        [over, title, under, ..] if is_adornment(over) && is_adornment(under) => Some(title),
        [title, under, ..] if is_adornment(under) && !is_adornment(title) => Some(title),
        _ => None,
    }
}

/// A line of one repeated punctuation character.
fn is_adornment(line: &str) -> bool {
    let t = line.trim_end();
    let mut chars = t.chars();
    let Some(c) = chars.next() else { return false };
    c.is_ascii_punctuation() && t.len() >= 2 && chars.all(|d| d == c)
}

/// Whether the stanza is a paragraph, not a list, a directive, a field list, a
/// table, a quote, a doctest or a definition.
fn is_paragraph(stanza: &[&str]) -> bool {
    let first = stanza[0];
    let t = first.trim_start();
    if t.len() != first.len() {
        return false;
    }
    let starts = ["..", "- ", "* ", "+ ", ">>>", "| ", "+-", "=="];
    if starts.iter().any(|s| t.starts_with(s)) || t == "-" || t == "*" || is_field(t) {
        return false;
    }
    if is_enumerator(t) {
        return false;
    }
    let indent = |l: &str| l.len() - l.trim_start().len();
    !stanza.get(1).is_some_and(|second| indent(second) > 0)
}

/// `:name: value`, a field list item.
fn is_field(t: &str) -> bool {
    let Some(rest) = t.strip_prefix(':') else { return false };
    rest.find(": ")
        .or_else(|| rest.strip_suffix(':').map(str::len))
        .is_some_and(|k| k > 0 && !rest[..k].starts_with(char::is_whitespace))
}

/// `1. `, `a) `, `(iv) `, `#. `: an enumerated list item.
fn is_enumerator(t: &str) -> bool {
    let body = t.strip_prefix('(').unwrap_or(t);
    let n = body.find(|c: char| !c.is_ascii_alphanumeric() && c != '#').unwrap_or(body.len());
    if n == 0 || n > 4 {
        return false;
    }
    let rest = &body[n..];
    let enumerator = &body[..n];
    let numeric = enumerator.chars().all(|c| c.is_ascii_digit()) || enumerator == "#";
    let single = enumerator.len() == 1;
    (numeric || single || enumerator.chars().all(|c| "ivxlcdmIVXLCDM".contains(c)))
        && (rest.starts_with(". ") || rest.starts_with(") "))
}

/// Whether inline markup starts and never ends (`*args` in prose), which makes
/// docutils report a problem and Sphinx keep every sentence.
fn has_open_markup(text: &str) -> bool {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if !(c == b'*' || c == b'`') || !starts_markup(b, i) {
            i += 1;
            continue;
        }
        let marker: &[u8] = if b.get(i + 1) == Some(&c) { &b[i..i + 2] } else { &b[i..=i] };
        let from = i + marker.len();
        match find_end(b, from, marker) {
            Some(end) => i = end,
            None => return true,
        }
    }
    false
}

/// Whether a start-string at `i` may start inline markup: after whitespace or
/// opening punctuation, before a character that is not whitespace.
fn starts_markup(b: &[u8], i: usize) -> bool {
    let before_ok = i == 0 || b[i - 1].is_ascii_whitespace() || b"'\"([{<-/:".contains(&b[i - 1]);
    let after = b.get(i + 1).copied();
    let after = if after == Some(b[i]) { b.get(i + 2).copied() } else { after };
    before_ok && after.is_some_and(|a| !a.is_ascii_whitespace())
}

/// The end of the end-string `marker` after `from`: one after a character that is not
/// whitespace, before whitespace, punctuation or the end.
fn find_end(b: &[u8], from: usize, marker: &[u8]) -> Option<usize> {
    let mut j = from + 1;
    while j + marker.len() <= b.len() {
        let matches = &b[j..j + marker.len()] == marker;
        let after = b.get(j + marker.len()).copied();
        let after_ok = after.is_none_or(|a| a.is_ascii_whitespace() || a.is_ascii_punctuation());
        if matches && !b[j - 1].is_ascii_whitespace() && after_ok {
            return Some(j + marker.len());
        }
        j += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &[&str]) -> Vec<String> {
        text.iter().map(|s| (*s).to_owned()).collect()
    }

    /// Sphinx's `test_extract_summary`.
    #[test]
    fn summaries_are_extracted_as_sphinx_extracts_them() {
        let doc = lines(&["", "This is a first sentence. And second one.", "", "Second block"]);
        assert_eq!(extract_summary(&doc), "This is a first sentence.");
        let doc = lines(&[
            "This sentence contains *emphasis text having dots.*,",
            "it does not break sentence.",
        ]);
        assert_eq!(extract_summary(&doc), doc.join(" "));
        for text in [
            "Blabla, i.e. bla.",
            "Blabla, (i.e. bla).",
            "Blabla, e.g. bla.",
            "Blabla, (e.g. bla).",
            "Blabla, et al. bla.",
        ] {
            assert_eq!(extract_summary(&lines(&[text])), text);
        }
        assert_eq!(extract_summary(&lines(&["blah blah::"])), "blah blah.");
        assert_eq!(extract_summary(&lines(&["blah blah", "========="])), "blah blah");
        assert_eq!(extract_summary(&lines(&["=========", "blah blah", "========="])), "blah blah");
        let doc = lines(&["Do `this <https://www.sphinx-doc.org/>`_ and that. blah blah blah."]);
        assert_eq!(extract_summary(&doc), "Do `this <https://www.sphinx-doc.org/>`_ and that.");
    }

    #[test]
    fn open_markup_keeps_every_sentence() {
        let doc = lines(&["Takes *args. And more."]);
        assert_eq!(extract_summary(&doc), "Takes *args. And more.");
        assert_eq!(extract_summary(&lines(&["- a list. Item."])), "- a list. Item.");
    }
}
