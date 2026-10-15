//! literalinclude reads a file through the host (`ReadContext::include_source`, which
//! confines it) and shows it as a Code_Block. The selection options apply in Sphinx's
//! order: pyobject, start-at/start-after, end-at/end-before, lines, dedent, prepend,
//! append; `:diff:` shows a unified diff from another file instead.

use std::borrow::Cow;

use guidedoc::{
    Arg, AttributeKey, Fail, Message, NodeKind, SourceSpan, SourceUse, arg_span, arg_static,
};
use guidedoc_rst::{Directive, Parser, path_of};

use crate::code::{
    caption_child, check_line_spec, code_common, code_numbers, columns_to_bytes, line_ranges,
};
use crate::diff::unified_diff;
use crate::directives::{class_attribute, parse_signed};
use crate::extension::Sphinx;
use crate::messages::{DEDENT_TEXT, ENCODING, INCLUDE_OPTIONS, INCLUDE_PATTERN, LINES_SPEC};
use crate::pyobject::find_pyobject;
use crate::text::{parse_int, trim_space};

/// A file being included: its lines (each with its "\n", except perhaps the last), the
/// first line's number, and the host's canonical name of the file.
struct Include<'t> {
    lines: Vec<Cow<'t, str>>,
    lineno_start: i64,
    name: String,
}

/// A loaded file: its normalized text and canonical name.
struct Loaded {
    text: String,
    name: String,
}

/// Runs literalinclude.
pub(crate) fn literalinclude(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
) -> Result<(), Fail> {
    if !include_options_ok(p, d) {
        return Ok(());
    }
    let arg = d.argument();
    let Some(file) = load(p, d, trim_space(&arg), d.span)? else { return Ok(()) };
    let diff_text;
    let mut inc =
        Include { lines: split_lines(&file.text), lineno_start: 1, name: file.name.clone() };
    if let Some(v) = d.option_value("lineno-start") {
        inc.lineno_start = parse_signed(&v);
    }
    let diff = d.option("diff").is_some();
    if let Some(o) = d.option("diff") {
        let Some(old) = load(p, d, &path_of(&o.lines), o.span)? else { return Ok(()) };
        let old_lines = split_lines(&old.text);
        let a: Vec<&str> = old_lines.iter().map(|l| l.as_ref()).collect();
        let b: Vec<&str> = inc.lines.iter().map(|l| l.as_ref()).collect();
        diff_text = unified_diff(&mut p.ctx.b, &a, &b, &old.name, &inc.name)?;
        inc.lines = split_lines(&diff_text);
    } else if !apply_filters(p, d, &mut inc) {
        return Ok(());
    }
    if let Some(v) = d.option_value("emphasize-lines") {
        let span = d.option("emphasize-lines").map_or(d.span, |o| o.span);
        match check_line_spec(&v, inc.lines.len()) {
            None => {
                report(p, LINES_SPEC, span);
                return Ok(());
            }
            Some(true) => report(p, LINES_SPEC, span),
            Some(false) => {}
        }
    }
    emit_include(sx, p, d, &inc, diff)
}

fn report(p: &mut Parser<'_, '_>, m: Message, span: SourceSpan) {
    p.report(m, span, arg_span(span), Arg::None);
}

fn emit_include(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
    inc: &Include<'_>,
    diff: bool,
) -> Result<(), Fail> {
    let id = p.open(NodeKind::CodeBlock, d.span)?;
    let mut language =
        d.option_value("language").map_or_else(|| sx.state().highlight.clone(), Cow::into_owned);
    if diff {
        language = "udiff".to_owned();
    }
    if !language.is_empty() {
        p.ctx.b.add_attribute_string(id, AttributeKey::Language, &language)?;
    }
    class_attribute(p, id, d)?;
    code_numbers(p, &sx.state(), id, d, inc.lines.len(), inc.lineno_start)?;
    code_common(p, id, d)?;
    if let Some(v) = d.option_value("caption") {
        let arg = d.argument();
        p.custom(id, "caption", if v.is_empty() { trim_space(&arg) } else { &v })?;
    }
    caption_child(p, d, &d.arg)?;
    let mut text: String = inc.lines.concat();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    let t = p.ctx.b.add_text(&text)?;
    p.ctx.b.set_text(id, t);
    p.close(id)
}

/// Reports the combinations Sphinx refuses.
fn include_options_ok(p: &mut Parser<'_, '_>, d: &Directive<'_>) -> bool {
    let pairs =
        [("start-at", "start-after"), ("end-at", "end-before"), ("lineno-match", "lineno-start")];
    if pairs.iter().any(|(a, b)| d.has_option(a) && d.has_option(b)) {
        let why = arg_static("each pair of start, end, and line-number options");
        p.report(INCLUDE_OPTIONS, d.name_span, arg_span(d.name_span), why);
        return false;
    }
    if let Some(v) = d.option_value("encoding")
        && !matches!(
            &*v,
            "utf-8" | "utf8" | "UTF-8" | "UTF8" | "utf-8-sig" | "utf_8" | "utf_8_sig"
        )
    {
        let span = d.option("encoding").map_or(d.span, |o| o.span);
        report(p, ENCODING, span);
        return false;
    }
    true
}

/// Reads a file, expands tabs when :tab-width: is given, and turns every line break
/// into "\n" (as Python's universal newlines do). `None` when the host refused it (and
/// reported why).
fn load(
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
    name: &str,
    span: SourceSpan,
) -> Result<Option<Loaded>, Fail> {
    let from = p.src();
    let Some(id) = p.ctx.include_source(from, name, span, SourceUse::Literal) else {
        return Ok(None);
    };
    let source = p.ctx.source_of(id);
    let canonical = p.ctx.source_entry(id).map(|e| e.name.clone()).unwrap_or_default();
    let mut text: &str = &source;
    let enc = d.option_value("encoding");
    let plain_utf8 = enc.as_deref().is_some_and(|e| matches!(e, "utf-8" | "utf8" | "UTF-8"));
    if !plain_utf8 {
        text = text.strip_prefix('\u{feff}').unwrap_or(text);
    }
    let tab =
        d.option_value("tab-width").map_or(0, |v| usize::try_from(parse_int(&v)).unwrap_or(0));
    let normalized = normalize(text, tab);
    p.ctx.b.spend(normalized.len() as u64 + 1)?;
    Ok(Some(Loaded { text: normalized, name: canonical }))
}

/// Returns `text` with universal newlines and expanded tabs.
fn normalize(text: &str, tab: usize) -> String {
    let mut out = String::with_capacity(text.len());
    let mut col = 0;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                out.push('\n');
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                col = 0;
            }
            '\n' => {
                out.push('\n');
                col = 0;
            }
            '\t' if tab > 0 => {
                let w = tab - col % tab;
                out.extend(std::iter::repeat_n(' ', w));
                col += w;
            }
            _ => {
                out.push(c);
                col += 1;
            }
        }
    }
    out
}

/// Splits text into lines that keep their "\n".
fn split_lines(text: &str) -> Vec<Cow<'_, str>> {
    text.split_inclusive('\n').map(Cow::Borrowed).collect()
}

fn apply_filters(p: &mut Parser<'_, '_>, d: &Directive<'_>, inc: &mut Include<'_>) -> bool {
    if let Some(v) = d.option_value("pyobject") {
        let Some((a, b)) = find_pyobject(&inc.lines, &v) else {
            let span = d.option("pyobject").map_or(d.span, |o| o.span);
            report(p, INCLUDE_PATTERN, span);
            return false;
        };
        inc.lines.truncate(b);
        inc.lines.drain(..a);
        if d.has_option("lineno-match") {
            inc.lineno_start = a as i64 + 1;
        }
    }
    if !start_filter(p, d, inc) || !end_filter(p, d, inc) || !lines_filter(p, d, inc) {
        return false;
    }
    if let Some(v) = d.option_value("dedent") {
        dedent_include(p, d, inc, &v);
    }
    if let Some(v) = d.option_value("prepend") {
        inc.lines.insert(0, Cow::Owned(format!("{v}\n")));
    }
    if let Some(v) = d.option_value("append") {
        inc.lines.push(Cow::Owned(format!("{v}\n")));
    }
    true
}

/// The option of a pair that was given, with its value.
fn either<'d>(
    d: &Directive<'d>,
    first: &'static str,
    second: &'static str,
) -> Option<(&'static str, Cow<'d, str>)> {
    d.option_value(first)
        .map(|v| (first, v))
        .or_else(|| d.option_value(second).map(|v| (second, v)))
}

fn start_filter(p: &mut Parser<'_, '_>, d: &Directive<'_>, inc: &mut Include<'_>) -> bool {
    let Some((name, v)) = either(d, "start-at", "start-after") else { return true };
    if v.is_empty() {
        return true;
    }
    if let Some(i) = inc.lines.iter().position(|l| l.contains(&*v)) {
        let skip = if name == "start-after" { i + 1 } else { i };
        if d.has_option("lineno-match") {
            inc.lineno_start += skip as i64;
        }
        inc.lines.drain(..skip);
        return true;
    }
    let span = d.option(name).map_or(d.span, |o| o.span);
    report(p, INCLUDE_PATTERN, span);
    false
}

fn end_filter(p: &mut Parser<'_, '_>, d: &Directive<'_>, inc: &mut Include<'_>) -> bool {
    let Some((name, v)) = either(d, "end-at", "end-before") else { return true };
    if v.is_empty() {
        return true;
    }
    for (i, l) in inc.lines.iter().enumerate() {
        if !l.contains(&*v) {
            continue;
        }
        if name == "end-at" {
            inc.lines.truncate(i + 1);
            return true;
        }
        if i == 0 {
            continue; // end-before ignores the first line, as Sphinx does.
        }
        inc.lines.truncate(i);
        return true;
    }
    let span = d.option(name).map_or(d.span, |o| o.span);
    report(p, INCLUDE_PATTERN, span);
    false
}

fn lines_filter(p: &mut Parser<'_, '_>, d: &Directive<'_>, inc: &mut Include<'_>) -> bool {
    let Some(spec) = d.option_value("lines") else { return true };
    let span = d.option("lines").map_or(d.span, |o| o.span);
    let total = inc.lines.len();
    match check_line_spec(&spec, total) {
        None => {
            report(p, LINES_SPEC, span);
            return false;
        }
        Some(true) => report(p, LINES_SPEC, span),
        Some(false) => {}
    }
    let mut picked: Vec<usize> = Vec::new();
    let (mut first, mut contiguous, mut next) = (None, true, 0);
    for (a, b) in line_ranges(&spec, total).flatten() {
        for n in a..b {
            match first {
                None => first = Some(n),
                Some(_) if n != next => contiguous = false,
                Some(_) => {}
            }
            next = n + 1;
            if n < total {
                picked.push(n);
            }
        }
    }
    if d.has_option("lineno-match") {
        if !contiguous {
            let why = arg_static("lineno-match needs one contiguous range of lines");
            p.report(INCLUDE_OPTIONS, span, arg_span(span), why);
            return false;
        }
        inc.lineno_start += first.unwrap_or(0) as i64;
    }
    if picked.is_empty() {
        report(p, INCLUDE_PATTERN, span);
        return false;
    }
    let lines = std::mem::take(&mut inc.lines);
    inc.lines = picked.into_iter().map(|n| lines[n].clone()).collect();
    true
}

/// Applies :dedent: to lines that keep their line breaks: without a value, the common
/// leading white space goes and blank lines become empty (as textwrap.dedent does);
/// with n, n characters go from every line.
fn dedent_include(p: &mut Parser<'_, '_>, d: &Directive<'_>, inc: &mut Include<'_>, v: &str) {
    if !v.is_empty() {
        let n = usize::try_from(parse_int(v)).unwrap_or(0);
        let mut lost = false;
        for l in &mut inc.lines {
            let body = l.strip_suffix('\n').unwrap_or(l);
            let limit = n.min(body.len());
            if body.bytes().take(limit).take_while(|&c| c == b' ' || c == b'\t').count() < limit {
                lost = true;
            }
            let had_newline = body.len() < l.len();
            let mut cut = l[columns_to_bytes(l, n)..].to_owned();
            if cut.is_empty() && had_newline {
                cut.push('\n');
            }
            *l = Cow::Owned(cut);
        }
        if lost {
            let span = d.option("dedent").map_or(d.span, |o| o.span);
            report(p, DEDENT_TEXT, span);
        }
        return;
    }
    let indent = |body: &str| body.bytes().take_while(|&c| c == b' ' || c == b'\t').count();
    let mut margin: Option<String> = None;
    for l in &inc.lines {
        let body = l.strip_suffix('\n').unwrap_or(l);
        let k = indent(body);
        if k == body.len() {
            continue;
        }
        margin = Some(match margin {
            None => body[..k].to_owned(),
            Some(m) => {
                let j = m.bytes().zip(body[..k].bytes()).take_while(|(x, y)| x == y).count();
                m[..j].to_owned()
            }
        });
    }
    let margin = margin.map_or(0, |m| m.len());
    for l in &mut inc.lines {
        let body_len = l.strip_suffix('\n').unwrap_or(l).len();
        let blank = indent(&l[..body_len]) == body_len;
        *l = if blank {
            Cow::Borrowed(if l.len() > body_len { "\n" } else { "" })
        } else {
            Cow::Owned(l[margin..].to_owned())
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_unifies_newlines_and_expands_tabs() {
        assert_eq!(normalize("a\r\nb\rc\td", 4), "a\nb\nc   d");
        assert_eq!(normalize("\tx", 0), "\tx");
    }
}
