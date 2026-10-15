//! The built-in globals: range, dict, lipsum, cycler, joiner, namespace and the
//! pass-through gettext functions of the i18n extension; and the render's seeded random
//! numbers.

use std::cell::Cell;
use std::rc::Rc;

use crate::calls::Call;
use crate::convert::{as_int, as_number, type_name};
use crate::environment::Environment;
use crate::error::ErrorKind;
use crate::escape::write_escaped;
use crate::filters_text::{Builtin, Outcome};
use crate::runtime::{Result, State, iterate};
use crate::text::capitalize;
use crate::value::{CallKind, Cycler, Dict, DictKind, Value};

pub(crate) fn register(env: &mut Environment) {
    let globals: &[(&str, Builtin)] = &[
        ("range", global_range),
        ("dict", |c| global_dict(c, DictKind::Dict)),
        ("namespace", |c| global_dict(c, DictKind::Namespace)),
        ("lipsum", global_lipsum),
        ("cycler", |c| {
            Ok(Value::Cycler(Rc::new(Cycler { items: c.args.clone(), pos: Cell::new(0) })))
        }),
        ("joiner", global_joiner),
        ("gettext", |c| Ok(c.arg_or(0, "message", Value::str("")))),
        ("_", |c| Ok(c.arg_or(0, "message", Value::str("")))),
        ("ngettext", global_ngettext),
    ];
    for &(name, f) in globals {
        env.add_function(name, f);
    }
}

fn global_range(c: &mut Call<'_, '_>) -> Outcome {
    if !c.kwargs.is_empty() {
        return Err(c.error("range() takes no keyword arguments"));
    }
    if c.args.is_empty() || c.args.len() > 3 {
        return Err(c.error(&format!("range expected at most 3 arguments, got {}", c.args.len())));
    }
    let mut bounds = [0i64, 0, 1];
    for (bound, v) in bounds.iter_mut().zip(&c.args) {
        *bound = as_int(v).ok_or_else(|| {
            c.error(&format!("'{}' object cannot be interpreted as an integer", type_name(v)))
        })?;
    }
    let [mut start, mut stop, step] = bounds;
    if c.args.len() == 1 {
        (start, stop) = (0, bounds[0]);
    }
    if step == 0 {
        return Err(c.error("range() arg 3 must not be zero"));
    }
    let count = range_len(start, stop, step);
    let limit = c.state_ref().env.options.max_iterations as u64;
    if count > limit {
        let message = format!("range() of {count} items exceeds the iteration limit {limit}");
        return Err(c.state_ref().error(ErrorKind::Limit, message).into());
    }
    let count = count as usize;
    crate::limits::reserve_bytes(c.state_ref(), count, size_of::<Value>())?;
    let items = (0..count as i64).map(|i| Value::Int(start + i * step));
    Ok(Value::list(items))
}

/// The number of items of range(start, stop, step), step not 0.
fn range_len(start: i64, stop: i64, step: i64) -> u64 {
    let (span, stride) = if step > 0 {
        (i128::from(stop) - i128::from(start), i128::from(step))
    } else {
        (i128::from(start) - i128::from(stop), -i128::from(step))
    };
    if span <= 0 { 0 } else { ((span + stride - 1) / stride) as u64 }
}

/// dict(...) and namespace(...): from a mapping or pairs, and keyword arguments.
fn global_dict(c: &mut Call<'_, '_>, kind: DictKind) -> Outcome {
    if c.args.len() > 1 {
        return Err(c.error(&format!("dict expected at most 1 argument, got {}", c.args.len())));
    }
    let d = Dict::new(kind);
    match c.args.first() {
        None => {}
        Some(Value::Dict(source)) => source.entries().into_iter().for_each(|(k, v)| d.set(k, v)),
        Some(pairs) => {
            for item in iterate(c.state_ref(), pairs)? {
                match item {
                    Value::List(pair) if pair.len() == 2 => {
                        d.set(pair.item(0).unwrap_or_default(), pair.item(1).unwrap_or_default());
                    }
                    _ => {
                        return Err(c.error("dictionary update sequence element has wrong length"));
                    }
                }
            }
        }
    }
    for kw in &c.kwargs {
        d.set(Value::from(kw.name.clone()), kw.value.clone());
    }
    Ok(Value::Dict(d))
}

/// joiner(sep): a function that returns "" the first time and sep after.
fn global_joiner(c: &mut Call<'_, '_>) -> Outcome {
    let sep = c.arg_or(0, "sep", Value::str(", "));
    let used = Cell::new(false);
    let join = move |_: &mut Call<'_, '_>| {
        Ok(if used.replace(true) { sep.clone() } else { Value::str("") })
    };
    let kind = CallKind::Native(Rc::new(join));
    Ok(c.state_ref().render_callable(Rc::from("joiner"), kind))
}

fn global_ngettext(c: &mut Call<'_, '_>) -> Outcome {
    let n = as_number(&c.arg_or(2, "n", Value::Int(1))).map_or(0.0, |n| n.f());
    let (index, name) = if n == 1.0 { (0, "singular") } else { (1, "plural") };
    Ok(c.arg_or(index, name, Value::str("")))
}

// ---------------------------------------------------------------------------------------
// Deterministic random numbers (PCG32), seeded from Options::random_seed

fn random_u32(s: &mut State<'_>) -> u32 {
    let old = s.rng;
    s.rng = old.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
    xorshifted.rotate_right((old >> 59) as u32)
}

/// A number in [0, n).
pub(crate) fn random_below(s: &mut State<'_>, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    ((u64::from(random_u32(s)) * n as u64) >> 32) as usize
}

const LOREM: &[&str] = &[
    "a",
    "ac",
    "accumsan",
    "ad",
    "adipiscing",
    "aenean",
    "aliquam",
    "aliquet",
    "amet",
    "ante",
    "aptent",
    "arcu",
    "at",
    "auctor",
    "augue",
    "bibendum",
    "blandit",
    "class",
    "commodo",
    "condimentum",
    "congue",
    "consectetuer",
    "consequat",
    "conubia",
    "convallis",
    "cras",
    "cubilia",
    "cum",
    "curabitur",
    "curae",
    "cursus",
    "dapibus",
    "diam",
    "dictum",
    "dictumst",
    "dignissim",
    "dis",
    "dolor",
    "donec",
    "dui",
    "duis",
    "egestas",
    "eget",
    "eleifend",
    "elementum",
    "elit",
    "enim",
    "erat",
    "eros",
    "est",
    "et",
    "etiam",
    "eu",
    "euismod",
    "facilisi",
    "facilisis",
    "fames",
    "faucibus",
    "felis",
    "fermentum",
    "feugiat",
    "fringilla",
    "fusce",
    "gravida",
    "habitant",
    "habitasse",
    "hac",
    "hendrerit",
    "hymenaeos",
    "iaculis",
    "id",
    "imperdiet",
    "in",
    "inceptos",
    "integer",
    "interdum",
    "ipsum",
    "justo",
    "lacinia",
    "lacus",
    "laoreet",
    "lectus",
    "leo",
    "libero",
    "ligula",
    "litora",
    "lobortis",
    "lorem",
    "luctus",
    "maecenas",
    "magna",
    "magnis",
    "malesuada",
    "massa",
    "mattis",
    "mauris",
    "metus",
    "mi",
    "molestie",
    "mollis",
    "montes",
    "morbi",
    "mus",
    "nam",
    "nascetur",
    "natoque",
    "nec",
    "neque",
    "netus",
    "nibh",
    "nisi",
    "nisl",
    "non",
    "nonummy",
    "nostra",
    "nulla",
    "nullam",
    "nunc",
    "odio",
    "orci",
    "ornare",
    "parturient",
    "pede",
    "pellentesque",
    "penatibus",
    "per",
    "pharetra",
    "phasellus",
    "placerat",
    "platea",
    "porta",
    "porttitor",
    "posuere",
    "potenti",
    "praesent",
    "pretium",
    "primis",
    "proin",
    "pulvinar",
    "purus",
    "quam",
    "quis",
    "quisque",
    "rhoncus",
    "ridiculus",
    "risus",
    "rutrum",
    "sagittis",
    "sapien",
    "scelerisque",
    "sed",
    "sem",
    "semper",
    "senectus",
    "sit",
    "sociis",
    "sociosqu",
    "sodales",
    "sollicitudin",
    "suscipit",
    "suspendisse",
    "taciti",
    "tellus",
    "tempor",
    "tempus",
    "tincidunt",
    "torquent",
    "tortor",
    "tristique",
    "turpis",
    "ullamcorper",
    "ultrices",
    "ultricies",
    "urna",
    "ut",
    "varius",
    "vehicula",
    "vel",
    "velit",
    "venenatis",
    "vestibulum",
    "vitae",
    "vivamus",
    "viverra",
    "volutpat",
    "vulputate",
];

/// Generates lorem ipsum paragraphs like Jinja's generate_lorem_ipsum: commas every few
/// words, a period at the end, capitalized sentences.
fn global_lipsum(c: &mut Call<'_, '_>) -> Outcome {
    let n = c.arg_int(0, "n", 5)?;
    let html = c.arg_flag(1, "html", true);
    let lo = c.arg_int(2, "min", 20)?;
    let hi = c.arg_int(3, "max", 100)?;
    let s = c.state();
    let mut paragraphs = Vec::new();
    for _ in 0..n {
        s.count_iteration()?;
        let span = usize::try_from((hi - lo).max(0)).unwrap_or(0);
        let count = lo + random_below(s, span + 1) as i64;
        paragraphs.push(lorem_paragraph(s, count)?);
    }
    if !html {
        return Ok(Value::from(paragraphs.join("\n\n")));
    }
    let mut out = String::new();
    for (i, p) in paragraphs.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str("<p>");
        write_escaped(&mut out, p);
        out.push_str("</p>");
    }
    Ok(Value::Markup(Rc::from(out)))
}

/// One paragraph of count words.
fn lorem_paragraph(s: &mut State<'_>, count: i64) -> Result<String> {
    let mut text = String::new();
    let mut next_capital = true;
    let (mut last_comma, mut last_fullstop) = (0i64, 0i64);
    let mut last_word = "";
    for i in 0..count {
        s.count_iteration()?;
        let mut word = LOREM[random_below(s, LOREM.len())];
        while word == last_word {
            word = LOREM[random_below(s, LOREM.len())];
        }
        last_word = word;
        if i > 0 {
            text.push(' ');
        }
        text.push_str(&if next_capital { capitalize(word) } else { word.to_string() });
        next_capital = false;
        if i - random_below(s, 7) as i64 - 3 > last_comma {
            last_comma = i;
            last_fullstop += 2;
            text.push(',');
        }
        if i - random_below(s, 11) as i64 - 10 > last_fullstop {
            (last_comma, last_fullstop) = (i, i);
            text.push('.');
            next_capital = true;
        }
    }
    let mut text = text.trim_end_matches(',').to_string();
    if !text.ends_with('.') {
        text.push('.');
    }
    Ok(text)
}
