//! Parsing: serde_json reads the text, a small `DeserializeSeed` builds the [`Value`] and
//! enforces [`Limits`] (duplicate keys, nesting depth) as it goes, in one pass.

use crate::error::Refusal;
use crate::{Error, ErrorKind, Object, Value};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::cell::RefCell;
use std::fmt;

/// The deepest nesting serde_json accepts: its built-in stack guard stops at 128 open
/// arrays and objects, so 127 is the most a text can use.
pub const MAX_DEPTH: usize = 127;

/// Bounds on what [`parse_with`] accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The deepest nesting of arrays and objects (default and maximum [`MAX_DEPTH`],
    /// 127; a larger value acts as 127).
    pub max_depth: usize,
    /// Whether an object may repeat a key; the last value then wins (default false).
    pub allow_duplicate_keys: bool,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits { max_depth: MAX_DEPTH, allow_duplicate_keys: false }
    }
}

/// Reads `text` as one JSON value with the default [`Limits`]. The value owns its
/// strings. Fails with an [`Error`] at the first mistake, including text after the value.
pub fn parse(text: &str) -> Result<Value, Error> {
    parse_with(text, &Limits::default())
}

/// Reads `text` as one JSON value within `limits`; a leading byte order mark is skipped.
/// Fails as [`parse`] does, with [`ErrorKind::Duplicate`] for a repeated key (unless
/// allowed) and [`ErrorKind::Depth`] beyond `limits.max_depth`.
pub fn parse_with(text: &str, limits: &Limits) -> Result<Value, Error> {
    let skipped = if text.starts_with('\u{feff}') { 3 } else { 0 };
    let refusal = RefCell::new(None);
    let build = Build { depth: 0, limits, refusal: &refusal };
    let mut reader = serde_json::Deserializer::from_str(&text[skipped..]);
    let result = build.deserialize(&mut reader).and_then(|value| {
        reader.end()?;
        Ok(value)
    });
    result.map_err(|err| Error::from_parse(&err, text, skipped, refusal.into_inner()))
}

/// Builds one value at nesting `depth`. A refusal is
/// recorded beside serde_json's error so the error keeps serde_json's position but
/// carries this crate's kind and message.
struct Build<'a> {
    depth: usize,
    limits: &'a Limits,
    refusal: &'a RefCell<Option<Refusal>>,
}

impl Build<'_> {
    fn child(&self) -> Build<'_> {
        Build { depth: self.depth + 1, limits: self.limits, refusal: self.refusal }
    }

    fn refuse<E: de::Error>(&self, refusal: Refusal) -> E {
        let error = E::custom(&refusal.message);
        *self.refusal.borrow_mut() = Some(refusal);
        error
    }

    /// Refuses to open an array or object at this depth when it is too deep.
    fn check_depth<E: de::Error>(&self) -> Result<(), E> {
        if self.depth < self.limits.max_depth {
            return Ok(());
        }
        let refusal = Refusal {
            kind: ErrorKind::Depth,
            message: format!("Arrays and objects nest more than {} deep.", self.limits.max_depth),
            hint: "Flatten the data, or raise the nesting limit if the text is trusted.",
            key: None,
        };
        Err(self.refuse(refusal))
    }
}

impl<'de> DeserializeSeed<'de> for Build<'_> {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, reader: D) -> Result<Value, D::Error> {
        reader.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Build<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_bool<E>(self, b: bool) -> Result<Value, E> {
        Ok(Value::Bool(b))
    }

    fn visit_i64<E>(self, n: i64) -> Result<Value, E> {
        Ok(Value::from(n))
    }

    fn visit_u64<E>(self, n: u64) -> Result<Value, E> {
        Ok(Value::from(n))
    }

    fn visit_f64<E>(self, f: f64) -> Result<Value, E> {
        Ok(Value::from(f))
    }

    fn visit_str<E>(self, s: &str) -> Result<Value, E> {
        Ok(Value::String(s.to_string()))
    }

    fn visit_string<E>(self, s: String) -> Result<Value, E> {
        Ok(Value::String(s))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut items: A) -> Result<Value, A::Error> {
        self.check_depth()?;
        let mut out = Vec::with_capacity(items.size_hint().unwrap_or(0));
        while let Some(item) = items.next_element_seed(self.child())? {
            out.push(item);
        }
        Ok(Value::Array(out))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut members: A) -> Result<Value, A::Error> {
        self.check_depth()?;
        let mut out = Object::new();
        while let Some(key) = members.next_key::<String>()? {
            if !self.limits.allow_duplicate_keys && out.contains_key(&key) {
                let refusal = Refusal {
                    kind: ErrorKind::Duplicate,
                    message: format!("The key `{key}` appears twice in this object."),
                    hint: "Remove or rename one of them; JSON readers disagree on which wins.",
                    key: Some(key),
                };
                return Err(self.refuse(refusal));
            }
            let value = members.next_value_seed(self.child())?;
            // A repeated key (when allowed) keeps its first place and takes the last value.
            out.insert(key, value);
        }
        Ok(Value::Object(out))
    }
}
