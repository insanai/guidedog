//! Line breaks in inline code and tabs in inline text.

use guidedoc::{Fail, NodeId, NodeKind, SourceSpan};

use super::Resolver;

impl Resolver<'_, '_> {
    /// Inline literals, code, math, and raw text keep their line breaks as the scanner
    /// reads them. LaTeX needs them in math, since "%" comments end at a line break,
    /// and raw formats and parsed literal blocks may too. Inline code elsewhere shows
    /// one line, so its breaks become spaces here, after parsing.
    pub fn unbreak_code(&mut self) {
        let mut kept = 0; // inside a Preformatted node up to here.
        for i in 1..self.used() {
            let id = NodeId::from_index(i);
            let kind = self.kind_of(id);
            if kind == NodeKind::Preformatted {
                kept = kept.max(self.node_end(id));
            }
            if kind != NodeKind::Code || i < kept {
                continue;
            }
            let t = self.ctx.b.snap.nodes[id].text;
            if let Some(bytes) = self.ctx.b.snap.text.get_mut(t.range()) {
                bytes.iter_mut().filter(|c| **c == b'\n').for_each(|c| *c = b' ');
            }
        }
    }

    /// Replaces the tabs of inline text with spaces to the next tab stop, as the block
    /// reader expands tabs in lines. The inline parser reads source bytes, so node
    /// spans stay exact; the stop is found from the column of the node's source line.
    pub fn expand_tabs(&mut self) -> Result<(), Fail> {
        let tab = if self.options.tab_width > 0 { self.options.tab_width } else { 8 };
        for i in 1..self.used() {
            let id = NodeId::from_index(i);
            let kind = self.kind_of(id);
            if !matches!(
                kind,
                NodeKind::Text | NodeKind::Code | NodeKind::Math | NodeKind::RawInline
            ) || !self.text_of(id).contains('\t')
            {
                continue;
            }
            let mut sp = self.node_span(id);
            if kind != NodeKind::Text {
                sp.start = self.content_start(sp);
            }
            let mut column = self.source_column(sp, tab);
            let text = self.text_of(id).to_owned();
            let b = &mut self.ctx.b;
            let start = b.text_open();
            for &c in text.as_bytes() {
                match c {
                    b'\t' => {
                        let n = tab - column % tab;
                        for _ in 0..n {
                            b.text_push_byte(b' ')?;
                        }
                        column += n;
                    }
                    b'\n' => {
                        b.text_push_byte(c)?;
                        column = 0;
                    }
                    _ => {
                        b.text_push_byte(c)?;
                        if !(0x80..0xc0).contains(&c) {
                            column += 1;
                        }
                    }
                }
            }
            let t = b.text_close(start);
            b.set_text(id, t);
        }
        Ok(())
    }

    /// The display column of a span's start in its source line, tabs expanded.
    fn source_column(&self, sp: SourceSpan, tab: usize) -> usize {
        let source = self.ctx.source_of(sp.source);
        let bytes = source.as_bytes();
        let at = (sp.start as usize).min(bytes.len());
        let line =
            bytes[..at].iter().rposition(|&c| c == b'\n' || c == b'\r').map_or(0, |k| k + 1);
        bytes[line..at].iter().fold(0, |column, &c| match c {
            b'\t' => column + tab - column % tab,
            0x80..0xc0 => column,
            _ => column + 1,
        })
    }

    /// Where the content of interpreted text or an inline literal starts: after the run
    /// of backquotes that opens it.
    fn content_start(&self, sp: SourceSpan) -> u32 {
        let source = self.ctx.source_of(sp.source);
        let bytes = source.as_bytes();
        let end = (sp.end as usize).min(bytes.len());
        let mut k = sp.start as usize;
        while k < end && bytes[k] != b'`' {
            k += 1;
        }
        while k < end && bytes[k] == b'`' {
            k += 1;
        }
        if k < end { k as u32 } else { sp.start }
    }
}
