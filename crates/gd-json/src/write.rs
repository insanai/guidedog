//! Writing values as JSON text: serde_json's serializer with a formatter that also
//! escapes the characters an HTML `<script>` element cannot hold.

use crate::Value;
use serde::Serialize;
use serde_json::ser::{CompactFormatter, Formatter, PrettyFormatter};
use std::io::{self, Write};

/// Writes `value` as compact JSON (no spaces or line breaks) into a new string.
pub fn to_string(value: &Value) -> String {
    let mut out = String::with_capacity(64);
    write_value(&mut out, value);
    out
}

/// Writes `value` as JSON indented by two spaces per level, without a final line break.
pub fn to_string_pretty(value: &Value) -> String {
    let mut out = String::with_capacity(128);
    write_pretty(&mut out, value, 0);
    out
}

/// Appends `value` as compact JSON to `out`. Object keys keep their order; NaN and
/// infinite floats cannot be stored in a [`Value`] (they become `null` on conversion).
pub fn write_value(out: &mut String, value: &Value) {
    serialize(out, value, HtmlSafe(CompactFormatter));
}

/// Appends `value` as JSON to `out`, indented as if it started at nesting `level`: each
/// array item and object member on its own line, two spaces deeper. Empty arrays and
/// objects stay `[]` and `{}`.
pub fn write_pretty(out: &mut String, value: &Value, level: usize) {
    let start = out.len();
    serialize(out, value, HtmlSafe(PrettyFormatter::with_indent(b"  ")));
    if level > 0 {
        // Strings never hold a raw line break (it is escaped), so every '\n' written is
        // a layout line break and can take the extra indentation.
        let indented = out[start..].replace('\n', &format!("\n{}", "  ".repeat(level)));
        out.truncate(start);
        out.push_str(&indented);
    }
}

/// Appends `text` to `out` as a JSON string with its quotes. Quotes, backslashes and
/// control characters are escaped, and so are `<`, `>`, `&` and U+2028/U+2029, so the
/// output can be embedded in an HTML `<script>` element unchanged.
pub fn write_string(out: &mut String, text: &str) {
    serialize(out, text, HtmlSafe(CompactFormatter));
}

/// Runs serde_json's serializer into a byte buffer and appends the text to `out`.
fn serialize<T: Serialize + ?Sized, F: Formatter>(out: &mut String, value: &T, format: F) {
    let mut bytes = Vec::with_capacity(64);
    let mut writer = serde_json::Serializer::with_formatter(&mut bytes, format);
    // Writing a Value or a str into a Vec cannot fail: keys are strings and the sink
    // never refuses bytes.
    let _ = value.serialize(&mut writer);
    // serde_json writes UTF-8 only, so this borrows without replacing anything.
    out.push_str(&String::from_utf8_lossy(&bytes));
}

/// Wraps a serde_json formatter (compact or pretty) so string contents also escape `<`,
/// `>`, `&`, U+2028 and U+2029. Everything else is delegated unchanged.
struct HtmlSafe<F>(F);

/// The characters escaped beyond what JSON itself requires, as `\uXXXX`.
fn html_escape(c: char) -> bool {
    matches!(c, '<' | '>' | '&' | '\u{2028}' | '\u{2029}')
}

impl<F: Formatter> Formatter for HtmlSafe<F> {
    /// serde_json passes the runs of a string that need no JSON escape here; split them
    /// further at the HTML-unsafe characters.
    fn write_string_fragment<W: ?Sized + Write>(
        &mut self,
        w: &mut W,
        frag: &str,
    ) -> io::Result<()> {
        let mut run = 0;
        for (i, c) in frag.char_indices() {
            if html_escape(c) {
                w.write_all(&frag.as_bytes()[run..i])?;
                write!(w, "\\u{:04x}", c as u32)?;
                run = i + c.len_utf8();
            }
        }
        w.write_all(&frag.as_bytes()[run..])
    }

    fn begin_array<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.begin_array(w)
    }

    fn end_array<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_array(w)
    }

    fn begin_array_value<W: ?Sized + Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        self.0.begin_array_value(w, first)
    }

    fn end_array_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_array_value(w)
    }

    fn begin_object<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.begin_object(w)
    }

    fn end_object<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_object(w)
    }

    fn begin_object_key<W: ?Sized + Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        self.0.begin_object_key(w, first)
    }

    fn begin_object_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.begin_object_value(w)
    }

    fn end_object_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_object_value(w)
    }
}
