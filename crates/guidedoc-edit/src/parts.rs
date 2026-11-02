//! The parts of a directive in the source: name, arguments, options and content. The
//! reader decides that a block is a directive and where it ends; we find its parts in
//! the text, with the directive's rules from the Sphinx layer's catalog.

use guidedoc_rst::ArgRule;
use guidedoc_sphinx::Config;

use crate::lines::{indent_of, is_blank, line_at, line_end_trimmed, line_start, next_line};
use crate::map::{DirectiveParts, OptionParts};
use crate::{Format, Span};

/// What reading a document's parts needs: its text, its format, and the configuration
/// that decides which directives exist.
#[derive(Clone, Copy)]
pub(crate) struct Ctx<'t> {
    pub text: &'t str,
    pub format: Format,
    pub config: &'t Config,
}

/// The parts of the directive that starts at `span.start`; `None` when the span does
/// not start with a directive marker.
pub(crate) fn directive(ctx: &Ctx<'_>, span: Span) -> Option<DirectiveParts> {
    match ctx.format {
        Format::Rst => rst_directive(ctx, span),
        Format::Markdown => myst_directive(ctx.text, span),
    }
}

/// The span of a directive's name on an RST marker line `.. name::`.
pub(crate) fn rst_marker(text: &str, at: usize) -> Option<Span> {
    let line = &text[at..line_end_trimmed(text, at)];
    let rest = line.strip_prefix("..")?;
    let name_from = rest.len() - rest.trim_start_matches(' ').len();
    if name_from == 0 {
        return None;
    }
    let name_start = at + 2 + name_from;
    let after = &text[name_start..at + line.len()];
    let colons = after.find("::")?;
    let name = &after[..colons];
    let tail = &after[colons + 2..];
    let bad_start = name.starts_with(['|', '_', '[']);
    if name.is_empty() || bad_start || name.contains(char::is_whitespace) {
        return None;
    }
    if !(tail.is_empty() || tail.starts_with([' ', '\t'])) {
        return None;
    }
    Some(Span::new(name_start, name_start + name.len()))
}

fn rst_directive(ctx: &Ctx<'_>, span: Span) -> Option<DirectiveParts> {
    let text = ctx.text;
    let name = rst_marker(text, span.start)?;
    let arg = guidedoc_sphinx::find_directive(ctx.config, name.text(text))
        .map_or(ArgRule::Optional, |d| d.arg);
    let mut parts = DirectiveParts { name, ..DirectiveParts::default() };
    let first_end = line_end_trimmed(text, span.start);
    let rest = skip_spaces(text, name.end + 2, first_end);
    let mut at = next_line(text, span.start);
    if arg == ArgRule::None {
        if rest < first_end {
            parts.content = content_from(text, rest, span.end);
            return Some(parts);
        }
    } else {
        let (arguments, next) = arguments(text, rest, first_end, at, span.end);
        parts.arguments = arguments;
        at = next;
    }
    at = field_options(text, at, span.end, &mut parts.options);
    parts.content = content_from(text, at, span.end);
    Some(parts)
}

/// The arguments: the rest of the marker line and the non-blank lines that follow it,
/// up to an option or a blank line. Returns them and where the options start.
fn arguments(
    text: &str,
    rest: usize,
    first_end: usize,
    mut at: usize,
    end: usize,
) -> (Option<Span>, usize) {
    let mut span = (rest < first_end).then_some(Span::new(rest, first_end));
    while at < end {
        let line = line_at(text, at);
        if is_blank(line) || option_name(text, at + indent_of(line)).is_some() {
            break;
        }
        let from = at + indent_of(line);
        let to = line_end_trimmed(text, at).min(end);
        span = Some(Span::new(span.map_or(from, |s| s.start), to));
        at = next_line(text, at);
    }
    (span, at)
}

/// Reads `:name: value` option lines from `at`, each with the more indented lines that
/// continue its value. Returns where the options end.
fn field_options(text: &str, mut at: usize, end: usize, out: &mut Vec<OptionParts>) -> usize {
    while at < end {
        let line = line_at(text, at);
        let indent = indent_of(line);
        let Some(name) = option_name(text, at + indent) else { break };
        let line_end = line_end_trimmed(text, at).min(end);
        let value_start = skip_spaces(text, name.end + 1, line_end);
        let mut value = (value_start < line_end).then_some(Span::new(value_start, line_end));
        let mut last = line_end;
        at = next_line(text, at);
        while at < end {
            let next = line_at(text, at);
            if is_blank(next) || indent_of(next) <= indent {
                break;
            }
            last = line_end_trimmed(text, at).min(end);
            let from = value.map_or(at + indent_of(next), |v| v.start);
            value = Some(Span::new(from, last));
            at = next_line(text, at);
        }
        out.push(OptionParts { span: Span::new(name.start - 1, last), name, value });
    }
    at
}

/// The name of an option field `:name:` at `at`, without its colons.
pub(crate) fn option_name(text: &str, at: usize) -> Option<Span> {
    let line = &text[at..line_end_trimmed(text, at)];
    let inner = line.strip_prefix(':')?;
    let close = inner.find(':')?;
    let name = &inner[..close];
    let tail = &inner[close + 1..];
    let ok = !name.is_empty() && !name.contains(char::is_whitespace);
    (ok && (tail.is_empty() || tail.starts_with([' ', '\t'])))
        .then_some(Span::new(at + 1, at + 1 + close))
}

/// The content from the first non-blank line at or after `at` to `end`.
fn content_from(text: &str, at: usize, end: usize) -> Option<Span> {
    let mut at = at;
    while at < end && is_blank(line_at(text, at)) {
        let next = next_line(text, at);
        if next == at {
            break;
        }
        at = next;
    }
    if at >= end {
        return None;
    }
    let start = if line_start(text, at) == at { at + indent_of(line_at(text, at)) } else { at };
    let kept = text[start..end].trim_end();
    (!kept.is_empty()).then_some(Span::new(start, start + kept.len()))
}

fn skip_spaces(text: &str, at: usize, end: usize) -> usize {
    let at = at.min(end);
    end - text[at..end].trim_start_matches([' ', '\t']).len()
}

/// A MyST fence that opens a directive: its character, its length, and the name.
pub(crate) fn myst_marker(text: &str, at: usize) -> Option<(char, usize, Span)> {
    let line_end = line_end_trimmed(text, at);
    let line = &text[at..line_end];
    let fence = line.chars().next().filter(|c| matches!(c, '`' | '~' | ':'))?;
    let count = line.len() - line.trim_start_matches(fence).len();
    let rest = &line[count..];
    if count < 3 || !rest.starts_with('{') {
        return None;
    }
    let close = rest.find('}')?;
    let name_start = at + count + 1;
    let name = Span::new(name_start, name_start + close - 1);
    (!name.is_empty()).then_some((fence, count, name))
}

fn myst_directive(text: &str, span: Span) -> Option<DirectiveParts> {
    let (fence, count, name) = myst_marker(text, span.start)?;
    let first_end = line_end_trimmed(text, span.start);
    let rest = skip_spaces(text, name.end + 1, first_end);
    let mut parts = DirectiveParts { name, ..DirectiveParts::default() };
    parts.arguments = (rest < first_end).then_some(Span::new(rest, first_end));
    let last = line_start(text, span.end.saturating_sub(1).max(span.start));
    let closing = line_at(text, last).trim();
    let closed =
        last > span.start && closing.len() >= count && closing.chars().all(|c| c == fence);
    let body_end = if closed { last } else { span.end };
    let mut at = next_line(text, span.start);
    if at < body_end && line_at(text, at).trim() == "---" {
        parts.yaml_options = true;
        at = yaml_options(text, next_line(text, at), body_end, &mut parts.options);
    } else {
        at = field_options(text, at, body_end, &mut parts.options);
    }
    parts.content = content_from(text, at, body_end);
    Some(parts)
}

/// Reads `key: value` lines up to the closing `---`; returns where the content starts.
fn yaml_options(text: &str, mut at: usize, end: usize, out: &mut Vec<OptionParts>) -> usize {
    while at < end {
        let line = line_at(text, at);
        if line.trim() == "---" {
            return next_line(text, at);
        }
        let from = at + indent_of(line);
        let line_end = line_end_trimmed(text, at);
        if let Some(colon) = text[from..line_end].find(':') {
            let name = Span::new(from, from + colon);
            let value_start = skip_spaces(text, name.end + 1, line_end);
            let value = (value_start < line_end).then_some(Span::new(value_start, line_end));
            out.push(OptionParts { span: Span::new(from, line_end), name, value });
        }
        at = next_line(text, at);
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(format: Format, text: &str) -> DirectiveParts {
        let config = Config::default();
        let ctx = Ctx { text, format, config: &config };
        directive(&ctx, Span::new(0, text.trim_end().len())).unwrap()
    }

    #[test]
    fn rst_parts() {
        let text = concat!(
            ".. code-block:: python\n   :linenos:\n   :caption: A long\n      caption\n\n",
            "   print(1)\n   x\n"
        );
        let p = parts(Format::Rst, text);
        assert_eq!(p.name.text(text), "code-block");
        assert_eq!(p.arguments.unwrap().text(text), "python");
        assert_eq!(p.options.len(), 2);
        assert_eq!(p.options[0].name.text(text), "linenos");
        assert!(p.options[0].value.is_none());
        assert_eq!(p.options[1].value.unwrap().text(text), "A long\n      caption");
        assert_eq!(p.content.unwrap().text(text), "print(1)\n   x");
        let note = ".. note:: Inline text\n   continues.\n";
        let p = parts(Format::Rst, note);
        assert!(p.arguments.is_none());
        assert_eq!(p.content.unwrap().text(note), "Inline text\n   continues.");
        assert!(rst_marker(".. _target:", 0).is_none());
        assert!(rst_marker(".. |x| replace:: y", 0).is_none());
    }

    #[test]
    fn myst_parts() {
        let text = "```{code-block} python\n:linenos:\n\nx = 1\n```\n";
        let p = parts(Format::Markdown, text);
        assert_eq!(p.name.text(text), "code-block");
        assert_eq!(p.arguments.unwrap().text(text), "python");
        assert_eq!(p.options[0].name.text(text), "linenos");
        assert_eq!(p.content.unwrap().text(text), "x = 1");
        let yaml = ":::{figure} a.png\n---\nwidth: 50%\n---\nCaption.\n:::\n";
        let p = parts(Format::Markdown, yaml);
        assert!(p.yaml_options);
        assert_eq!(p.options[0].value.unwrap().text(yaml), "50%");
        assert_eq!(p.content.unwrap().text(yaml), "Caption.");
    }
}
