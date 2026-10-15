//! Python's str(), repr(), truthiness, numbers, equality and ordering.

use std::borrow::Cow;
use std::fmt::Write as _;

use crate::value::{CallKind, Dict, DictKind, List, Value};

/// How deep repr, equality, ordering and tojson follow nested lists and dicts.
pub const MAX_VALUE_DEPTH: usize = 200;

/// Converts a value as Python's `str()` does (Undefined gives ""). A string or Markup
/// value is borrowed; anything else is written into a new string.
pub fn to_str(v: &Value) -> Cow<'_, str> {
    match v {
        Value::Str(s) | Value::Markup(s) => Cow::Borrowed(s),
        _ => {
            let mut out = String::new();
            write_str(&mut out, v);
            Cow::Owned(out)
        }
    }
}

/// Converts a value as Python's `repr()` does.
pub fn repr(v: &Value) -> String {
    let mut out = String::new();
    write_repr(&mut out, v, 0);
    out
}

/// Appends `str(v)` to out.
pub(crate) fn write_str(out: &mut String, v: &Value) {
    match v {
        Value::Undefined(_) => {}
        Value::None => out.push_str("None"),
        Value::Bool(b) => out.push_str(if *b { "True" } else { "False" }),
        Value::Int(i) => {
            let _ = write!(out, "{i}");
        }
        Value::Float(f) => write_float_repr(out, *f),
        Value::Str(s) | Value::Markup(s) => out.push_str(s),
        _ => write_repr(out, v, 0),
    }
}

/// Appends `repr(v)` to out, following nested values `depth` levels down already.
pub(crate) fn write_repr(out: &mut String, v: &Value, depth: usize) {
    match v {
        Value::Undefined(_) => out.push_str("Undefined"),
        Value::None | Value::Bool(_) | Value::Int(_) | Value::Float(_) => write_str(out, v),
        Value::Str(s) => write_string_repr(out, s),
        Value::Markup(s) => {
            out.push_str("Markup(");
            write_string_repr(out, s);
            out.push(')');
        }
        Value::List(l) => write_list_repr(out, l, depth),
        Value::Dict(d) => match d.kind() {
            DictKind::Dict => write_dict_repr(out, d, depth),
            DictKind::Namespace => {
                out.push_str("<Namespace ");
                write_dict_repr(out, d, depth);
                out.push('>');
            }
            DictKind::Module => {
                out.push_str("<TemplateModule ");
                write_string_repr(out, d.name());
                out.push('>');
            }
        },
        Value::Callable(c) => {
            if c.is_macro() {
                out.push_str("<Macro ");
                write_string_repr(out, c.name());
                out.push('>');
            } else {
                let _ = write!(out, "<built-in function {}>", c.name());
            }
        }
        Value::Loop(l) => {
            let _ = write!(out, "<LoopContext {}/{}>", l.index0() + 1, l.len());
        }
        Value::Cycler(_) => out.push_str("<Cycler object>"),
    }
}

/// Appends the repr of a list; past MAX_VALUE_DEPTH (a list that contains itself) it
/// prints Python's `[...]`.
pub(crate) fn write_list_repr(out: &mut String, l: &List, depth: usize) {
    let tuple = l.is_tuple();
    if depth >= MAX_VALUE_DEPTH {
        out.push_str(if tuple { "(...)" } else { "[...]" });
        return;
    }
    out.push(if tuple { '(' } else { '[' });
    let items = l.items();
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_repr(out, item, depth + 1);
    }
    if tuple && items.len() == 1 {
        out.push(',');
    }
    out.push(if tuple { ')' } else { ']' });
}

/// Appends the repr of a dict's entries.
pub(crate) fn write_dict_repr(out: &mut String, d: &Dict, depth: usize) {
    if depth >= MAX_VALUE_DEPTH {
        out.push_str("{...}");
        return;
    }
    out.push('{');
    d.each(|keys, values| {
        for (i, (k, v)) in keys.iter().zip(values).enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            write_repr(out, k, depth + 1);
            out.push_str(": ");
            write_repr(out, v, depth + 1);
        }
    });
    out.push('}');
}

/// Quotes like Python: single quotes unless the text holds a single quote and no
/// double quote.
pub(crate) fn write_string_repr(out: &mut String, s: &str) {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ if c == quote => {
                out.push('\\');
                out.push(c);
            }
            _ if (c as u32) < 0x20 || c == '\x7f' || ('\u{80}'..'\u{a0}').contains(&c) => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            _ => out.push(c),
        }
    }
    out.push(quote);
}

/// The quoted repr of a string, for messages.
pub(crate) fn quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    write_string_repr(&mut out, s);
    out
}

/// The shortest decimal digits that round-trip f, and the position of the decimal point
/// relative to the first digit (Python's decpt).
fn shortest_digits(f: f64) -> (String, i32) {
    let text = format!("{:e}", f.abs());
    let (mantissa, exponent) = text.split_once('e').unwrap_or((&text, "0"));
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    (digits, exponent.parse::<i32>().unwrap_or(0) + 1)
}

/// Appends a float as Python's repr(): 1.0, 0.1, 1e+16, 1.5e-05.
pub(crate) fn write_float_repr(out: &mut String, f: f64) {
    if f.is_nan() {
        out.push_str("nan");
        return;
    }
    if f.is_infinite() {
        out.push_str(if f > 0.0 { "inf" } else { "-inf" });
        return;
    }
    if f.is_sign_negative() {
        out.push('-');
    }
    if f == 0.0 {
        out.push_str("0.0");
        return;
    }
    let (digits, point) = shortest_digits(f);
    let n = digits.len() as i32;
    if point <= -4 || point > 16 {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        write_exponent(out, point - 1);
    } else if point <= 0 {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', (-point) as usize));
        out.push_str(&digits);
    } else if point >= n {
        out.push_str(&digits);
        out.extend(std::iter::repeat_n('0', (point - n) as usize));
        out.push_str(".0");
    } else {
        out.push_str(&digits[..point as usize]);
        out.push('.');
        out.push_str(&digits[point as usize..]);
    }
}

/// Appends an exponent as Python writes it: e+05, e-10.
pub(crate) fn write_exponent(out: &mut String, exponent: i32) {
    out.push('e');
    out.push(if exponent < 0 { '-' } else { '+' });
    let _ = write!(out, "{:02}", exponent.unsigned_abs());
}

// ---------------------------------------------------------------------------------------
// Truth, numbers, equality and ordering

/// Python truthiness (Undefined is false).
pub(crate) fn truthy(v: &Value) -> bool {
    match v {
        Value::Undefined(_) | Value::None => false,
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::Str(s) | Value::Markup(s) => !s.is_empty(),
        Value::List(l) => !l.is_empty(),
        Value::Dict(d) => d.kind() != DictKind::Dict || !d.is_empty(),
        Value::Callable(_) | Value::Loop(_) | Value::Cycler(_) => true,
    }
}

/// The Python type name used in Jinja's error messages.
pub(crate) fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Undefined(_) => "Undefined",
        Value::None => "NoneType",
        Value::Bool(_) => "bool",
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::Str(_) => "str",
        Value::Markup(_) => "Markup",
        Value::List(l) if l.is_tuple() => "tuple",
        Value::List(_) => "list",
        Value::Dict(d) => match d.kind() {
            DictKind::Dict => "dict",
            DictKind::Namespace => "Namespace",
            DictKind::Module => "TemplateModule",
        },
        Value::Callable(c) => match c.kind {
            CallKind::Macro(_) => "Macro",
            CallKind::Method(_) => "method",
            _ => "builtin_function_or_method",
        },
        Value::Loop(_) => "LoopContext",
        Value::Cycler(_) => "Cycler",
    }
}

/// How Jinja names a value that lacks an attribute: "dict object".
pub(crate) fn object_name(v: &Value) -> String {
    if v.is_none() {
        return "None".to_string();
    }
    format!("{} object", type_name(v))
}

/// A number read from a bool, int or float value (bools are ints, as in Python).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Num {
    Int(i64),
    Float(f64),
}

impl Num {
    pub(crate) fn f(self) -> f64 {
        match self {
            Num::Int(i) => i as f64,
            Num::Float(f) => f,
        }
    }
}

/// Reads bool, int and float values as numbers.
pub(crate) fn as_number(v: &Value) -> Option<Num> {
    match v {
        Value::Bool(b) => Some(Num::Int(i64::from(*b))),
        Value::Int(i) => Some(Num::Int(*i)),
        Value::Float(f) => Some(Num::Float(*f)),
        _ => None,
    }
}

/// An integer argument read from a bool or int (not a float).
pub(crate) fn as_int(v: &Value) -> Option<i64> {
    match as_number(v)? {
        Num::Int(i) => Some(i),
        Num::Float(_) => None,
    }
}

/// Compares as Python's == does: lists and dicts by content, 200 levels deep.
pub(crate) fn values_equal(a: &Value, b: &Value) -> bool {
    equal_at(a, b, 0)
}

/// Compares values nested depth levels deep. Like Python, a list or dict equals itself;
/// values nested past MAX_VALUE_DEPTH compare unequal.
fn equal_at(a: &Value, b: &Value, depth: usize) -> bool {
    if depth > MAX_VALUE_DEPTH {
        return false;
    }
    if let (Some(x), Some(y)) = (as_number(a), as_number(b)) {
        return match (x, y) {
            (Num::Int(i), Num::Int(j)) => i == j,
            _ => x.f() == y.f(),
        };
    }
    if let Some(sa) = a.as_str() {
        return b.as_str() == Some(sa);
    }
    match (a, b) {
        (Value::Undefined(_), _) => b.is_undefined(),
        (Value::None, _) => b.is_none(),
        (Value::List(x), Value::List(y)) => {
            if std::rc::Rc::ptr_eq(x, y) {
                return true;
            }
            let (xs, ys) = (x.items(), y.items());
            x.is_tuple() == y.is_tuple()
                && xs.len() == ys.len()
                && xs.iter().zip(ys.iter()).all(|(p, q)| equal_at(p, q, depth + 1))
        }
        (Value::Dict(x), Value::Dict(y)) => dicts_equal(x, y, depth),
        (Value::Callable(x), Value::Callable(y)) => {
            std::rc::Rc::ptr_eq(x, y) || callables_equal(x, y, depth)
        }
        (Value::Loop(x), Value::Loop(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Cycler(x), Value::Cycler(y)) => std::rc::Rc::ptr_eq(x, y),
        _ => false,
    }
}

fn dicts_equal(x: &std::rc::Rc<Dict>, y: &std::rc::Rc<Dict>, depth: usize) -> bool {
    if x.len() != y.len() {
        return false;
    }
    if std::rc::Rc::ptr_eq(x, y) {
        return true;
    }
    if x.kind() != DictKind::Dict || y.kind() != DictKind::Dict {
        return false;
    }
    x.each(|keys, values| {
        keys.iter().zip(values).all(|(k, v)| match y.get(k) {
            Some(other) => equal_at(v, &other, depth + 1),
            None => false,
        })
    })
}

/// Bound methods are equal when they are the same method of equal receivers.
fn callables_equal(x: &crate::value::Callable, y: &crate::value::Callable, depth: usize) -> bool {
    match (&x.kind, &y.kind) {
        (CallKind::Method(a), CallKind::Method(b)) => {
            x.name == y.name && equal_at(a, b, depth + 1)
        }
        (CallKind::Native(a), CallKind::Native(b)) => {
            x.name == y.name && std::rc::Rc::ptr_eq(a, b)
        }
        (CallKind::Local, CallKind::Local) => x.name == y.name && x.render == y.render,
        _ => false,
    }
}

/// The order of two values as Python orders them: -1, 0 or 1, and 2 when a NaN makes
/// them unordered. None when Python raises TypeError ("'<' not supported between
/// instances of ..."), and for lists nested past MAX_VALUE_DEPTH.
pub(crate) fn compare_values(a: &Value, b: &Value) -> Option<i32> {
    compare_at(a, b, 0)
}

fn compare_at(a: &Value, b: &Value, depth: usize) -> Option<i32> {
    if depth > MAX_VALUE_DEPTH {
        return None;
    }
    if let (Some(x), Some(y)) = (as_number(a), as_number(b)) {
        if let (Num::Int(i), Num::Int(j)) = (x, y) {
            return Some(i.cmp(&j) as i32);
        }
        let (f, g) = (x.f(), y.f());
        return Some(f.partial_cmp(&g).map_or(2, |o| o as i32));
    }
    if let Some(sa) = a.as_str() {
        return Some(sa.cmp(b.as_str()?) as i32);
    }
    let (Value::List(la), Value::List(lb)) = (a, b) else { return None };
    if la.is_tuple() != lb.is_tuple() {
        return None;
    }
    let (xs, ys) = (la.items(), lb.items());
    for (p, q) in xs.iter().zip(ys.iter()) {
        if !equal_at(p, q, depth + 1) {
            return compare_at(p, q, depth + 1);
        }
    }
    Some(xs.len().cmp(&ys.len()) as i32)
}

// ---------------------------------------------------------------------------------------
// Characters

/// `s[start:end]` counted in characters, with Python's clamping.
pub(crate) fn char_slice(s: &str, start: usize, end: usize) -> &str {
    if start >= end {
        return "";
    }
    let byte_at = |n: usize| s.char_indices().nth(n).map_or(s.len(), |(i, _)| i);
    let lo = byte_at(start);
    let hi = lo + s[lo..].char_indices().nth(end - start).map_or(s.len() - lo, |(i, _)| i);
    &s[lo..hi]
}

/// The number of characters of s.
pub(crate) fn char_count(s: &str) -> usize {
    s.chars().count()
}
