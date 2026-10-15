//! Docutils XML (the reference trees) and its conversion into the common form.

use std::collections::{HashMap, HashSet};

use super::norm::*;

/// One XML element or text node in an arena; `name` is "" for text.
#[derive(Debug, Default)]
pub struct Xml {
    pub name: String,
    pub attrs: HashMap<String, String>,
    pub text: String,
    pub children: Vec<usize>,
    pub parent: Option<usize>,
}

/// A parsed XML document: nodes in an arena, `root` the document element.
pub struct Tree {
    pub nodes: Vec<Xml>,
    pub root: usize,
}

impl Tree {
    fn attr(&self, x: usize, key: &str) -> &str {
        self.nodes[x].attrs.get(key).map_or("", String::as_str)
    }
}

fn push(nodes: &mut Vec<Xml>, parent: usize, x: Xml) -> usize {
    nodes.push(Xml { parent: Some(parent), ..x });
    let id = nodes.len() - 1;
    nodes[parent].children.push(id);
    id
}

pub fn parse_xml(s: &str) -> Tree {
    let mut nodes = vec![Xml { name: "#root".into(), ..Xml::default() }];
    let mut stack = vec![0usize];
    let mut i = 0;
    while i < s.len() {
        let top = *stack.last().unwrap();
        let rest = &s[i..];
        if !rest.starts_with('<') {
            let j = rest.find('<').unwrap_or(rest.len());
            push(&mut nodes, top, Xml { text: xml_decode(&rest[..j]), ..Xml::default() });
            i += j;
            continue;
        }
        if rest.starts_with("<!--") {
            i += rest.find("-->").map_or(rest.len(), |j| j + 3);
            continue;
        }
        if rest.starts_with("</") {
            if stack.len() > 1 {
                stack.pop();
            }
            i += rest.find('>').map_or(rest.len(), |j| j + 1);
            continue;
        }
        let (x, end, closed) = parse_tag(s, i);
        let id = push(&mut nodes, top, x);
        if !closed {
            stack.push(id);
        }
        i = end;
    }
    let root =
        nodes[0].children.iter().copied().find(|&c| nodes[c].name == "document").unwrap_or(0);
    Tree { nodes, root }
}

fn parse_tag(s: &str, start: usize) -> (Xml, usize, bool) {
    let b = s.as_bytes();
    let mut x = Xml::default();
    let mut i = start + 1;
    let mut j = i;
    while j < b.len() && !matches!(b[j], b' ' | b'>' | b'/') {
        j += 1;
    }
    x.name = s[i..j].to_owned();
    i = j;
    while i < b.len() {
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        match b[i] {
            b'>' => return (x, i + 1, false),
            b'/' => return (x, i + 2, true),
            _ => {}
        }
        let mut k = i;
        while k < b.len() && b[k] != b'=' {
            k += 1;
        }
        let key = s[i..k].to_owned();
        let quote = b[k + 1];
        let v = k + 2;
        let mut z = v;
        while z < b.len() && b[z] != quote {
            z += 1;
        }
        x.attrs.insert(key, xml_decode(&s[v..z]));
        i = z + 1;
    }
    (x, s.len(), true)
}

fn xml_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let j = rest[i..].find(';').map_or(rest.len(), |j| i + j);
        let entity = &rest[i + 1..j];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e if e.starts_with("#x") => {
                u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32)
            }
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        out.extend(decoded);
        rest = &rest[(j + 1).min(rest.len())..];
    }
    out.push_str(rest);
    out
}

/// The conversion's state.
pub struct State<'t> {
    t: &'t Tree,
    ids: HashMap<String, usize>,
    /// Ids of footnotes made by target-notes.
    notes: HashSet<String>,
    /// Targets right after a reference: named embedded URIs and aliases.
    embedded: HashSet<usize>,
    pub counts: Counts,
    sectnum: bool,
    signing: bool,
}

pub fn docutils_norm(text: &str) -> (Norm, Counts) {
    let t = parse_xml(text);
    let mut st = State {
        t: &t,
        ids: HashMap::new(),
        notes: HashSet::new(),
        embedded: HashSet::new(),
        counts: Counts::default(),
        sectnum: false,
        signing: false,
    };
    index_ids(&mut st, t.root);
    let mut doc = Norm::new("document");
    for &c in &t.nodes[t.root].children {
        from_docutils(&mut st, c, &mut doc, "document");
    }
    if st.sectnum {
        doc.attr("sectnum", "1");
    }
    hoist(&mut doc);
    (doc, st.counts)
}

fn index_ids(st: &mut State<'_>, x: usize) {
    let t = st.t;
    for id in t.attr(x, "ids").split_whitespace() {
        st.ids.insert(id.to_owned(), x);
    }
    if t.nodes[x].name == "footnote" && t.attr(x, "names").starts_with("TARGET_NOTE:") {
        for id in t.attr(x, "ids").split_whitespace() {
            st.notes.insert(id.to_owned());
        }
    }
    let children = &t.nodes[x].children;
    for (k, &c) in children.iter().enumerate() {
        let n = &t.nodes[c];
        if k > 0
            && n.name == "target"
            && n.children.is_empty()
            && t.nodes[children[k - 1]].name == "reference"
        {
            st.embedded.insert(c);
        }
        index_ids(st, c);
    }
}

fn xml_plain(t: &Tree, x: usize, out: &mut String) {
    if t.nodes[x].name.is_empty() {
        out.push_str(&t.nodes[x].text);
    }
    for &c in &t.nodes[x].children {
        xml_plain(t, c, out);
    }
}

fn xml_children(st: &mut State<'_>, x: usize, out: &mut Norm) {
    let t = st.t;
    for &c in &t.nodes[x].children {
        from_docutils(st, c, out, &t.nodes[x].name);
    }
}

/// Elements whose content is inline: raw and images inside them are inline.
fn text_element(name: &str) -> bool {
    matches!(
        name,
        "paragraph"
            | "title"
            | "subtitle"
            | "line"
            | "term"
            | "classifier"
            | "field_name"
            | "attribution"
            | "caption"
            | "rubric"
            | "substitution_definition"
            | "emphasis"
            | "strong"
            | "reference"
            | "literal"
            | "inline"
            | "subscript"
            | "superscript"
            | "title_reference"
            | "abbreviation"
            | "acronym"
            | "target"
            | "literal_block"
            | "author"
            | "organization"
            | "contact"
            | "address"
            | "version"
            | "revision"
            | "status"
            | "date"
            | "copyright"
            | "problematic"
    )
}

const ADMONITIONS: &[&str] = &[
    "attention",
    "caution",
    "danger",
    "error",
    "hint",
    "important",
    "note",
    "tip",
    "warning",
    "admonition",
];

const BIBLIOGRAPHIC: &[&str] = &[
    "author",
    "authors",
    "organization",
    "address",
    "contact",
    "version",
    "revision",
    "status",
    "date",
    "copyright",
];

pub fn from_docutils(st: &mut State<'_>, x: usize, out: &mut Norm, parent: &str) {
    let t = st.t;
    let n = &t.nodes[x];
    if n.name.is_empty() {
        out.text(&n.text);
        return;
    }
    let classes = t.attr(x, "classes").to_owned();
    if ADMONITIONS.contains(&n.name.as_str()) {
        let a = out.child("admonition");
        a.attr("kind", &n.name);
        set_classes(a, &classes, &[]);
        xml_children(st, x, a);
        return;
    }
    if structure(st, x, out, parent, &classes)
        || inline(st, x, out, parent, &classes)
        || special(st, x, out)
    {
        return;
    }
    let unknown = out.child(&format!("?{}", n.name));
    xml_children(st, x, unknown);
}

/// Elements that differ from Guidedoc kinds only by name.
fn renamed(name: &str) -> Option<&str> {
    Some(match name {
        "section" | "paragraph" | "term" | "classifier" | "definition" | "caption" | "legend"
        | "attribution" | "rubric" | "compound" | "container" | "sidebar" | "subtitle"
        | "figure" | "topic" | "citation" | "field" | "line" | "row" => name,
        "list_item" => "list-item",
        "definition_list" => "definition-list",
        "definition_list_item" => "definition-item",
        "field_list" => "field-list",
        "field_name" => "field-name",
        "field_body" => "field-body",
        "option_list" => "option-list",
        "option_list_item" => "option-item",
        "option_group" => "option-group",
        "description" => "option-description",
        "line_block" => "line-block",
        "block_quote" => "block-quote",
        "doctest_block" => "doctest-block",
        "transition" => "thematic-break",
        "thead" => "table-head",
        "tbody" => "table-body",
        "substitution_definition" => "substitution-definition",
        "math_block" => "math-block",
        _ => return None,
    })
}

fn structure(st: &mut State<'_>, x: usize, out: &mut Norm, parent: &str, classes: &str) -> bool {
    let t = st.t;
    match t.nodes[x].name.as_str() {
        "title" => {
            xml_children(st, x, out.child(if parent == "section" { "heading" } else { "title" }))
        }
        "subtitle" => {
            // Section subtitles are paragraphs of class section-subtitle.
            let n = out.child(if parent == "section" { "paragraph" } else { "subtitle" });
            if parent == "section" {
                n.attr("class", "section-subtitle");
            }
            xml_children(st, x, n);
        }
        "section" if has_class(classes, "system-messages") => {
            xml_children(st, x, &mut Norm::new("dropped"))
        }
        "topic" if has_class(classes, "contents") => contents(st, x, out, classes),
        "bullet_list" | "enumerated_list" => list(st, x, out, classes),
        "docinfo" => {
            let n = out.child("field-list");
            n.attr("class", "docinfo");
            for &c in &t.nodes[x].children {
                bibliographic(st, c, n);
            }
        }
        "option" => option(st, x, out),
        "table" => table(st, x, out, classes),
        "entry" => {
            let n = out.child("cell");
            let rows = t.attr(x, "morerows").parse::<usize>().unwrap_or(0) + 1;
            let cols = t.attr(x, "morecols").parse::<usize>().unwrap_or(0) + 1;
            if rows > 1 || cols > 1 {
                n.attr("span", &format!("{rows}x{cols}"));
            }
            set_classes(n, classes, &[]);
            xml_children(st, x, n);
        }
        "footnote" | "citation" => note(st, x, out, classes),
        "literal_block" => literal_block(st, x, out, classes),
        "image" => image(t, x, out, classes),
        "figure" => {
            let n = out.child("figure");
            for key in ["width", "align"] {
                if let Some(v) = t.nodes[x].attrs.get(key) {
                    n.attr(key, v);
                }
            }
            set_classes(n, classes, &[]);
            xml_children(st, x, n);
        }
        name => {
            let Some(kind) = renamed(name) else {
                return false;
            };
            let n = out.child(kind);
            set_classes(n, classes, &[]);
            xml_children(st, x, n);
        }
    }
    true
}

fn contents(st: &mut State<'_>, x: usize, out: &mut Norm, classes: &str) {
    let t = st.t;
    let n = out.child("contents");
    for &c in t.nodes[x].children.iter().filter(|&&c| t.nodes[c].name == "title") {
        let mut s = String::new();
        xml_plain(t, c, &mut s);
        n.attr("title", &collapse(&s));
    }
    if has_class(classes, "local") {
        n.attr("local", "1");
    }
    set_classes(n, classes, &["contents", "local"]);
}

fn table(st: &mut State<'_>, x: usize, out: &mut Norm, classes: &str) {
    let t = st.t;
    let n = out.child("table");
    set_classes(n, classes, &[]);
    for &c in &t.nodes[x].children {
        if t.nodes[c].name == "tgroup" {
            n.attr("cols", t.attr(c, "cols"));
            for &g in t.nodes[c].children.iter().filter(|&&g| t.nodes[g].name != "colspec") {
                from_docutils(st, g, n, "tgroup");
            }
        } else {
            from_docutils(st, c, n, "table");
        }
    }
}

fn is_line_number(t: &Tree, c: usize) -> bool {
    t.nodes[c].name == "inline" && has_class(t.attr(c, "classes"), "ln")
}

fn literal_block(st: &mut State<'_>, x: usize, out: &mut Norm, classes: &str) {
    let t = st.t;
    let children = &t.nodes[x].children;
    let numbered = children.iter().any(|&c| is_line_number(t, c));
    let markup = children.iter().any(|&c| !is_line_number(t, c) && !t.nodes[c].name.is_empty());
    let n = out.child(if markup { "preformatted" } else { "code-block" });
    set_classes(n, classes, &["code"]);
    if numbered {
        n.attr("numbered", "1");
    }
    for &c in children.iter().filter(|&&c| !is_line_number(t, c)) {
        from_docutils(st, c, n, "literal_block");
    }
}

fn image(t: &Tree, x: usize, out: &mut Norm, classes: &str) {
    let n = out.child("image");
    n.attr("uri", t.attr(x, "uri"));
    for key in ["alt", "width", "height", "scale", "align"] {
        if let Some(v) = t.nodes[x].attrs.get(key) {
            n.attr(key, v);
        }
    }
    set_classes(n, classes, &[]);
}

/// Converts a docinfo child: known fields become fields marked with their name,
/// holding paragraphs (one per author) or preformatted text (address).
fn bibliographic(st: &mut State<'_>, x: usize, out: &mut Norm) {
    let t = st.t;
    let name = t.nodes[x].name.as_str();
    if !BIBLIOGRAPHIC.contains(&name) {
        from_docutils(st, x, out, "docinfo");
        return;
    }
    let f = out.child("field");
    f.attr("docinfo", name);
    set_classes(f, t.attr(x, "classes"), &[]);
    let body = f.child("field-body");
    match name {
        "authors" => {
            for &a in t.nodes[x].children.iter().filter(|&&a| t.nodes[a].name == "author") {
                xml_children(st, a, body.child("paragraph"));
            }
        }
        "address" => {
            let markup = t.nodes[x].children.iter().any(|&c| !t.nodes[c].name.is_empty());
            xml_children(st, x, body.child(if markup { "preformatted" } else { "code-block" }));
        }
        _ => xml_children(st, x, body.child("paragraph")),
    }
}

fn list(st: &mut State<'_>, x: usize, out: &mut Norm, classes: &str) {
    let t = st.t;
    let n = out.child("list");
    if t.nodes[x].name == "bullet_list" {
        n.attr("style", "bullet");
        n.attr("bullet", t.attr(x, "bullet"));
    } else {
        n.attr("style", t.attr(x, "enumtype"));
        n.attr("delimiter", &format!("{}{}", t.attr(x, "prefix"), t.attr(x, "suffix")));
        if let Some(s) = t.nodes[x].attrs.get("start").filter(|s| *s != "1") {
            n.attr("start", s);
        }
    }
    set_classes(n, classes, &[]);
    xml_children(st, x, n);
}

fn option(st: &mut State<'_>, x: usize, out: &mut Norm) {
    let t = st.t;
    let n = out.child("option");
    for &c in &t.nodes[x].children {
        match t.nodes[c].name.as_str() {
            "option_string" => {
                let mut s = String::new();
                xml_plain(t, c, &mut s);
                n.text(&s);
            }
            "option_argument" => {
                let a = n.child("option-argument");
                a.attr("delimiter", &format!("'{}'", t.attr(c, "delimiter")));
                xml_children(st, c, a);
            }
            _ => {}
        }
    }
}

/// Converts a footnote or citation; the footnotes target-notes makes become one
/// target-notes marker, as Guidedoc keeps the directive.
fn note(st: &mut State<'_>, x: usize, out: &mut Norm, classes: &str) {
    let t = st.t;
    let name = t.nodes[x].name.as_str();
    if name == "footnote" && t.attr(x, "names").starts_with("TARGET_NOTE:") {
        if out.children.last().is_none_or(|l| l.kind != "target-notes") {
            out.child("target-notes");
        }
        return;
    }
    let n = out.child(name);
    set_classes(n, classes, &[]);
    for &c in &t.nodes[x].children {
        if t.nodes[c].name == "label" {
            let mut s = String::new();
            xml_plain(t, c, &mut s);
            n.attr("label", &s);
        } else {
            from_docutils(st, c, n, name);
        }
    }
}

/// Inline elements that differ from Guidedoc kinds only by name.
fn inline_kind_of(name: &str) -> Option<&str> {
    Some(match name {
        "emphasis" | "strong" | "subscript" | "superscript" | "math" => name,
        "title_reference" => "title-reference",
        "literal" => "code",
        "abbreviation" | "acronym" => "abbreviation",
        "inline" => "role",
        _ => return None,
    })
}

fn inline(st: &mut State<'_>, x: usize, out: &mut Norm, parent: &str, classes: &str) -> bool {
    let t = st.t;
    let name = t.nodes[x].name.as_str();
    if let Some(kind) = inline_kind_of(name) {
        let n = out.child(kind);
        set_classes(n, classes, if kind == "code" { &["code"] } else { &[] });
        xml_children(st, x, n);
        return true;
    }
    match name {
        "reference" => {
            let attrs = &t.nodes[x].attrs;
            let to = attrs.get("refid").filter(|_| !st.signing).map(|id| signature_of(st, id));
            let n = out.child("reference");
            set_classes(n, classes, &[]);
            if let Some(uri) = attrs.get("refuri") {
                n.attr("uri", &canonical_uri(uri));
            } else if let Some(to) = to {
                n.attr("to", &to);
            } else if attrs.contains_key("refname") {
                n.attr("unresolved", "1");
            }
            xml_children(st, x, n);
        }
        "footnote_reference" => {
            if st.notes.contains(t.attr(x, "refid")) {
                // target-notes adds a space and a reference after each external reference.
                if let Some(last) = out.children.last_mut().filter(|l| l.kind == TEXT)
                    && let Some(s) = last.text.strip_suffix(' ')
                {
                    last.text = s.to_owned();
                }
                return true;
            }
            xml_children(st, x, out.child("footnote-reference"));
        }
        "citation_reference" => xml_children(st, x, out.child("citation-reference")),
        "target" => target(st, x, out, classes),
        "problematic" => xml_children(st, x, out),
        "generated" if has_class(classes, "sectnum") => st.sectnum = true,
        "generated" => xml_children(st, x, out),
        "raw" => {
            let n = out.child(if text_element(parent) { "raw-inline" } else { "raw-block" });
            n.attr("format", t.attr(x, "format"));
            set_classes(n, classes, &[]);
            xml_children(st, x, n);
        }
        _ => return false,
    }
    true
}

fn target(st: &mut State<'_>, x: usize, out: &mut Norm, classes: &str) {
    let t = st.t;
    if st.embedded.contains(&x) {
        return;
    }
    if !t.nodes[x].children.is_empty() {
        let n = out.child("inline-target");
        set_classes(n, classes, &[]);
        xml_children(st, x, n);
        return;
    }
    let uri = target_uri(st, x);
    let n = out.child("target");
    if let Some(uri) = uri {
        n.attr("uri", &canonical_uri(&uri));
    }
    if t.nodes[x].attrs.contains_key("anonymous") {
        n.attr("anonymous", "1");
    }
    set_classes(n, classes, &[]);
}

fn special(st: &mut State<'_>, x: usize, out: &mut Norm) -> bool {
    let t = st.t;
    match t.nodes[x].name.as_str() {
        "system_message" => {
            if !st.signing {
                match t.attr(x, "level") {
                    "2" => st.counts.warnings += 1,
                    "3" | "4" => st.counts.errors += 1,
                    _ => {}
                }
            }
        }
        "pending" | "label" | "colspec" => {}
        "comment" => xml_children(st, x, out.child("comment")),
        "decoration" => {
            for &c in &t.nodes[x].children {
                let n = out.child(&t.nodes[c].name);
                xml_children(st, c, n);
            }
        }
        "meta" => {
            let n = out.child("meta");
            for (k, v) in &t.nodes[x].attrs {
                n.attr(k, v);
            }
        }
        _ => return false,
    }
    true
}

/// Identifies the element with the given id.
fn signature_of(st: &mut State<'_>, id: &str) -> String {
    let t = st.t;
    let Some(&mut_target) = st.ids.get(id) else {
        return "missing".into();
    };
    let mut target = mut_target;
    if t.nodes[target].name == "document"
        && let Some(&title) =
            t.nodes[target].children.iter().rfind(|&&c| t.nodes[c].name == "title")
    {
        target = title;
    }
    let parent = t.nodes[target].parent.map_or("section", |p| t.nodes[p].name.as_str());
    let mut tmp = Norm::new("tmp");
    let was = std::mem::replace(&mut st.signing, true);
    from_docutils(st, target, &mut tmp, parent);
    st.signing = was;
    match tmp.children.first_mut() {
        None => "none".into(),
        Some(first) => {
            tidy(first, false);
            signature(first)
        }
    }
}

/// A target's URI, following a chained target (refid) to the target that received its
/// names.
fn target_uri(st: &State<'_>, x: usize) -> Option<String> {
    let t = st.t;
    let mut cur = x;
    for _ in 0..16 {
        if let Some(uri) = t.nodes[cur].attrs.get("refuri") {
            return Some(uri.clone());
        }
        let next = *st.ids.get(t.nodes[cur].attrs.get("refid")?)?;
        if next == cur || t.nodes[next].name != "target" {
            return None;
        }
        cur = next;
    }
    None
}
