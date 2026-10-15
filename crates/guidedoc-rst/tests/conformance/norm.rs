//! The common structural form of the conformance harness. Both trees are turned into
//! `Norm` nodes whose kinds are Guidedoc's kind names; identifiers, names, source
//! positions, and presentation details are dropped. See
//! docs/manual/internals/rst-conformance.rst.

pub const TEXT: &str = "#text";
/// Markers of Unicode substitution trimming, applied by `tidy` and then removed.
pub const LTRIM: &str = "#ltrim";
pub const RTRIM: &str = "#rtrim";

#[derive(Debug, Clone, Default)]
pub struct Norm {
    pub kind: String,
    /// "key=value"
    pub attrs: Vec<String>,
    pub text: String,
    pub children: Vec<Norm>,
}

/// Warnings and errors a read reported.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub warnings: usize,
    pub errors: usize,
}

impl Norm {
    pub fn new(kind: &str) -> Norm {
        Norm { kind: kind.to_owned(), ..Norm::default() }
    }

    /// Appends a child of `kind` and returns it.
    pub fn child(&mut self, kind: &str) -> &mut Norm {
        self.children.push(Norm::new(kind));
        self.children.last_mut().unwrap()
    }

    pub fn text(&mut self, s: &str) {
        if !s.is_empty() {
            self.children.push(Norm { kind: TEXT.into(), text: s.into(), ..Norm::default() });
        }
    }

    pub fn attr(&mut self, key: &str, value: &str) {
        self.attrs.push(format!("{key}={value}"));
    }

    pub fn has_attr(&self, attr: &str) -> bool {
        self.attrs.iter().any(|a| a == attr)
    }
}

/// Kinds whose text is compared exactly; elsewhere white space runs are one space.
fn verbatim(kind: &str) -> bool {
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
            | LTRIM
            | RTRIM
    )
}

fn trim_space(s: &str) -> &str {
    s.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c'))
}

/// Merges adjacent text, applies substitution trimming, and normalizes white space.
pub fn tidy(n: &mut Norm, inherited: bool) {
    let exact = inherited || verbatim(&n.kind);
    for c in n.children.iter_mut().filter(|c| c.kind != TEXT) {
        tidy(c, exact);
    }
    let mut out: Vec<Norm> = Vec::with_capacity(n.children.len());
    let mut trim_next = false;
    for mut c in std::mem::take(&mut n.children) {
        match c.kind.as_str() {
            LTRIM => {
                if let Some(last) = out.last_mut().filter(|l| l.kind == TEXT) {
                    last.text = last.text.trim_end_matches(is_space).to_owned();
                }
                continue;
            }
            RTRIM => {
                trim_next = true;
                continue;
            }
            TEXT => {
                let text = if trim_next {
                    c.text.trim_start_matches(is_space).to_owned()
                } else {
                    c.text.clone()
                };
                trim_next = false;
                if let Some(last) = out.last_mut().filter(|l| l.kind == TEXT) {
                    last.text.push_str(&text);
                    continue;
                }
                c.text = text;
            }
            _ => trim_next = false,
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

fn is_space(c: char) -> bool {
    c.is_whitespace()
}

/// Turns every run of white space into one space.
pub fn collapse(s: &str) -> String {
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

pub fn norm_string(root: &mut Norm) -> String {
    tidy(root, false);
    let mut out = String::new();
    write_norm(&mut out, root, 0);
    out
}

fn write_norm(out: &mut String, n: &mut Norm, depth: usize) {
    out.push_str(&"  ".repeat(depth));
    if n.kind == TEXT {
        write_quoted(out, &n.text);
        out.push('\n');
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

fn write_quoted(out: &mut String, s: &str) {
    out.push('"');
    for r in s.chars() {
        match r {
            '\n' => out.push_str("\\n"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            r => out.push(r),
        }
    }
    out.push('"');
}

/// Concatenates the text of a subtree.
pub fn plain(n: &Norm, out: &mut String) {
    if n.kind == TEXT {
        out.push_str(&n.text);
        return;
    }
    for c in &n.children {
        plain(c, out);
    }
}

/// Identifies a reference's target: its kind and the start of its text.
pub fn signature(n: &Norm) -> String {
    let mut source = n;
    if n.kind == "section"
        && let Some(h) = n.children.iter().rfind(|c| c.kind == "heading")
    {
        source = h;
    }
    let mut text = String::new();
    plain(source, &mut text);
    let text = collapse(&text);
    let text: String = trim_space(&text).chars().take(40).collect();
    format!("{}:{text:?}", n.kind)
}

pub fn has_class(classes: &str, class: &str) -> bool {
    classes.split_whitespace().any(|c| c == class)
}

/// The classes without the given implicit ones, deduplicated and sorted.
pub fn sorted_classes(classes: &str, implicit: &[&str]) -> String {
    let mut list: Vec<&str> = Vec::new();
    for c in classes.split_whitespace() {
        if !implicit.contains(&c) && !list.contains(&c) {
            list.push(c);
        }
    }
    list.sort_unstable();
    list.join(" ")
}

pub fn set_classes(n: &mut Norm, classes: &str, implicit: &[&str]) {
    let s = sorted_classes(classes, implicit);
    if !s.is_empty() {
        n.attr("class", &s);
    }
}

/// Writes PEP and RFC links in one form, since the reader's default base URLs differ
/// from Docutils' on purpose.
pub fn canonical_uri(uri: &str) -> String {
    if let Some(rest) = uri.strip_prefix("https://peps.python.org/pep-") {
        let rest = rest.trim_end_matches('/');
        let rest = rest.strip_suffix(".html").unwrap_or(rest);
        return format!("pep:{}", rest.trim_start_matches('0'));
    }
    for base in ["https://tools.ietf.org/html/rfc", "https://datatracker.ietf.org/doc/html/rfc"] {
        if let Some(rest) = uri.strip_prefix(base) {
            return format!("rfc:{}", rest.replacen(".html", "", 1));
        }
    }
    uri.to_owned()
}

/// Moves decoration, meta, and bibliographic nodes of a document into Docutils' order:
/// title, subtitle, decoration, meta, docinfo, then the rest in source order.
pub fn hoist(doc: &mut Norm) {
    let (mut head, mut metas, mut info, mut rest) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut decoration: Option<Norm> = None;
    for c in std::mem::take(&mut doc.children) {
        match c.kind.as_str() {
            "title" | "subtitle" => head.push(c),
            "header" | "footer" => {
                merge_decoration(decoration.get_or_insert_with(|| Norm::new("decoration")), c)
            }
            "meta" => metas.push(c),
            "field-list" if c.has_attr("class=docinfo") => info.push(c),
            _ => rest.push(c),
        }
    }
    doc.children = head;
    if let Some(mut d) = decoration {
        // Header before footer, as Docutils places them.
        d.children.sort_by_key(|c| c.kind != "header");
        doc.children.push(d);
    }
    doc.children.extend(metas);
    doc.children.extend(info);
    doc.children.extend(rest);
}

fn merge_decoration(decoration: &mut Norm, part: Norm) {
    if let Some(c) = decoration.children.iter_mut().find(|c| c.kind == part.kind) {
        c.children.extend(part.children);
        return;
    }
    decoration.children.push(part);
}
