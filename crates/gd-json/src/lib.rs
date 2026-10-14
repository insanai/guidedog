//! JSON (RFC 8259): a thin API over [`serde_json`] with a validating parser, an
//! HTML-safe writer and Elm-style errors.
//!
//! [`Value`] and [`Object`] *are* `serde_json::Value` and `serde_json::Map<String, Value>`,
//! so values pass to and from any serde-based code without conversion. Objects keep their
//! keys in insertion order so that written output is stable from run to run. [`parse`]
//! refuses duplicate keys (unless [`Limits::allow_duplicate_keys`]) and nesting deeper
//! than [`Limits::max_depth`], and every failure is an [`Error`] that can become an
//! Elm-style report. The writers escape `<`, `>`, `&`, U+2028 and U+2029 so their output
//! can sit inside an HTML `<script>` element.
//!
//! ```
//! let v = gd_json::parse(r#"{"name": "guide", "pages": [1, 2]}"#).unwrap();
//! assert_eq!(v.get("name").and_then(|n| n.as_str()), Some("guide"));
//! assert_eq!(gd_json::to_string(&v), r#"{"name":"guide","pages":[1,2]}"#);
//! ```

mod error;
mod parse;
mod write;

pub use error::{Error, ErrorKind};
pub use parse::{Limits, MAX_DEPTH, parse, parse_with};
pub use write::{to_string, to_string_pretty, write_pretty, write_string, write_value};

/// One JSON value: `serde_json::Value` itself, re-exported for zero-cost interop.
///
/// Numbers are `Value::Number`; use `as_i64`, `as_u64`, `as_f64`, `is_i64` and
/// `is_f64` to read them. Its `Display` writes compact JSON *without* the HTML-safe
/// escaping; use [`to_string`] for text that may end up in a page.
pub use serde_json::Value;

/// A JSON number (an integer that fits `i64`/`u64`, or a finite float).
pub use serde_json::Number;

/// A JSON object: `serde_json::Map<String, Value>`, keys in insertion order.
///
/// Use `shift_remove` to delete a key and keep the others in order (`remove` swaps the
/// last key into the hole).
pub type Object = serde_json::Map<String, Value>;

/// The name of `value`'s kind for messages: `"null"`, `"a boolean"`, `"an integer"`,
/// `"a number"`, `"a string"`, `"an array"` or `"an object"`.
pub fn kind_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(n) if n.is_f64() => "a number",
        Value::Number(_) => "an integer",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Converts `value` into any type that implements `serde::Deserialize`, consuming it.
/// Fails with [`ErrorKind::Data`] (and no position) when the shape does not fit, such as
/// a missing field or a string where a number belongs.
pub fn from_value<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, Error> {
    serde_json::from_value(value).map_err(|e| Error::from_data(&e))
}

/// Converts any `serde::Serialize` value into a [`Value`]. Fails with
/// [`ErrorKind::Data`] when it cannot be JSON, such as a map whose keys are not strings.
pub fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, Error> {
    serde_json::to_value(value).map_err(|e| Error::from_data(&e))
}

#[cfg(test)]
mod tests;
