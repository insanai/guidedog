//! `json_dumps`: a TOML value as Python's `json.dumps` writes it with its defaults, for
//! `mathjax3_config` and `mathjax4_config`, which a page sets as `window.MathJax`.

use gd_toml::{Data, Value};

/// Writes `value` as `json.dumps` does: `", "` and `": "` separators, and every
/// character outside printable ASCII escaped. Date-times become strings.
pub fn json_dumps(value: &Value) -> String {
    let mut out = String::new();
    write_json(&mut out, value);
    out
}

fn write_json(out: &mut String, value: &Value) {
    match &value.data {
        Data::Table(table) => {
            out.push('{');
            for (i, (key, item)) in table.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_json_string(out, key);
                out.push_str(": ");
                write_json(out, item);
            }
            out.push('}');
        }
        Data::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_json(out, item);
            }
            out.push(']');
        }
        Data::String(text) => write_json_string(out, text),
        Data::Datetime(when) => write_json_string(out, &when.to_string()),
        Data::Integer(n) => out.push_str(&n.to_string()),
        Data::Float(f) => write_json_float(out, *f),
        Data::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
    }
}

/// Writes a float as Python's repr: the shortest digits, `.0` when whole.
fn write_json_float(out: &mut String, f: f64) {
    if f.is_nan() {
        out.push_str("NaN");
    } else if f.is_infinite() {
        out.push_str(if f > 0.0 { "Infinity" } else { "-Infinity" });
    } else if f == f.trunc() && f.abs() < 1e16 {
        out.push_str(&format!("{f:.1}"));
    } else {
        out.push_str(&format!("{f}"));
    }
}

fn write_json_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ' '..='~' => out.push(c),
            _ => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_written_as_python_dumps_them() {
        let table =
            gd_toml::parse("t = {a = 1, b = [true, 2.5, 3.0], c = \"\u{e9}\\n\"}\n").unwrap();
        let text = json_dumps(table.get("t").unwrap());
        assert_eq!(text, "{\"a\": 1, \"b\": [true, 2.5, 3.0], \"c\": \"\\u00e9\\n\"}");
    }
}
