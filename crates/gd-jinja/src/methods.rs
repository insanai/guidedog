//! Methods of Python's builtin types that templates call: `"a,b".split(",")`,
//! `d.items()`, `items.append(x)`.

use std::rc::Rc;

use crate::calls::Call;
use crate::convert::{Num, as_number, char_count, quoted, repr, to_str, type_name, values_equal};
use crate::escape::{escaped_text, striptags, unescape_html};
use crate::filters_text::{Outcome, keep_markup};
use crate::runtime::iterate;
use crate::text;
use crate::value::{Dict, DictKind, List, Value};

const STRING_METHODS: &[&str] = &[
    "upper",
    "lower",
    "title",
    "capitalize",
    "swapcase",
    "casefold",
    "strip",
    "lstrip",
    "rstrip",
    "split",
    "rsplit",
    "splitlines",
    "join",
    "replace",
    "startswith",
    "endswith",
    "find",
    "rfind",
    "index",
    "rindex",
    "count",
    "format",
    "format_map",
    "center",
    "ljust",
    "rjust",
    "zfill",
    "isdigit",
    "isalpha",
    "isalnum",
    "isspace",
    "islower",
    "isupper",
    "istitle",
    "isdecimal",
    "isnumeric",
    "isidentifier",
    "isascii",
    "partition",
    "rpartition",
    "removeprefix",
    "removesuffix",
    "expandtabs",
    "encode",
];

/// Whether strings (Markup when markup is set) have a method called name.
pub(crate) fn is_string_method(name: &str, markup: bool) -> bool {
    STRING_METHODS.contains(&name)
        || (markup && matches!(name, "striptags" | "unescape" | "escape"))
}

/// Whether lists (tuples when tuple is set) have a method called name.
pub(crate) fn is_list_method(name: &str, tuple: bool) -> bool {
    match name {
        "index" | "count" => true,
        "append" | "extend" | "insert" | "pop" | "remove" | "reverse" | "sort" | "clear"
        | "copy" => !tuple,
        _ => false,
    }
}

/// Whether dicts have a method called name.
pub(crate) fn is_dict_method(name: &str) -> bool {
    matches!(
        name,
        "items"
            | "keys"
            | "values"
            | "get"
            | "pop"
            | "popitem"
            | "setdefault"
            | "update"
            | "copy"
            | "clear"
    )
}

/// The procedure of every bound method value: c.this is the receiver.
pub(crate) fn dispatch_method(c: &mut Call<'_, '_>) -> Outcome {
    match c.this.clone() {
        Value::Str(text) => string_method(c, &text, false),
        Value::Markup(text) => string_method(c, &text, true),
        Value::List(l) => list_method(c, &l),
        Value::Dict(d) => dict_method(c, &d),
        Value::Loop(l) => crate::loops::loop_method(c, &l),
        Value::Cycler(cy) => {
            if &*c.name == "reset" {
                cy.pos.set(0);
                return Ok(Value::None);
            }
            let Some(v) = cy.items.get(cy.pos.get()).cloned() else { return Ok(Value::None) };
            cy.pos.set((cy.pos.get() + 1) % cy.items.len());
            Ok(v)
        }
        Value::Int(_) | Value::Float(_) | Value::Bool(_) => number_method(c),
        _ => Err(no_attribute(c)),
    }
}

fn no_attribute(c: &Call<'_, '_>) -> crate::Error {
    let what = type_name(&c.this);
    c.error(&format!("'{what}' object has no attribute {}", quoted(&c.name)))
}

fn number_method(c: &Call<'_, '_>) -> Outcome {
    match (&*c.name, as_number(&c.this)) {
        ("is_integer", Some(Num::Float(f))) => Ok(Value::Bool(f.fract() == 0.0)),
        ("is_integer", _) => Ok(Value::Bool(true)),
        ("conjugate", _) => Ok(c.this.clone()),
        ("bit_length", Some(Num::Int(i))) => {
            Ok(Value::Int(i64::from(64 - i.unsigned_abs().leading_zeros())))
        }
        _ => Err(no_attribute(c)),
    }
}

// ---------------------------------------------------------------------------------------
// str

fn string_method(c: &mut Call<'_, '_>, text: &str, markup: bool) -> Outcome {
    let wrap = |out: String| keep_markup(out, markup);
    Ok(match &*c.name {
        "upper" => wrap(text::to_upper(text)),
        "lower" | "casefold" => wrap(text::to_lower(text)),
        "title" => wrap(text::python_title(text)),
        "capitalize" => wrap(text::capitalize(text)),
        "swapcase" => wrap(text.chars().map(swap_case).collect()),
        "strip" | "lstrip" | "rstrip" => {
            let chars = c.arg_or(0, "chars", Value::None);
            let (left, right) = (&*c.name != "rstrip", &*c.name != "lstrip");
            wrap(text::strip(text, chars.as_str(), left, right).to_string())
        }
        "split" | "rsplit" => split_method(c, text, markup)?,
        "splitlines" => {
            let lines = text::splitlines(text, c.arg_flag(0, "keepends", false));
            Value::list(lines.into_iter().map(|l| wrap(l.to_string())))
        }
        "join" => join_method(c, text, markup)?,
        "replace" => {
            let (old, new) = (c.arg(0, "old"), c.arg(1, "new"));
            let count = c.arg_int(2, "count", -1)?;
            if markup {
                wrap(text::replace(text, &escaped_text(&old), &escaped_text(&new), count))
            } else {
                Value::from(text::replace(text, &to_str(&old), &to_str(&new), count))
            }
        }
        "format" | "format_map" => crate::format::str_format(c, text, markup)?,
        _ => return string_method_more(c, text, markup),
    })
}

fn swap_case(ch: char) -> char {
    if ch.is_uppercase() { text::lower_char(ch) } else { text::upper_char(ch) }
}

fn split_method(c: &Call<'_, '_>, text: &str, markup: bool) -> Outcome {
    let sep = c.arg_or(0, "sep", Value::None);
    let maxsplit = c.arg_int(1, "maxsplit", -1)?;
    let from_right = &*c.name == "rsplit";
    let sep_text = to_str(&sep);
    let parts = if sep.is_none() {
        text::split_ws(text, maxsplit, from_right)
    } else if sep_text.is_empty() {
        return Err(c.error("empty separator"));
    } else {
        text::split_sep(text, &sep_text, maxsplit, from_right)
    };
    Ok(Value::list(parts.into_iter().map(|p| keep_markup(p.to_string(), markup))))
}

fn join_method(c: &Call<'_, '_>, sep: &str, markup: bool) -> Outcome {
    let items = iterate(c.state_ref(), &c.arg(0, "iterable"))?;
    let mut parts = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        match item.as_str() {
            _ if markup => parts.push(escaped_text(item)),
            Some(s) => parts.push(std::borrow::Cow::Borrowed(s)),
            None => {
                let what = type_name(item);
                return Err(
                    c.error(&format!("sequence item {i}: expected str instance, {what} found"))
                );
            }
        }
    }
    let out = crate::filters_seq::join_sized(c.state_ref(), &parts, sep)?;
    Ok(keep_markup(out, markup))
}

/// The position of sub in text, in characters, or -1.
fn find_in(text: &str, sub: &str, from_right: bool) -> i64 {
    let at = if from_right { text.rfind(sub) } else { text.find(sub) };
    at.map_or(-1, |i| char_count(&text[..i]) as i64)
}

fn string_method_more(c: &mut Call<'_, '_>, text: &str, markup: bool) -> Outcome {
    let arg_text = |i: usize, name: &str| to_str(&c.arg(i, name)).into_owned();
    Ok(match &*c.name {
        "startswith" | "endswith" => {
            let options = match c.arg(0, "prefix") {
                Value::List(l) => l.snapshot(),
                prefix => vec![prefix],
            };
            let start = &*c.name == "startswith";
            Value::Bool(
                options
                    .iter()
                    .map(to_str)
                    .any(|p| if start { text.starts_with(&*p) } else { text.ends_with(&*p) }),
            )
        }
        "find" | "rfind" | "index" | "rindex" => {
            let i = find_in(text, &arg_text(0, "sub"), c.name.starts_with('r'));
            if i < 0 && c.name.ends_with("index") {
                return Err(c.error("substring not found"));
            }
            Value::Int(i)
        }
        "count" => match arg_text(0, "sub") {
            sub if sub.is_empty() => Value::Int(char_count(text) as i64 + 1),
            sub => Value::Int(text.matches(&sub).count() as i64),
        },
        "center" | "ljust" | "rjust" => justify(c, text, markup)?,
        "zfill" => zfill(c, text)?,
        "partition" | "rpartition" => {
            partition(text, &arg_text(0, "sep"), &*c.name == "rpartition")
        }
        "removeprefix" => Value::str(text.strip_prefix(&*arg_text(0, "prefix")).unwrap_or(text)),
        "removesuffix" => Value::str(text.strip_suffix(&*arg_text(0, "suffix")).unwrap_or(text)),
        "expandtabs" => Value::from(text::expand_tabs(text, c.arg_int(0, "tabsize", 8)?)),
        "encode" => Value::str(text),
        "striptags" => Value::from(striptags(text)),
        "unescape" => Value::str(&unescape_html(text)),
        "escape" => c.this.clone(),
        _ => return string_predicate(c, text),
    })
}

/// str.partition / rpartition: the text before sep, sep, and the text after.
fn partition(text: &str, sep: &str, from_right: bool) -> Value {
    let at = if from_right { text.rfind(sep) } else { text.find(sep) };
    let parts = match at {
        Some(i) => [&text[..i], sep, &text[i + sep.len()..]],
        None if from_right => ["", "", text],
        None => [text, "", ""],
    };
    Value::tuple(parts.into_iter().map(Value::str))
}

fn justify(c: &Call<'_, '_>, text: &str, markup: bool) -> Outcome {
    let width = c.arg_count(0, "width", 0)?;
    let fill = to_str(&c.arg_or(1, "fillchar", Value::str(" "))).into_owned();
    let align = match &*c.name {
        "center" => '^',
        "ljust" => '<',
        _ => '>',
    };
    Ok(keep_markup(crate::format::pad(c.state_ref(), text, width, &fill, align)?, markup))
}

fn zfill(c: &Call<'_, '_>, text: &str) -> Outcome {
    let width = c.arg_count(0, "width", 0)?;
    let n = char_count(text);
    if width <= n {
        return Ok(c.this.clone());
    }
    let (sign, body) = match text.as_bytes().first() {
        Some(b'-' | b'+') => text.split_at(1),
        _ => ("", text),
    };
    let zeros = crate::limits::repeat_text(c.state_ref(), "0", width - n)?;
    Ok(Value::from(format!("{sign}{zeros}{body}")))
}

fn string_predicate(c: &Call<'_, '_>, text: &str) -> Outcome {
    if text.is_empty() && &*c.name != "isascii" {
        return Ok(Value::Bool(false));
    }
    let all = |f: fn(char) -> bool| Ok(Value::Bool(text.chars().all(f)));
    match &*c.name {
        "isdigit" | "isdecimal" | "isnumeric" => all(char::is_numeric),
        "isalpha" => all(char::is_alphabetic),
        "isalnum" => all(char::is_alphanumeric),
        "isspace" => all(text::is_space),
        "isascii" => all(|ch| ch.is_ascii()),
        "isidentifier" => Ok(Value::Bool(
            text.chars()
                .enumerate()
                .all(|(i, ch)| ch == '_' || ch.is_alphabetic() || (i > 0 && ch.is_numeric())),
        )),
        "islower" | "isupper" => {
            let want_lower = &*c.name == "islower";
            let cased: Vec<char> =
                text.chars().filter(|ch| ch.is_uppercase() || ch.is_lowercase()).collect();
            let fits = cased.iter().all(|ch| ch.is_lowercase() == want_lower);
            Ok(Value::Bool(!cased.is_empty() && fits))
        }
        "istitle" => {
            let title = text::python_title(text);
            Ok(Value::Bool(text == title && title != text::to_lower(text)))
        }
        _ => Err(no_attribute(c)),
    }
}

// ---------------------------------------------------------------------------------------
// list and dict

fn list_method(c: &mut Call<'_, '_>, l: &Rc<List>) -> Outcome {
    let found = |x: &Value| l.items().iter().position(|item| values_equal(item, x));
    match &*c.name {
        "index" => {
            let x = c.arg(0, "value");
            match found(&x) {
                Some(i) => Ok(Value::Int(i as i64)),
                None => Err(c.error(&format!("{} is not in list", repr(&x)))),
            }
        }
        "count" => {
            let x = c.arg(0, "value");
            Ok(Value::Int(l.items().iter().filter(|item| values_equal(item, &x)).count() as i64))
        }
        "append" => {
            l.push(c.arg(0, "object"));
            Ok(Value::None)
        }
        "extend" => {
            // The items are read before the list grows, so a list may extend itself.
            l.extend(iterate(c.state_ref(), &c.arg(0, "iterable"))?);
            Ok(Value::None)
        }
        "insert" => {
            let i = c.arg_int(0, "index", 0)?;
            let n = l.len() as i64;
            let at = if i < 0 { (i + n).max(0) } else { i.min(n) };
            l.insert(at as usize, c.arg(1, "object"));
            Ok(Value::None)
        }
        "pop" => list_pop(c, l),
        "remove" => match found(&c.arg(0, "value")) {
            Some(i) => {
                l.remove(i);
                Ok(Value::None)
            }
            None => Err(c.error("list.remove(x): x not in list")),
        },
        "reverse" => {
            l.reverse();
            Ok(Value::None)
        }
        "sort" => {
            let reverse = c.arg_flag(1, "reverse", false);
            let sorted = crate::filters_seq::sort_values(
                c.state_ref(),
                &l.snapshot(),
                reverse,
                true,
                None,
            )?;
            l.replace(sorted);
            Ok(Value::None)
        }
        "clear" => {
            l.replace(Vec::new());
            Ok(Value::None)
        }
        "copy" => Ok(Value::list(l.snapshot())),
        _ => Err(no_attribute(c)),
    }
}

fn list_pop(c: &Call<'_, '_>, l: &List) -> Outcome {
    if l.is_empty() {
        return Err(c.error("pop from empty list"));
    }
    let n = l.len() as i64;
    let i = c.arg_int(0, "index", -1)?;
    let i = if i < 0 { i + n } else { i };
    if !(0..n).contains(&i) {
        return Err(c.error("pop index out of range"));
    }
    Ok(l.remove(i as usize))
}

fn dict_method(c: &mut Call<'_, '_>, d: &Rc<Dict>) -> Outcome {
    Ok(match &*c.name {
        "items" => Value::list(d.entries().into_iter().map(|(k, v)| Value::tuple([k, v]))),
        "keys" => Value::list(d.keys()),
        "values" => Value::list(d.values()),
        "get" => d.get(&c.arg(0, "key")).unwrap_or_else(|| c.arg_or(1, "default", Value::None)),
        "pop" => {
            let key = c.arg(0, "key");
            match d.remove(&key) {
                Some(v) => v,
                None if c.has_arg(1, "default") => c.arg(1, "default"),
                None => return Err(c.error(&repr(&key))),
            }
        }
        "popitem" => {
            let Some(key) = d.keys().pop() else {
                return Err(c.error("'popitem(): dictionary is empty'"));
            };
            let v = d.remove(&key).unwrap_or_default();
            Value::tuple([key, v])
        }
        "setdefault" => {
            let key = c.arg(0, "key");
            if let Some(v) = d.get(&key) {
                return Ok(v);
            }
            let v = c.arg_or(1, "default", Value::None);
            d.set(key, v.clone());
            v
        }
        "update" => {
            if let Value::Dict(other) = c.arg_or(0, "other", Value::None) {
                other.entries().into_iter().for_each(|(k, v)| d.set(k, v));
            }
            for kw in &c.kwargs {
                d.set(Value::from(kw.name.clone()), kw.value.clone());
            }
            Value::None
        }
        "copy" => Value::Dict(d.copy(DictKind::Dict)),
        "clear" => {
            d.clear();
            Value::None
        }
        _ => return Err(no_attribute(c)),
    })
}
