//! Escaping for the HTML gd-site writes itself (index pages, the sidebar, script tags)
//! and for the strings of the search index.

/// Appends `text` escaped for HTML content and attribute values: `&`, `<`, `>`, `"` and
/// `'`.
pub(crate) fn push_escaped(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
}

/// Appends `text` as a JSON string that is also safe inside a `<script>`: `<` is
/// written `\u003c`, so the text can never close the script.
pub(crate) fn push_js_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '<' => out.push_str("\\u003c"),
            '\u{0}'..='\u{1f}' | '\u{2028}' | '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escapes_markup_and_quotes() {
        let mut out = String::new();
        push_escaped(&mut out, "a<b>&\"c'");
        assert_eq!(out, "a&lt;b&gt;&amp;&quot;c&#39;");
    }

    #[test]
    fn js_strings_never_close_a_script() {
        let mut out = String::new();
        push_js_string(&mut out, "</script>\n\"\\\u{2028}");
        assert_eq!(out, "\"\\u003c/script>\\u000a\\\"\\\\\\u2028\"");
    }
}
