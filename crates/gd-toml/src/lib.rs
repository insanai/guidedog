//! TOML 1.0: a parser into an ordered value tree and a writer back to text.
//!
//! [`parse`] reads every TOML 1.0 form: tables and arrays of tables, dotted and quoted
//! keys, inline tables, arrays, the four string kinds, integers in all bases, floats
//! (with `inf` and `nan`), booleans and the four date-time kinds. Tables keep their keys
//! in definition order and every value remembers its line and byte offset, so a setting
//! that is valid TOML but wrong for the program can still point at itself.
//!
//! A malformed source is an [`Error`] with a position, an explanation and a hint; turn it
//! into an Elm-style report with [`Error::report`]. [`to_string`] writes a table back as a
//! TOML document, and [`write_value`], [`write_string`] and [`write_key`] write pieces.
//!
//! ```
//! let table = gd_toml::parse("[site]\ntitle = \"Guide\"\n").unwrap();
//! let title = table.get("site").and_then(|s| s.get("title")).and_then(|t| t.as_str());
//! assert_eq!(title, Some("Guide"));
//! ```

#![forbid(unsafe_code)]

mod datetime;
mod error;
mod number;
mod parse;
mod string;
mod table;
mod write;

pub use datetime::{Date, Datetime, Offset, Time};
pub use error::{Error, ErrorKind};
pub use parse::{MAX_DEPTH, parse};
pub use table::Table;
pub use write::{to_string, write_key, write_string, write_table, write_value};

/// One TOML value with the place it was written.
///
/// Two values are equal when their data is; where they were written does not matter.
#[derive(Debug, Clone)]
pub struct Value {
    /// The value itself.
    pub data: Data,
    /// The 1-based line where the value starts, or 0 for a value built in code.
    pub line: usize,
    /// The byte offset in the source where the value starts (0 for a built value).
    pub offset: usize,
}

/// The kinds of TOML value and their contents.
#[derive(Debug, Clone, PartialEq)]
pub enum Data {
    /// A string of any of the four kinds, with escapes resolved.
    String(String),
    /// A 64-bit signed integer.
    Integer(i64),
    /// A 64-bit float, possibly infinite or NaN.
    Float(f64),
    /// `true` or `false`.
    Boolean(bool),
    /// An offset or local date-time, a local date or a local time.
    Datetime(Datetime),
    /// An array, inline or of tables.
    Array(Vec<Value>),
    /// A table, from a header, dotted keys or an inline table.
    Table(Table),
}

impl Value {
    /// Wraps `data` as a value with no source position (line 0), for building documents.
    pub fn new(data: Data) -> Value {
        Value { data, line: 0, offset: 0 }
    }

    /// The name of the value's kind as a message would say it: `"a string"`,
    /// `"an integer"`, `"a float"`, `"a boolean"`, `"a date-time"`, `"an array"` or
    /// `"a table"`.
    pub fn kind_name(&self) -> &'static str {
        match self.data {
            Data::String(_) => "a string",
            Data::Integer(_) => "an integer",
            Data::Float(_) => "a float",
            Data::Boolean(_) => "a boolean",
            Data::Datetime(_) => "a date-time",
            Data::Array(_) => "an array",
            Data::Table(_) => "a table",
        }
    }

    /// The text of a string value, borrowed from it; `None` for other kinds.
    pub fn as_str(&self) -> Option<&str> {
        match &self.data {
            Data::String(s) => Some(s),
            _ => None,
        }
    }

    /// The number of an integer value; `None` for other kinds.
    pub fn as_integer(&self) -> Option<i64> {
        match self.data {
            Data::Integer(n) => Some(n),
            _ => None,
        }
    }

    /// The number of a float value, or of an integer converted to a float.
    pub fn as_float(&self) -> Option<f64> {
        match self.data {
            Data::Float(f) => Some(f),
            Data::Integer(n) => Some(n as f64),
            _ => None,
        }
    }

    /// The truth of a boolean value; `None` for other kinds.
    pub fn as_bool(&self) -> Option<bool> {
        match self.data {
            Data::Boolean(b) => Some(b),
            _ => None,
        }
    }

    /// The date-time of a date-time value; `None` for other kinds.
    pub fn as_datetime(&self) -> Option<&Datetime> {
        match &self.data {
            Data::Datetime(d) => Some(d),
            _ => None,
        }
    }

    /// The items of an array value, borrowed; `None` for other kinds.
    pub fn as_array(&self) -> Option<&[Value]> {
        match &self.data {
            Data::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The table of a table value, borrowed; `None` for other kinds.
    pub fn as_table(&self) -> Option<&Table> {
        match &self.data {
            Data::Table(t) => Some(t),
            _ => None,
        }
    }

    /// The table of a table value, mutably borrowed; `None` for other kinds.
    pub fn as_table_mut(&mut self) -> Option<&mut Table> {
        match &mut self.data {
            Data::Table(t) => Some(t),
            _ => None,
        }
    }

    /// The value of `key` when this is a table that has it; `None` otherwise.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_table().and_then(|t| t.get(key))
    }

    /// Reports whether the value is a table.
    pub fn is_table(&self) -> bool {
        matches!(self.data, Data::Table(_))
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        self.data == other.data
    }
}

impl From<Data> for Value {
    fn from(data: Data) -> Value {
        Value::new(data)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::new(Data::String(s.to_string()))
    }
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::new(Data::String(s))
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Value {
        Value::new(Data::Integer(n))
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Value {
        Value::new(Data::Float(f))
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::new(Data::Boolean(b))
    }
}

impl From<Datetime> for Value {
    fn from(d: Datetime) -> Value {
        Value::new(Data::Datetime(d))
    }
}

impl From<Vec<Value>> for Value {
    fn from(items: Vec<Value>) -> Value {
        Value::new(Data::Array(items))
    }
}

impl From<Table> for Value {
    fn from(t: Table) -> Value {
        Value::new(Data::Table(t))
    }
}

#[cfg(test)]
mod tests;
