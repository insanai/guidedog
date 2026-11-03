//! The text between mapped blocks. A directive that leaves no node of its own (such as
//! `highlight` or an `include` read without its file) still occupies source lines; we
//! cut such text into chunks the way reStructuredText cuts blocks and give each chunk a
//! block, so a map covers every non-blank byte of its region.

use crate::Span;
use crate::lines::{indent_of, line_end_trimmed, line_start, next_line};
use crate::map::BlockKind;
use crate::parts::{self, Ctx};
use crate::walk::{Draft, code_lines};

/// Inserts a block for every chunk of text in `region` that no block of `blocks`
/// covers. `blocks` are sorted and do not overlap.
pub(crate) fn fill(ctx: &Ctx<'_>, blocks: &mut Vec<Draft>, region: Span, quotes: usize) {
    let mut out = Vec::with_capacity(blocks.len());
    let mut cursor = region.start;
    for block in blocks.drain(..) {
        if block.span.start > cursor {
            out.extend(chunks(ctx, Span::new(cursor, block.span.start), quotes));
        }
        cursor = cursor.max(block.span.end);
        out.push(block);
    }
    if region.end > cursor {
        out.extend(chunks(ctx, Span::new(cursor, region.end), quotes));
    }
    *blocks = out;
}

/// The chunks of `span`: a chunk starts at a non-blank line and takes the lines that
/// follow it without a blank line between, and after a blank line those indented
/// deeper than its first line. A chunk that is a directive becomes a directive block,
/// any other a source block.
pub(crate) fn chunks(ctx: &Ctx<'_>, span: Span, quotes: usize) -> Vec<Draft> {
    let text = ctx.text;
    let mut out = Vec::new();
    let mut current: Option<(Span, usize)> = None;
    let mut after_blank = false;
    let mut at = span.start;
    while at < span.end {
        let end = line_end_trimmed(text, at).min(span.end);
        let line = &text[at..end];
        let content = content_start(line, quotes);
        match (content, current) {
            (None, _) => after_blank = true,
            (Some(c), Some((chunk, indent))) if !after_blank || c > indent => {
                current = Some((Span::new(chunk.start, end), indent));
                after_blank = false;
            }
            (Some(c), _) => {
                out.extend(current.map(|(chunk, _)| chunk));
                let column = at - line_start(text, at) + c;
                current = Some((Span::new(at + c, end), column));
                after_blank = false;
            }
        }
        let next = next_line(text, at);
        if next == at {
            break;
        }
        at = next;
    }
    out.extend(current.map(|(chunk, _)| chunk));
    out.into_iter().map(|chunk| block(ctx, chunk, quotes)).collect()
}

/// The offset of the first character of `line` after white space and `quotes` quote
/// markers; `None` for a line that holds nothing else.
fn content_start(line: &str, quotes: usize) -> Option<usize> {
    let mut at = 0;
    let mut quotes = quotes;
    loop {
        at += indent_of(&line[at..]);
        if quotes > 0 && line[at..].starts_with('>') {
            at += 1;
            quotes -= 1;
        } else {
            break;
        }
    }
    (at < line.len()).then_some(at)
}

fn block(ctx: &Ctx<'_>, chunk: Span, quotes: usize) -> Draft {
    match parts::directive(ctx, chunk, quotes) {
        Some(parts) => {
            let content =
                parts.content.map(|c| code_lines(ctx.text, c, quotes)).unwrap_or_default();
            Draft { content, directive: Some(parts), ..Draft::leaf(BlockKind::Directive, chunk) }
        }
        None => Draft::leaf(BlockKind::Source, chunk),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Format;

    #[test]
    fn chunks_follow_indentation() {
        let text = ".. highlight:: python\n\n.. include:: x.rst\n   :literal:\n\n   indented\n\n\
                    odd\nmore\n\nlast";
        let config = guidedoc_sphinx::Config::default();
        let ctx = Ctx { text, format: Format::Rst, config: &config };
        let found = chunks(&ctx, Span::new(0, text.len()), 0);
        let texts: Vec<&str> = found.iter().map(|d| d.span.text(text)).collect();
        assert_eq!(
            texts,
            [
                ".. highlight:: python",
                ".. include:: x.rst\n   :literal:\n\n   indented",
                "odd\nmore",
                "last"
            ]
        );
        assert_eq!(found[0].kind, BlockKind::Directive);
        assert_eq!(found[2].kind, BlockKind::Source);
    }
}
