//! A document's words for the search index, collected by section while the collector
//! walks it and kept beside its object (`search/<docname>.<digest16>.txt`): one line per
//! section, `anchor\ttitle\ttext`. gd-site joins every document's lines into the index.

use gd_env::SectionInfo;

/// A document's words by section: the first part holds the words before any section,
/// then one part per section, in the order the sections were found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SearchText {
    parts: Vec<String>,
}

impl SearchText {
    /// Adds a text node's words to `section` (`None`: before any section), with tabs and
    /// line breaks turned into spaces so a part stays on one line.
    pub(crate) fn add(&mut self, section: Option<u32>, text: &str) {
        let index = section.map_or(0, |s| s as usize + 1);
        if self.parts.len() <= index {
            self.parts.resize_with(index + 1, String::new);
        }
        let part = &mut self.parts[index];
        part.extend(text.chars().map(|c| if matches!(c, '\t' | '\n' | '\r') { ' ' } else { c }));
        part.push(' ');
    }

    /// The search file's lines: one per part that has words, with the section's anchor
    /// and title (tabs in the title become spaces), or empty ones before any section.
    pub(crate) fn lines(&self, sections: &[SectionInfo]) -> String {
        let mut out = String::new();
        for (i, part) in self.parts.iter().enumerate() {
            let text = part.trim();
            if text.is_empty() {
                continue;
            }
            let section = i.checked_sub(1).and_then(|s| sections.get(s));
            let (anchor, title) = section.map_or(("", ""), |s| (&s.anchor[..], &s.title[..]));
            out.push_str(anchor);
            out.push('\t');
            out.push_str(&flat(title));
            out.push('\t');
            out.push_str(text);
            out.push('\n');
        }
        out
    }
}

/// `s` with its tabs turned into spaces, so it fits one field of a search line.
fn flat(s: &str) -> String {
    s.replace('\t', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_kept_by_section_on_one_line_each() {
        let mut s = SearchText::default();
        s.add(None, "Before\tall");
        s.add(Some(0), "First\nsection");
        s.add(Some(0), "more");
        s.add(Some(2), "Third");
        let sections = [
            SectionInfo { anchor: "a".into(), title: "One\tTitle".into(), ..Default::default() },
            SectionInfo { anchor: "b".into(), title: "Two".into(), ..Default::default() },
            SectionInfo { anchor: "c".into(), title: "Three".into(), ..Default::default() },
        ];
        assert_eq!(
            s.lines(&sections),
            "\t\tBefore all\na\tOne Title\tFirst section more\nc\tThree\tThird\n"
        );
    }
}
