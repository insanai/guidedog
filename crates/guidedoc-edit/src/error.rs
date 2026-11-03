//! Why an edit was refused, as an Elm-style report.

use std::fmt;

use gd_report::Report;

use crate::Span;

/// A refused edit. No edit is ever half applied: the caller's text is unchanged.
///
/// The codes are stable: `edit.unknown-block`, `edit.unsupported`, `edit.invalid-text`,
/// `edit.unsafe`, `edit.truncated`, `edit.unknown-option`, `edit.option-value`,
/// `edit.missing-option` and `edit.renumber`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditError {
    /// A stable code such as `edit.unsafe`.
    pub code: &'static str,
    /// A short upper-case title.
    pub title: &'static str,
    /// The plain explanation.
    pub message: String,
    /// How to fix it.
    pub hint: String,
    /// The block or option the edit concerned, in the text the edit was asked of.
    pub span: Option<Span>,
}

impl EditError {
    /// Makes an error from its parts.
    pub(crate) fn new(
        code: &'static str,
        title: &'static str,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> EditError {
        EditError { code, title, message: message.into(), hint: hint.into(), span: None }
    }

    /// The same error located at `span`.
    pub(crate) fn at(mut self, span: Span) -> EditError {
        self.span = Some(span);
        self
    }

    /// The error as a report on the file `path` holding `text`, with an excerpt of the
    /// block when the error has a span inside `text`.
    pub fn report(&self, path: &str, text: &str) -> Report {
        let message = format!("{} No source edit was applied.", self.message);
        let report = Report::error(self.code, self.title, &message).hint(&self.hint);
        match self.span {
            Some(span) if span.end <= text.len() => report.at(path, text, span.start, span.end),
            _ => Report { path: path.to_string(), ..report },
        }
    }

    pub(crate) fn unknown_block() -> EditError {
        EditError::new(
            "edit.unknown-block",
            "NO SUCH BLOCK",
            "The edit names a block the document's map does not have.",
            "Map the current text again and choose the block from that map.",
        )
    }

    pub(crate) fn unsupported(what: &str, span: Span) -> EditError {
        EditError::new(
            "edit.unsupported",
            "NOT EDITABLE THIS WAY",
            format!("There is no tested edit for {what}."),
            "Edit this construct in the source.",
        )
        .at(span)
    }

    pub(crate) fn invalid_text(why: &str, span: Span) -> EditError {
        EditError::new(
            "edit.invalid-text",
            "TEXT DOES NOT FIT THE BLOCK",
            format!("The new text {why}."),
            "Keep the text within the block, or make structural changes in the source.",
        )
        .at(span)
    }

    pub(crate) fn truncated(span: Span, max_blocks: usize) -> EditError {
        EditError::new(
            "edit.truncated",
            "DOCUMENT TOO LONG TO CHECK",
            format!(
                "The map lists only the first {max_blocks} blocks, so the edit cannot be \
                 checked against the rest of the document."
            ),
            "Raise `MapOptions::max_blocks`, split the document, or edit it in the source.",
        )
        .at(span)
    }

    pub(crate) fn unsafe_edit(span: Span) -> EditError {
        EditError::new(
            "edit.unsafe",
            "EDIT WOULD CHANGE THE STRUCTURE",
            "Read again, the edited text would split, merge, or change the kind of blocks \
             the edit should leave alone.",
            "Check the new text's markup, or make structural changes in the source.",
        )
        .at(span)
    }
}

impl fmt::Display for EditError {
    /// Writes the title rule, the explanation and the hint, as Elm-style reports do.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rule = "-".repeat(70usize.saturating_sub(self.title.len() + 4));
        writeln!(f, "-- {} {rule}", self.title)?;
        writeln!(f)?;
        writeln!(f, "{} No source edit was applied.", self.message)?;
        writeln!(f)?;
        write!(f, "Hint: {}", self.hint)
    }
}

impl std::error::Error for EditError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_carry_the_location_and_hint() {
        let text = "Title\n=====\n\nPara.\n";
        let error = EditError::unsafe_edit(Span::new(13, 18));
        let report = error.report("intro.rst", text);
        assert_eq!((report.line, report.column), (4, 1));
        assert_eq!(report.code, "edit.unsafe");
        let shown = error.to_string();
        assert!(shown.starts_with("-- EDIT WOULD CHANGE THE STRUCTURE ---"), "{shown}");
        assert!(shown.contains("Hint: Check"), "{shown}");
    }
}
