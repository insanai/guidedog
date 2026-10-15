//! The YAML that MyST front matter and directive options use: block mappings and
//! sequences, plain, quoted, literal (|) and folded (>) scalars with chomping and
//! indentation indicators, flow mappings and sequences, and comments. Anchors, aliases,
//! tags, and multiple documents are not supported and are reported as errors.

use super::{trim_right, trim_space};
use crate::extension::Line;

/// How deeply collections may nest, so hostile input cannot exhaust the stack.
const MAX_YAML_DEPTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum YamlKind {
    #[default]
    Null,
    Scalar,
    Mapping,
    Sequence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum YamlStyle {
    #[default]
    Plain,
    Quoted,
    Block,
}

/// One node: a mapping entry's key and value, or a sequence item.
#[derive(Debug, Clone, Default)]
pub(crate) struct YamlNode {
    pub kind: YamlKind,
    pub style: YamlStyle,
    /// The key of a mapping entry.
    pub key: String,
    /// A scalar's value.
    pub text: String,
    /// Source offset of the value.
    pub offset: u32,
    pub first: Option<usize>,
    last: Option<usize>,
    pub next: Option<usize>,
}

/// A parsed YAML document.
#[derive(Debug, Default)]
pub(crate) struct Yaml {
    pub nodes: Vec<YamlNode>,
}

/// Why YAML did not parse, and on which line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct YamlError {
    pub why: &'static str,
    pub line: usize,
}

struct Parser<'y, 'l> {
    y: &'y mut Yaml,
    lines: &'y [Line<'l>],
    error: Option<YamlError>,
    depth: usize,
}

/// Reads `lines` as one YAML document and returns it with its root node.
pub(crate) fn yaml_parse(lines: &[Line<'_>]) -> Result<(Yaml, usize), YamlError> {
    let mut y = Yaml::default();
    let mut p = Parser { y: &mut y, lines, error: None, depth: 0 };
    let i = p.skip(0);
    let root = if i >= lines.len() {
        p.node(YamlKind::Null, lines.first().map_or(0, |l| l.offset))
    } else {
        let (root, next) = p.block(i, indent_of(lines[i].text));
        let rest = p.skip(next);
        if rest < lines.len() {
            p.fail(rest, "unexpected content after the document");
        }
        root
    };
    match p.error {
        Some(e) => Err(e),
        None => Ok((y, root)),
    }
}

pub(crate) fn indent_of(s: &str) -> usize {
    s.bytes().take_while(|&c| c == b' ').count()
}

/// A line of white space or only a comment.
fn is_blank(s: &str) -> bool {
    let t = trim_space(s);
    t.is_empty() || t.starts_with('#')
}

fn is_dash(t: &str) -> bool {
    t.starts_with('-') && (t.len() == 1 || t.as_bytes()[1] == b' ')
}

/// Splits "key: rest" and returns the key and where the rest starts.
fn split_key(t: &str) -> Option<(&str, usize)> {
    let b = t.as_bytes();
    let q = *b.first()?;
    if q == b'"' || q == b'\'' {
        let mut e = 1;
        while e < b.len() && b[e] != q {
            if q == b'"' && b[e] == b'\\' {
                e += 1;
            }
            e += 1;
        }
        if e >= b.len() {
            return None;
        }
        let k = e + 1 + b[e + 1..].iter().take_while(|&&c| c == b' ').count();
        let colon = b.get(k) == Some(&b':') && (k + 1 >= b.len() || b[k + 1] == b' ');
        return colon.then(|| (&t[..e + 1], k + 1));
    }
    if is_dash(t) {
        return None;
    }
    for k in 0..b.len() {
        if b[k] == b'#' && k > 0 && b[k - 1] == b' ' {
            return None;
        }
        if b[k] == b':' && (k + 1 >= b.len() || b[k + 1] == b' ' || b[k + 1] == b'\t') {
            return (k > 0).then(|| (trim_right(&t[..k]), k + 1));
        }
    }
    None
}

/// Removes a trailing " # comment" from a plain scalar's line.
fn strip_comment(s: &str) -> &str {
    let b = s.as_bytes();
    for k in 0..b.len() {
        if b[k] == b'#' && (k == 0 || b[k - 1] == b' ' || b[k - 1] == b'\t') {
            return trim_right(&s[..k]);
        }
    }
    trim_right(s)
}

fn hex_value(c: u8) -> Option<u32> {
    (c as char).to_digit(16)
}

impl Parser<'_, '_> {
    fn node(&mut self, kind: YamlKind, offset: u32) -> usize {
        self.y.nodes.push(YamlNode { kind, offset, ..YamlNode::default() });
        self.y.nodes.len() - 1
    }

    fn append(&mut self, parent: usize, child: usize) {
        match self.y.nodes[parent].last {
            Some(l) => self.y.nodes[l].next = Some(child),
            None => self.y.nodes[parent].first = Some(child),
        }
        self.y.nodes[parent].last = Some(child);
    }

    fn fail(&mut self, line: usize, why: &'static str) {
        if self.error.is_none() {
            self.error = Some(YamlError { why, line });
        }
    }

    fn ok(&self) -> bool {
        self.error.is_none()
    }

    fn skip(&self, i: usize) -> usize {
        let mut k = i;
        while k < self.lines.len() && is_blank(self.lines[k].text) {
            k += 1;
        }
        k
    }

    fn offset(&self, i: usize, col: usize) -> u32 {
        self.lines.get(i).map_or(0, |l| l.offset + col as u32)
    }

    /// Parses the block node whose first line is `i`, at indentation `indent`.
    fn block(&mut self, i: usize, indent: usize) -> (usize, usize) {
        if self.depth >= MAX_YAML_DEPTH {
            self.fail(i, "the YAML nests too deeply");
            return (self.node(YamlKind::Null, 0), self.lines.len());
        }
        self.depth += 1;
        let t = &self.lines[i].text[indent..];
        let r = if is_dash(t) {
            self.sequence(i, indent)
        } else if split_key(t).is_some() {
            self.mapping(i, indent)
        } else {
            self.scalar_lines(i, indent, indent, t)
        };
        self.depth -= 1;
        r
    }

    fn mapping(&mut self, i: usize, indent: usize) -> (usize, usize) {
        let m = self.node(YamlKind::Mapping, self.offset(i, indent));
        let mut k = i;
        while k < self.lines.len() && self.ok() {
            k = self.skip(k);
            if k >= self.lines.len() {
                break;
            }
            let line = self.lines[k].text;
            let n = indent_of(line);
            if n < indent {
                break;
            }
            if n > indent {
                self.fail(k, "unexpected indentation");
                break;
            }
            let Some((key, rest)) = split_key(&line[n..]) else {
                self.fail(k, "expected ':' after key");
                break;
            };
            let (value, next) = self.value(k, indent, n + rest);
            self.y.nodes[value].key = unquote_key(key);
            self.append(m, value);
            k = next;
        }
        (m, k)
    }

    fn sequence(&mut self, i: usize, indent: usize) -> (usize, usize) {
        let sq = self.node(YamlKind::Sequence, self.offset(i, indent));
        let mut k = i;
        while k < self.lines.len() && self.ok() {
            k = self.skip(k);
            if k >= self.lines.len() {
                break;
            }
            let line = self.lines[k].text;
            let n = indent_of(line);
            if n != indent || !is_dash(&line[n..]) {
                break;
            }
            let rest = n + 1 + line[n + 1..].bytes().take_while(|&c| c == b' ').count();
            let (item, next) = if rest < line.len() && !is_blank(&line[rest..]) {
                if split_key(&line[rest..]).is_some() {
                    self.inline_mapping(k, rest)
                } else {
                    self.value(k, indent, n + 1)
                }
            } else {
                self.value(k, indent, line.len())
            };
            self.append(sq, item);
            k = next;
        }
        (sq, k)
    }

    /// Parses "- key: value" whose mapping continues at the column of key.
    fn inline_mapping(&mut self, i: usize, col: usize) -> (usize, usize) {
        let m = self.node(YamlKind::Mapping, self.offset(i, col));
        let line = self.lines[i].text;
        let Some((key, rest)) = split_key(&line[col..]) else { return (m, i + 1) };
        let (value, next) = self.value(i, col, col + rest);
        self.y.nodes[value].key = unquote_key(key);
        self.append(m, value);
        let mut next = self.skip(next);
        if next < self.lines.len() && indent_of(self.lines[next].text) == col && self.ok() {
            let (more, after) = self.mapping(next, col);
            let mut c = self.y.nodes[more].first;
            while let Some(child) = c {
                c = self.y.nodes[child].next;
                self.y.nodes[child].next = None;
                self.append(m, child);
            }
            next = after;
        }
        (m, next)
    }

    /// Parses the value after a key or dash at column `at` of line `i`; the node owning
    /// it is at indentation `indent`.
    fn value(&mut self, i: usize, indent: usize, at: usize) -> (usize, usize) {
        let line = self.lines[i].text;
        let a = at
            + line[at.min(line.len())..].bytes().take_while(|&c| c == b' ' || c == b'\t').count();
        let rest = line.get(a..).unwrap_or("");
        if rest.is_empty() || rest.starts_with('#') {
            let k = self.skip(i + 1);
            if k < self.lines.len() {
                let n = indent_of(self.lines[k].text);
                let t = &self.lines[k].text[n..];
                if n > indent || (n == indent && is_dash(t)) {
                    return self.block(k, n);
                }
            }
            return (self.node(YamlKind::Null, self.offset(i, a)), i + 1);
        }
        match rest.as_bytes()[0] {
            b'|' | b'>' => self.block_scalar(i, indent, a),
            b'"' | b'\'' => self.quoted(i, a),
            b'[' | b'{' => self.flow(i, a),
            b'&' | b'*' | b'!' => {
                self.fail(i, "anchors, aliases, and tags are not supported");
                (self.node(YamlKind::Null, 0), i + 1)
            }
            _ => self.scalar_lines(i, indent, a, rest),
        }
    }

    /// Reads a plain scalar that starts with `first` on line `i` and may continue on
    /// lines indented more than `indent`; line breaks fold into spaces.
    fn scalar_lines(&mut self, i: usize, indent: usize, at: usize, first: &str) -> (usize, usize) {
        let node = self.node(YamlKind::Scalar, self.offset(i, at));
        let mut text = strip_comment(first).to_owned();
        let mut k = i + 1;
        let mut blanks = 0;
        while k < self.lines.len() {
            let line = self.lines[k].text;
            if trim_space(line).is_empty() {
                blanks += 1;
                k += 1;
                continue;
            }
            let n = indent_of(line);
            if n <= indent && !(indent == 0 && at == 0 && n == 0 && i == 0) {
                break;
            }
            let t = strip_comment(&line[n..]);
            if t.is_empty() || t.starts_with('#') || (split_key(t).is_some() && n <= indent) {
                break;
            }
            if blanks > 0 {
                text.extend(std::iter::repeat_n('\n', blanks));
            } else {
                text.push(' ');
            }
            text.push_str(t);
            blanks = 0;
            k += 1;
        }
        self.y.nodes[node].text = text;
        (node, k - blanks)
    }

    /// Reads "|" and ">" scalars with their chomping and indentation indicators.
    fn block_scalar(&mut self, i: usize, indent: usize, at: usize) -> (usize, usize) {
        const BAD: &str = "expected a comment or a line break after the block scalar indicator";
        let line = self.lines[i].text;
        let b = line.as_bytes();
        let folded = b[at] == b'>';
        let (mut chomp, mut explicit) = (0u8, 0usize);
        let mut k = at + 1;
        while k < b.len() && b[k] != b' ' && b[k] != b'\t' {
            match b[k] {
                c @ (b'+' | b'-') => chomp = c,
                c @ b'1'..=b'9' => explicit = usize::from(c - b'0'),
                _ => {
                    self.fail(i, BAD);
                    return (self.node(YamlKind::Null, 0), i + 1);
                }
            }
            k += 1;
        }
        if !is_blank(&line[k..]) {
            self.fail(i, BAD);
            return (self.node(YamlKind::Null, 0), i + 1);
        }
        let node = self.node(YamlKind::Scalar, self.offset(i, at));
        let mut content = (explicit > 0).then_some(indent + explicit);
        let mut end = i + 1;
        while end < self.lines.len() {
            let t = self.lines[end].text;
            if !trim_space(t).is_empty() {
                let n = indent_of(t);
                let c = *content.get_or_insert(n);
                if n < c || n <= indent {
                    break;
                }
            }
            end += 1;
        }
        let text = self.block_text(i + 1, end, content.unwrap_or(0), folded, chomp);
        let n = &mut self.y.nodes[node];
        n.text = text;
        n.style = YamlStyle::Block;
        (node, end)
    }

    /// Joins the lines of a block scalar: literally for "|"; for ">", a break between
    /// two normal lines becomes a space, and empty and more-indented lines keep their
    /// breaks. Chomping: clip keeps one final break, "-" none, "+" all.
    fn block_text(
        &self,
        from: usize,
        to: usize,
        content: usize,
        folded: bool,
        chomp: u8,
    ) -> String {
        let mut out = String::new();
        let mut last = to;
        while last > from && trim_space(self.lines[last - 1].text).is_empty() {
            last -= 1;
        }
        let (mut empties, mut first, mut prev_more) = (0, true, false);
        for line in &self.lines[from..last] {
            let t = line.text;
            if trim_space(t).is_empty() {
                empties += 1;
                continue;
            }
            let body = &t[content.min(t.len())..];
            let more = body.starts_with([' ', '\t']);
            let mut breaks = empties + 1;
            if first {
                breaks = empties;
            } else if folded && !more && !prev_more {
                if empties == 0 {
                    out.push(' ');
                }
                breaks = empties;
            }
            out.extend(std::iter::repeat_n('\n', breaks));
            out.push_str(body);
            empties = 0;
            first = false;
            prev_more = more;
        }
        if chomp != b'-' && !first {
            out.push('\n');
        }
        if chomp == b'+' {
            out.extend(std::iter::repeat_n('\n', to - last));
        }
        out
    }

    /// Reads a single- or double-quoted scalar, which may span lines.
    fn quoted(&mut self, i: usize, at: usize) -> (usize, usize) {
        let node = self.node(YamlKind::Scalar, self.offset(i, at));
        let q = self.lines[i].text.as_bytes()[at];
        let mut text = String::new();
        let (mut k, mut col) = (i, at + 1);
        while k < self.lines.len() {
            let line = self.lines[k].text;
            if let Some(end) = quoted_run(line, col, q, &mut text) {
                let after = &line[end..];
                if !is_blank(after) && !after_flow(after) {
                    self.fail(k, "unexpected text after a quoted scalar");
                }
                let n = &mut self.y.nodes[node];
                n.text = text;
                n.style = YamlStyle::Quoted;
                return (node, k + 1);
            }
            // A line break inside quotes folds: into a space, or into the empty lines.
            k += 1;
            let mut blanks = 0;
            while k < self.lines.len() && trim_space(self.lines[k].text).is_empty() {
                blanks += 1;
                k += 1;
            }
            if k >= self.lines.len() {
                break;
            }
            text.truncate(text.trim_end_matches([' ', '\t']).len());
            if blanks == 0 {
                text.push(' ');
            }
            text.extend(std::iter::repeat_n('\n', blanks));
            col = indent_of(self.lines[k].text);
        }
        self.fail(i, "while scanning a quoted scalar, found unexpected end of stream");
        (node, self.lines.len())
    }

    /// Reads a flow collection, `[a, b]` or `{k: v}`, which may span lines.
    fn flow(&mut self, i: usize, at: usize) -> (usize, usize) {
        let mut f = Flow { k: i, col: at };
        let node = self.flow_node(&mut f);
        if !self.ok() {
            return (node, self.lines.len());
        }
        let rest = self.lines.get(f.k).map_or("", |l| l.text.get(f.col..).unwrap_or(""));
        if !is_blank(rest) {
            self.fail(f.k, "unexpected text after a flow collection");
        }
        (node, f.k + 1)
    }

    fn flow_peek(&self, f: &Flow) -> u8 {
        match self.lines.get(f.k) {
            None => 0,
            Some(l) => l.text.as_bytes().get(f.col).copied().unwrap_or(b'\n'),
        }
    }

    /// Skips white space, line breaks, and comments.
    fn flow_space(&self, f: &mut Flow) {
        while f.k < self.lines.len() {
            let line = self.lines[f.k].text.as_bytes();
            while f.col < line.len() && (line[f.col] == b' ' || line[f.col] == b'\t') {
                f.col += 1;
            }
            if f.col < line.len() && line[f.col] != b'#' {
                return;
            }
            if f.k + 1 >= self.lines.len() {
                f.col = line.len();
                return;
            }
            f.k += 1;
            f.col = 0;
        }
    }

    fn flow_node(&mut self, f: &mut Flow) -> usize {
        self.flow_space(f);
        let c = self.flow_peek(f);
        if c == 0 || c == b'\n' {
            self.fail(
                f.k,
                "while parsing a flow node, expected the node content, but found the \
                end of the stream",
            );
            return self.node(YamlKind::Null, 0);
        }
        if c != b'[' && c != b'{' {
            return self.flow_scalar(f);
        }
        if self.depth >= MAX_YAML_DEPTH {
            self.fail(f.k, "the YAML nests too deeply");
            return self.node(YamlKind::Null, 0);
        }
        self.depth += 1;
        let node = self.flow_collection(f, c);
        self.depth -= 1;
        node
    }

    fn flow_collection(&mut self, f: &mut Flow, open: u8) -> usize {
        let (kind, close) =
            if open == b'[' { (YamlKind::Sequence, b']') } else { (YamlKind::Mapping, b'}') };
        let node = self.node(kind, self.offset(f.k, f.col));
        f.col += 1;
        while self.ok() {
            self.flow_space(f);
            let d = self.flow_peek(f);
            if d == close {
                f.col += 1;
                return node;
            }
            if d == 0 || d == b'\n' {
                self.flow_node(f);
                return node;
            }
            let (key, item) = if kind == YamlKind::Mapping {
                let k = self.flow_scalar(f);
                let key = std::mem::take(&mut self.y.nodes[k].text);
                self.y.nodes.pop();
                self.flow_space(f);
                if self.flow_peek(f) == b':' {
                    f.col += 1;
                    (key, self.flow_node(f))
                } else {
                    (key, self.node(YamlKind::Null, self.offset(f.k, f.col)))
                }
            } else {
                (String::new(), self.flow_node(f))
            };
            self.y.nodes[item].key = key;
            self.append(node, item);
            self.flow_space(f);
            if self.flow_peek(f) == b',' {
                f.col += 1;
            }
        }
        node
    }

    /// Reads a scalar inside a flow collection.
    fn flow_scalar(&mut self, f: &mut Flow) -> usize {
        let line = self.lines[f.k].text;
        let b = line.as_bytes();
        if b[f.col] == b'"' || b[f.col] == b'\'' {
            let node = self.node(YamlKind::Scalar, self.offset(f.k, f.col));
            let mut text = String::new();
            match quoted_run(line, f.col + 1, b[f.col], &mut text) {
                Some(end) => {
                    f.col = end;
                    let n = &mut self.y.nodes[node];
                    n.text = text;
                    n.style = YamlStyle::Quoted;
                }
                None => {
                    self.fail(f.k, "while scanning a quoted scalar, found unexpected end of line")
                }
            }
            return node;
        }
        let a = f.col;
        while f.col < b.len() {
            let c = b[f.col];
            if b",][{}".contains(&c)
                || (c == b':' && (f.col + 1 >= b.len() || b[f.col + 1] == b' '))
                || (c == b'#' && f.col > a && b[f.col - 1] == b' ')
            {
                break;
            }
            f.col += 1;
        }
        let node = self.node(YamlKind::Scalar, self.offset(f.k, a));
        self.y.nodes[node].text = trim_right(&line[a..f.col]).to_owned();
        node
    }
}

/// A position in a flow collection: the line and column.
#[derive(Debug, Clone, Copy)]
struct Flow {
    k: usize,
    col: usize,
}

fn after_flow(s: &str) -> bool {
    trim_space(s).starts_with([',', ']', '}', ':'])
}

/// Reads quoted text from `col` of `line` up to the closing `q` into `out`; returns
/// where the text after the closing quote starts, or `None` at the end of the line.
fn quoted_run(line: &str, col: usize, q: u8, out: &mut String) -> Option<usize> {
    let b = line.as_bytes();
    let mut j = col;
    let mut run = col;
    while j < b.len() {
        let c = b[j];
        if c == q {
            if q == b'\'' && b.get(j + 1) == Some(&b'\'') {
                out.push_str(&line[run..=j]);
                j += 2;
                run = j;
                continue;
            }
            out.push_str(&line[run..j]);
            return Some(j + 1);
        }
        if q == b'"' && c == b'\\' && j + 1 < b.len() {
            out.push_str(&line[run..j]);
            j = escape(line, j + 1, out);
            run = j;
            continue;
        }
        j += 1;
    }
    out.push_str(&line[run..]);
    None
}

/// Writes the double-quoted escape at `line[j]` and returns where it ends.
fn escape(line: &str, j: usize, out: &mut String) -> usize {
    let b = line.as_bytes();
    let c = b[j];
    let simple = match c {
        b'0' => '\0',
        b'a' => '\x07',
        b'b' => '\x08',
        b't' | b'\t' => '\t',
        b'n' => '\n',
        b'v' => '\x0b',
        b'f' => '\x0c',
        b'r' => '\r',
        b'e' => '\x1b',
        b' ' | b'"' | b'/' | b'\\' => c as char,
        b'x' | b'u' | b'U' => {
            let width = match c {
                b'x' => 2,
                b'u' => 4,
                _ => 8,
            };
            let digits = &b[j + 1..(j + 1 + width).min(b.len())];
            let value = digits.iter().map_while(|&d| hex_value(d)).fold(0u32, |v, d| v * 16 + d);
            out.push(char::from_u32(value).unwrap_or('\u{fffd}'));
            return (j + 1 + width).min(b.len());
        }
        _ => {
            out.push('\\');
            return j;
        }
    };
    out.push(simple);
    j + 1
}

/// Processes a quoted key; plain keys are returned as written.
fn unquote_key(key: &str) -> String {
    let b = key.as_bytes();
    if b.len() < 2 || (b[0] != b'"' && b[0] != b'\'') {
        return key.to_owned();
    }
    let mut out = String::new();
    let inner = &key[..key.len() - 1];
    quoted_run(inner, 1, b[0], &mut out);
    out
}

/// How YAML 1.1 (as PyYAML's safe_load) resolves a plain scalar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarType {
    String,
    Null,
    Bool,
    Int,
    Float,
}

pub(crate) fn scalar_type(n: &YamlNode) -> ScalarType {
    if n.kind == YamlKind::Null {
        return ScalarType::Null;
    }
    if n.kind != YamlKind::Scalar || n.style != YamlStyle::Plain {
        return ScalarType::String;
    }
    let t = n.text.as_str();
    match t {
        "" | "~" | "null" | "Null" | "NULL" => return ScalarType::Null,
        "true" | "True" | "TRUE" | "false" | "False" | "FALSE" | "yes" | "Yes" | "YES" | "no"
        | "No" | "NO" | "on" | "On" | "ON" | "off" | "Off" | "OFF" => return ScalarType::Bool,
        _ => {}
    }
    let s = t.strip_prefix(['-', '+']).unwrap_or(t);
    let digits = s.bytes().filter(u8::is_ascii_digit).count();
    let dots = s.bytes().filter(|&c| c == b'.').count();
    let other = s.bytes().filter(|&c| !c.is_ascii_digit() && c != b'.' && c != b'_').count();
    if digits == 0 || other > 0 {
        return ScalarType::String;
    }
    match dots {
        0 => ScalarType::Int,
        1 => ScalarType::Float,
        _ => ScalarType::String,
    }
}

/// A Bool scalar's value.
pub(crate) fn bool_true(t: &str) -> bool {
    matches!(t, "true" | "True" | "TRUE" | "yes" | "Yes" | "YES" | "on" | "On" | "ON")
}

impl Yaml {
    /// Finds a mapping's entry by key.
    pub(crate) fn child(&self, mapping: usize, key: &str) -> Option<usize> {
        if self.nodes[mapping].kind != YamlKind::Mapping {
            return None;
        }
        self.children(mapping).find(|&c| self.nodes[c].key == key)
    }

    /// The children of a mapping or sequence.
    pub(crate) fn children(&self, id: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(self.nodes[id].first, move |&c| self.nodes[c].next)
    }

    /// Writes a value as MyST shows a front matter field: a scalar as Python's str()
    /// would (True, None, 1.5), a collection as JSON.
    pub(crate) fn write_value(&self, out: &mut String, id: usize) {
        let n = &self.nodes[id];
        if matches!(n.kind, YamlKind::Mapping | YamlKind::Sequence) {
            return self.write_json(out, id);
        }
        match scalar_type(n) {
            ScalarType::Null => out.push_str("None"),
            ScalarType::Bool => out.push_str(if bool_true(&n.text) { "True" } else { "False" }),
            _ => out.push_str(&n.text),
        }
    }

    /// Writes a node as json.dumps does.
    fn write_json(&self, out: &mut String, id: usize) {
        let n = &self.nodes[id];
        match n.kind {
            YamlKind::Null => out.push_str("null"),
            YamlKind::Scalar => match scalar_type(n) {
                ScalarType::Null => out.push_str("null"),
                ScalarType::Bool => {
                    out.push_str(if bool_true(&n.text) { "true" } else { "false" })
                }
                ScalarType::Int | ScalarType::Float => out.push_str(&n.text),
                ScalarType::String => json_string(out, &n.text),
            },
            YamlKind::Mapping | YamlKind::Sequence => {
                let mapping = n.kind == YamlKind::Mapping;
                out.push(if mapping { '{' } else { '[' });
                for (k, c) in self.children(id).enumerate() {
                    if k > 0 {
                        out.push_str(", ");
                    }
                    if mapping {
                        json_string(out, &self.nodes[c].key);
                        out.push_str(": ");
                    }
                    self.write_json(out, c);
                }
                out.push(if mapping { '}' } else { ']' });
            }
        }
    }
}

fn json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::split_source;

    /// Parses YAML and returns the text of the key's value, or the error.
    fn value_of(text: &str, key: &str) -> String {
        let lines = split_source(text, 0);
        match yaml_parse(&lines) {
            Err(e) => e.why.to_owned(),
            Ok((y, root)) if y.nodes[root].kind != YamlKind::Mapping => "(not a mapping)".into(),
            Ok((y, root)) => match y.child(root, key) {
                Some(c) => y.nodes[c].text.clone(),
                None => "(missing)".into(),
            },
        }
    }

    #[test]
    fn yaml_scalars_follow_myst_option_parsing() {
        let cases = [
            ("key3:\n    val3\nkey4: val4.1\n    val4.2 # c\n", "key4", "val4.1 val4.2"),
            ("e: \"\\\"\\e\\x07\"\n", "e", "\"\x1b\x07"),
            ("k: |\n  a\n  b\n\n  c\n", "k", "a\nb\n\nc\n"),
            ("k: |2\n    a\n  b\n", "k", "  a\nb\n"),
            ("k: >\n  a\n  b\n\n  c\n", "k", "a b\nc\n"),
            ("k: >-\n  a\n  b\n", "k", "a b"),
            ("k: |+\n  a\n\n", "k", "a\n\n"),
            ("k1\n", "k1", "(not a mapping)"),
            ("k1\n  k2: v\n", "k1", "(not a mapping)"),
            ("k: \"v\n", "k", "while scanning a quoted scalar, found unexpected end of stream"),
            (
                "k: | v\n",
                "k",
                "expected a comment or a line break after the block scalar indicator",
            ),
            ("'q k': [1, {a: b}]\n", "q k", ""),
        ];
        for (text, key, want) in cases {
            assert_eq!(value_of(text, key), want, "{text:?}");
        }
    }

    #[test]
    fn deep_flow_collections_are_refused() {
        let text = format!("k: {}\n", "[".repeat(10_000));
        assert_eq!(value_of(&text, "k"), "the YAML nests too deeply");
    }
}
