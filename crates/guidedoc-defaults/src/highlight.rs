//! Highlighters that join `gd-highlight` to Guidedoc's renderers.
//!
//! `gd-highlight` knows nothing about the engine, and the engine knows nothing about
//! lexers; these two small adapters implement [`guidedoc::Highlighter`] on top of the
//! highlighter's string writers.

use std::cell::RefCell;

use gd_highlight::{Lexer, find_lexer, resolve_lexer, write_code_html, write_code_typst};
use guidedoc::{Emitter, Highlighter};

/// Highlights code blocks as the HTML Pygments writes for Sphinx, with its short CSS
/// classes.
///
/// The language resolves as Sphinx resolves it: `default` is Python (or a Python console
/// session), falling back to plain text when the code does not lex as Python; `guess`
/// guesses; an unknown language is plain text, escaped as Pygments escapes it.
///
/// One value keeps a reusable buffer, so a renderer that highlights many blocks does not
/// allocate per block. It is not `Sync`: give each rendering thread its own.
#[derive(Debug, Default)]
pub struct HtmlHighlighter {
    buffer: RefCell<String>,
}

impl HtmlHighlighter {
    /// Creates a highlighter with an empty buffer.
    pub fn new() -> HtmlHighlighter {
        HtmlHighlighter::default()
    }
}

impl Highlighter for HtmlHighlighter {
    fn highlight(&self, language: &str, code: &str, out: &mut Emitter<'_>) -> bool {
        let lexer = resolve_lexer(find_lexer(language).unwrap_or(Lexer::Text), code);
        let mut buffer = self.buffer.borrow_mut();
        buffer.clear();
        write_code_html(lexer, code, &mut buffer);
        out.emit(&buffer);
        true
    }
}

/// Highlights code blocks as Typst data for the book template's `gd-highlight` function,
/// so books draw code with Guidedog's colours rather than Typst's.
///
/// It declines (returns `false`) plain text and languages it does not know, so the
/// renderer keeps Typst's own raw block for those. Like [`HtmlHighlighter`], one value
/// reuses its buffer and belongs to one thread.
#[derive(Debug, Default)]
pub struct TypstHighlighter {
    buffer: RefCell<String>,
}

impl TypstHighlighter {
    /// Creates a highlighter with an empty buffer.
    pub fn new() -> TypstHighlighter {
        TypstHighlighter::default()
    }
}

impl Highlighter for TypstHighlighter {
    fn highlight(&self, language: &str, code: &str, out: &mut Emitter<'_>) -> bool {
        let Some(lexer) = find_lexer(language) else { return false };
        let lexer = resolve_lexer(lexer, code);
        if lexer == Lexer::Text {
            return false;
        }
        let mut buffer = self.buffer.borrow_mut();
        buffer.clear();
        write_code_typst(lexer, code, &mut buffer);
        out.emit(&buffer);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(h: &dyn Highlighter, language: &str, code: &str) -> Option<String> {
        let mut bytes = Vec::new();
        let mut emitter = Emitter::new(&mut bytes);
        let accepted = h.highlight(language, code, &mut emitter);
        accepted.then(|| String::from_utf8(bytes).expect("highlighters write UTF-8"))
    }

    #[test]
    fn html_uses_pygments_classes_and_reuses_its_buffer() {
        let h = HtmlHighlighter::new();
        let first = render(&h, "python", "def f(): pass\n").expect("html always accepts");
        assert!(first.contains("<span class=\"k\">def</span>"), "{first}");
        let second = render(&h, "python", "x = 1\n").expect("html always accepts");
        assert!(!second.contains("def"), "the buffer is cleared between blocks: {second}");
    }

    #[test]
    fn html_escapes_unknown_languages_as_text() {
        let html = render(&HtmlHighlighter::new(), "no-such-language", "a < b\n");
        assert_eq!(html.as_deref().map(|s| s.contains("a &lt; b")), Some(true));
    }

    #[test]
    fn typst_declines_plain_text_and_unknown_languages() {
        let h = TypstHighlighter::new();
        assert!(render(&h, "text", "plain\n").is_none());
        assert!(render(&h, "no-such-language", "x\n").is_none());
        assert!(render(&h, "python", "def f(): pass\n").is_some());
    }
}
