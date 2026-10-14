//! Expressions written back as Sphinx's unparser (or Python's `ast.unparse`) writes
//! them. Every writer appends to one `String` sink.

use crate::literal::{StringKind, repr_bytes, repr_str, string_value};
use crate::scan::function_of;
use crate::syntax::{Node, Syntax};
use crate::{Function, ParamKind};

/// Whose unparser to follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnparseMode {
    /// Sphinx's `sphinx.pycode.ast.unparse`, which `preserve_defaults` uses: numbers as
    /// written, lambda bodies as `...`, and no parentheses added for precedence.
    #[default]
    Sphinx,
    /// Sphinx's unparser without the source, as for overload signatures: numbers as
    /// `repr` writes them.
    SphinxRepr,
    /// Python's own `ast.unparse`, which makes the annotation strings of `from
    /// __future__ import annotations`: parentheses kept, every expression written.
    Python,
}

/// Writes an expression back; what the mode's unparser refuses is left out.
pub fn unparse(n: Node<'_>, mode: UnparseMode) -> String {
    try_unparse(n, mode).0
}

/// [`unparse`] that also reports whether the unparser accepted every part
/// (Sphinx's refuses comparisons, comprehensions, f-strings, conditionals...).
pub fn try_unparse(n: Node<'_>, mode: UnparseMode) -> (String, bool) {
    let mut out = String::new();
    let ok = write_expr(&mut out, n, mode);
    (out, ok)
}

fn write_expr(b: &mut String, n: Node<'_>, mode: UnparseMode) -> bool {
    match n.kind() {
        "" => return false,
        "identifier" | "true" | "false" | "none" | "ellipsis" => b.push_str(n.text()),
        "type" => return write_expr(b, n.named_child(0), mode),
        "parenthesized_expression" => return write_parenthesized(b, n, mode),
        "attribute" => {
            let ok = write_expr(b, n.field("object"), mode);
            b.push('.');
            b.push_str(n.field_text("attribute"));
            return ok;
        }
        "integer" | "float" => match mode {
            UnparseMode::Sphinx => b.push_str(n.text()),
            _ => b.push_str(&number_repr(n.text())),
        },
        "string" | "concatenated_string" => return write_string(b, n, mode),
        "binary_operator" => return write_binary(b, n, mode),
        "boolean_operator" => {
            let ok = write_expr(b, n.field("left"), mode);
            b.push(' ');
            b.push_str(n.field_text("operator"));
            b.push(' ');
            return write_expr(b, n.field("right"), mode) && ok;
        }
        "unary_operator" => {
            b.push_str(n.field_text("operator"));
            return write_expr(b, n.field("argument"), mode);
        }
        "not_operator" => {
            b.push_str("not ");
            return write_expr(b, n.field("argument"), mode);
        }
        "call" => return write_call(b, n, mode),
        "list" => return write_bracketed(b, n, mode, '[', ']'),
        "set" => return write_bracketed(b, n, mode, '{', '}'),
        "tuple" | "expression_list" => return write_tuple(b, n, mode),
        "dictionary" => return write_dict(b, n, mode),
        "subscript" | "generic_type" => return write_subscript(b, n, mode),
        "slice" => return write_slice(b, n, mode),
        "lambda" => return write_lambda(b, n, mode),
        "list_splat" | "list_splat_pattern" => {
            b.push('*');
            return write_expr(b, n.named_child(0), mode);
        }
        _ => return mode == UnparseMode::Python && write_raw(b, n),
    }
    true
}

fn write_parenthesized(b: &mut String, n: Node<'_>, mode: UnparseMode) -> bool {
    if mode != UnparseMode::Python {
        return write_expr(b, n.named_child(0), mode);
    }
    b.push('(');
    let ok = write_expr(b, n.named_child(0), mode);
    b.push(')');
    ok
}

fn write_string(b: &mut String, n: Node<'_>, mode: UnparseMode) -> bool {
    match string_value(n) {
        Some((v, StringKind::Str)) => b.push_str(&repr_str(&v)),
        Some((v, StringKind::Bytes)) => b.push_str(&repr_bytes(&v)),
        _ => return mode == UnparseMode::Python && write_raw(b, n),
    }
    true
}

/// Writes source text with its runs of white space collapsed: the Python unparser's
/// output for what this one does not rebuild.
fn write_raw(b: &mut String, n: Node<'_>) -> bool {
    let mut space = false;
    for c in n.text().chars() {
        if matches!(c, ' ' | '\n' | '\t') {
            space = true;
            continue;
        }
        if space && !b.is_empty() {
            b.push(' ');
        }
        space = false;
        b.push(c);
    }
    true
}

fn write_binary(b: &mut String, n: Node<'_>, mode: UnparseMode) -> bool {
    let op = n.field_text("operator");
    let ok = write_expr(b, n.field("left"), mode);
    if op == "**" {
        b.push_str(op);
    } else {
        b.push(' ');
        b.push_str(op);
        b.push(' ');
    }
    write_expr(b, n.field("right"), mode) && ok
}

/// The items of a container: its named children other than comments.
fn items<'a>(n: Node<'a>) -> impl Iterator<Item = Node<'a>> {
    n.named_children().filter(|i| i.kind() != "comment")
}

fn write_items(b: &mut String, n: Node<'_>, mode: UnparseMode) -> bool {
    let mut ok = true;
    for (i, item) in items(n).enumerate() {
        if i > 0 {
            b.push_str(", ");
        }
        ok &= write_expr(b, item, mode);
    }
    ok
}

fn write_bracketed(
    b: &mut String,
    n: Node<'_>,
    mode: UnparseMode,
    open: char,
    close: char,
) -> bool {
    b.push(open);
    let ok = write_items(b, n, mode);
    b.push(close);
    ok
}

fn write_tuple(b: &mut String, n: Node<'_>, mode: UnparseMode) -> bool {
    let count = items(n).count();
    if count == 0 {
        b.push_str("()");
        return true;
    }
    b.push('(');
    let ok = write_items(b, n, mode);
    if count == 1 {
        b.push(',');
    }
    b.push(')');
    ok
}

fn write_dict(b: &mut String, n: Node<'_>, mode: UnparseMode) -> bool {
    b.push('{');
    let mut ok = true;
    let mut first = true;
    for item in n.named_children() {
        if item.kind() != "pair" {
            ok &= item.kind() == "comment";
            continue;
        }
        if !first {
            b.push_str(", ");
        }
        first = false;
        ok &= write_expr(b, item.field("key"), mode);
        b.push_str(": ");
        ok &= write_expr(b, item.field("value"), mode);
    }
    b.push('}');
    ok
}

fn write_call(b: &mut String, n: Node<'_>, mode: UnparseMode) -> bool {
    let mut ok = write_expr(b, n.field("function"), mode);
    let args = n.field("arguments");
    if args.kind() != "argument_list" {
        return false; // a generator argument.
    }
    b.push('(');
    // Sphinx writes positional arguments first, then keywords, each keyword as
    // name=value; **mapping has no name and comes out as None=mapping.
    let mut first = true;
    for keywords in [false, true] {
        for a in items(args) {
            let keyword = matches!(a.kind(), "keyword_argument" | "dictionary_splat");
            if keyword != keywords {
                continue;
            }
            if !first {
                b.push_str(", ");
            }
            first = false;
            ok &= write_argument(b, a, mode);
        }
    }
    b.push(')');
    ok
}

fn write_argument(b: &mut String, a: Node<'_>, mode: UnparseMode) -> bool {
    match a.kind() {
        "keyword_argument" => {
            b.push_str(a.field_text("name"));
            b.push('=');
            write_expr(b, a.field("value"), mode)
        }
        "dictionary_splat" => {
            b.push_str(if mode == UnparseMode::Python { "**" } else { "None=" });
            write_expr(b, a.named_child(0), mode)
        }
        _ => write_expr(b, a, mode),
    }
}

/// The subscripted value and its subscripts, for `subscript` and `generic_type` nodes.
pub(crate) fn subscript_parts(n: Node<'_>) -> (Node<'_>, Vec<Node<'_>>) {
    if n.kind() == "generic_type" {
        return (n.named_child(0), n.named_child(1).named_children().collect());
    }
    let subs = n.children().filter(|c| c.field_name() == Some("subscript")).collect();
    (n.field("value"), subs)
}

fn write_subscript(b: &mut String, n: Node<'_>, mode: UnparseMode) -> bool {
    let (value, subscripts) = subscript_parts(n);
    let mut ok = write_expr(b, value, mode);
    b.push('[');
    // x[a, b] is a subscript by a tuple, written without parentheses.
    for (i, s) in subscripts.into_iter().enumerate() {
        if i > 0 {
            b.push_str(", ");
        }
        ok &= write_expr(b, s, mode);
    }
    b.push(']');
    ok
}

fn write_slice(b: &mut String, n: Node<'_>, mode: UnparseMode) -> bool {
    // slice children: [start] ":" [stop] [":" [step]]
    let mut parts: [Option<Node<'_>>; 3] = [None; 3];
    let mut k = 0;
    for c in n.children() {
        if !c.is_named() {
            k += usize::from(c.text() == ":");
        } else if k < 3 {
            parts[k] = Some(c);
        }
    }
    let mut ok = true;
    if let Some(start) = parts[0] {
        ok = write_expr(b, start, mode);
    }
    b.push(':');
    if let Some(stop) = parts[1] {
        ok &= write_expr(b, stop, mode);
    }
    if let Some(step) = parts[2] {
        b.push(':');
        ok &= write_expr(b, step, mode);
    }
    ok
}

fn write_lambda(b: &mut String, n: Node<'_>, mode: UnparseMode) -> bool {
    let f = function_of(n, "<lambda>");
    b.push_str("lambda");
    if !f.params.is_empty() {
        b.push(' ');
    }
    let ok = write_arguments(b, n.syntax(), &f, mode);
    b.push_str(": ");
    if mode == UnparseMode::Python {
        return write_expr(b, n.field("body"), mode) && ok;
    }
    b.push_str("...");
    ok
}

/// Sphinx's `visit_arguments`: a function's parameter list without parentheses, written
/// to `b`. `syntax` is the function's module's. False when a part was refused.
pub fn write_arguments(b: &mut String, syntax: &Syntax, f: &Function, mode: UnparseMode) -> bool {
    let mut ok = true;
    let mut last = ParamKind::PositionalOrKeyword;
    let mut starred = false;
    let mut first = true;
    for p in &f.params {
        if !first && last == ParamKind::PositionalOnly && p.kind != ParamKind::PositionalOnly {
            separate(b, &mut first);
            b.push('/');
        }
        if p.kind == ParamKind::KeywordOnly && !starred {
            separate(b, &mut first);
            b.push('*');
            starred = true;
        }
        starred |= p.kind == ParamKind::VarPositional;
        separate(b, &mut first);
        match p.kind {
            ParamKind::VarPositional => b.push('*'),
            ParamKind::VarKeyword => b.push_str("**"),
            _ => {}
        }
        b.push_str(&p.name);
        if let Some(annotation) = p.annotation {
            b.push_str(": ");
            ok &= write_expr(b, syntax.node(annotation), mode);
        }
        if let Some(default) = p.default {
            b.push_str(if p.annotation.is_some() { " = " } else { "=" });
            ok &= write_expr(b, syntax.node(default), mode);
        }
        last = p.kind;
    }
    if last == ParamKind::PositionalOnly && !f.params.is_empty() {
        separate(b, &mut first);
        b.push('/');
    }
    ok
}

/// Writes ", " before every part of a list but the first.
fn separate(b: &mut String, first: &mut bool) {
    if !*first {
        b.push_str(", ");
    }
    *first = false;
}

/// Python's `repr` of a numeric literal: integers in decimal, floats in their shortest
/// round-trip form, imaginary numbers with `j`.
pub fn number_repr(literal: &str) -> String {
    let s: String = literal.chars().filter(|&c| c != '_').collect();
    if s.is_empty() {
        return literal.to_string();
    }
    if let Some(inner) = s.strip_suffix(['j', 'J']) {
        let inner = number_repr(inner);
        return format!("{}j", inner.strip_suffix(".0").unwrap_or(&inner));
    }
    let lower = s.to_ascii_lowercase();
    let is_float = !lower.starts_with("0x") && (lower.contains('.') || lower.contains('e'));
    if !is_float {
        return parse_int(&lower).map_or_else(|| literal.to_string(), |v| v.to_string());
    }
    match lower.parse::<f64>() {
        Ok(f) => float_repr(f),
        Err(_) => literal.to_string(),
    }
}

/// Parses a Python integer literal (decimal, or `0x`, `0o`, `0b`), lower-cased.
pub(crate) fn parse_int(s: &str) -> Option<i128> {
    let (digits, radix) = match s.get(..2) {
        Some("0x") => (&s[2..], 16),
        Some("0o") => (&s[2..], 8),
        Some("0b") => (&s[2..], 2),
        _ => (s, 10),
    };
    i128::from_str_radix(digits, radix).ok()
}

/// Python's `repr(float)`: the shortest digits that round-trip, positional when the
/// exponent is from -4 to 15, else scientific with a two-digit exponent.
fn float_repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    let e = format!("{:e}", f.abs()); // d[.ddd]e[-]X, shortest round-trip digits.
    let (mantissa, exponent) = e.split_once('e').unwrap_or((&e, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(|&c| c != '.').collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let mut b = String::with_capacity(24);
    if f.is_sign_negative() {
        b.push('-');
    }
    if (-4..16).contains(&exponent) {
        write_positional(&mut b, digits, exponent);
    } else {
        b.push_str(&digits[..1]);
        if digits.len() > 1 {
            b.push('.');
            b.push_str(&digits[1..]);
        }
        let sign = if exponent < 0 { '-' } else { '+' };
        b.push_str(&format!("e{sign}{:02}", exponent.abs()));
    }
    b
}

fn write_positional(b: &mut String, digits: &str, exponent: i32) {
    if exponent < 0 {
        b.push_str("0.");
        b.extend(std::iter::repeat_n('0', (-exponent - 1) as usize));
        b.push_str(digits);
        return;
    }
    let whole = (exponent + 1) as usize;
    if digits.len() <= whole {
        b.push_str(digits);
        b.extend(std::iter::repeat_n('0', whole - digits.len()));
        b.push_str(".0");
        return;
    }
    b.push_str(&digits[..whole]);
    b.push('.');
    b.push_str(&digits[whole..]);
}
