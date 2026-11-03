//! New list items: the marker of the anchor item, the item's lines at its indentation,
//! and the renumbering an explicitly numbered RST list needs.

use crate::Span;
use crate::edit::{Position, TextEdit};
use crate::error::EditError;
use crate::lines::{continuation, line_end_trimmed, line_start};
use crate::map::{BlockId, SourceMap};

/// The marker of the item at `start`: the token up to the first space, and the token
/// with the spaces after it. An empty item's marker may end its line or the text.
fn marker(text: &str, start: usize) -> (Span, Span) {
    let end = line_end_trimmed(text, start);
    let token = text[start..end].find([' ', '\t']).map_or(end, |i| start + i);
    let body = end - text[token..end].trim_start().len();
    let body = if body == end { (token + 1).min(end) } else { body };
    (Span::new(start, token), Span::new(start, body.max(token)))
}

/// The number of an explicitly numbered RST item marker, such as `3.`, `3)` or `(3)`,
/// and the span of its digits; `None` for bullets and `#.`.
fn number(text: &str, token: Span) -> Option<(u64, Span)> {
    let t = token.text(text);
    let lead = usize::from(t.starts_with('('));
    let digits =
        t[lead..].len() - t[lead..].trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let span = Span::new(token.start + lead, token.start + lead + digits);
    span.text(text).parse().ok().map(|n| (n, span))
}

/// Tells whether an RST marker is enumerated with letters or Roman numerals, which we
/// do not renumber.
fn lettered(text: &str, token: Span) -> bool {
    let t = token.text(text).trim_start_matches('(');
    let body = t.trim_end_matches(['.', ')']);
    !body.is_empty() && body != "#" && body.chars().all(|c| c.is_ascii_alphabetic())
}

/// The edits that insert an item with text `new` next to item `anchor`, and renumber
/// the items after it in an explicitly numbered RST list.
pub(crate) fn item_edits(
    text: &str,
    map: &SourceMap,
    anchor: BlockId,
    position: Position,
    new: &str,
    markdown: bool,
) -> Result<Vec<TextEdit>, EditError> {
    let item = &map.blocks[anchor.index()];
    let siblings = map.siblings(anchor);
    let index = siblings.iter().position(|&s| s == anchor).unwrap_or(0);
    let line = line_start(text, item.span.start);
    let prefix = &text[line..item.span.start];
    let (token, with_space) = marker(text, item.span.start);
    // An empty item has no space after its marker; the new item takes one.
    let spaces = match &text[token.end..with_space.end] {
        "" => " ",
        spaces => spaces,
    };
    let renumber = !markdown && number(text, token).is_some();
    if !markdown && !renumber && lettered(text, token) {
        return Err(renumber_error(item.span));
    }
    let mut mark = token.text(text).to_string();
    if renumber && position == Position::After {
        mark = renumbered(text, token, 1).ok_or_else(|| renumber_error(item.span))?;
    }
    let indent = continuation(&format!("{prefix}{mark}{spaces}"), markdown);
    let body = format!("{mark}{spaces}{}", new.replace('\n', &format!("\n{indent}")));
    let loose = is_loose(text, map, siblings);
    let sep = if loose { format!("\n{}\n", prefix.trim_end()) } else { "\n".to_string() };
    let mut edits = vec![match position {
        Position::Before => TextEdit {
            start: item.span.start,
            end: item.span.start,
            insert: format!("{body}{sep}{prefix}"),
        },
        Position::After => TextEdit {
            start: item.span.end,
            end: item.span.end,
            insert: format!("{sep}{prefix}{body}"),
        },
    }];
    if renumber {
        let first = if position == Position::Before { index } else { index + 1 };
        for &later in &siblings[first.min(siblings.len())..] {
            let span = map.blocks[later.index()].span;
            let (token, _) = marker(text, span.start);
            let digits =
                number(text, token).map(|(_, d)| d).ok_or_else(|| renumber_error(span))?;
            let next = renumbered(text, token, 1).ok_or_else(|| renumber_error(span))?;
            let (_, new_digits) =
                number(&next, Span::new(0, next.len())).ok_or_else(|| renumber_error(span))?;
            let insert = new_digits.text(&next).to_string();
            edits.push(TextEdit { start: digits.start, end: digits.end, insert });
        }
    }
    edits.sort_by_key(|e| e.start);
    Ok(edits)
}

/// The marker `token` with its number raised by `by`; `None` when the number would
/// change width, which would move the item's text.
fn renumbered(text: &str, token: Span, by: u64) -> Option<String> {
    let (n, digits) = number(text, token)?;
    let raised = (n + by).to_string();
    if raised.len() != digits.len() {
        return None;
    }
    let t = token.text(text);
    let at = digits.start - token.start;
    Some(format!("{}{raised}{}", &t[..at], &t[at + digits.len()..]))
}

/// Tells whether a list separates its items with blank lines.
fn is_loose(text: &str, map: &SourceMap, items: &[BlockId]) -> bool {
    items.windows(2).any(|pair| {
        let (a, b) = (map.blocks[pair[0].index()].span, map.blocks[pair[1].index()].span);
        text[a.end..b.start].matches('\n').count() > 1
    })
}

fn renumber_error(span: Span) -> EditError {
    EditError::new(
        "edit.renumber",
        "CANNOT RENUMBER THE LIST",
        "The list is numbered explicitly, and the new numbering would change the width \
         of a marker or uses letters or Roman numerals.",
        "Number the list with `#.` so that items number themselves, or edit the source.",
    )
    .at(span)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_and_numbers() {
        let text = "(12) item";
        let (token, body) = marker(text, 0);
        assert_eq!((token.text(text), body.text(text)), ("(12)", "(12) "));
        assert_eq!(number(text, token).map(|(n, _)| n), Some(12));
        assert_eq!(renumbered(text, token, 1).as_deref(), Some("(13)"));
        assert_eq!(renumbered("9. x", Span::new(0, 2), 1), None);
        assert!(lettered("a. x", Span::new(0, 2)));
        assert!(!lettered("#. x", Span::new(0, 2)));
    }
}
