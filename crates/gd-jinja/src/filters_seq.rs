//! Filters over sequences and mappings, and the sorting they share.

use std::borrow::Cow;

use crate::ast::BinOp;
use crate::calls::Call;
use crate::convert::{
    MAX_VALUE_DEPTH, compare_values, object_name, to_str, type_name, values_equal,
};
use crate::environment::Environment;
use crate::error::ErrorKind;
use crate::escape::escaped_text;
use crate::eval::{attribute_of, binary_op, get_item};
use crate::filters_text::{Builtin, Outcome, keep_markup};
use crate::runtime::{Result, State, iterate};
use crate::text::to_lower;
use crate::value::{List, Value};

pub(crate) fn register(env: &mut Environment) {
    let filters: &[(&str, Builtin)] = &[
        ("attr", filter_attr),
        ("batch", filter_batch),
        ("count", filter_length),
        ("length", filter_length),
        ("default", filter_default),
        ("d", filter_default),
        ("dictsort", filter_dictsort),
        ("first", |c| Ok(first_or(items_arg(c, "seq")?.into_iter().next(), "first"))),
        ("last", |c| Ok(first_or(items_arg(c, "seq")?.pop(), "last"))),
        ("groupby", filter_groupby),
        ("items", filter_items),
        ("join", filter_join),
        ("list", |c| Ok(Value::list(items_arg(c, "value")?))),
        ("map", filter_map),
        ("max", filter_minmax),
        ("min", filter_minmax),
        ("random", filter_random),
        ("select", filter_select),
        ("reject", filter_select),
        ("selectattr", filter_select),
        ("rejectattr", filter_select),
        ("reverse", filter_reverse),
        ("slice", filter_slice),
        ("sort", filter_sort),
        ("sum", filter_sum),
        ("unique", filter_unique),
    ];
    for &(name, f) in filters {
        env.add_filter(name, f);
    }
}

/// The items argument 0 (or the keyword `name`) iterates, like Python's `list(value)`.
fn items_arg(c: &Call<'_, '_>, name: &str) -> Result<Vec<Value>> {
    iterate(c.state_ref(), &c.arg(0, name))
}

/// item, or the undefined value of the first or last item of an empty sequence.
fn first_or(item: Option<Value>, which: &str) -> Value {
    item.unwrap_or_else(|| Value::undefined_hint(&format!("No {which} item, sequence was empty.")))
}

/// Follows a dotted attribute path ("a.b.0") with environment.getitem; a missing value
/// gives default (when it is defined), and strings are lowercased when lower is set.
pub(crate) fn attribute_getter(
    s: &State<'_>,
    item: &Value,
    attribute: &Value,
    default: &Value,
    lower: bool,
) -> Result<Value> {
    let mut v = item.clone();
    if let Some(path) = attribute.as_str() {
        for part in path.split('.') {
            let key = match part.parse::<i64>() {
                Ok(n) if part.as_bytes().first().is_some_and(u8::is_ascii_digit) => Value::Int(n),
                _ => Value::str(part),
            };
            v = get_item(s, &v, &key)?;
        }
    } else if !attribute.is_none() {
        v = get_item(s, &v, attribute)?;
    }
    if v.is_undefined() && !default.is_undefined() {
        v = default.clone();
    }
    Ok(match &v {
        Value::Str(text) if lower => Value::from(to_lower(text)),
        _ => v,
    })
}

/// attribute_getter with no default and no lowercasing.
fn attribute_of_item(s: &State<'_>, item: &Value, attribute: &Value) -> Result<Value> {
    attribute_getter(s, item, attribute, &Value::default(), false)
}

fn filter_attr(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("attr", &["obj", "name"], 2)?;
    let obj = c.arg(0, "obj");
    let name = to_str(&c.arg(1, "name")).into_owned();
    if obj.is_undefined() {
        return Err(c.state_ref().undefined_error(&obj).into());
    }
    Ok(attribute_of(&obj, &name)
        .unwrap_or_else(|| Value::undefined_with(&name, &object_name(&obj), "")))
}

fn filter_batch(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("batch", &["value", "linecount", "fill_with"], 2)?;
    let items = items_arg(c, "value")?;
    let size = c.arg_int(1, "linecount", 1)?;
    if size <= 0 {
        return Err(c.error("batch size must be positive"));
    }
    let size = size as usize;
    let fill = c.arg_or(2, "fill_with", Value::None);
    let rows = items.chunks(size).map(|chunk| {
        let mut row = chunk.to_vec();
        if !fill.is_none() {
            row.resize(size, fill.clone());
        }
        Value::list(row)
    });
    Ok(Value::list(rows))
}

fn filter_length(c: &mut Call<'_, '_>) -> Outcome {
    let v = c.arg(0, "obj");
    match &v {
        Value::Str(_) | Value::Markup(_) | Value::List(_) | Value::Dict(_) => {
            Ok(Value::Int(v.len() as i64))
        }
        Value::Undefined(_) => Ok(c.state_ref().strict_check(&v).map(|()| Value::Int(0))?),
        _ => Err(c.error(&format!("object of type '{}' has no len()", type_name(&v)))),
    }
}

fn filter_default(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("default", &["value", "default_value", "boolean"], 0)?;
    let v = c.arg(0, "value");
    let use_default = v.is_undefined() || (c.arg_flag(2, "boolean", false) && !v.is_truthy());
    Ok(if use_default { c.arg_or(1, "default_value", Value::str("")) } else { v })
}

fn filter_dictsort(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("dictsort", &["value", "case_sensitive", "by", "reverse"], 0)?;
    let wrong = || c.error("You can only sort by either \"key\" or \"value\"");
    let Value::Dict(d) = c.arg(0, "value") else { return Err(wrong()) };
    let by_value = match to_str(&c.arg_or(2, "by", Value::str("key"))).as_ref() {
        "key" => false,
        "value" => true,
        _ => return Err(wrong()),
    };
    let entries = d.entries();
    let keys: Vec<Value> =
        entries.iter().map(|(k, v)| if by_value { v } else { k }.clone()).collect();
    let pairs: Vec<Value> = entries.into_iter().map(|(k, v)| Value::tuple([k, v])).collect();
    let reverse = c.arg_flag(3, "reverse", false);
    let case_sensitive = c.arg_flag(1, "case_sensitive", false);
    Ok(Value::list(sort_values(c.state_ref(), &pairs, reverse, case_sensitive, Some(&keys))?))
}

/// The field names of the groups groupby makes.
const GROUP_FIELDS: &[&str] = &["grouper", "list"];

fn filter_groupby(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("groupby", &["value", "attribute", "default", "case_sensitive"], 2)?;
    let items = items_arg(c, "value")?;
    let (attribute, default) = (c.arg(1, "attribute"), c.arg(2, "default"));
    let case_sensitive = c.arg_flag(3, "case_sensitive", false);
    let s = c.state_ref();
    let keys = items
        .iter()
        .map(|item| attribute_getter(s, item, &attribute, &default, !case_sensitive))
        .collect::<Result<Vec<_>>>()?;
    let order = sort_order(s, &keys, false, true)?;
    let mut groups = Vec::new();
    // Runs of equal keys in sorted order are the groups.
    for run in order.chunk_by(|a, b| values_equal(&keys[*a], &keys[*b])) {
        let group: Vec<Value> = run.iter().map(|&i| items[i].clone()).collect();
        let grouper = if case_sensitive {
            keys[run[0]].clone()
        } else {
            attribute_getter(s, &group[0], &attribute, &default, false)?
        };
        groups.push(Value::List(List::named(vec![grouper, Value::list(group)], GROUP_FIELDS)));
    }
    Ok(Value::list(groups))
}

fn filter_items(c: &mut Call<'_, '_>) -> Outcome {
    match c.arg(0, "value") {
        Value::Undefined(_) => Ok(Value::list([])),
        Value::Dict(d) => {
            Ok(Value::list(d.entries().into_iter().map(|(k, v)| Value::tuple([k, v]))))
        }
        _ => Err(c.error("Can only get item pairs from a mapping.")),
    }
}

/// parts joined by sep, sized first and allocated once.
pub(crate) fn join_sized<S: AsRef<str>>(s: &State<'_>, parts: &[S], sep: &str) -> Result<String> {
    let total: usize = parts.iter().map(|p| p.as_ref().len()).sum();
    let seps = crate::limits::reserve_bytes(s, parts.len().saturating_sub(1), sep.len())?;
    let mut out = String::new();
    crate::limits::reserve_text(s, &mut out, crate::limits::sum_bytes(s, total, seps)?)?;
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            out.push_str(sep);
        }
        out.push_str(part.as_ref());
    }
    Ok(out)
}

fn filter_join(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("join", &["value", "d", "attribute"], 0)?;
    let mut items = items_arg(c, "value")?;
    let sep = c.arg_or(1, "d", Value::str(""));
    let attribute = c.arg_or(2, "attribute", Value::None);
    let s = c.state_ref();
    if !attribute.is_none() {
        items = items
            .iter()
            .map(|item| attribute_of_item(s, item, &attribute))
            .collect::<Result<_>>()?;
    }
    let markup = c.autoescaping() && (sep.is_markup() || items.iter().any(Value::is_markup));
    let text =
        |v: &Value| -> String { if markup { escaped_text(v) } else { to_str(v) }.into_owned() };
    let parts: Vec<String> = items.iter().map(text).collect();
    Ok(keep_markup(join_sized(s, &parts, &text(&sep))?, markup))
}

fn filter_map(c: &mut Call<'_, '_>) -> Outcome {
    let items = items_arg(c, "value")?;
    if let Some(attribute) = c.keyword("attribute") {
        let default = c.keyword("default").unwrap_or_default();
        let s = c.state_ref();
        let mapped =
            items.iter().map(|item| attribute_getter(s, item, &attribute, &default, false));
        return Ok(Value::list(mapped.collect::<Result<Vec<_>>>()?));
    }
    if c.args.len() < 2 {
        return Err(c.error("map requires a filter argument"));
    }
    let name = to_str(&c.args[1]).into_owned();
    let extra = c.args[2..].to_vec();
    let kwargs = c.kwargs.clone();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let args = std::iter::once(item).chain(extra.iter().cloned()).collect();
        out.push(c.call_filter(&name, args, kwargs.clone())?);
    }
    Ok(Value::list(out))
}

fn filter_minmax(c: &mut Call<'_, '_>) -> Outcome {
    let who = c.name.clone();
    c.check_args(&who, &["value", "case_sensitive", "attribute"], 0)?;
    let items = items_arg(c, "value")?;
    if items.is_empty() {
        return Ok(Value::undefined_hint("No aggregated item, sequence was empty."));
    }
    let attribute = c.arg_or(2, "attribute", Value::None);
    let lower = !c.arg_flag(1, "case_sensitive", false);
    let want = if &*who == "max" { 1 } else { -1 };
    let s = c.state_ref();
    let none = Value::default();
    let mut best = 0;
    let mut best_key = attribute_getter(s, &items[0], &attribute, &none, lower)?;
    for (i, item) in items.iter().enumerate().skip(1) {
        let key = attribute_getter(s, item, &attribute, &none, lower)?;
        let order = compare_values(&key, &best_key).ok_or_else(|| {
            let (a, b) = (type_name(&key), type_name(&best_key));
            c.error(&format!("'<' not supported between instances of '{a}' and '{b}'"))
        })?;
        if order == want {
            (best, best_key) = (i, key);
        }
    }
    Ok(items[best].clone())
}

fn filter_random(c: &mut Call<'_, '_>) -> Outcome {
    let items = items_arg(c, "seq")?;
    if items.is_empty() {
        return Ok(Value::undefined_hint("No random item, sequence was empty."));
    }
    let i = crate::globals::random_below(c.state(), items.len());
    Ok(items[i].clone())
}

/// select, reject, selectattr and rejectattr.
fn filter_select(c: &mut Call<'_, '_>) -> Outcome {
    let items = items_arg(c, "value")?;
    let by_attr = c.name.ends_with("attr");
    let want = c.name.starts_with("select");
    let first = if by_attr { 2 } else { 1 };
    if by_attr && c.args.len() < 2 {
        return Err(c.error("Missing parameter for attribute name"));
    }
    let test = c.args.get(first).map(|t| to_str(t).into_owned());
    let extra = c.args.get(first + 1..).unwrap_or_default().to_vec();
    let attribute = c.args.get(1).cloned().unwrap_or_default();
    let kwargs = c.kwargs.clone();
    let mut out = Vec::new();
    for item in items {
        let v = if by_attr {
            attribute_of_item(c.state_ref(), &item, &attribute)?
        } else {
            item.clone()
        };
        let passed = match &test {
            Some(name) => {
                let args = std::iter::once(v).chain(extra.iter().cloned()).collect();
                c.call_test(name, args, kwargs.clone())?
            }
            None => c.state_ref().is_true(&v)?,
        };
        if passed == want {
            out.push(item);
        }
    }
    Ok(Value::list(out))
}

fn filter_reverse(c: &mut Call<'_, '_>) -> Outcome {
    let v = c.arg(0, "value");
    if let Some(text) = v.as_str() {
        return Ok(keep_markup(text.chars().rev().collect(), v.is_markup()));
    }
    let mut items = iterate(c.state_ref(), &v)?;
    items.reverse();
    Ok(Value::list(items))
}

fn filter_slice(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("slice", &["value", "slices", "fill_with"], 2)?;
    let items = items_arg(c, "value")?;
    let slices = c.arg_int(1, "slices", 1)?;
    if slices <= 0 {
        return Err(c.error("slices must be positive"));
    }
    let fill = c.arg_or(2, "fill_with", Value::None);
    let slices = usize::try_from(slices).unwrap_or(usize::MAX);
    crate::limits::reserve_bytes(c.state_ref(), slices, size_of::<Value>())?;
    // The first `extra` slices take one item more than the others.
    let (per, extra) = (items.len() / slices, items.len() % slices);
    let mut out = Vec::with_capacity(slices);
    let mut start = 0;
    for i in 0..slices {
        let end = start + per + usize::from(i < extra);
        let mut part = items[start..end].to_vec();
        if !fill.is_none() && i >= extra {
            part.push(fill.clone());
        }
        out.push(Value::list(part));
        start = end;
    }
    Ok(Value::list(out))
}

fn filter_sort(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("sort", &["value", "reverse", "case_sensitive", "attribute"], 0)?;
    let items = items_arg(c, "value")?;
    let attribute = c.arg_or(3, "attribute", Value::None);
    let reverse = c.arg_flag(1, "reverse", false);
    let case_sensitive = c.arg_flag(2, "case_sensitive", false);
    let s = c.state_ref();
    if attribute.is_none() {
        return Ok(Value::list(sort_values(s, &items, reverse, case_sensitive, None)?));
    }
    let path = to_str(&attribute).into_owned();
    let parts: Vec<Value> = path.split(',').map(|p| Value::str(p.trim())).collect();
    let none = Value::default();
    let mut keys = Vec::with_capacity(items.len());
    for item in &items {
        let key = parts.iter().map(|p| attribute_getter(s, item, p, &none, !case_sensitive));
        keys.push(Value::tuple(key.collect::<Result<Vec<_>>>()?));
    }
    Ok(Value::list(sort_values(s, &items, reverse, true, Some(&keys))?))
}

fn filter_sum(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("sum", &["iterable", "attribute", "start"], 0)?;
    let items = items_arg(c, "iterable")?;
    let attribute = c.arg_or(1, "attribute", Value::None);
    let s = c.state_ref();
    let sum = items.iter().try_fold(c.arg_or(2, "start", Value::Int(0)), |total, item| {
        binary_op(s, BinOp::Add, &total, &attribute_of_item(s, item, &attribute)?)
    });
    Ok(sum?)
}

fn filter_unique(c: &mut Call<'_, '_>) -> Outcome {
    c.check_args("unique", &["value", "case_sensitive", "attribute"], 0)?;
    let items = items_arg(c, "value")?;
    let attribute = c.arg_or(2, "attribute", Value::None);
    let lower = !c.arg_flag(1, "case_sensitive", false);
    let s = c.state_ref();
    let mut seen: Vec<Value> = Vec::new();
    let mut out = Vec::new();
    for item in items {
        let key = attribute_getter(s, &item, &attribute, &Value::default(), lower)?;
        if !seen.iter().any(|k| values_equal(k, &key)) {
            seen.push(key);
            out.push(item);
        }
    }
    Ok(Value::list(out))
}

// ---------------------------------------------------------------------------------------
// Sorting: a stable merge sort that reports incomparable values as errors.

/// Sorts items by keys (the items themselves when keys is None). Strings are compared
/// case-insensitively unless case_sensitive is set.
pub(crate) fn sort_values(
    s: &State<'_>,
    items: &[Value],
    reverse: bool,
    case_sensitive: bool,
    keys: Option<&[Value]>,
) -> Result<Vec<Value>> {
    let order = sort_order(s, keys.unwrap_or(items), reverse, case_sensitive)?;
    Ok(order.into_iter().map(|i| items[i].clone()).collect())
}

/// The order that sorts keys: indices into keys.
pub(crate) fn sort_order(
    s: &State<'_>,
    keys: &[Value],
    reverse: bool,
    case_sensitive: bool,
) -> Result<Vec<usize>> {
    let keys: Cow<'_, [Value]> = if case_sensitive {
        Cow::Borrowed(keys)
    } else {
        Cow::Owned(keys.iter().map(|k| fold_case(k, 0)).collect())
    };
    let mut order: Vec<usize> = (0..keys.len()).collect();
    let mut tmp = vec![0; keys.len()];
    let mut sorter = Sorter { keys: &keys, reverse, error: None };
    merge_sort(&mut sorter, &mut order, &mut tmp);
    match sorter.error {
        Some(message) => Err(s.error(ErrorKind::Type, message)),
        None => Ok(order),
    }
}

/// Lowercases strings (also inside tuples) for case-insensitive comparisons, down to
/// MAX_VALUE_DEPTH (a list may contain itself).
fn fold_case(v: &Value, depth: usize) -> Value {
    match v {
        Value::Str(text) | Value::Markup(text) => Value::from(to_lower(text)),
        Value::List(l) if depth < MAX_VALUE_DEPTH => {
            let items = l.items().iter().map(|item| fold_case(item, depth + 1)).collect();
            Value::List(List::new(items, l.is_tuple()))
        }
        _ => v.clone(),
    }
}

/// Compares keys for merge_sort, remembering the first pair that cannot be ordered.
struct Sorter<'a> {
    keys: &'a [Value],
    reverse: bool,
    error: Option<String>,
}

impl Sorter<'_> {
    fn less(&mut self, a: usize, b: usize) -> bool {
        if self.error.is_some() {
            return false;
        }
        let (x, y) = if self.reverse {
            (&self.keys[b], &self.keys[a])
        } else {
            (&self.keys[a], &self.keys[b])
        };
        match compare_values(x, y) {
            Some(order) => order == -1,
            None => {
                let (p, q) = (type_name(x), type_name(y));
                self.error =
                    Some(format!("'<' not supported between instances of '{p}' and '{q}'"));
                false
            }
        }
    }
}

/// A stable merge sort of order by the sorter's keys; tmp is scratch of the same length.
fn merge_sort(sorter: &mut Sorter<'_>, order: &mut [usize], tmp: &mut [usize]) {
    let n = order.len();
    if n < 2 {
        return;
    }
    let mid = n / 2;
    merge_sort(sorter, &mut order[..mid], &mut tmp[..mid]);
    merge_sort(sorter, &mut order[mid..], &mut tmp[mid..]);
    let (mut i, mut j, mut k) = (0, mid, 0);
    while i < mid && j < n {
        if sorter.less(order[j], order[i]) {
            tmp[k] = order[j];
            j += 1;
        } else {
            tmp[k] = order[i];
            i += 1;
        }
        k += 1;
    }
    tmp[k..k + mid - i].copy_from_slice(&order[i..mid]);
    k += mid - i;
    tmp[k..k + n - j].copy_from_slice(&order[j..n]);
    order.copy_from_slice(&tmp[..n]);
}
