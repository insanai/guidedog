//! code-block, sourcecode, and code with Sphinx's options, and the line specifications
//! (`:emphasize-lines:`, literalinclude's `:lines:`) they share.

use guidedoc::{AttributeKey, Fail, NodeId, NodeKind};
use guidedoc_rst::{Directive, Line, Parser, dedent};

use crate::directives::{class_attribute, parse_signed, value_attribute};
use crate::extension::Sphinx;
use crate::messages::{DEDENT_TEXT, LINES_SPEC};
use crate::state::State;
use crate::text::{all_digits, parse_int, trim_space};

/// Walks a Sphinx line specification such as "1,3-5,8-": comma-separated line numbers
/// and ranges, open at either end. Items are 0-based half-open ranges; `Err(())` marks
/// a malformed part.
pub(crate) fn line_ranges(
    spec: &str,
    total: usize,
) -> impl Iterator<Item = Result<(usize, usize), ()>> + '_ {
    spec.split(',').map(move |part| line_range(trim_space(part), total))
}

/// One part of a line specification.
fn line_range(part: &str, total: usize) -> Result<(usize, usize), ()> {
    let number = |s: &str| -> Result<usize, ()> {
        if s.len() > 9 || !all_digits(s) {
            return Err(());
        }
        usize::try_from(parse_int(s)).map_err(|_| ())
    };
    let Some((left, right)) = part.split_once('-') else {
        let n = number(part)?;
        return Ok((n.saturating_sub(1), n));
    };
    if right.contains('-') || (left.is_empty() && right.is_empty()) {
        return Err(());
    }
    let start = if left.is_empty() { 1 } else { number(left)? };
    let end = if right.is_empty() { start.max(total) } else { number(right)? };
    if start > end {
        return Err(());
    }
    Ok((start.saturating_sub(1), end))
}

/// Validates a specification against a number of lines: `None` for a malformed one;
/// `Some(true)` when a line lies past the end.
pub(crate) fn check_line_spec(spec: &str, total: usize) -> Option<bool> {
    let mut beyond = false;
    for r in line_ranges(spec, total) {
        let (a, b) = r.ok()?;
        if b > total || a >= total {
            beyond = true;
        }
    }
    Some(beyond)
}

/// code-block, sourcecode, and code.
pub(crate) fn code_directive(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
) -> Result<(), Fail> {
    let mut lines = d.content.clone();
    if let Some(v) = d.option_value("dedent") {
        dedent_code(p, d, &mut lines, &v);
    }
    if let Some(v) = d.option_value("emphasize-lines") {
        let span = d.option("emphasize-lines").map_or(d.span, |o| o.span);
        match check_line_spec(&v, lines.len()) {
            None => {
                p.report(LINES_SPEC, span, guidedoc::arg_span(span), guidedoc::Arg::None);
                return Ok(());
            }
            Some(true) => {
                p.report(LINES_SPEC, span, guidedoc::arg_span(span), guidedoc::Arg::None)
            }
            Some(false) => {}
        }
    }
    let id = p.open(NodeKind::CodeBlock, d.span)?;
    let arg = d.argument();
    let language =
        if d.arg.is_empty() { sx.state().highlight.clone() } else { trim_space(&arg).to_owned() };
    if !language.is_empty() {
        p.ctx.b.add_attribute_string(id, AttributeKey::Language, &language)?;
    }
    class_attribute(p, id, d)?;
    code_numbers(p, &sx.state(), id, d, lines.len(), 1)?;
    code_common(p, id, d)?;
    value_attribute(p, id, d, "caption")?;
    caption_child(p, d, &[])?;
    let text = p.code_text(&lines)?;
    p.ctx.b.set_text(id, text);
    p.close(id)
}

/// Sets line numbering: :linenos:, :lineno-start:, Docutils' :number-lines:, or the
/// highlight directive's line-count threshold.
pub(crate) fn code_numbers(
    p: &mut Parser<'_, '_>,
    st: &State,
    id: NodeId,
    d: &Directive<'_>,
    count: usize,
    match_start: i64,
) -> Result<(), Fail> {
    let mut start = i64::from(d.has_option("linenos"));
    if let Some(v) = d.option_value("lineno-start") {
        start = parse_signed(&v).max(1);
    }
    if let Some(v) = d.option_value("number-lines") {
        start = if v.is_empty() { 1 } else { parse_int(&v).max(1) };
    }
    if d.has_option("lineno-match") {
        start = match_start.max(1);
    }
    let explicit =
        d.has_option("linenos") || d.has_option("lineno-start") || d.has_option("lineno-match");
    if let Some(threshold) = st.linenothreshold
        && !explicit
        && start == 0
        && threshold > 0
        && count as i64 >= threshold
    {
        start = 1;
    }
    if start == 0 {
        return Ok(());
    }
    p.ctx.b.set_value(id, i32::try_from(start).unwrap_or(i32::MAX));
    p.custom(id, "linenos", "1")
}

/// Parses a :caption: as Sphinx's container_wrapper (and graphviz's figure_wrapper)
/// does: its inline content becomes the Caption child of the block, after the block's
/// attributes. An empty caption parses `fallback` (literalinclude's file name).
pub(crate) fn caption_child(
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
    fallback: &[Line<'_>],
) -> Result<(), Fail> {
    let Some(o) = d.option("caption") else { return Ok(()) };
    let empty = d.option_value("caption").is_none_or(|v| v.is_empty());
    let lines = if empty { fallback } else { &o.lines[..] };
    p.inline_node(NodeKind::Caption, lines)
}

/// Adds :force:, :name:, and :emphasize-lines:.
pub(crate) fn code_common(
    p: &mut Parser<'_, '_>,
    id: NodeId,
    d: &Directive<'_>,
) -> Result<(), Fail> {
    if d.has_option("force") {
        p.custom(id, "force", "1")?;
    }
    value_attribute(p, id, d, "name")?;
    value_attribute(p, id, d, "emphasize-lines")
}

/// Applies :dedent:: without a value the common indentation goes; with n, n columns go
/// from every line, with a warning when that removes text.
fn dedent_code(p: &mut Parser<'_, '_>, d: &Directive<'_>, lines: &mut [Line<'_>], v: &str) {
    if v.is_empty() {
        dedent(lines);
        return;
    }
    let n = usize::try_from(parse_int(v)).unwrap_or(0);
    let mut lost = false;
    for l in lines.iter_mut() {
        let limit = n.min(l.text.len());
        let spaces = l.text.bytes().take(limit).take_while(|&c| c == b' ').count();
        if spaces < limit {
            lost = true;
        }
        *l = l.cut(columns_to_bytes(l.text, n));
    }
    if lost {
        let span = d.option("dedent").map_or(d.span, |o| o.span);
        p.report(DEDENT_TEXT, span, guidedoc::arg_span(span), guidedoc::Arg::None);
    }
}

/// Returns the byte length of the first `n` characters of `s`.
pub(crate) fn columns_to_bytes(s: &str, n: usize) -> usize {
    s.char_indices().nth(n).map_or(s.len(), |(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_specs_read_numbers_and_open_ranges() {
        let got: Vec<_> = line_ranges("1,3-5,8-", 10).collect();
        assert_eq!(got, [Ok((0, 1)), Ok((2, 5)), Ok((7, 10))]);
        assert_eq!(check_line_spec("2-1", 5), None);
        assert_eq!(check_line_spec("6", 5), Some(true));
        assert_eq!(check_line_spec("-", 5), None);
    }
}
