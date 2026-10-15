//! Filters over text and numbers.

use std::rc::Rc;

use crate::calls::Call;
use crate::convert::{Num, as_number, char_count, char_slice, repr, to_str, type_name};
use crate::environment::Environment;
use crate::escape::escaped_text;
use crate::format::{fixed_float, pad, percent_format};
use crate::lexer::parse_digits;
use crate::runtime::Result;
use crate::text::{self, WrapOptions};
use crate::value::{Dict, DictKind, Value};

/// What the built-in filters, tests and methods return.
pub(crate) type Outcome = std::result::Result<Value, crate::Error>;

/// The signature of the built-in filters, tests and global functions.
pub(crate) type Builtin = fn(&mut Call<'_, '_>) -> Outcome;

pub(crate) fn register(env: &mut Environment) {
    let filters: &[(&str, Builtin)] = &[
        ("abs", filter_abs),
        ("capitalize", filter_case),
        ("lower", filter_case),
        ("upper", filter_case),
        ("title", filter_case),
        ("center", filter_center),
        ("filesizeformat", filter_filesizeformat),
        ("float", filter_float),
        ("format", filter_format),
        ("indent", filter_indent),
        ("int", filter_int),
        ("pprint", |c| Ok(Value::from(repr(&c.arg(0, "value"))))),
        ("replace", filter_replace),
        ("round", filter_round),
        ("string", filter_string),
        ("trim", filter_trim),
        ("truncate", filter_truncate),
        ("wordcount", filter_wordcount),
        ("wordwrap", filter_wordwrap),
    ];
    for &(name, f) in filters {
        env.add_filter(name, f);
    }
}

/// Argument 0 as text, and whether it is Markup (Jinja's soft_str).
pub(crate) fn text_arg(c: &Call<'_, '_>) -> Result<(String, bool)> {
    let v = c.arg(0, "s");
    c.state_ref().strict_check(&v)?;
    Ok((to_str(&v).into_owned(), v.is_markup()))
}

/// text as a value, Markup when markup is set.
pub(crate) fn keep_markup(text: String, markup: bool) -> Value {
    if markup { Value::Markup(Rc::from(text)) } else { Value::from(text) }
}

fn filter_abs(c: &mut Call<'_, '_>) -> Outcome {
    let v = c.arg(0, "x");
    match as_number(&v) {
        Some(Num::Int(i)) => Ok(i
            .checked_abs()
            .map(Value::Int)
            .ok_or_else(|| crate::eval::overflow_error(c.state_ref()))?),
        Some(Num::Float(f)) => Ok(Value::Float(f.abs())),
        None => Err(c.error(&format!("bad operand type for abs(): '{}'", type_name(&v)))),
    }
}

fn filter_case(c: &mut Call<'_, '_>) -> Outcome {
    let (text, markup) = text_arg(c)?;
    let out = match &*c.name {
        "capitalize" => text::capitalize(&text),
        "lower" => text::to_lower(&text),
        "upper" => text::to_upper(&text),
        _ => text::title_case(&text),
    };
    Ok(keep_markup(out, markup))
}

fn filter_center(c: &mut Call<'_, '_>) -> Outcome {
    let (text, markup) = text_arg(c)?;
    let width = c.arg_count(1, "width", 80)?;
    Ok(keep_markup(pad(c.state_ref(), &text, width, " ", '^')?, markup))
}

fn filter_filesizeformat(c: &mut Call<'_, '_>) -> Outcome {
    let v = c.arg(0, "value");
    let size = match as_number(&v) {
        Some(n) => n.f(),
        None => to_str(&v)
            .trim()
            .parse::<f64>()
            .map_err(|_| c.error("could not convert string to float"))?,
    };
    let binary = c.arg_flag(1, "binary", false);
    let base: f64 = if binary { 1024.0 } else { 1000.0 };
    let prefixes = if binary {
        ["KiB", "MiB", "GiB", "TiB", "PiB", "EiB", "ZiB", "YiB"]
    } else {
        ["kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"]
    };
    if size == 1.0 {
        return Ok(Value::str("1 Byte"));
    }
    if size < base {
        return Ok(Value::from(format!("{} Bytes", size as i64)));
    }
    // The first unit the size is below, or the largest.
    let (unit, prefix) = (2..)
        .zip(prefixes)
        .map(|(power, prefix)| (base.powi(power), prefix))
        .find(|(unit, _)| size < *unit)
        .unwrap_or((base.powi(9), prefixes[7]));
    Ok(Value::from(format!("{} {prefix}", fixed_float(base * size / unit, 1))))
}

/// Converts like Jinja's float filter; None when conversion fails.
fn to_float(v: &Value) -> Option<f64> {
    if let Some(n) = as_number(v) {
        return Some(n.f());
    }
    let t = v.as_str()?.trim();
    match text::to_lower(t).as_str() {
        "inf" | "+inf" | "infinity" => Some(f64::INFINITY),
        "-inf" | "-infinity" => Some(f64::NEG_INFINITY),
        "nan" | "+nan" | "-nan" => Some(f64::NAN),
        _ => t.replace('_', "").parse().ok(),
    }
}

fn filter_float(c: &mut Call<'_, '_>) -> Outcome {
    Ok(match to_float(&c.arg(0, "value")) {
        Some(f) => Value::Float(f),
        None => c.arg_or(1, "default", Value::Float(0.0)),
    })
}

/// Parses int(text, base), with prefixes, signs and underscores.
fn parse_python_int(text: &str, base: i64) -> Option<i64> {
    let t = text.trim();
    let (negative, t) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let mut b = u32::try_from(base).ok()?;
    let mut digits = t;
    if t.len() > 2 && t.starts_with('0') {
        let prefix_base = match t.as_bytes()[1] | 0x20 {
            b'x' => 16,
            b'o' => 8,
            b'b' => 2,
            _ => 0,
        };
        if prefix_base != 0 && (b == 0 || b == prefix_base) {
            b = prefix_base;
            digits = &t[2..];
        }
    }
    if b == 0 {
        b = 10;
    }
    if digits.is_empty() || digits.starts_with('_') || digits.ends_with('_') {
        return None;
    }
    let v = parse_digits(&digits.replace('_', ""), b)?;
    Some(if negative { -v } else { v })
}

fn filter_int(c: &mut Call<'_, '_>) -> Outcome {
    let v = c.arg(0, "value");
    let default = c.arg_or(1, "default", Value::Int(0));
    let base = c.arg_int(2, "base", 10)?;
    Ok(match as_number(&v) {
        Some(Num::Int(i)) => Value::Int(i),
        Some(Num::Float(f)) if f.is_finite() => Value::Int(f as i64),
        Some(Num::Float(_)) => default,
        None => {
            let Some(text) = v.as_str() else { return Ok(default) };
            match parse_python_int(text, base) {
                Some(n) => Value::Int(n),
                None => match text.trim().parse::<f64>() {
                    Ok(f) if f.is_finite() => Value::Int(f as i64),
                    _ => default,
                },
            }
        }
    })
}

fn filter_format(c: &mut Call<'_, '_>) -> Outcome {
    if c.args.len() > 1 && !c.kwargs.is_empty() {
        return Err(c.error("can't handle positional and keyword arguments at the same time"));
    }
    let v = c.arg(0, "value");
    let params = if c.kwargs.is_empty() {
        Value::tuple(c.args.iter().skip(1).cloned())
    } else {
        let d = Dict::new(DictKind::Dict);
        for kw in &c.kwargs {
            d.set(Value::from(kw.name.clone()), kw.value.clone());
        }
        Value::Dict(d)
    };
    Ok(percent_format(c.state_ref(), v.is_markup(), &to_str(&v), &params)?)
}

fn filter_indent(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("indent", &["s", "width", "first", "blank"], 0)?;
    let (text, markup) = text_arg(c)?;
    let indention = match c.arg_or(1, "width", Value::Int(4)) {
        Value::Str(w) | Value::Markup(w) => w.to_string(),
        _ => crate::limits::repeat_text(c.state_ref(), " ", c.arg_count(1, "width", 4)?)?,
    };
    let joined = format!("{text}\n");
    let lines = text::splitlines(&joined, false);
    // The indented text is sized first, and allocated once.
    let s = c.state_ref();
    let indents = crate::limits::reserve_bytes(s, lines.len() + 1, indention.len())?;
    let size = crate::limits::sum_bytes(s, indents, text.len() + lines.len())?;
    let mut out = String::new();
    crate::limits::reserve_text(s, &mut out, size)?;
    let blank = c.arg_flag(3, "blank", false);
    if c.arg_flag(2, "first", false) {
        out.push_str(&indention);
    }
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
            if blank || !line.is_empty() {
                out.push_str(&indention);
            }
        }
        out.push_str(line);
    }
    Ok(keep_markup(out, markup))
}

fn filter_replace(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("replace", &["s", "old", "new", "count"], 3)?;
    let (v, old, new) = (c.arg(0, "s"), c.arg(1, "old"), c.arg(2, "new"));
    let count = c.arg_int(3, "count", -1)?;
    if c.autoescaping() && (old.is_markup() || new.is_markup() || v.is_markup()) {
        let out =
            text::replace(&escaped_text(&v), &escaped_text(&old), &escaped_text(&new), count);
        return Ok(Value::Markup(Rc::from(out)));
    }
    Ok(Value::from(text::replace(&to_str(&v), &to_str(&old), &to_str(&new), count)))
}

/// Rounds half to even on the exact binary value, like Python's round().
fn python_round(f: f64, precision: i64) -> f64 {
    if !f.is_finite() {
        return f;
    }
    if precision < 0 {
        let scale = 10f64.powi(i32::try_from(-precision).unwrap_or(i32::MAX));
        return python_round(f / scale, 0) * scale;
    }
    let digits = usize::try_from(precision).unwrap_or(0).min(400);
    let v: f64 = fixed_float(f, digits).parse().unwrap_or(0.0);
    if f < 0.0 { -v } else { v }
}

fn filter_round(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("round", &["value", "precision", "method"], 0)?;
    let v = c.arg(0, "value");
    let precision = c.arg_int(1, "precision", 0)?;
    let method = to_str(&c.arg_or(2, "method", Value::str("common"))).into_owned();
    let Some(n) = as_number(&v) else {
        return Err(c.error(&format!("type {} doesn't define __round__ method", type_name(&v))));
    };
    match (method.as_str(), n) {
        ("common", Num::Int(i)) => Ok(Value::Int(i)),
        ("common", Num::Float(f)) => Ok(Value::Float(python_round(f, precision))),
        ("ceil" | "floor", _) => {
            let scale = 10f64.powi(i32::try_from(precision).unwrap_or(0));
            let x = n.f() * scale;
            let r = if method == "ceil" { x.ceil() } else { x.floor() };
            Ok(Value::Float(r / scale))
        }
        _ => Err(c.error("method must be common, ceil or floor")),
    }
}

fn filter_string(c: &mut Call<'_, '_>) -> Outcome {
    let v = c.arg(0, "value");
    Ok(if v.is_markup() { v } else { Value::from(to_str(&v).into_owned()) })
}

fn filter_trim(c: &mut Call<'_, '_>) -> Outcome {
    let (text, markup) = text_arg(c)?;
    let chars = c.arg_or(1, "chars", Value::None);
    Ok(keep_markup(text::strip(&text, chars.as_str(), true, true).to_string(), markup))
}

fn filter_truncate(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("truncate", &["s", "length", "killwords", "end", "leeway"], 0)?;
    let (text, _) = text_arg(c)?;
    let size = c.arg_int(1, "length", 255)?;
    let end = to_str(&c.arg_or(3, "end", Value::str("..."))).into_owned();
    let leeway = c.arg_int(4, "leeway", 5)?;
    let end_n = char_count(&end) as i64;
    if size < end_n {
        return Err(c.error(&format!("expected length >= {end_n}, got {size}")));
    }
    if leeway < 0 {
        return Err(c.error(&format!("expected leeway >= 0, got {leeway}")));
    }
    if char_count(&text) as i64 <= size.saturating_add(leeway) {
        return Ok(Value::from(text));
    }
    let mut head = char_slice(&text, 0, (size - end_n) as usize);
    if !c.arg_flag(2, "killwords", false)
        && let Some(space) = head.rfind(' ')
    {
        head = &head[..space];
    }
    Ok(Value::from(format!("{head}{end}")))
}

fn filter_wordcount(c: &mut Call<'_, '_>) -> Outcome {
    let (text, _) = text_arg(c)?;
    let mut count = 0;
    let mut in_word = false;
    for ch in text.chars() {
        let word = text::is_word_char(ch);
        if word && !in_word {
            count += 1;
        }
        in_word = word;
    }
    Ok(Value::Int(count))
}

fn filter_wordwrap(c: &mut Call<'_, '_>) -> Outcome {
    let names = ["s", "width", "break_long_words", "wrapstring", "break_on_hyphens"];
    c.check_args("wordwrap", &names, 0)?;
    let (text, _) = text_arg(c)?;
    let width = c.arg_int(1, "width", 79)?;
    if width <= 0 {
        return Err(c.error(&format!("invalid width {width} (must be > 0)")));
    }
    let opts = WrapOptions {
        width: width as usize,
        break_long_words: c.arg_flag(2, "break_long_words", true),
        break_on_hyphens: c.arg_flag(4, "break_on_hyphens", true),
    };
    let wrapstring = match c.arg_or(3, "wrapstring", Value::None) {
        Value::None => "\n".to_string(),
        v => to_str(&v).into_owned(),
    };
    let lines: Vec<String> = text::splitlines(&text, false)
        .iter()
        .map(|line| text::wrap(line, &opts).join(&wrapstring))
        .collect();
    Ok(Value::from(lines.join(&wrapstring)))
}
