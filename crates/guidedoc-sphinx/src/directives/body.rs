//! The core directives that build body nodes or change reader state: toctree, only,
//! index, centered, hlist, rubric, the version and author directives, highlight,
//! default-domain, default-role, todo, and acks.

use guidedoc::{AdmonitionKind, Arg, AttributeKey, Fail, NodeKind, arg_span};
use guidedoc_rst::{Directive, Line, Parser, bullet_width, standard_role};

use super::{check_index_entry, class_attribute, flag_attribute, parse_signed, value_attribute};
use crate::domains::domain_known;
use crate::extension::Sphinx;
use crate::messages::{HLIST_CONTENT, ROLE_UNKNOWN};
use crate::roles::role_known;
use crate::text::{equal_fold, parse_int, split_title, trim_space};

/// toctree: Toctree{value = maxdepth} > Toctree_Entry{text = entry; Title}.
pub(super) fn toctree_directive(p: &mut Parser<'_, '_>, d: &Directive<'_>) -> Result<(), Fail> {
    let id = p.open(NodeKind::Toctree, d.span)?;
    if let Some(v) = d.option_value("maxdepth") {
        let depth = parse_signed(&v).max(0);
        p.ctx.b.set_value(id, i32::try_from(depth).unwrap_or(i32::MAX));
    }
    class_attribute(p, id, d)?;
    value_attribute(p, id, d, "caption")?;
    value_attribute(p, id, d, "name")?;
    for name in ["hidden", "includehidden", "titlesonly", "glob", "reversed"] {
        flag_attribute(p, id, d, name)?;
    }
    if let Some(v) = d.option_value("numbered") {
        p.custom(id, "numbered", if v.is_empty() { "999" } else { &v })?;
    }
    for l in &d.content {
        let entry = trim_space(l.text);
        if entry.is_empty() {
            continue;
        }
        let span = p.line_span(l);
        let (title, target, explicit) = split_title(entry);
        let b = &mut p.ctx.b;
        let e = b.begin_node(NodeKind::ToctreeEntry, span)?;
        let t = b.add_text(if explicit { target } else { entry })?;
        b.set_text(e, t);
        if explicit {
            b.add_attribute_string(e, AttributeKey::Title, title)?;
        }
        b.end_node(e)?;
    }
    p.close(id)
}

/// only and ifconfig: Only{expression or ifconfig} > blocks. The host evaluates the
/// expression and keeps or drops the content. Where sections are allowed, titles in it
/// begin sections, as Sphinx parses it: in a title context of its own, and placed where
/// its first title's level belongs when the document already uses that adornment.
pub(super) fn only_directive(
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
    name: &str,
) -> Result<(), Fail> {
    let titled = d.sections && !p.is_hosted();
    if titled {
        p.place_titled(&d.content)?;
    }
    let id = p.open(NodeKind::Only, d.span)?;
    let key = if equal_fold(name, "ifconfig") { "ifconfig" } else { "expression" };
    p.custom(id, key, trim_space(&d.argument()))?;
    if titled {
        p.parse_titled(&d.content)?;
    } else {
        p.parse_body(&d.content, false, None)?;
    }
    p.close(id)
}

/// index: Index{Id "index-N"; entries = one entry per argument line, as written}.
pub(super) fn index_directive(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
) -> Result<(), Fail> {
    let index = sx.state().index_id();
    let id = p.open(NodeKind::Index, d.span)?;
    p.ctx.b.add_attribute_string(id, AttributeKey::Id, &index)?;
    let mut entries = String::new();
    for l in &d.arg {
        let entry = trim_space(l.text);
        if entry.is_empty() {
            continue;
        }
        if let Some(m) = check_index_entry(entry) {
            let span = p.line_span(l);
            p.report(m, span, arg_span(span), Arg::None);
        }
        if !entries.is_empty() {
            entries.push('\n');
        }
        entries.push_str(entry);
    }
    p.custom(id, "entries", &entries)?;
    value_attribute(p, id, d, "name")?;
    p.close(id)
}

/// centered: a Paragraph with class "centered".
pub(super) fn centered_directive(p: &mut Parser<'_, '_>, d: &Directive<'_>) -> Result<(), Fail> {
    let id = p.open(NodeKind::Paragraph, d.span)?;
    p.ctx.b.add_attribute_string(id, AttributeKey::Class, "centered")?;
    p.inline(&d.arg)?;
    p.close(id)
}

/// rubric: Rubric (inlines) with Sphinx's :heading-level:.
pub(super) fn rubric_directive(p: &mut Parser<'_, '_>, d: &Directive<'_>) -> Result<(), Fail> {
    let id = p.open(NodeKind::Rubric, d.span)?;
    p.add_common(id, d, "class")?;
    value_attribute(p, id, d, "heading-level")?;
    p.inline(&d.arg)?;
    p.close(id)
}

/// hlist: Hlist{value = columns} > one bullet List per column, items distributed as
/// Sphinx does: the first (items % columns) columns take one item more.
pub(super) fn hlist_directive(p: &mut Parser<'_, '_>, d: &Directive<'_>) -> Result<(), Fail> {
    let columns = d.option_value("columns").map_or(2, |v| parse_int(&v).max(1)) as usize;
    let Some(starts) = bullet_items(p, d) else { return Ok(()) };
    let count = starts.len();
    let id = p.open(NodeKind::Hlist, d.span)?;
    p.ctx.b.set_value(id, i32::try_from(columns).unwrap_or(i32::MAX));
    let (per, more) = (count / columns, count % columns);
    let mut item = 0;
    for c in 0..columns {
        let n = per + usize::from(c < more);
        if n == 0 {
            continue;
        }
        let first = starts[item];
        let last = starts.get(item + n).copied().unwrap_or(d.content.len());
        p.parse_body(&d.content[first..last], false, None)?;
        item += n;
    }
    p.close(id)
}

/// Finds the item lines of content that must be one bullet list; `None` (reported)
/// when it is not.
fn bullet_items(p: &mut Parser<'_, '_>, d: &Directive<'_>) -> Option<Vec<usize>> {
    let mut starts = Vec::new();
    let mut bullet = None;
    for (i, l) in d.content.iter().enumerate() {
        if l.is_blank() || l.is_indented() {
            continue;
        }
        let first = l.text.as_bytes()[0];
        if bullet_width(l.text) == 0 || bullet.is_some_and(|b| b != first) {
            starts.clear();
            break;
        }
        bullet = Some(first);
        starts.push(i);
    }
    if starts.is_empty() {
        p.report(HLIST_CONTENT, d.name_span, arg_span(d.name_span), Arg::None);
        return None;
    }
    Some(starts)
}

/// acks: Directive{Name "acks"} > the bullet list of acknowledgements.
pub(super) fn acks_directive(p: &mut Parser<'_, '_>, d: &Directive<'_>) -> Result<(), Fail> {
    if bullet_items(p, d).is_none() {
        return Ok(());
    }
    let id = p.directive_node(d, "acks")?;
    p.parse_body(&d.content, false, None)?;
    p.close(id)
}

/// versionadded, versionchanged, deprecated, versionremoved: Version_Modified{type,
/// version} > the explanation (the argument's rest as a paragraph), then the content.
pub(super) fn version_directive(
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
    name: &str,
) -> Result<(), Fail> {
    let Some(first) = d.arg.first().copied() else { return Ok(()) };
    let k = first.text.find(|c: char| c.is_ascii_whitespace()).unwrap_or(first.text.len());
    let id = p.open(NodeKind::VersionModified, d.span)?;
    p.custom(id, "type", name)?;
    p.custom(id, "version", &first.text[..k])?;
    let rest = first.cut(k);
    let rest = rest.cut(guidedoc_rst::indent_of(rest.text));
    let mut explanation: Vec<Line<'_>> = Vec::with_capacity(d.arg.len());
    if !rest.is_blank() {
        explanation.push(rest);
    }
    explanation.extend_from_slice(&d.arg[1..]);
    if !explanation.is_empty() {
        p.inline_node(NodeKind::Paragraph, &explanation)?;
    }
    p.parse_body(&d.content, false, None)?;
    p.close(id)
}

/// sectionauthor, moduleauthor, codeauthor: Directive{Name; author}, only when
/// show_authors is set, as Sphinx shows them.
pub(super) fn author_directive(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
    name: &str,
) -> Result<(), Fail> {
    if !sx.config.show_authors {
        return Ok(());
    }
    let id = p.directive_node(d, name)?;
    p.custom(id, "author", trim_space(&d.argument()))?;
    p.close(id)
}

/// highlight sets the language of later code blocks and the line-number threshold.
pub(super) fn highlight_directive(sx: &Sphinx<'_>, d: &Directive<'_>) -> Result<(), Fail> {
    let mut st = sx.state();
    st.highlight = trim_space(&d.argument()).to_owned();
    st.linenothreshold = d.option_value("linenothreshold").map(|v| parse_int(&v));
    Ok(())
}

/// default-domain changes the domain of names without one; an unknown domain means
/// none, as in Sphinx.
pub(super) fn default_domain_directive(sx: &Sphinx<'_>, d: &Directive<'_>) -> Result<(), Fail> {
    let name = trim_space(&d.argument()).to_ascii_lowercase();
    sx.state().domain = if domain_known(&name) { name } else { String::new() };
    Ok(())
}

/// default-role sets the role of interpreted text without one; without an argument it
/// restores Docutils' title-reference. Sphinx and domain roles are allowed.
pub(super) fn default_role_directive(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
) -> Result<(), Fail> {
    if d.arg.is_empty() {
        p.inline_state().default_role.clear();
        return Ok(());
    }
    let arg = d.argument();
    let name = trim_space(&arg);
    let standard = standard_role(name).is_some();
    let defined = p.find_role(name)?.is_some();
    if !standard && !defined && !role_known(sx.config, &sx.state(), name) {
        let span = p.lines_span(&d.arg);
        p.report(ROLE_UNKNOWN, span, arg_span(span), Arg::None);
        return Ok(());
    }
    p.inline_state().default_role = name.to_lowercase();
    Ok(())
}

/// todo: Admonition{Todo}, only when todo_include_todos is set.
pub(super) fn todo_directive(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
) -> Result<(), Fail> {
    if !sx.config.todo_include_todos {
        return Ok(());
    }
    let id = p.open(NodeKind::Admonition, d.span)?;
    p.ctx.b.set_value(id, AdmonitionKind::Todo as i32);
    p.add_common(id, d, "class")?;
    p.parse_body(&d.content, false, None)?;
    p.close(id)
}
