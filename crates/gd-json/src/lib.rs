//! JSON (RFC 8259): a value type with ordered objects, a bounded parser and a writer.
//!
//! Objects keep their keys in insertion order so that written output is stable from run
//! to run. Integers that fit in 64 bits stay integers ([`Value::Integer`]); other numbers
//! are floats. [`parse`] refuses invalid UTF-16 escapes, raw control characters,
//! duplicate keys (unless [`Limits::allow_duplicate_keys`]) and nesting deeper than
//! [`Limits::max_depth`], each with an [`Error`] that can become an Elm-style report.
//!
//! ```
//! let v = gd_json::parse(r#"{"name": "guide", "pages": [1, 2]}"#).unwrap();
//! assert_eq!(v.get("name").and_then(|n| n.as_str()), Some("guide"));
//! assert_eq!(gd_json::to_string(&v), r#"{"name":"guide","pages":[1,2]}"#);
//! ```

mod error;
mod object;
mod parse;
mod write;

pub use error::{Error, ErrorKind};
pub use object::Object;
pub use parse::{Limits, parse, parse_with};
pub use write::{to_string, to_string_pretty, write_pretty, write_string, write_value};

/// One JSON value.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    /// `null`.
    #[default]
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number without fraction or exponent that fits in an `i64`.
    Integer(i64),
    /// Any other number. Writing a NaN or infinity produces `null`.
    Float(f64),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Value>),
    /// An object with keys in insertion order.
    Object(Object),
}

impl Value {
    /// Reports whether the value is `null`.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// The truth of a boolean; `None` for other kinds.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The number of an integer; `None` for floats and other kinds.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Integer(n) => Some(*n),
            _ => None,
        }
    }

    /// The number of a float, or of an integer converted to a float.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Integer(n) => Some(*n as f64),
            _ => None,
        }
    }

    /// The text of a string, borrowed; `None` for other kinds.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// The items of an array, borrowed; `None` for other kinds.
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The object, borrowed; `None` for other kinds.
    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// The object, mutably borrowed; `None` for other kinds.
    pub fn as_object_mut(&mut self) -> Option<&mut Object> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// The value of `key` when this is an object that has it; `None` otherwise.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_object().and_then(|o| o.get(key))
    }

    /// The name of the value's kind for messages: `"null"`, `"a boolean"`, `"an integer"`,
    /// `"a number"`, `"a string"`, `"an array"` or `"an object"`.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "a boolean",
            Value::Integer(_) => "an integer",
            Value::Float(_) => "a number",
            Value::String(_) => "a string",
            Value::Array(_) => "an array",
            Value::Object(_) => "an object",
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Value {
        Value::Integer(n)
    }
}

impl From<usize> for Value {
    /// Converts a count; one beyond `i64::MAX` becomes a float.
    fn from(n: usize) -> Value {
        match i64::try_from(n) {
            Ok(n) => Value::Integer(n),
            Err(_) => Value::Float(n as f64),
        }
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Value {
        Value::Float(f)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::String(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::String(s)
    }
}

impl From<Vec<Value>> for Value {
    fn from(items: Vec<Value>) -> Value {
        Value::Array(items)
    }
}

impl From<Object> for Value {
    fn from(o: Object) -> Value {
        Value::Object(o)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    /// `None` becomes `null`.
    fn from(v: Option<T>) -> Value {
        v.map_or(Value::Null, Into::into)
    }
}

impl std::fmt::Display for Value {
    /// Writes the value as compact JSON, as [`to_string`] does.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = String::new();
        write_value(&mut out, self);
        f.write_str(&out)
    }
}

impl std::str::FromStr for Value {
    type Err = Error;

    /// Parses JSON text with the default [`Limits`], as [`parse`] does.
    fn from_str(text: &str) -> Result<Value, Error> {
        parse(text)
    }
}

#[cfg(test)]
mod tests;
