//! The standard domain's glossary and productionlist directives.

use guidedoc::{
    Arg, AttributeKey, Fail, INVALID_NODE, LinkInfo, LinkKind, NodeKind, SourceSpan, arg_span,
};
use guidedoc_rst::{Directive, Line, Parser, indent_of};

use crate::extension::Sphinx;
use crate::ids::sphinx_id;
use crate::messages::{GLOSSARY_CONTENT, GLOSSARY_DUPLICATE};
use crate::text::{is_identifier_byte, trim_space};

/// One definition: its term lines and definition lines.
#[derive(Debug, Clone, Default)]
struct Entry {
    /// Indexes of the term lines.
    terms: Vec<usize>,
    /// Content lines [def_first, def_end).
    def_first: usize,
    def_end: usize,
    /// The definition's indentation.
    indent: usize,
}

/// glossary: Glossary > Definition_List{class glossary} > Definition_Item > Term+,
/// Definition. Several unindented lines are several terms of one definition; each Term
/// has Id "term-<term>" and Name = the term, and ends with an inline Index entry
/// "!single: <term>" (a main entry, keyed by the classifier after " : ").
pub(crate) fn glossary_directive(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
) -> Result<(), Fail> {
    let lines = &d.content;
    let mut entries = collect_glossary(p, lines);
    let sorted = d.has_option("sorted");
    if sorted {
        // A stable sort by the first term, case-insensitively.
        entries.sort_by(|a, b| fold_cmp(lines[a.terms[0]].text, lines[b.terms[0]].text));
    }
    let id = p.open(NodeKind::Glossary, d.span)?;
    if sorted {
        p.custom(id, "sorted", "1")?;
    }
    let list = p.ctx.b.begin_node(NodeKind::DefinitionList, d.span)?;
    p.ctx.b.add_attribute_string(list, AttributeKey::Class, "glossary")?;
    for e in &entries {
        let span = p.line_span(&lines[e.terms[0]]);
        let item = p.ctx.b.begin_node(NodeKind::DefinitionItem, span)?;
        for &t in &e.terms {
            glossary_term(sx, p, lines[t])?;
        }
        let body = &lines[e.def_first..e.def_end];
        let def = p.ctx.b.begin_node(NodeKind::Definition, p.lines_span(body))?;
        if !body.is_empty() {
            let cut: Vec<Line<'_>> =
                body.iter().map(|l| l.cut(e.indent.min(indent_of(l.text)))).collect();
            p.parse_body(&cut, false, None)?;
        }
        p.ctx.b.end_node(def)?;
        p.ctx.b.end_node(item)?;
    }
    p.ctx.b.end_node(list)?;
    p.close(id)
}

/// Compares ASCII case-insensitively.
fn fold_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let x = a.bytes().map(|c| c.to_ascii_lowercase());
    x.cmp(b.bytes().map(|c| c.to_ascii_lowercase()))
}

/// Follows Sphinx's reading of a glossary's lines: terms flush left, definitions
/// indented, ".. " comments skipped.
fn collect_glossary(p: &mut Parser<'_, '_>, lines: &[Line<'_>]) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    let (mut in_definition, mut in_comment, mut was_empty) = (true, false, true);
    for (i, l) in lines.iter().enumerate() {
        if l.is_blank() {
            was_empty = true;
            continue;
        }
        if !l.is_indented() {
            if l.text.starts_with(".. ") {
                in_comment = true;
                continue;
            }
            in_comment = false;
            if in_definition {
                if !was_empty {
                    glossary_warning(p, l);
                }
                entries.push(Entry { def_first: i + 1, def_end: i + 1, ..Entry::default() });
                in_definition = false;
            } else if was_empty || entries.is_empty() {
                glossary_warning(p, l);
            }
            if let Some(e) = entries.last_mut() {
                e.terms.push(i);
                (e.def_first, e.def_end) = (i + 1, i + 1);
            }
        } else if !in_comment {
            match entries.last_mut() {
                None => glossary_warning(p, l),
                Some(e) => {
                    if !in_definition {
                        in_definition = true;
                        e.indent = indent_of(l.text);
                        e.def_first = i;
                    }
                    e.def_end = i + 1;
                }
            }
        }
        was_empty = false;
    }
    entries
}

fn glossary_warning(p: &mut Parser<'_, '_>, l: &Line<'_>) {
    let span = p.line_span(l);
    p.report(GLOSSARY_CONTENT, span, arg_span(span), Arg::None);
}

/// Splits "term : classifier" at the first " : " (spaces around).
pub(crate) fn split_classifier(t: &str) -> (&str, &str) {
    let b = t.as_bytes();
    let found = |s: &[u8], from: usize| {
        (from..s.len().saturating_sub(2))
            .find(|&i| s[i] == b':' && s[i - 1] == b' ' && s[i + 1] == b' ')
    };
    let Some(i) = found(b, 1) else { return (t, "") };
    let term = t[..i - 1].trim_end_matches(' ');
    let mut rest = t[i + 1..].trim_start_matches(' ');
    if let Some(j) = found(rest.as_bytes(), 1) {
        rest = rest[..j - 1].trim_end_matches(' ');
    }
    (term, rest)
}

/// Emits one Term with its id, name, inline content, and index entry.
fn glossary_term(sx: &Sphinx<'_>, p: &mut Parser<'_, '_>, l: Line<'_>) -> Result<(), Fail> {
    let (text, classifier) = split_classifier(l.text);
    let line = l.head(text.len());
    let views = [line];
    let plain = inline_plain(sx, p, &views, NodeKind::Term)?;
    let span = p.line_span(&line);
    let id = p.ctx.b.begin_node(NodeKind::Term, span)?;
    let duplicate = sx.state().id_taken(&sphinx_id("term", &plain));
    if duplicate {
        p.report(GLOSSARY_DUPLICATE, span, arg_span(span), Arg::None);
    }
    let made = sx.state().make_object_id("term", &plain);
    p.ctx.b.add_attribute_string(id, AttributeKey::Id, &made)?;
    p.ctx.b.add_attribute_string(id, AttributeKey::Name, &plain)?;
    p.inline(&views)?;
    let b = &mut p.ctx.b;
    let ix = b.begin_node(NodeKind::Index, span)?;
    b.add_custom_attribute(ix, "entries", &format!("!single: {plain}"))?;
    if !classifier.is_empty() {
        b.add_custom_attribute(ix, "key", classifier)?;
    }
    b.add_custom_attribute(ix, "targets", &made)?; // the term, as in Sphinx.
    b.end_node(ix)?;
    b.end_node(id)
}

/// Parses lines as inline content of a throwaway node of `kind`, takes its plain text,
/// and removes everything the parse added: attributes of a node must come before its
/// children, so a term's id is made from its text this way. Reports are muted during
/// the trial; the real parse reports them.
pub(crate) fn inline_plain(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    lines: &[Line<'_>],
    kind: NodeKind,
) -> Result<String, Fail> {
    let snap = &p.ctx.b.snap;
    let marks = (
        snap.nodes.len(),
        snap.attributes.len(),
        snap.links.len(),
        snap.lists.len(),
        snap.cells.len(),
        snap.text.len(),
    );
    let serial = sx.state().index_serial;
    let was = p.ctx.reports().set_muted(true);
    let span = p.lines_span(lines);
    let parsed = p.ctx.b.begin_node(kind, span).and_then(|id| {
        p.inline(lines)?;
        p.ctx.b.end_node(id)?;
        Ok(id)
    });
    p.ctx.reports().set_muted(was);
    let id = parsed?;
    let mut out = String::new();
    p.ctx.b.document().plain_text(id, &mut out);
    let snap = &mut p.ctx.b.snap;
    snap.nodes.truncate(marks.0);
    snap.attributes.truncate(marks.1);
    snap.links.truncate(marks.2);
    snap.lists.truncate(marks.3);
    snap.cells.truncate(marks.4);
    snap.text.truncate(marks.5);
    sx.state().index_serial = serial;
    Ok(out)
}

/// productionlist: Production_List{group} > Production{text = token; Id
/// "grammar-token-..."} > the definition, with every `token` as an Xref{token}. As in
/// Sphinx's ProductionList: a line ending in a backslash goes on in the next; a first
/// line without a colon names the group; productions end at a line without one.
pub(crate) fn productionlist_directive(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
) -> Result<(), Fail> {
    let id = p.open(NodeKind::ProductionList, d.span)?;
    // The argument as written, with the indentation continued lines keep.
    let lines = if d.arg_block.is_empty() { &d.arg } else { &d.arg_block };
    let mut group = String::new();
    let mut i = 0;
    if let Some((first, after)) = logical_line(lines, 0)
        && !first.1.contains(':')
    {
        group = trim_space(&first.1).to_owned();
        i = after;
    }
    if !group.is_empty() {
        p.custom(id, "group", &group)?;
    }
    while let Some((l, next)) = logical_line(lines, i) {
        if !l.1.contains(':') {
            break;
        }
        production(sx, p, l, &group)?;
        i = next;
    }
    p.close(id)
}

/// The line at `i` with the lines a trailing backslash continues it into joined on, as
/// Sphinx removes each backslash-newline: (the line and its text, the next index).
fn logical_line<'s>(lines: &[Line<'s>], i: usize) -> Option<((Line<'s>, String), usize)> {
    let l = *lines.get(i)?;
    if !l.text.ends_with('\\') {
        return Some(((l, l.text.to_owned()), i + 1));
    }
    let mut text = String::new();
    let mut next = i + 1;
    for (j, line) in lines.iter().enumerate().skip(i) {
        next = j + 1;
        match line.text.strip_suffix('\\') {
            Some(t) => text.push_str(t),
            None => {
                text.push_str(line.text);
                break;
            }
        }
    }
    Some(((l, text), next))
}

fn production(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    l: (Line<'_>, String),
    group: &str,
) -> Result<(), Fail> {
    let (line, text) = l;
    let colon = text.find(':').unwrap_or(text.len());
    let name = trim_space(&text[..colon]);
    let span = p.line_span(&line);
    let b = &mut p.ctx.b;
    let id = b.begin_node(NodeKind::Production, span)?;
    let t = b.add_text(name)?;
    b.set_text(id, t);
    if !name.is_empty() {
        let prefix = if group.is_empty() {
            "grammar-token".to_owned()
        } else {
            format!("grammar-token-{group}")
        };
        let made = sx.state().make_object_id(&prefix, name);
        b.add_attribute_string(id, AttributeKey::Id, &made)?;
    }
    token_xrefs(b, text.get(colon + 1..).unwrap_or(""), group, span)?;
    b.end_node(id)
}

/// Emits text with every `token` (or `group:token`, `~group:token`) as an Xref{reftype
/// "token"} to the token in the production's group.
pub(crate) fn token_xrefs(
    b: &mut guidedoc::Builder<'_>,
    text: &str,
    group: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let t = text.as_bytes();
    let (mut pos, mut i) = (0, 0);
    while i < t.len() {
        if t[i] != b'`' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        if j < t.len() && t[j] == b'~' {
            j += 1;
        }
        let mut colon = None;
        let mut k = j;
        while k < t.len() && (is_identifier_byte(t[k]) || t[k] == b':') {
            if t[k] == b':' {
                if colon.is_some() {
                    break;
                }
                colon = Some(k);
            }
            k += 1;
        }
        let valid = k < t.len() && t[k] == b'`' && k > j && t[k - 1] != b':';
        if !valid || (colon == Some(j) && t[i + 1] == b'~') {
            i += 1;
            continue;
        }
        if i > pos {
            b.add_text_leaf(NodeKind::Text, span, &text[pos..i])?;
        }
        let token = &text[i + 1..k];
        let (mut title, mut target) = (token, token);
        if let Some(c) = colon {
            if let Some(rest) = token.strip_prefix('~') {
                (title, target) = (&text[c + 1..k], rest);
            } else if let Some(rest) = token.strip_prefix(':') {
                (title, target) = (rest, rest);
            }
        }
        let full = if colon.is_none() && !group.is_empty() {
            format!("{group}:{target}")
        } else {
            target.to_owned()
        };
        let name = b.add_text(&full)?;
        let x = b.begin_node(NodeKind::Xref, span)?;
        b.set_link(
            x,
            LinkInfo {
                name,
                kind: LinkKind::Unresolved,
                resolved: INVALID_NODE,
                ..LinkInfo::UNRESOLVED
            },
        )?;
        b.add_custom_attribute(x, "reftype", "token")?;
        b.add_text_leaf(NodeKind::Code, span, title)?;
        b.end_node(x)?;
        i = k + 1;
        pos = i;
    }
    if pos < t.len() {
        b.add_text_leaf(NodeKind::Text, span, &text[pos..])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifiers_split_at_spaced_colons() {
        assert_eq!(split_classifier("term : kind"), ("term", "kind"));
        assert_eq!(split_classifier("a : b : c"), ("a", "b"));
        assert_eq!(split_classifier("a:b"), ("a:b", ""));
    }
}
