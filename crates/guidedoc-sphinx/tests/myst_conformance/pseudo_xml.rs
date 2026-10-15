//! Docutils pseudo-XML (`document.pformat()`) read and brought into the common form.

use super::norm::{Counts, Norm, Px, link_key, percent_decode, xref_key};

const ADMONITIONS: [&str; 11] = [
    "attention",
    "caution",
    "danger",
    "error",
    "hint",
    "important",
    "note",
    "tip",
    "warning",
    "seealso",
    "admonition",
];

/// Reads pformat output: one node per line, children indented four spaces more; other
/// lines are text.
pub fn parse_pseudo_xml(s: &str) -> Px {
    let mut stack: Vec<(Px, isize)> = vec![(Px { name: "#root".into(), ..Px::default() }, -1)];
    for line in s.split('\n') {
        let body = line.trim_start_matches(' ');
        let depth = ((line.len() - body.len()) / 4) as isize;
        if line.trim().is_empty() && stack.len() > 1 {
            add_text(&mut stack.last_mut().expect("root").0, "");
            continue;
        }
        while stack.len() > 1 && stack.last().expect("root").1 >= depth {
            close(&mut stack);
        }
        let (top, top_depth) = stack.last_mut().expect("root");
        let markup = top.name == "literal_block" && inline_markup(body);
        if element_line(body) && (!verbatim(&top.name) || markup) {
            stack.push((element(body), depth));
        } else {
            let cut = (4 * (*top_depth + 1)).max(0) as usize;
            let text = line.get(cut.min(line.len())..).unwrap_or("").to_owned();
            add_text(top, &text);
        }
    }
    while stack.len() > 1 {
        close(&mut stack);
    }
    stack.pop().expect("root").0
}

fn close(stack: &mut Vec<(Px, isize)>) {
    let (done, _) = stack.pop().expect("non-root");
    stack.last_mut().expect("root").0.children.push(done);
}

fn add_text(parent: &mut Px, s: &str) {
    if let Some(last) = parent.children.last_mut().filter(|c| c.name.is_empty()) {
        last.text.push('\n');
        last.text.push_str(s);
        return;
    }
    parent.children.push(Px { text: s.to_owned(), ..Px::default() });
}

fn element(body: &str) -> Px {
    let s = &body[1..body.len() - 1];
    let b = s.as_bytes();
    let mut i = b.iter().position(|&c| c == b' ').unwrap_or(b.len());
    let mut e = Px { name: s[..i].to_owned(), ..Px::default() };
    while i < b.len() {
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        let k = i + b[i..].iter().position(|&c| c == b'=').unwrap_or(b.len() - i);
        if k + 1 >= b.len() || b[k + 1] != b'"' {
            break;
        }
        let v = k + 2;
        let mut z = v;
        while z < b.len() && !(b[z] == b'"' && (z + 1 >= b.len() || b[z + 1] == b' ')) {
            z += 1;
        }
        e.attrs.insert(s[i..k].to_owned(), s[v..z.min(b.len())].to_owned());
        i = z + 1;
    }
    e
}

/// A line that is one node, `<name key="value" ...>`, as pformat writes.
fn element_line(body: &str) -> bool {
    let b = body.as_bytes();
    if b.len() < 3 || b[0] != b'<' || b[b.len() - 1] != b'>' {
        return false;
    }
    let s = &b[1..b.len() - 1];
    let mut i = s
        .iter()
        .take_while(|&&c| c.is_ascii_lowercase() || c == b'_' || c.is_ascii_digit())
        .count();
    if i == 0 {
        return false;
    }
    while i < s.len() {
        if s[i] != b' ' {
            return false;
        }
        i += 1;
        let k = i + s[i..].iter().take_while(|&&c| c != b'=' && c != b' ').count();
        if k == i || k + 1 >= s.len() || s[k] != b'=' || s[k + 1] != b'"' {
            return false;
        }
        let mut z = k + 2;
        while z < s.len() && !(s[z] == b'"' && (z + 1 >= s.len() || s[z + 1] == b' ')) {
            z += 1;
        }
        if z >= s.len() {
            return false;
        }
        i = z + 1;
    }
    true
}

fn verbatim(name: &str) -> bool {
    matches!(name, "raw" | "literal_block" | "math_block" | "comment" | "doctest_block")
}

/// An element line inside a literal block that is markup (parsed literal or Pygments
/// tokens) rather than text that looks like a tag.
fn inline_markup(body: &str) -> bool {
    let names = [
        "inline",
        "emphasis",
        "strong",
        "literal",
        "reference",
        "math",
        "subscript",
        "superscript",
        "title_reference",
    ];
    names.iter().any(|name| {
        body[1..].starts_with(name)
            && (body.len() == name.len() + 2
                || matches!(body.as_bytes()[name.len() + 1], b' ' | b'>'))
    })
}

/// Reads pseudo-XML into the common form, with its warning and error counts.
pub fn pseudo_xml_norm(text: &str) -> (Norm, Counts) {
    let root = parse_pseudo_xml(text);
    let mut counts = Counts::default();
    let mut doc = Norm::new("document");
    for c in &root.children {
        if c.name == "document" {
            for k in &c.children {
                node(k, &mut doc, &mut counts, true);
            }
        } else {
            node(c, &mut doc, &mut counts, true);
        }
    }
    (doc, counts)
}

fn children(x: &Px, out: &mut Norm, counts: &mut Counts, top: bool) {
    for c in &x.children {
        node(c, out, counts, top);
    }
}

/// Converts one node; `top` is set where a title is a section heading.
fn node(x: &Px, out: &mut Norm, counts: &mut Counts, top: bool) {
    match x.name.as_str() {
        "" => out.text(&x.text),
        "system_message" => match x.attr("type") {
            "ERROR" | "SEVERE" => counts.errors += 1,
            "WARNING" => counts.warnings += 1,
            _ => {}
        },
        "pending" | "label" | "colspec" | "tabular_col_spec" | "highlightlang" | "meta"
        | "index" | "acks_placeholder" => {}
        "target" => {
            let generated = ["ids", "refid"]
                .iter()
                .any(|k| x.attr(k).starts_with("equation-") || x.attr(k).starts_with("index-"));
            if !x.attrs.contains_key("refuri") && !generated {
                out.child("target");
            }
        }
        "paragraph" if x.attr("translatable") == "False" => {}
        "centered" => children(x, out.child("paragraph"), counts, false),
        "hlist" => hlist(x, out, counts),
        "topic" if x.attr("classes").contains("contents") => {
            out.child("contents");
        }
        "literal"
            if !x.attr("role").is_empty() || matches!(x.attr("classes"), "kbd" | "regexp") =>
        {
            children(x, out, counts, false);
        }
        "desc_addname" if x.children.is_empty() => {}
        "download_reference" => {
            let n = out.child("xref");
            n.attr("target", x.attr("reftarget"));
            children(x, n, counts, false);
        }
        "tgroup"
        | "inline"
        | "problematic"
        | "desc_sig_name"
        | "desc_sig_punctuation"
        | "desc_sig_operator"
        | "desc_sig_space"
        | "literal_strong"
        | "literal_emphasis"
        | "decoration" => children(x, out, counts, false),
        "raw" => raw(x, out),
        _ => element_node(x, out, counts, top),
    }
}

fn raw(x: &Px, out: &mut Norm) {
    let text = x.text_of();
    if x.attr("format") == "latex" && text.trim() == "\\\\" {
        return;
    }
    if x.attr("format") == "html" && text.trim() == "<br />" {
        out.text("\n");
        return;
    }
    let n = out.child("raw");
    n.attr("format", x.attr("format"));
    n.text(&text);
}

fn element_node(x: &Px, out: &mut Norm, counts: &mut Counts, top: bool) {
    if ADMONITIONS.contains(&x.name.as_str()) {
        let n = out.child("admonition");
        n.attr("kind", &x.name);
        return children(x, n, counts, false);
    }
    if x.name == "compound" && x.attr("classes").contains("toctree-wrapper") {
        return children(x, out, counts, false);
    }
    let name = if x.name == "title" && top { "heading" } else { kind_name(&x.name) };
    let parent_paragraph = out.kind == "paragraph";
    let n = out.child(name);
    match x.name.as_str() {
        "literal_block" => {
            language(x, n);
            if literal(x, n, counts) {
                return;
            }
        }
        "reference" => {
            if let Some(uri) = x.attrs.get("refuri") {
                n.kind = "link".into();
                n.attr("dest", &link_key(&percent_decode(uri)));
            }
        }
        "pending_xref" => {
            let target = xref_key(x.attr("reftarget"));
            n.attr("target", &target);
            if target == "#" {
                return;
            }
        }
        "image" => {
            let uri = percent_decode(x.attr("uri"));
            n.attr("uri", uri.strip_prefix('/').unwrap_or(&uri));
        }
        "math_block" => {
            n.text(x.text_of().trim());
            if parent_paragraph {
                n.kind = "math".into();
            }
            return;
        }
        "footnote_reference" => return,
        "field_body" if x.children.first().is_some_and(|c| c.name.is_empty()) => {
            n.child("paragraph").text(&x.text_of());
            return;
        }
        "bullet_list" => n.attr("style", "bullet"),
        "enumerated_list" => n.attr("style", x.attr("enumtype")),
        _ => {}
    }
    children(x, n, counts, x.name == "section");
}

fn language(x: &Px, n: &mut Norm) {
    if let Some(lang) = x.attrs.get("language") {
        return n.attr("language", lang);
    }
    let classes: Vec<&str> = x.attr("classes").split_whitespace().collect();
    if classes.len() >= 2 && classes[0] == "code" {
        n.attr("language", classes[1]);
    }
}

/// A literal block with element children: Pygments' highlighting flattens to its text;
/// other markup makes it a parsed literal. True when it did the block's children.
fn literal(x: &Px, n: &mut Norm, counts: &mut Counts) -> bool {
    let elements: Vec<&Px> = x.children.iter().filter(|c| !c.name.is_empty()).collect();
    if elements.is_empty() {
        return false;
    }
    if elements.iter().any(|c| c.name != "inline") {
        n.kind = "preformatted".into();
        children(x, n, counts, false);
        return true;
    }
    let mut text = String::new();
    flatten(x, &mut text);
    n.text(&text);
    true
}

fn flatten(x: &Px, out: &mut String) {
    for c in &x.children {
        if c.name.is_empty() {
            out.push_str(&c.text);
        } else if !c.attr("classes").contains("ln") {
            flatten(c, out);
        }
    }
}

/// Joins the columns of a Sphinx hlist into one list, as Guidedog keeps it.
fn hlist(x: &Px, out: &mut Norm, counts: &mut Counts) {
    let h = out.child("hlist");
    let mut has_list = false;
    for col in &x.children {
        for l in col.children.iter().filter(|l| l.name.ends_with("_list")) {
            if l.name != "bullet_list" && l.name != "enumerated_list" {
                continue;
            }
            if !has_list {
                let style = if l.name == "bullet_list" { "bullet" } else { l.attr("enumtype") };
                h.child("list").attr("style", style);
                has_list = true;
            }
            let list = h.children.last_mut().expect("the list");
            children(l, list, counts, false);
        }
    }
}

fn kind_name(name: &str) -> &str {
    match name {
        "literal" => "code",
        "literal_block" => "code-block",
        "math_block" => "math-block",
        "transition" => "thematic-break",
        "bullet_list" | "enumerated_list" => "list",
        "list_item" => "list-item",
        "block_quote" => "block-quote",
        "footnote_reference" => "footnote-reference",
        "thead" => "table-head",
        "tbody" => "table-body",
        "entry" => "cell",
        "definition_list" => "definition-list",
        "definition_list_item" => "definition-item",
        "field_list" => "field-list",
        "field_name" => "field-name",
        "field_body" => "field-body",
        "pending_xref" => "xref",
        "line_block" => "line-block",
        "title_reference" => "title-reference",
        "acronym" => "abbreviation",
        "versionmodified" => "version-modified",
        "productionlist" => "production-list",
        "desc_signature" => "desc-signature",
        "desc_name" => "desc-name",
        "desc_addname" => "desc-addname",
        "desc_annotation" => "desc-annotation",
        "desc_parameterlist" => "desc-parameterlist",
        "desc_parameter" => "desc-parameter",
        "desc_optional" => "desc-optional",
        "desc_returns" => "desc-returns",
        "desc_content" => "desc-content",
        "substitution_reference" => "substitution-reference",
        other => other,
    }
}
