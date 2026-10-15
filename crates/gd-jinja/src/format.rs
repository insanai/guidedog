//! Python's format mini-language: `format(v, spec)`, `"%s" % args` and `str.format`.

use std::borrow::Cow;
use std::rc::Rc;

use crate::calls::{Call, Keyword};
use crate::convert::{
    Num, as_number, char_count, char_slice, quoted, repr, to_str, type_name, write_exponent,
    write_float_repr,
};
use crate::error::ErrorKind;
use crate::escape::{escape_string, write_escaped};
use crate::eval::{get_attr, get_item};
use crate::limits::{reserve_bytes, reserve_text, sum_bytes};
use crate::runtime::{Result, State};
use crate::text::to_upper;
use crate::value::Value;

/// A parsed format specification ("%-8.2f" or "{:>8,.2f}").
#[derive(Debug, Clone, Default)]
struct Spec {
    fill: Option<char>,
    align: Option<char>, // '<', '>', '^', '='
    sign: Option<char>,  // '+', '-', ' '
    alt: bool,
    zero: bool,
    width: usize,
    grouping: Option<char>, // ',' or '_'
    precision: Option<usize>,
    kind: Option<char>, // the presentation type
}

/// The error of a format code that does not apply to a value of type `what`.
fn unknown_code(s: &State<'_>, kind: char, what: &str) -> Box<crate::Error> {
    let message = format!("Unknown format code '{kind}' for object of type '{what}'");
    s.error(ErrorKind::Type, message)
}

/// Appends v formatted by spec, as Python's `format(v, spec)` does.
fn format_value(s: &State<'_>, out: &mut String, v: &Value, spec: &Spec) -> Result<()> {
    let number = match v {
        Value::Int(_) | Value::Float(_) => true,
        Value::Bool(_) => spec.kind.is_some(),
        _ => false,
    };
    let (body, negative) = if number {
        format_number(s, v, spec)?
    } else if let Some(kind) = spec.kind.filter(|k| *k != 's') {
        return Err(unknown_code(s, kind, type_name(v)));
    } else {
        let text = to_str(v);
        match spec.precision {
            Some(p) => (char_slice(&text, 0, p).to_string(), false),
            None => (text.into_owned(), false),
        }
    };
    let sign = match (number, negative, spec.sign) {
        (true, true, _) => "-",
        (true, false, Some('+')) => "+",
        (true, false, Some(' ')) => " ",
        _ => "",
    };
    let mut fill = spec.fill.unwrap_or(' ');
    let mut align = spec.align;
    if spec.zero && align.is_none() && number {
        (fill, align) = ('0', Some('='));
    }
    match align.unwrap_or(if number { '>' } else { '<' }) {
        // Padding goes between the sign and the digits.
        '=' => {
            let pads = spec.width.saturating_sub(sign.len() + char_count(&body));
            let size =
                sum_bytes(s, reserve_bytes(s, pads, fill.len_utf8())?, sign.len() + body.len())?;
            reserve_text(s, out, size)?;
            out.push_str(sign);
            out.extend(std::iter::repeat_n(fill, pads));
            out.push_str(&body);
        }
        align => {
            let text = format!("{sign}{body}");
            let align = if align == '^' { 'c' } else { align };
            out.push_str(&pad(s, &text, spec.width, &fill.to_string(), align)?);
        }
    }
    Ok(())
}

/// Formats the magnitude of a number and reports its sign separately.
fn format_number(s: &State<'_>, v: &Value, spec: &Spec) -> Result<(String, bool)> {
    let n = as_number(v).unwrap_or(Num::Int(0));
    match (spec.kind, n) {
        (None | Some('d' | 'n' | 'x' | 'X' | 'o' | 'b' | 'c'), Num::Int(i)) => {
            Ok((format_int(i, spec), i < 0))
        }
        (None, Num::Float(f)) => Ok(format_float(f, spec)),
        (Some('d' | 'n' | 'x' | 'X' | 'o' | 'b' | 'c'), Num::Float(_)) => {
            Err(unknown_code(s, spec.kind.unwrap_or('d'), "float"))
        }
        (Some('f' | 'F' | 'e' | 'E' | 'g' | 'G' | '%'), _) => Ok(format_float(n.f(), spec)),
        (Some(kind), _) => Err(unknown_code(s, kind, type_name(v))),
    }
}

fn format_int(i: i64, spec: &Spec) -> String {
    let mag = i.unsigned_abs();
    let (digits, prefix) = match spec.kind {
        Some('x') => (format!("{mag:x}"), "0x"),
        Some('X') => (format!("{mag:X}"), "0X"),
        Some('o') => (format!("{mag:o}"), "0o"),
        Some('b') => (format!("{mag:b}"), "0b"),
        Some('c') => {
            let c = u32::try_from(i).ok().and_then(char::from_u32);
            return c.map(String::from).unwrap_or_default();
        }
        _ => (mag.to_string(), ""),
    };
    let prefix = if spec.alt { prefix } else { "" };
    format!("{prefix}{}", group_digits(&digits, spec.grouping))
}

/// Inserts a separator between groups of three digits.
fn group_digits(digits: &str, sep: Option<char>) -> String {
    let Some(sep) = sep.filter(|_| digits.len() > 3) else { return digits.to_string() };
    let first = match digits.len() % 3 {
        0 => 3,
        n => n,
    };
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    out.push_str(&digits[..first]);
    for group in digits.as_bytes()[first..].chunks(3) {
        out.push(sep);
        out.extend(group.iter().map(|&b| char::from(b)));
    }
    out
}

/// `|f|` with precision digits after the point, rounding half-even on the exact binary
/// value like Python.
pub(crate) fn fixed_float(f: f64, precision: usize) -> String {
    format!("{:.*}", precision, f.abs())
}

/// `"%.{p}e"` of |f| with at least two exponent digits.
fn sci_float(f: f64, p: usize) -> String {
    let text = format!("{:.*e}", p, f.abs());
    let Some((mantissa, exponent)) = text.split_once('e') else { return text };
    let mut out = mantissa.to_string();
    write_exponent(&mut out, exponent.parse().unwrap_or(0));
    out
}

/// Python's "g" presentation.
fn general_float(f: f64, precision: usize, alt: bool) -> String {
    let p = precision.max(1);
    if f == 0.0 {
        return if alt { format!("0.{}", "0".repeat(p - 1)) } else { "0".to_string() };
    }
    let sci = sci_float(f, p - 1);
    let exp: i64 = sci.split_once('e').map_or(0, |(_, e)| e.parse().unwrap_or(0));
    let out = if -4 <= exp && exp < p as i64 {
        fixed_float(f, (p as i64 - 1 - exp) as usize)
    } else {
        sci
    };
    if alt {
        return out;
    }
    let (mantissa, suffix) = out.split_at(out.find('e').unwrap_or(out.len()));
    let mantissa = if mantissa.contains('.') {
        mantissa.trim_end_matches('0').trim_end_matches('.')
    } else {
        mantissa
    };
    format!("{mantissa}{suffix}")
}

fn format_float(f: f64, spec: &Spec) -> (String, bool) {
    let negative = f.is_sign_negative() && !f.is_nan();
    let upper = matches!(spec.kind, Some('F' | 'E' | 'G'));
    if !f.is_finite() {
        let word = if f.is_nan() { "nan" } else { "inf" };
        return (if upper { word.to_uppercase() } else { word.to_string() }, negative);
    }
    let p = spec.precision;
    let mut body = match spec.kind {
        Some('f' | 'F') => fixed_float(f, p.unwrap_or(6)),
        Some('%') => format!("{}%", fixed_float(f * 100.0, p.unwrap_or(6))),
        Some('e' | 'E') => sci_float(f, p.unwrap_or(6)),
        Some('g' | 'G') => general_float(f, p.unwrap_or(6), spec.alt),
        _ => match p {
            None => {
                let mut text = String::new();
                write_float_repr(&mut text, f.abs());
                text
            }
            Some(p) => {
                let text = general_float(f, p, spec.alt);
                if text.contains(['.', 'e']) { text } else { text + ".0" }
            }
        },
    };
    if upper {
        body = to_upper(&body);
    }
    if spec.grouping.is_some() {
        let dot = body.find(['.', 'e', '%']).unwrap_or(body.len());
        body = group_digits(&body[..dot], spec.grouping) + &body[dot..];
    }
    (body, negative)
}

/// Parses the format() mini-language; None when it is malformed.
fn parse_format_spec(text: &str) -> Option<Spec> {
    let mut spec = Spec::default();
    let chars: Vec<char> = text.chars().collect();
    let is_align = |c: &char| matches!(c, '<' | '>' | '^' | '=');
    let mut i = 0;
    if chars.get(1).is_some_and(is_align) {
        (spec.fill, spec.align, i) = (Some(chars[0]), Some(chars[1]), 2);
    } else if chars.first().is_some_and(is_align) {
        (spec.align, i) = (Some(chars[0]), 1);
    }
    if let Some(&c @ ('+' | '-' | ' ')) = chars.get(i) {
        (spec.sign, i) = (Some(c), i + 1);
    }
    if chars.get(i) == Some(&'#') {
        (spec.alt, i) = (true, i + 1);
    }
    if chars.get(i) == Some(&'0') {
        (spec.zero, i) = (true, i + 1);
    }
    (spec.width, i) = read_number(&chars, i);
    if let Some(&c @ (',' | '_')) = chars.get(i) {
        (spec.grouping, i) = (Some(c), i + 1);
    }
    if chars.get(i) == Some(&'.') {
        if !chars.get(i + 1).is_some_and(char::is_ascii_digit) {
            return None;
        }
        let (precision, next) = read_number(&chars, i + 1);
        (spec.precision, i) = (Some(precision), next);
    }
    if let Some(&c) = chars.get(i) {
        (spec.kind, i) = (Some(c), i + 1);
    }
    (i == chars.len()).then_some(spec)
}

/// Reads decimal digits from chars at i: their value and where they end.
fn read_number(chars: &[char], mut i: usize) -> (usize, usize) {
    let mut n = 0usize;
    while let Some(d) = chars.get(i).and_then(|c| c.to_digit(10)) {
        n = n.saturating_mul(10).saturating_add(d as usize);
        i += 1;
    }
    (n, i)
}

/// Centers or justifies text to width characters with fill: '<' and '>' justify, '^'
/// centers as str.center does and 'c' as format's '^' does. The padded text is sized
/// first and allocated once.
pub(crate) fn pad(
    s: &State<'_>,
    text: &str,
    width: usize,
    fill: &str,
    align: char,
) -> Result<String> {
    let n = char_count(text);
    if width <= n || fill.is_empty() {
        return Ok(text.to_string());
    }
    let total = width - n;
    let left = match align {
        '>' => total,
        '^' => total / 2 + (total & width & 1),
        'c' => total / 2,
        _ => 0,
    };
    let size = sum_bytes(s, reserve_bytes(s, total, fill.len())?, text.len())?;
    let mut out = String::new();
    reserve_text(s, &mut out, size)?;
    (0..left).for_each(|_| out.push_str(fill));
    out.push_str(text);
    (left..total).for_each(|_| out.push_str(fill));
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// printf-style formatting: "%s and %d" % (a, b)

/// `text % args`; the result is Markup (with the arguments escaped) when markup is set.
pub(crate) fn percent_format(
    s: &State<'_>,
    markup: bool,
    text: &str,
    args: &Value,
) -> Result<Value> {
    let positional = match args {
        Value::List(l) if l.is_tuple() => l.snapshot(),
        _ => vec![args.clone()],
    };
    let mapping = match args {
        Value::Dict(d) => Some(d.clone()),
        _ => None,
    };
    let mut out = String::with_capacity(text.len());
    let mut next = 0;
    let mut rest = text;
    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        rest = &rest[at + 1..];
        if rest.is_empty() {
            return Err(s.error(ErrorKind::Type, "incomplete format"));
        }
        if let Some(after) = rest.strip_prefix('%') {
            out.push('%');
            rest = after;
            continue;
        }
        let mut value = None;
        if let Some(keyed) = rest.strip_prefix('(') {
            let end = keyed
                .find(')')
                .ok_or_else(|| s.error(ErrorKind::Type, "incomplete format key"))?;
            let d = mapping
                .as_ref()
                .ok_or_else(|| s.error(ErrorKind::Type, "format requires a mapping"))?;
            let key = &keyed[..end];
            let v = d
                .get_str(key)
                .ok_or_else(|| s.error(ErrorKind::Type, format!("KeyError: {}", quoted(key))))?;
            value = Some(v);
            rest = &keyed[end + 1..];
        }
        let (spec, used) = parse_percent_spec(s, rest, &positional, &mut next)?;
        rest = &rest[used..];
        let value = match value {
            Some(v) => v,
            None => {
                let v = positional.get(next).cloned().ok_or_else(|| {
                    s.error(ErrorKind::Type, "not enough arguments for format string")
                })?;
                next += 1;
                v
            }
        };
        write_percent_field(s, &mut out, &value, spec, markup)?;
    }
    out.push_str(rest);
    if mapping.is_none() && next < positional.len() {
        let message = "not all arguments converted during string formatting";
        return Err(s.error(ErrorKind::Type, message));
    }
    Ok(if markup { Value::Markup(Rc::from(out)) } else { Value::from(out) })
}

/// Parses the flags, width, precision and type of a % field: the spec and its length.
fn parse_percent_spec(
    s: &State<'_>,
    text: &str,
    args: &[Value],
    next: &mut usize,
) -> Result<(Spec, usize)> {
    let mut spec = Spec::default();
    let bytes = text.as_bytes();
    let mut i = 0;
    while let Some(&c) = bytes.get(i).filter(|c| b"-+ #0".contains(c)) {
        match c {
            b'-' => spec.align = Some('<'),
            b'+' => spec.sign = Some('+'),
            b' ' => spec.sign = spec.sign.or(Some(' ')),
            b'#' => spec.alt = true,
            _ => spec.zero = true,
        }
        i += 1;
    }
    let read_digits = |i: &mut usize, n: &mut usize| {
        while let Some(d) = bytes.get(*i).filter(|c| c.is_ascii_digit()) {
            *n = n.saturating_mul(10).saturating_add(usize::from(d - b'0'));
            *i += 1;
        }
    };
    if bytes.get(i) == Some(&b'*') {
        spec.width = star_argument(s, args, next)?;
        i += 1;
    }
    read_digits(&mut i, &mut spec.width);
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        let mut precision = 0;
        if bytes.get(i) == Some(&b'*') {
            precision = star_argument(s, args, next)?;
            i += 1;
        }
        read_digits(&mut i, &mut precision);
        spec.precision = Some(precision);
    }
    while bytes.get(i).is_some_and(|c| b"hlL".contains(c)) {
        i += 1;
    }
    let kind =
        text[i..].chars().next().ok_or_else(|| s.error(ErrorKind::Type, "incomplete format"))?;
    spec.kind = Some(kind);
    if spec.align == Some('<') {
        spec.zero = false;
    }
    Ok((spec, i + kind.len_utf8()))
}

/// The value of a `*` width or precision, taken from the arguments.
fn star_argument(s: &State<'_>, args: &[Value], next: &mut usize) -> Result<usize> {
    let v = args
        .get(*next)
        .ok_or_else(|| s.error(ErrorKind::Type, "not enough arguments for format string"))?;
    *next += 1;
    match as_number(v) {
        Some(Num::Int(n)) => Ok(usize::try_from(n).unwrap_or(0)),
        _ => Err(s.error(ErrorKind::Type, "* wants int")),
    }
}

fn write_percent_field(
    s: &State<'_>,
    out: &mut String,
    v: &Value,
    mut spec: Spec,
    markup: bool,
) -> Result<()> {
    let kind = spec.kind.unwrap_or('s');
    if matches!(kind, 's' | 'r' | 'a') {
        let mut text = if kind == 's' { to_str(v).into_owned() } else { repr(v) };
        if markup && !(kind == 's' && v.is_markup()) {
            text = escape_string(&text).into_owned();
        }
        spec.kind = None;
        spec.zero = false;
        spec.align = spec.align.or(Some('>'));
        return format_value(s, out, &Value::from(text), &spec);
    }
    if !"diufFeEgGxXoc".contains(kind) {
        return Err(s.error(ErrorKind::Type, format!("unsupported format character '{kind}'")));
    }
    let Some(n) = as_number(v) else {
        let message = format!("%{kind} format: a real number is required, not {}", type_name(v));
        return Err(s.error(ErrorKind::Type, message));
    };
    match (kind, n) {
        ('d' | 'i' | 'u', _) => {
            spec.kind = Some('d');
            spec.precision = None;
            let i = match n {
                Num::Int(i) => i,
                Num::Float(f) => f as i64,
            };
            format_value(s, out, &Value::Int(i), &spec)
        }
        ('x' | 'X' | 'o' | 'c', Num::Float(_)) => {
            let message = format!("%{kind} format: an integer is required, not float");
            Err(s.error(ErrorKind::Type, message))
        }
        ('x' | 'X' | 'o' | 'c', Num::Int(i)) => format_value(s, out, &Value::Int(i), &spec),
        _ => format_value(s, out, &Value::Float(n.f()), &spec),
    }
}

// ---------------------------------------------------------------------------------------
// str.format: "{} {name:>8}".format(a, name=b)

/// The arguments replacement fields read.
struct Fields<'a> {
    positional: &'a [Value],
    kwargs: &'a [Keyword],
}

/// `text.format(...)` or `text.format_map(mapping)`.
pub(crate) fn str_format(c: &mut Call<'_, '_>, text: &str, markup: bool) -> Result<Value> {
    let out = if &*c.name == "format_map" {
        let Value::Dict(d) = c.arg(0, "mapping") else {
            return Err(c.error("format_map() argument must be a mapping").into());
        };
        let keyword =
            |(k, v): (Value, Value)| Keyword { name: Rc::from(to_str(&k).as_ref()), value: v };
        let kwargs: Vec<Keyword> = d.entries().into_iter().map(keyword).collect();
        format_with(c.state_ref(), text, &Fields { positional: &[], kwargs: &kwargs }, markup)?
    } else {
        let fields = Fields { positional: &c.args, kwargs: &c.kwargs };
        format_with(c.state_ref(), text, &fields, markup)?
    };
    Ok(if markup { Value::Markup(Rc::from(out)) } else { Value::from(out) })
}

/// Expands the replacement fields of text.
fn format_with(s: &State<'_>, text: &str, fields: &Fields<'_>, markup: bool) -> Result<String> {
    let mut out = String::with_capacity(text.len());
    let mut auto = 0;
    let mut rest = text;
    while let Some(at) = rest.find(['{', '}']) {
        out.push_str(&rest[..at]);
        let brace = rest.as_bytes()[at];
        rest = &rest[at + 1..];
        if rest.as_bytes().first() == Some(&brace) {
            out.push(char::from(brace));
            rest = &rest[1..];
            continue;
        }
        if brace == b'}' {
            return Err(s.error(ErrorKind::Runtime, "Single '}' encountered in format string"));
        }
        let end = field_end(rest)
            .ok_or_else(|| s.error(ErrorKind::Runtime, "expected '}' before end of string"))?;
        format_field(s, &mut out, &rest[..end], fields, &mut auto, markup)?;
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Where the replacement field that text starts inside ends (its closing brace).
fn field_end(text: &str) -> Option<usize> {
    let mut depth = 1;
    for (i, b) in text.bytes().enumerate() {
        match b {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return Some(i);
        }
    }
    None
}

fn format_field(
    s: &State<'_>,
    out: &mut String,
    field: &str,
    fields: &Fields<'_>,
    auto: &mut usize,
    markup: bool,
) -> Result<()> {
    let (name, spec_text) = field.split_once(':').unwrap_or((field, ""));
    let (name, conversion) = name.split_once('!').unwrap_or((name, ""));
    let spec_text = if spec_text.contains('{') {
        Cow::Owned(format_with(s, spec_text, fields, false)?)
    } else {
        Cow::Borrowed(spec_text)
    };
    let (root, path) = name.split_at(name.find(['.', '[']).unwrap_or(name.len()));
    let v = format_lookup(s, field_root(s, root, fields, auto)?, path)?;
    let v = match conversion {
        "r" | "a" => Value::from(repr(&v)),
        "s" => Value::from(to_str(&v).into_owned()),
        _ => v,
    };
    let spec = parse_format_spec(&spec_text).ok_or_else(|| {
        s.error(ErrorKind::Runtime, format!("Invalid format specifier {}", quoted(&spec_text)))
    })?;
    if !markup || v.is_markup() {
        return format_value(s, out, &v, &spec);
    }
    let mut plain = String::new();
    format_value(s, &mut plain, &v, &spec)?;
    write_escaped(out, &plain);
    Ok(())
}

/// The argument a replacement field names: by position (or the next one) or keyword.
fn field_root(s: &State<'_>, root: &str, fields: &Fields<'_>, auto: &mut usize) -> Result<Value> {
    if root.is_empty() || root.as_bytes()[0].is_ascii_digit() {
        let index = if root.is_empty() {
            *auto += 1;
            *auto - 1
        } else {
            root.parse().unwrap_or(usize::MAX)
        };
        return fields.positional.get(index).cloned().ok_or_else(|| {
            let message =
                format!("Replacement index {index} out of range for positional args tuple");
            s.error(ErrorKind::Runtime, message)
        });
    }
    let found = fields.kwargs.iter().find(|kw| &*kw.name == root);
    found
        .map(|kw| kw.value.clone())
        .ok_or_else(|| s.error(ErrorKind::Runtime, format!("KeyError: {}", quoted(root))))
}

/// Follows the ".attr" and "[key]" parts of a replacement field.
fn format_lookup(s: &State<'_>, mut v: Value, path: &str) -> Result<Value> {
    let mut rest = path;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('.') {
            let end = after.find(['.', '[']).unwrap_or(after.len());
            v = get_attr(s, &v, &after[..end])?;
            rest = &after[end..];
        } else if let Some(after) = rest.strip_prefix('[') {
            let end = after
                .find(']')
                .ok_or_else(|| s.error(ErrorKind::Runtime, "Missing ']' in format string"))?;
            let key = &after[..end];
            let key = key.parse::<i64>().map_or_else(|_| Value::str(key), Value::Int);
            v = get_item(s, &v, &key)?;
            rest = &after[end + 1..];
        } else {
            let message = "Only '.' or '[' may follow ']' in format field";
            return Err(s.error(ErrorKind::Runtime, message));
        }
    }
    Ok(v)
}
