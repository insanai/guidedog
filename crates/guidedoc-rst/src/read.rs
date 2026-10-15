//! The reader's entry points: whole documents, inline spans, and registered sources.

use guidedoc::Arg;
use guidedoc::{
    Conformance, Fail, ReadContext, Reader, SourceId, SourceSpan, arg_span, diagnostic,
};

use crate::extension::Extension;
use crate::lines::{Line, split_lines, strip_bom};
use crate::messages::{CLASS_DANGLING, INLINE_SPAN, TRANSITION_END};
use crate::parser::{Parser, ReaderState};
use crate::settings::Settings;

/// The reader's version: the project version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How complete the reader is.
pub const CONFORMANCE: Conformance = Conformance::Partial;

/// Returns the "rst" reader descriptor, for a `guidedoc::Registry`.
pub const fn reader() -> Reader {
    Reader {
        id: "rst",
        name: "reStructuredText",
        version: VERSION,
        conformance: CONFORMANCE,
        read,
        read_inline: Some(read_inline),
    }
}

/// Returns the reader's settings: those `ReadOptions::extension` carries, or the
/// defaults. Settings of another type are reported and refused with `InvalidInput`.
fn settings_of(ctx: &mut ReadContext<'_>) -> Result<Settings, Fail> {
    Ok(ctx.reader_settings::<Settings>()?.cloned().unwrap_or_default())
}

/// Reads `ctx.source` as reStructuredText into `ctx.b` (`guidedoc::ReadFn`). Problems go
/// to the reports; it fails when the builder fails (a limit, capacity) or reference
/// resolution does.
pub fn read(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let settings = settings_of(ctx)?;
    read_with(ctx, &settings, None)
}

/// Reads `ctx.source` with explicit settings and a dialect's handlers (the Sphinx
/// layer's entry point). The extension's hooks run at their points of the read: begin,
/// prologue (after the leading field lines), end, then resolution with its default
/// substitutions.
pub fn read_with(
    ctx: &mut ReadContext<'_>,
    settings: &Settings,
    ext: Option<&dyn Extension>,
) -> Result<(), Fail> {
    let mut st = ReaderState::new(ctx, settings);
    let text = ctx.source_of(ctx.source);
    let (body, base) = strip_bom(&text);
    let mut buf = String::new();
    let lines = split_lines(body, base, st.tab, &mut buf);
    let mut p = st.parser(ctx, ext, None);
    p.ctx.b.spend(lines.len() as u64 / 256)?;
    if let Some(e) = ext {
        e.begin(&mut p)?;
    }
    // A dialect's prologue goes after the leading field lines, which then read on their
    // own, as Sphinx inserts rst_prolog into the text; without one the document reads
    // whole.
    let fields =
        if ext.is_some_and(|e| e.has_prologue()) { leading_field_lines(&lines) } else { 0 };
    if fields > 0 {
        p.parse_blocks(&lines[..fields], true, None)?;
    }
    if let Some(e) = ext {
        e.prologue(&mut p)?;
    }
    p.parse_blocks(&lines[fields..], true, None)?;
    p.embed_end()
}

/// Parses a span of `ctx.source` as inline content of the builder's open node, as
/// Sphinx parses a translated message (`guidedoc::ReadInlineFn`). Each line's leading
/// white space is dropped. References are left for the caller to resolve.
pub fn read_inline(ctx: &mut ReadContext<'_>, span: SourceSpan) -> Result<(), Fail> {
    let settings = settings_of(ctx)?;
    read_inline_with(ctx, span, &settings, None)
}

/// `read_inline` with explicit settings and a dialect's roles.
pub fn read_inline_with(
    ctx: &mut ReadContext<'_>,
    span: SourceSpan,
    settings: &Settings,
    ext: Option<&dyn Extension>,
) -> Result<(), Fail> {
    let text = ctx.source_of(span.source);
    let range = span.start as usize..span.end as usize;
    let Some(part) = text.get(range).filter(|_| span.source == ctx.source) else {
        return Err(ctx.b.fail(diagnostic(INLINE_SPAN, span, arg_span(span), Arg::None)));
    };
    let mut st = ReaderState::new(ctx, settings);
    let mut buf = String::new();
    let lines = split_lines(part, span.start, st.tab, &mut buf);
    let mut p = st.parser(ctx, ext, None);
    if let Some(e) = ext {
        e.begin(&mut p)?;
    }
    let tab = p.tab();
    let views: Vec<_> = lines
        .iter()
        .map(|l| {
            let v = l.view(tab);
            let skip = v.text.len() - v.text.trim_start_matches([' ', '\t']).len();
            guidedoc::LineView { text: &v.text[skip..], offset: v.offset + skip as u32 }
        })
        .filter(|v| !v.text.is_empty())
        .collect();
    if views.is_empty() {
        return Ok(());
    }
    p.inline_views(&views)
}

/// Counts the source's first lines that start with a field marker, as Sphinx counts
/// them to put rst_prolog after a document's bibliographic fields.
fn leading_field_lines(lines: &[Line<'_>]) -> usize {
    lines.iter().take_while(|l| field_marker_line(l.text)).count()
}

/// Matches Docutils' field_marker pattern at the start of a line: ":name:" followed by
/// spaces or the end, where the name neither starts with a colon or space nor ends with
/// a space, and holds no colon followed by a space, "`", or the end.
fn field_marker_line(t: &str) -> bool {
    let b = t.as_bytes();
    if b.len() < 3 || b[0] != b':' || b[1] == b':' || b[1] == b' ' {
        return false;
    }
    let mut i = 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            b':' => {
                let end = i + 1 == b.len() || b[i + 1] == b' ';
                if end && b[i - 1] != b' ' {
                    return true;
                }
                if end || b[i + 1] == b'`' {
                    return false;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// Where the parser stood before `enter_source`.
#[derive(Debug, Clone, Copy)]
pub struct SourceFrame {
    src: SourceId,
    text_len: u32,
    tab: usize,
}

impl Parser<'_, '_> {
    /// Closes sections and reports what the document left dangling.
    pub(crate) fn finish_document(&mut self) -> Result<(), Fail> {
        self.close_sections(1)?;
        if let Some(p) = self.st.pending.take() {
            self.report_at(CLASS_DANGLING, p.span);
        }
        if self.st.top_transition {
            let span = self.st.transition_span;
            self.report_at(TRANSITION_END, span);
        }
        Ok(())
    }

    /// Makes the lines of source `id` current, with tab stops `tab`, for spans and
    /// inline parsing; `leave_source` restores the previous source.
    pub fn enter_source(&mut self, id: SourceId, tab: usize) -> SourceFrame {
        let frame = SourceFrame { src: self.st.src, text_len: self.st.text_len, tab: self.st.tab };
        self.st.src = id;
        self.st.text_len = u32::try_from(self.ctx.source_of(id).len()).unwrap_or(u32::MAX);
        self.st.tab = tab.max(1);
        frame
    }

    /// Makes the source `enter_source` returned `frame` for current again.
    pub fn leave_source(&mut self, frame: SourceFrame) {
        self.st.src = frame.src;
        self.st.text_len = frame.text_len;
        self.st.tab = frame.tab;
    }

    /// Runs `f` with source `id` current (see `enter_source`).
    pub(crate) fn in_source<R>(
        &mut self,
        id: SourceId,
        tab: usize,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let frame = self.enter_source(id, tab);
        let r = f(self);
        self.leave_source(frame);
        r
    }

    /// Parses a whole registered source (`ReadContext::add_source`) as body content at
    /// the current position, as if its lines stood there. `sections` allows titles.
    pub fn parse_source(&mut self, id: SourceId, sections: bool) -> Result<(), Fail> {
        let text = self.ctx.source_of(id);
        let (body, base) = strip_bom(&text);
        let mut buf = String::new();
        let tab = self.st.tab;
        let lines = split_lines(body, base, tab, &mut buf);
        self.in_source(id, tab, |p| p.parse_blocks(&lines, sections, None))
    }
}

#[cfg(test)]
mod tests {
    use super::field_marker_line;

    // Field marker lines are counted as Docutils' field_marker pattern matches them,
    // which Sphinx relies on to put rst_prolog after a document's bibliographic fields.
    #[test]
    fn field_marker_lines_match_docutils() {
        for s in [":title: x", ":author:", ":a b: c", ":x\\: y: z", ":a:b: c", ":orphan:"] {
            assert!(field_marker_line(s), "{s:?} is a field marker");
        }
        for s in [":mod:`foo`", "::", ": x:", ":x :", "text", ":", " :title: x"] {
            assert!(!field_marker_line(s), "{s:?} is not a field marker");
        }
    }
}
