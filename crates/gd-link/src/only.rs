//! The `only` directive's expressions (link.odin's evaluate_only): tags joined by
//! `and`, `or`, `not` and parentheses.

use gd_env::{BuilderKind, OutputFormat};

/// The tags an `only` expression is evaluated against: the builder's name, the format
/// of the page (`html`, `pdf` or `latex`, `text`), and the `-t` tags.
#[derive(Debug, Clone, Copy)]
pub struct Tags<'a> {
    /// The builder writing.
    pub kind: BuilderKind,
    /// The format the page is linked for.
    pub format: OutputFormat,
    /// The tags given with `-t`.
    pub tags: &'a [String],
}

impl Tags<'_> {
    /// Whether `tag` is set.
    pub fn has(&self, tag: &str) -> bool {
        if tag == self.kind.name() {
            return true;
        }
        match tag {
            "html" | "format_html" => return self.format == OutputFormat::Html,
            "pdf" | "latex" | "format_latex" | "format_pdf" => {
                return self.format == OutputFormat::Pdf;
            }
            "text" | "format_text" => return self.format == OutputFormat::Text,
            "builder_html" => return self.kind == BuilderKind::Html,
            _ => {}
        }
        self.tags.iter().any(|t| t == tag)
    }
}

/// Evaluates an `only` expression. Words other than `and`, `or`, `not` and
/// parentheses are tags; a missing operand is false and a missing `)` is forgiven.
pub fn evaluate_only(expression: &str, tags: &Tags<'_>) -> bool {
    let spaced = expression.replace('(', " ( ").replace(')', " ) ");
    let words: Vec<&str> = spaced.split_whitespace().collect();
    let mut parser = Parser { words: &words, at: 0, tags };
    parser.or()
}

struct Parser<'w, 't> {
    words: &'w [&'w str],
    at: usize,
    tags: &'t Tags<'t>,
}

impl<'w> Parser<'w, '_> {
    fn peek(&self) -> Option<&'w str> {
        self.words.get(self.at).copied()
    }

    fn or(&mut self) -> bool {
        let mut value = self.and();
        while self.peek() == Some("or") {
            self.at += 1;
            let right = self.and();
            value = value || right;
        }
        value
    }

    fn and(&mut self) -> bool {
        let mut value = self.not();
        while self.peek() == Some("and") {
            self.at += 1;
            let right = self.not();
            value = value && right;
        }
        value
    }

    fn not(&mut self) -> bool {
        let Some(word) = self.peek() else { return false };
        self.at += 1;
        match word {
            "not" => !self.not(),
            "(" => {
                let value = self.or();
                if self.peek() == Some(")") {
                    self.at += 1;
                }
                value
            }
            tag => self.tags.has(tag),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_expressions_follow_sphinx() {
        let extra = vec!["draft".to_string()];
        let html = Tags { kind: BuilderKind::Html, format: OutputFormat::Html, tags: &extra };
        assert!(evaluate_only("html", &html));
        assert!(evaluate_only("builder_html and format_html", &html));
        assert!(!evaluate_only("latex or text", &html));
        assert!(evaluate_only("not (latex or text) and draft", &html));
        assert!(!evaluate_only("", &html));
        let pdf = Tags { kind: BuilderKind::Pdf, format: OutputFormat::Pdf, tags: &[] };
        assert!(evaluate_only("latex", &pdf));
        assert!(evaluate_only("pdf and not html", &pdf));
        assert!(!evaluate_only("draft", &pdf));
    }
}
