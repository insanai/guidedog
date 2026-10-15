//! Sizing values before they are built.
//!
//! Values whose size is known before they are built (a repetition, padding, a join) are
//! sized first: a size past `Options::max_output` is a Limit error, a size that cannot be
//! counted or allocated is a Memory error, and otherwise the value is allocated once, at
//! its exact size.

use crate::error::{Error, ErrorKind, Hinted, MEMORY_HINT_RENDER, MEMORY_MESSAGE};
use crate::runtime::{Result, State};
use crate::value::{List, Value};

/// Writes a byte count for people: "12 bytes", "3.5 MiB".
pub(crate) fn byte_count(bytes: usize) -> String {
    let b = bytes as f64;
    if bytes >= 1 << 30 {
        format!("{:.1} GiB", b / f64::from(1u32 << 30))
    } else if bytes >= 1 << 20 {
        format!("{:.1} MiB", b / f64::from(1u32 << 20))
    } else if bytes >= 1 << 10 {
        format!("{:.1} KiB", b / 1024.0)
    } else {
        format!("{bytes} bytes")
    }
}

/// The Memory error of a value that cannot be held: bytes more than could be allocated,
/// or more than can be counted (None).
fn memory_error(s: &State<'_>, bytes: Option<usize>) -> Box<Error> {
    let message = match bytes {
        Some(n) => format!(
            "Rendering this template needs {} more memory here than it may use.",
            byte_count(n)
        ),
        None => MEMORY_MESSAGE.to_string(),
    };
    s.error(ErrorKind::Memory, message).hinted(MEMORY_HINT_RENDER)
}

/// Checks a value of count items of size bytes that the render is about to build, and
/// returns its byte count.
pub(crate) fn reserve_bytes(s: &State<'_>, count: usize, size: usize) -> Result<usize> {
    let total = count.checked_mul(size).ok_or_else(|| memory_error(s, None))?;
    let limit = s.env.options.max_output;
    if limit > 0 && total > limit {
        let built = byte_count(total);
        let message =
            format!("this expression builds {built}, more than the output limit of {limit} bytes");
        let hint = "Build a smaller value here, or raise Options::max_output.";
        return Err(s.error(ErrorKind::Limit, message).hinted(hint));
    }
    Ok(total)
}

/// Adds two byte counts, failing on a sum that cannot be counted.
pub(crate) fn sum_bytes(s: &State<'_>, a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or_else(|| memory_error(s, None))
}

/// Grows out so that extra more bytes fit, after reserve_bytes checks them.
pub(crate) fn reserve_text(s: &State<'_>, out: &mut String, extra: usize) -> Result<()> {
    let more = reserve_bytes(s, extra, 1)?;
    out.try_reserve_exact(more).map_err(|_| memory_error(s, Some(more)))
}

/// text repeated count times, sized and checked first.
pub(crate) fn repeat_text(s: &State<'_>, text: &str, count: usize) -> Result<String> {
    let total = reserve_bytes(s, text.len(), count)?;
    let mut out = String::new();
    if total == 0 {
        return Ok(out);
    }
    out.try_reserve_exact(total).map_err(|_| memory_error(s, Some(total)))?;
    for _ in 0..count {
        out.push_str(text);
    }
    Ok(out)
}

/// `sequence * n`.
pub(crate) fn repeat_value(s: &State<'_>, v: &Value, times: i64) -> Result<Value> {
    let count = usize::try_from(times.max(0)).unwrap_or(usize::MAX);
    match v {
        Value::Str(text) => Ok(Value::from(repeat_text(s, text, count)?)),
        Value::Markup(text) => Ok(Value::Markup(std::rc::Rc::from(repeat_text(s, text, count)?))),
        Value::List(l) => {
            let items = l.snapshot();
            let n = reserve_bytes(s, items.len(), count)?;
            reserve_bytes(s, n, size_of::<Value>())?;
            let mut out = Vec::new();
            if n > 0 {
                let bytes = n.saturating_mul(size_of::<Value>());
                out.try_reserve_exact(n).map_err(|_| memory_error(s, Some(bytes)))?;
                for _ in 0..count {
                    out.extend(items.iter().cloned());
                }
            }
            Ok(Value::List(List::new(out, l.is_tuple())))
        }
        _ => {
            let message =
                format!("can't multiply sequence by non-int of type '{}'", v.type_name());
            Err(s.error(ErrorKind::Type, message))
        }
    }
}
