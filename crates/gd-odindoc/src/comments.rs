//! Doc comments are plain text. `doc_rest` writes one as reStructuredText that reads as
//! it was written, with the conventions of Odin's core library made into markup:
//!
//! - Paragraphs (lines up to a blank line) stay paragraphs. A line indented more than
//!   the text, directly after a paragraph line, list item, or field, continues it.
//! - An indented block (a tab or spaces) after a blank line, or after a line ending in
//!   ":" ("Example:"), is code: `.. code-block:: odin`, or `text` after "Output:".
//! - "Inputs:" on a line of its own starts parameter items: "- name: text" becomes
//!   `:param name: text`. "Returns:" starts result items: "- name: text" becomes
//!   `:result name: text`, items without a name become one `:returns:` field, and plain
//!   text after it (or on its line: "Returns: the length") becomes `:returns: text`.
//! - A line starting "NOTE:" or "WARNING:" (any case) is a note or warning admonition,
//!   with the lines up to the next blank line.
//! - "- item" and "* item" lines are a bullet list.
//! - `code` is inline code when reStructuredText can read it as such. Everything else
//!   that reStructuredText would read as markup is escaped: *, |, \, a _ ending a word
//!   (a reference), stray backquotes, and at the start of a line a list, field, option,
//!   comment, doctest, or line of punctuation (a section underline or transition); a
//!   trailing "::" keeps both colons.

use crate::text::{dedent, split_lines};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    None,
    Para,
    Item,
    Field,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    Inputs,
    Returns,
}

struct Converter<'d> {
    lines: Vec<&'d str>,
    i: usize,
    out: Vec<String>,
    block: Block,
    section: Section,
    /// Result items without a name.
    returns: Vec<String>,
    /// A blank line came since the last text.
    blank: bool,
    /// The last line of text read, trimmed.
    last: &'d str,
}

/// Converts a doc comment to reStructuredText lines (no trailing blank line); an empty
/// comment gives none.
pub(crate) fn doc_rest(doc: &str) -> Vec<String> {
    if doc.trim().is_empty() {
        return Vec::new();
    }
    let mut c = Converter {
        lines: split_lines(doc).collect(),
        i: 0,
        out: Vec::new(),
        block: Block::None,
        section: Section::None,
        returns: Vec::new(),
        blank: false,
        last: "",
    };
    while c.i < c.lines.len() {
        c.step();
    }
    c.end_section();
    while c.out.last().is_some_and(String::is_empty) {
        c.out.pop();
    }
    c.out
}

impl<'d> Converter<'d> {
    fn step(&mut self) {
        let line = self.lines[self.i];
        let text = line.trim();
        if text.is_empty() {
            self.blank = true;
            self.i += 1;
            return;
        }
        let indented = indent_of(line) > 0;
        if indented && self.section != Section::None && self.section_indented(text) {
            return;
        }
        if indented {
            let continues = !self.blank && self.block != Block::None && !self.last.ends_with(':');
            if continues {
                self.continue_block(text);
            } else {
                self.code_block();
            }
            return;
        }
        self.i += 1;
        self.text_line(text);
        self.blank = false;
        self.last = text;
    }

    /// A line of text at the left margin.
    fn text_line(&mut self, text: &'d str) {
        if self.section != Section::None && self.section_line(text) {
            return;
        }
        if self.blank && self.block != Block::Note && self.section != Section::None {
            self.end_section();
        }
        if text == "Inputs:" || text == "Returns:" {
            self.end_section();
            self.section = if text == "Inputs:" { Section::Inputs } else { Section::Returns };
            self.block = Block::None;
        } else if let Some(rest) = text.strip_prefix("Returns: ") {
            self.end_section();
            self.section = Section::Returns;
            self.start(Block::Field, format!(":returns: {}", escape_line(rest)));
        } else if let Some(kind) = note_kind(text) {
            self.start(Block::Note, format!(".. {kind}::"));
            let colon = text.find(':').map_or(text.len(), |k| k + 1);
            let rest = text[colon..].trim();
            if !rest.is_empty() {
                self.out.push(format!("   {}", escape_line(rest)));
            }
        } else if let Some(item) = text.strip_prefix("- ").or_else(|| text.strip_prefix("* ")) {
            self.start(Block::Item, format!("- {}", escape_line(item)));
        } else if self.block == Block::Note && !self.blank {
            self.out.push(format!("   {}", escape_line(text)));
        } else if self.block == Block::Para && !self.blank {
            self.out.push(escape_line(text));
        } else {
            self.start(Block::Para, escape_line(text));
        }
    }

    /// Reads a line of an Inputs or Returns section, and reports whether it was one: an
    /// item, or the text of a Returns section.
    fn section_line(&mut self, text: &str) -> bool {
        let Some(rest) = text.strip_prefix("- ") else {
            if self.section != Section::Returns || self.blank || self.block == Block::Para {
                return false;
            }
            if self.block == Block::None {
                self.start(Block::Field, format!(":returns: {}", escape_line(text)));
                return true;
            }
            return false;
        };
        let item = rest.trim();
        match (split_item(item), self.section) {
            (Some((name, rest)), Section::Inputs) => {
                self.start(Block::Field, format!(":param {name}: {}", escape_line(rest)));
            }
            (Some((name, rest)), _) => {
                self.start(Block::Field, format!(":result {name}: {}", escape_line(rest)));
            }
            (None, Section::Returns) => {
                self.returns.push(escape_line(item));
                self.block = Block::None;
            }
            (None, _) => self.start(Block::Item, format!("- {}", escape_line(item))),
        }
        true
    }

    /// Reads an indented line in an Inputs or Returns section: an item, the text of a
    /// Returns section, or the continuation of an item. It reports whether it read the
    /// line; otherwise the line is code.
    fn section_indented(&mut self, text: &'d str) -> bool {
        let after_item = !self.blank && (self.block == Block::Field || !self.returns.is_empty());
        if text.starts_with("- ") {
            self.i += 1;
            self.blank = false;
            self.section_line(text);
        } else if after_item {
            self.continue_block(text);
            self.blank = false;
        } else if self.section == Section::Returns
            && self.block == Block::None
            && self.returns.is_empty()
        {
            self.i += 1;
            self.blank = false;
            self.start(Block::Field, format!(":returns: {}", escape_line(text)));
        } else {
            return false;
        }
        self.last = text;
        true
    }

    fn end_section(&mut self) {
        if self.section == Section::None {
            return;
        }
        self.section = Section::None;
        if self.returns.is_empty() {
            return;
        }
        let returns = std::mem::take(&mut self.returns);
        if let [only] = returns.as_slice() {
            self.start(Block::Field, format!(":returns: {only}"));
        } else {
            self.start(Block::Field, ":returns:".to_string());
            self.out.extend(returns.iter().map(|r| format!("   - {r}")));
        }
        self.block = Block::None;
    }

    /// Begins a block with its first line; a blank line separates it from the one
    /// before, except between items of one list or fields of one list.
    fn start(&mut self, block: Block, first: String) {
        let listed = matches!(block, Block::Item | Block::Field);
        let same = block == self.block && listed && !self.blank;
        let together = block == Block::Field && self.block == Block::Field;
        if self.out.last().is_some_and(|l| !l.is_empty()) && !same && !together {
            self.out.push(String::new());
        }
        self.out.push(first);
        self.block = block;
    }

    fn continue_block(&mut self, text: &'d str) {
        self.i += 1;
        self.last = text;
        // A continuation of a result item without a name joins that item.
        if self.section == Section::Returns
            && self.block == Block::None
            && let Some(r) = self.returns.last_mut()
        {
            r.push(' ');
            r.push_str(&escape_line(text));
            return;
        }
        let pad = match self.block {
            Block::Item => "  ",
            Block::Field | Block::Note => "   ",
            Block::Para | Block::None => "",
        };
        self.out.push(format!("{pad}{}", escape_line(text)));
    }

    /// Writes the indented lines from here as a code block.
    fn code_block(&mut self) {
        let lang = if self.last == "Output:" { "text" } else { "odin" };
        let mut end = self.i;
        while end < self.lines.len() {
            let l = self.lines[end];
            if !l.trim().is_empty() && indent_of(l) == 0 {
                break;
            }
            end += 1;
        }
        let body: Vec<String> = self.lines[self.i..end].iter().map(|l| expand_tabs(l)).collect();
        let refs: Vec<&str> = body.iter().map(String::as_str).collect();
        let code = dedent(&refs);
        self.i = end;
        self.end_section();
        self.start(Block::None, format!(".. code-block:: {lang}"));
        self.out.push(String::new());
        for l in code {
            self.out.push(if l.is_empty() { String::new() } else { format!("   {l}") });
        }
        self.out.push(String::new());
        self.block = Block::None;
        self.blank = true;
    }
}

/// Splits "name: text" when name is an identifier.
fn split_item(item: &str) -> Option<(&str, &str)> {
    let colon = item.find(':').filter(|&k| k > 0)?;
    let name = &item[..colon];
    if !name.bytes().all(is_word) {
        return None;
    }
    Some((name, item[colon + 1..].trim()))
}

fn is_word(c: u8) -> bool {
    c == b'_' || c.is_ascii_alphanumeric()
}

fn indent_of(line: &str) -> usize {
    let mut n = 0;
    for c in line.bytes() {
        match c {
            b' ' => n += 1,
            b'\t' => n += 4,
            _ => return n,
        }
    }
    n
}

fn expand_tabs(line: &str) -> String {
    let k = line.bytes().take_while(|&c| c == b' ' || c == b'\t').count();
    if !line[..k].contains('\t') {
        return line.to_string();
    }
    format!("{}{}", " ".repeat(indent_of(line)), &line[k..])
}

fn note_kind(text: &str) -> Option<&'static str> {
    let starts = |prefix: &str| {
        text.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix))
    };
    if starts("NOTE:") {
        Some("note")
    } else if starts("WARNING:") {
        Some("warning")
    } else {
        None
    }
}

/// Escapes one line of text; see the module comment.
pub(crate) fn escape_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    if needs_start_escape(text) {
        out.push('\\');
    }
    escape_inline(&mut out, text);
    if out.ends_with("::") {
        out.pop();
        out.push_str("\\:");
    }
    out
}

/// Whether a line's start would be read as a list, field, option, comment, doctest, or
/// underline.
fn needs_start_escape(t: &str) -> bool {
    let b = t.as_bytes();
    let Some(&c) = b.first() else {
        return false;
    };
    match c {
        b':' | b'+' | b'>' | b'#' => true,
        b'-' if b.len() == 1 || b[1] != b' ' => true,
        b'-' | b'*' | b'|' | b'\\' | b'`' | b'_' => false,
        _ if t.starts_with("..") => true,
        _ if punctuation_line(b) => true,
        b'0'..=b'9' | b'(' => {
            let mut k = usize::from(c == b'(');
            while k < b.len() && b[k].is_ascii_digit() {
                k += 1;
            }
            k < b.len() && matches!(b[k], b'.' | b')') && (k + 1 == b.len() || b[k + 1] == b' ')
        }
        _ => false,
    }
}

fn punctuation_line(b: &[u8]) -> bool {
    if b.len() < 2 || is_word(b[0]) || b[0] == b' ' {
        return false;
    }
    b[1..].iter().all(|&c| c == b[0])
}

/// Writes text with its inline markup escaped and `code` as literals.
fn escape_inline(out: &mut String, t: &str) {
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            b'`' => {
                i = code_span(out, t, i);
                continue;
            }
            b'\\' | b'*' | b'|' => out.push('\\'),
            b'_' if i + 1 == b.len() || !is_alnum(b[i + 1]) => out.push('\\'),
            _ => {}
        }
        let width = t[i..].chars().next().map_or(1, char::len_utf8);
        out.push_str(&t[i..i + width]);
        i += width;
    }
}

/// Writes the `code` span at `i` as ``code`` when it can be read as one, or an escaped
/// backquote; returns where reading goes on.
fn code_span(out: &mut String, t: &str, i: usize) -> usize {
    let width = if t[i..].starts_with("``") { 2 } else { 1 };
    let marker = &t[i..i + width];
    if let Some(close) = t[i + width..].find(marker).filter(|&k| k > 0) {
        let j = i + width + close;
        let content = &t[i + width..j];
        let b = t.as_bytes();
        let before_ok = i == 0 || b" \t-:/'\"<([{".contains(&b[i - 1]);
        let after = j + width;
        let after_ok = after == b.len() || b" \t-.,:;!?\\/'\")]}>".contains(&b[after]);
        let trimmed = content.trim() == content;
        if before_ok && after_ok && trimmed && !content.contains("``") {
            out.push_str("``");
            out.push_str(content);
            out.push_str("``");
            return after;
        }
    }
    out.push_str("\\`");
    i + 1
}

fn is_alnum(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c >= 0x80
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_comments_become_rest() {
        let doc = "Clones a string\n\n*Allocates Using Provided Allocator*\n\nInputs:\n\
            - s: The string to be cloned\n- allocator: (default: context.allocator)\n\
            \tmore about it\n\nNOTE: The created string is only valid\n\
            \t  as long as the pointer.\n\nReturns:\n- res: The cloned string\n\
            - err: An error, `nil` otherwise\n\nExample:\n\n\timport \"core:fmt\"\n\
            \tfmt.println(x)\n\nOutput:\n\n\t42\n\nSee foo_ and [1]_ or a|b, `x`y.\n\
            - one\n- two\nAfter.\n..odd\n====\nends::";
        let want = "Clones a string\n\n\\*Allocates Using Provided Allocator\\*\n\n\
            :param s: The string to be cloned\n\
            :param allocator: (default: context.allocator)\n   more about it\n\n\
            .. note::\n   The created string is only valid\n   as long as the pointer.\n\n\
            :result res: The cloned string\n:result err: An error, ``nil`` otherwise\n\n\
            Example:\n\n.. code-block:: odin\n\n   import \"core:fmt\"\n   fmt.println(x)\n\n\
            Output:\n\n.. code-block:: text\n\n   42\n\n\
            See foo\\_ and [1]\\_ or a\\|b, \\`x\\`y.\n\n- one\n- two\n\nAfter.\n\\..odd\n\
            \\====\nends:\\:";
        assert_eq!(doc_rest(doc).join("\n"), want);
        assert_eq!(doc_rest("Returns: the length").join("\n"), ":returns: the length");
        assert_eq!(doc_rest("Returns:\n- a\n- b").join("\n"), ":returns:\n   - a\n   - b");
    }
}
