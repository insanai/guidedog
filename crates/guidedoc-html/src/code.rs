//! Literal blocks, with captions, line numbers, emphasized lines, and highlighting, and
//! displayed math.

use guidedoc::{AttributeKey, Label, MathStyle, NodeId, label};

use crate::state::Html;
use crate::walk::Visit;

impl Html<'_, '_> {
    /// Writes a literal block. The reference profile keeps CommonMark's
    /// `<pre><code class="language-x">`; otherwise a block with a language takes
    /// Sphinx's `<div class="highlight-x notranslate"><div class="highlight"><pre>`
    /// structure, and a caption wraps it in a literal-block-wrapper container that
    /// carries the ids.
    pub fn enter_code(&mut self, id: NodeId) -> Visit {
        if self.w.reference {
            return self.enter_reference_code(id);
        }
        let language = self.doc.attribute(id, AttributeKey::Language).unwrap_or("");
        let caption = self.custom(id, "caption");
        self.w.cr();
        if !caption.is_empty() {
            self.write_code_caption(id, caption);
        }
        let labelled = caption.is_empty();
        if language.is_empty() {
            self.w.put("<pre");
            if labelled {
                self.write_id_attribute(id);
                self.write_classes(id, "");
            }
            self.w.put("><code>");
            if labelled {
                self.write_aliases(id);
            }
        } else {
            self.w.put("<div");
            if labelled {
                self.write_id_attribute(id);
            }
            self.w.put(" class=\"highlight-");
            self.w.esc(language);
            self.w.put(" notranslate");
            if labelled {
                self.write_more_classes(id);
            }
            self.w.put("\">");
            if labelled {
                self.write_aliases(id);
            }
            self.w.put("<div class=\"highlight\"><pre>");
        }
        self.write_code_body(id, language, self.doc.node_text(id));
        self.w.put(if language.is_empty() { "</code></pre>" } else { "</pre></div></div>" });
        if !caption.is_empty() {
            self.w.put("\n</div>");
        }
        self.w.cr();
        Visit::Skip
    }

    /// Opens the literal-block-wrapper and writes the caption, numbered and linked.
    fn write_code_caption(&mut self, id: NodeId, caption: &str) {
        self.open_tag("div", id, "literal-block-wrapper docutils container");
        self.w.put_byte(b'>');
        self.write_aliases(id);
        self.w.put("\n<div class=\"code-block-caption\">");
        let number = self.custom(id, "number-label");
        if !number.is_empty() {
            self.w.put("<span class=\"caption-number\">");
            self.w.esc(number);
            self.w.put(" </span>");
        }
        self.w.put("<span class=\"caption-text\">");
        match self.doc.code_caption(id) {
            Some(parsed) => self.walk(parsed + 1, self.doc.subtree_end(parsed), Some(parsed)),
            None => self.w.esc(caption),
        }
        self.w.put("</span>");
        self.headerlink(Some(id), label(self.options, Label::LinkCode));
        self.w.put("</div>\n");
    }

    fn enter_reference_code(&mut self, id: NodeId) -> Visit {
        self.w.cr();
        self.open_tag("pre", id, "");
        self.w.put("><code");
        let language = self.doc.attribute(id, AttributeKey::Language).unwrap_or("");
        if !language.is_empty() {
            self.w.put(" class=\"language-");
            self.w.esc(language);
            self.w.put_byte(b'"');
        }
        self.w.put_byte(b'>');
        self.w.esc(self.doc.node_text(id));
        self.w.put("</code></pre>");
        self.w.cr();
        Visit::Skip
    }

    /// Writes the code, highlighted when a highlighter is set. Numbered or emphasized
    /// lines are highlighted one line at a time, so each line can be wrapped:
    /// `<span class="linenos">` before it, `<span class="hll">` around it, as Pygments
    /// writes.
    fn write_code_body(&mut self, id: NodeId, language: &str, code: &str) {
        let emphasize = self.custom(id, "emphasize-lines");
        let first = i64::from(self.doc.node(id).value);
        let numbered = first > 0 || self.custom(id, "linenos") == "1";
        if !numbered && emphasize.is_empty() {
            if !self.w.highlight(language, code) {
                self.w.esc_code(code);
            }
            return;
        }
        let first = first.max(1);
        let lines = code.split_inclusive('\n').count() as i64;
        let width = digit_count(first + lines.max(1) - 1);
        for (index, text) in code.split_inclusive('\n').enumerate() {
            let line = index as i64 + 1;
            let (text, newline) = match text.strip_suffix('\n') {
                Some(t) => (t, true),
                None => (text, false),
            };
            if numbered {
                let number = first + line - 1;
                self.w.put("<span class=\"linenos\">");
                for _ in digit_count(number)..width {
                    self.w.put_byte(b' ');
                }
                self.w.put_int(number);
                self.w.put("</span>");
            }
            let marked = !emphasize.is_empty() && in_line_ranges(emphasize, line);
            if marked {
                self.w.put("<span class=\"hll\">");
            }
            if !self.w.highlight(language, text) {
                self.w.esc_code(text);
            }
            if newline {
                self.w.put_byte(b'\n');
            }
            if marked {
                self.w.put("</span>");
            }
        }
    }

    /// Writes display math for MathJax or KaTeX. A numbered equation shows its number,
    /// as Sphinx does: `<span class="eqno">(1)<a class="headerlink">`.
    pub fn enter_math_block(&mut self, id: NodeId) -> Visit {
        let o = self.options;
        let sphinx = o.math == MathStyle::Sphinx;
        self.line_open("div", id, if sphinx { "math notranslate nohighlight" } else { "math" });
        let nowrap = self.custom(id, "nowrap") == "1";
        let text = self.doc.node_text(id);
        if sphinx && nowrap {
            self.w.esc(text); // as Sphinx: the text alone, without a number.
            self.line_close("div");
            return Visit::Skip;
        }
        let number = self.custom(id, "number");
        if self.has_anchor(id) && !number.is_empty() {
            self.w.put("<span class=\"eqno\">(");
            self.w.esc(number);
            self.w.put_byte(b')');
            self.headerlink(Some(id), label(o, Label::LinkEquation));
            self.w.put("</span>");
        }
        if nowrap {
            self.w.esc(text);
        } else if sphinx {
            let [open, close] = o.math_display;
            self.w.put(if open.is_empty() { "\\[" } else { open });
            self.write_display_parts(text);
            self.w.put(if close.is_empty() { "\\]" } else { close });
        } else {
            self.w.put("\\[");
            self.w.esc(text);
            self.w.put("\\]");
        }
        self.line_close("div");
        Visit::Skip
    }

    /// Writes displayed math as Sphinx's mathjax extension does: parts separated by
    /// blank lines are aligned together, and a part with `\\` is a split.
    fn write_display_parts(&mut self, text: &str) {
        let count = display_parts(text).count();
        if count > 1 {
            self.w.put(" \\begin{align}\\begin{aligned}");
        }
        for (i, part) in display_parts(text).enumerate() {
            let split = part.contains("\\\\");
            if split {
                self.w.put("\\begin{split}");
            }
            self.w.esc(part);
            if split {
                self.w.put("\\end{split}");
            }
            if i + 1 < count {
                self.w.put("\\\\");
            }
        }
        if count > 1 {
            self.w.put("\\end{aligned}\\end{align} ");
        }
    }
}

/// The parts of displayed math that are not blank, separated by blank lines.
fn display_parts(text: &str) -> impl Iterator<Item = &str> {
    text.split("\n\n").filter(|part| !part.trim().is_empty())
}

fn digit_count(n: i64) -> usize {
    let mut d = 1;
    let mut v = n;
    while v >= 10 {
        v /= 10;
        d += 1;
    }
    d
}

/// Tells whether `line` (from 1) is in a list such as "1,3-5"; "7-" runs to the end.
pub(crate) fn in_line_ranges(spec: &str, line: i64) -> bool {
    let bytes = spec.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i] == b',' || bytes[i] == b' ') {
            i += 1;
        }
        let (lo, next) = read_number(bytes, i);
        i = next;
        let mut hi = lo;
        if i < bytes.len() && bytes[i] == b'-' {
            (hi, i) = read_number(bytes, i + 1);
            if hi == 0 {
                hi = i64::MAX;
            }
        }
        if lo > 0 && line >= lo && line <= hi {
            return true;
        }
        while i < bytes.len() && bytes[i] != b',' {
            i += 1;
        }
    }
    false
}

/// Reads a number after optional spaces; it stops growing past a million.
fn read_number(s: &[u8], start: usize) -> (i64, usize) {
    let mut i = start;
    while i < s.len() && s[i] == b' ' {
        i += 1;
    }
    let mut value = 0i64;
    while i < s.len() && s[i].is_ascii_digit() {
        if value < 1_000_000 {
            value = value * 10 + i64::from(s[i] - b'0');
        }
        i += 1;
    }
    (value, i)
}
