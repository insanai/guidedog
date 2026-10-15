//! Emitting text: escapes processed ("cooked"), restored ("raw"), and names.

use guidedoc::{Builder, Fail, NodeKind, SourceSpan, Text, normalize_label};

use crate::chars::is_space;
use crate::scan::Scanner;

/// What an unescaped line break becomes in emitted text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BreakMode {
    /// A SoftBreak node.
    Breaks,
    /// A space (literals and roles).
    Spaces,
}

/// Fails with the builder's failure when it has failed.
pub(crate) fn check(b: &mut Builder<'_>) -> Result<(), Fail> {
    b.spend(0)
}

impl Scanner<'_, '_> {
    /// Appends `[a, b)` to the text pool with escapes processed: an escaped space or
    /// line break disappears with its backslash, any other escaped character stays as
    /// itself, and an unescaped line break becomes a space.
    pub fn push_cooked(&mut self, a: usize, b: usize) -> Result<(), Fail> {
        let mut run = a;
        let mut i = a;
        while i < b {
            match self.text.as_bytes()[i] {
                0 => {
                    self.ctx.b.text_push_bytes(&self.text.as_bytes()[run..i])?;
                    i += 1;
                    if i < b && matches!(self.text.as_bytes()[i], b' ' | b'\n') {
                        i += 1;
                    }
                    run = i;
                }
                b'\n' => {
                    self.ctx.b.text_push_bytes(&self.text.as_bytes()[run..i])?;
                    self.ctx.b.text_push_byte(b' ')?;
                    i += 1;
                    run = i;
                }
                _ => i += 1,
            }
        }
        if run < b {
            self.ctx.b.text_push_bytes(&self.text.as_bytes()[run..b])?;
        }
        Ok(())
    }

    /// Appends `[a, b)` with backslashes restored, as inline literals, math, code, and
    /// raw roles keep them; line breaks stay (inline code loses them on resolution).
    pub fn push_raw(&mut self, a: usize, b: usize) -> Result<(), Fail> {
        let mut run = a;
        for i in a..b {
            let c = self.text.as_bytes()[i];
            if c != 0 && c != b'\n' {
                continue;
            }
            self.ctx.b.text_push_bytes(&self.text.as_bytes()[run..i])?;
            self.ctx.b.text_push_byte(if c == 0 { b'\\' } else { b'\n' })?;
            run = i + 1;
        }
        if run < b {
            self.ctx.b.text_push_bytes(&self.text.as_bytes()[run..b])?;
        }
        Ok(())
    }

    /// Closes the text begun at `start` and adds it as a Text node over `[a, b)`.
    pub fn text_leaf(&mut self, start: u32, a: usize, b: usize) -> Result<(), Fail> {
        let t = self.ctx.b.text_close(start);
        if t.len > 0 {
            let span = self.span(a, b);
            self.ctx.b.add_leaf(NodeKind::Text, span, t)?;
        }
        Ok(())
    }

    /// Emits `[a, b)` as Text nodes; see `BreakMode` for line breaks.
    pub fn emit_cooked(&mut self, a: usize, b: usize, mode: BreakMode) -> Result<(), Fail> {
        let mut seg = a;
        if mode == BreakMode::Breaks {
            for i in a..b {
                let bytes = self.text.as_bytes();
                if bytes[i] != b'\n' || (i > a && bytes[i - 1] == 0) {
                    continue;
                }
                let start = self.ctx.b.text_open();
                self.push_cooked(seg, i)?;
                self.text_leaf(start, seg, i)?;
                let at = self.offset(i + 1);
                let source = self.state.source;
                self.ctx.b.add_leaf(
                    NodeKind::SoftBreak,
                    SourceSpan::new(source, at, at),
                    Text::default(),
                )?;
                seg = i + 1;
            }
        }
        let start = self.ctx.b.text_open();
        self.push_cooked(seg, b)?;
        self.text_leaf(start, seg, b)
    }

    /// Writes the processed text of `[a, b)` to `self.temp`, replacing what it held.
    pub fn cook_temp(&mut self, a: usize, b: usize) {
        self.temp.clear();
        cook_into(&mut self.temp, &self.text.as_bytes()[a..b]);
    }

    /// Stores the reference name of `[a, b)`: escapes processed, white space
    /// collapsed, and, when `fold` is set, case folded.
    pub fn name_text(&mut self, a: usize, b: usize, fold: bool) -> Result<Text, Fail> {
        let mut temp = std::mem::take(&mut self.temp);
        temp.clear();
        cook_into(&mut temp, &self.text.as_bytes()[a..b]);
        let t = pool_name(&mut self.ctx.b, &temp, fold);
        self.temp = temp;
        t
    }
}

/// Appends marked bytes with escapes processed, line breaks as spaces.
fn cook_into(out: &mut String, marked: &[u8]) {
    let mut run = 0;
    let mut i = 0;
    while i < marked.len() {
        let c = marked[i];
        if c != 0 && c != b'\n' {
            i += 1;
            continue;
        }
        out.push_str(&String::from_utf8_lossy(&marked[run..i]));
        i += 1;
        if c == b'\n' {
            out.push(' ');
        } else if i < marked.len() && matches!(marked[i], b' ' | b'\n') {
            i += 1;
        }
        run = i;
    }
    out.push_str(&String::from_utf8_lossy(&marked[run..]));
}

/// Writes a normalized name straight into the text pool.
pub(crate) fn pool_name(b: &mut Builder<'_>, s: &str, fold: bool) -> Result<Text, Fail> {
    let start = b.text_open();
    if fold {
        normalize_label(b, s);
    } else {
        collapse_space(b, s);
    }
    check(b)?;
    Ok(b.text_close(start))
}

/// Joins the white-space separated words of `s` with single spaces.
pub(crate) fn collapse_space(out: &mut impl guidedoc::TextSink, s: &str) {
    let mut space = false;
    let mut wrote = false;
    for r in s.chars() {
        if is_space(r) {
            space = wrote;
            continue;
        }
        if space {
            out.put_char(' ');
        }
        space = false;
        out.put_char(r);
        wrote = true;
    }
}
