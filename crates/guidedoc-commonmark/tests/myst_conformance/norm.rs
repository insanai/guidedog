//! The common structural form of the MyST conformance harness: Docutils pseudo-XML and
//! Guidedoc trees become `Norm` nodes named by Guidedoc's kinds, keeping structure,
//! text, and the attributes that carry meaning (code language, admonition kind, link
//! destination, cross-reference target, raw format, list style). Identifiers, names,
//! classes, spans, and Sphinx's bookkeeping attributes are dropped. The rules are listed
//! in docs/manual/internals/commonmark-conformance.rst.

use std::collections::HashMap;

pub const TEXT: &str = "#text";

/// One node of the common form.
#[derive(Debug, Default)]
pub struct Norm {
    pub kind: String,
    /// "key=value"
    pub attrs: Vec<String>,
    pub text: String,
    pub children: Vec<Norm>,
}

/// Warnings and errors a case reports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub warnings: usize,
    pub errors: usize,
}

impl Norm {
    pub fn new(kind: &str) -> Norm {
        Norm { kind: kind.to_owned(), ..Norm::default() }
    }

    pub fn child(&mut self, kind: &str) -> &mut Norm {
        self.children.push(Norm::new(kind));
        self.children.last_mut().expect("just pushed")
    }

    pub fn text(&mut self, s: &str) {
        if !s.is_empty() {
            self.children.push(Norm { kind: TEXT.into(), text: s.into(), ..Norm::default() });
        }
    }

    pub fn attr(&mut self, key: &str, value: &str) {
        self.attrs.push(format!("{key}={value}"));
    }
}

/// Kinds whose text is compared exactly; elsewhere white space runs are one space.
pub fn verbatim(kind: &str) -> bool {
    matches!(
        kind,
        "code-block"
            | "doctest-block"
            | "math-block"
            | "raw-block"
            | "raw-inline"
            | "preformatted"
            | "math"
    )
}

fn inline_kind(kind: &str) -> bool {
    matches!(
        kind,
        TEXT | "emphasis"
            | "strong"
            | "code"
            | "reference"
            | "inline-target"
            | "footnote-reference"
            | "citation-reference"
            | "math"
            | "raw-inline"
            | "subscript"
            | "superscript"
            | "title-reference"
            | "abbreviation"
            | "role"
            | "image"
    )
}

/// Merges adjacent text and normalizes white space.
pub fn tidy(n: &mut Norm, inherited: bool) {
    let exact = inherited || verbatim(&n.kind);
    for c in n.children.iter_mut().filter(|c| c.kind != TEXT) {
        tidy(c, exact);
    }
    let mut out: Vec<Norm> = Vec::new();
    for c in std::mem::take(&mut n.children) {
        if c.kind == TEXT
            && let Some(last) = out.last_mut().filter(|l| l.kind == TEXT)
        {
            last.text.push_str(&c.text);
            continue;
        }
        out.push(c);
    }
    n.children = out;
    if !exact {
        for c in n.children.iter_mut().filter(|c| c.kind == TEXT) {
            c.text = collapse(&c.text);
        }
        if !inline_kind(&n.kind) {
            if let Some(first) = n.children.first_mut().filter(|c| c.kind == TEXT) {
                first.text = first.text.trim_start_matches(' ').to_owned();
            }
            if let Some(last) = n.children.last_mut().filter(|c| c.kind == TEXT) {
                last.text = last.text.trim_end_matches(' ').to_owned();
            }
        }
    }
    n.children.retain(|c| c.kind != TEXT || !c.text.is_empty());
}

/// Turns every run of white space into one space.
fn collapse(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for r in s.chars() {
        if matches!(r, ' ' | '\n' | '\t' | '\r' | '\x0c' | '\x0b') {
            space = true;
            continue;
        }
        if space {
            out.push(' ');
        }
        space = false;
        out.push(r);
    }
    if space {
        out.push(' ');
    }
    out
}

/// Tidies a tree and writes it one node per line.
pub fn norm_string(n: &mut Norm) -> String {
    tidy(n, false);
    trim_texts(n, false);
    hoist_transitions(n);
    collapse_math(n);
    tidy(n, false);
    let mut out = String::new();
    write_norm(&mut out, n, 0);
    out
}

fn write_norm(out: &mut String, n: &mut Norm, depth: usize) {
    out.push_str(&"  ".repeat(depth));
    if n.kind == TEXT {
        out.push_str(&format!("{:?}\n", n.text));
        return;
    }
    out.push_str(&n.kind);
    n.attrs.sort();
    for a in &n.attrs {
        out.push(' ');
        out.push_str(a);
    }
    out.push('\n');
    for c in &mut n.children {
        write_norm(out, c, depth + 1);
    }
}

/// Removes white space at both ends of every text outside verbatim nodes and drops
/// texts left empty: pformat output loses trailing spaces, so they cannot count.
fn trim_texts(n: &mut Norm, exact: bool) {
    for c in &mut n.children {
        if c.kind == TEXT && !exact {
            c.text = c.text.trim().to_owned();
        } else if c.kind != TEXT {
            trim_texts(c, exact || verbatim(&c.kind));
        }
    }
    n.children.retain(|c| c.kind != TEXT || exact || !c.text.is_empty());
}

/// Moves a transition that ends a section after it, as Docutils' transform does.
fn hoist_transitions(n: &mut Norm) {
    let mut out = Vec::new();
    for mut c in std::mem::take(&mut n.children) {
        hoist_transitions(&mut c);
        let hoisted = (c.kind == "section"
            && c.children.last().is_some_and(|l| l.kind == "thematic-break"))
        .then(|| c.children.pop())
        .flatten();
        out.push(c);
        out.extend(hoisted);
    }
    n.children = out;
}

/// Makes white space runs in math one space: LaTeX ignores them.
fn collapse_math(n: &mut Norm) {
    let math = n.kind == "math-block";
    for c in &mut n.children {
        if math && c.kind == TEXT {
            c.text = c.text.split_whitespace().collect::<Vec<_>>().join(" ");
        }
        collapse_math(c);
    }
}

/// Removes all white space from code blocks: Sphinx's trees hold Pygments tokens, whose
/// white space pformat does not keep.
pub fn strip_code_space(n: &mut Norm) {
    let code = n.kind == "code-block";
    for c in &mut n.children {
        if code && c.kind == TEXT {
            c.text.retain(|r| !matches!(r, ' ' | '\n' | '\t'));
        }
        strip_code_space(c);
    }
}

/// Writes the first differing line of two normalized trees, with context.
pub fn first_difference(want: &str, got: &str) -> String {
    let w: Vec<&str> = want.lines().collect();
    let g: Vec<&str> = got.lines().collect();
    let i = w.iter().zip(&g).take_while(|(a, b)| a == b).count();
    let mut out = format!("first difference at normalized line {}\n  common context:\n", i + 1);
    for line in &w[i.saturating_sub(6)..i] {
        out.push_str(&format!("    {line}\n"));
    }
    out.push_str("  myst-parser:\n");
    for line in w.iter().skip(i).take(8) {
        out.push_str(&format!("  - {line}\n"));
    }
    out.push_str("  Guidedoc:\n");
    for line in g.iter().skip(i).take(8) {
        out.push_str(&format!("  + {line}\n"));
    }
    out
}

/// A cross-reference target as both trees record it: a document path without "./"
/// and suffix, an anchor-only target as "#".
pub fn xref_key(target: &str) -> String {
    let mut t = target;
    while let Some(rest) = t.strip_prefix("./") {
        t = rest;
    }
    if let Some(i) = t.find('#') {
        if i == 0 {
            return "#".into();
        }
        t = &t[..i];
    }
    for suffix in [".md", ".rst"] {
        t = t.strip_suffix(suffix).unwrap_or(t);
    }
    t.to_owned()
}

/// PEP and RFC links reduced to "pep:N" and "rfc:N": the base URLs differ by design.
pub fn link_key(dest: &str) -> String {
    let groups = [
        ("pep:", ["http://www.python.org/dev/peps/pep-", "https://peps.python.org/pep-"]),
        ("rfc:", ["http://tools.ietf.org/html/rfc", "https://datatracker.ietf.org/doc/html/rfc"]),
    ];
    for (key, prefixes) in groups {
        for prefix in prefixes {
            if let Some(rest) = dest.strip_prefix(prefix) {
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                let n = digits.trim_start_matches('0');
                return format!("{key}{}", if n.is_empty() { "0" } else { n });
            }
        }
    }
    dest.to_owned()
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// One node of Docutils pseudo-XML.
#[derive(Debug, Default)]
pub struct Px {
    /// "" for text.
    pub name: String,
    pub attrs: HashMap<String, String>,
    pub text: String,
    pub children: Vec<Px>,
}

impl Px {
    pub fn attr(&self, key: &str) -> &str {
        self.attrs.get(key).map_or("", String::as_str)
    }

    pub fn text_of(&self) -> String {
        self.children.iter().filter(|c| c.name.is_empty()).map(|c| c.text.as_str()).collect()
    }
}
