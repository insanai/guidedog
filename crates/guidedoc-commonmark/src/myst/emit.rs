//! Emission of the MyST blocks: sections from headings, targets, comments, definition
//! and field lists, task lists, and heading anchors.

use guidedoc::{
    Arg, AttributeKey, Fail, LinkInfo, NodeKind, SourceSpan, is_unicode_punctuation,
    is_unicode_whitespace,
};

use super::subs::expand_substitution;
use super::{Slug, inline_lines, open_section, sections_here, trim_right, trim_space};
use crate::blocks::{Bid, BlockKind, Skeleton};
use crate::emit::{emit_children, open_block};
use crate::extension::Line;
use crate::host::Host;
use crate::inline::{leaf_text, parse_leaf};
use crate::messages::HEADING_NESTED;
use crate::settings::SyntaxExtension;

/// Emits a heading: a section at the top of a document or section, else a rubric.
pub(crate) fn emit_heading(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    id: Bid,
    top: bool,
) -> Result<(), Fail> {
    let span = sk.block_span(id);
    if !(top && sections_here(h, sk)) {
        h.report(HEADING_NESTED, span, Arg::None, Arg::None);
        let node = open_block(h, NodeKind::Rubric, span)?;
        parse_leaf(h, sk, id)?;
        return h.b().end_node(node);
    }
    let ordinal = sk.main.then(|| {
        h.ms.headings += 1;
        h.ms.headings - 1
    });
    let (section, depth) = open_section(h, usize::from(sk.blocks[id].level), span)?;
    if let Some(ordinal) = ordinal {
        // Slugs are in heading order, so one cursor finds each heading's in turn.
        while let Some(s) = h.ms.slugs.get(h.ms.slug_next).filter(|s| s.ordinal <= ordinal) {
            let (k, found) = (h.ms.slug_next, s.ordinal == ordinal);
            h.ms.slug_next += 1;
            if !found {
                continue;
            }
            h.ms.slugs[k].node = Some(section);
            let slug = h.ms.slugs[k].slug.clone();
            h.b().add_custom_attribute(section, "slug", &slug)?;
        }
    }
    let heading = h.b().begin_node(NodeKind::Heading, span)?;
    h.b().set_value(heading, depth as i32);
    parse_leaf(h, sk, id)?;
    h.b().end_node(heading)
}

/// Emits the MyST leaves: targets, comments, block breaks, math, and tables.
pub(crate) fn emit_leaf(h: &mut Host<'_, '_>, sk: &Skeleton<'_>, id: Bid) -> Result<(), Fail> {
    let span = sk.block_span(id);
    match sk.blocks[id].kind {
        BlockKind::Target => {
            let t = trim_space(sk.content(id));
            let label = if t.len() >= 4 { trim_space(&t[1..trim_right(t).len() - 2]) } else { "" };
            let name = h.normalized_name(label)?;
            let b = h.b();
            let node = b.begin_node(NodeKind::Target, span)?;
            b.set_text(node, name);
            b.set_link(node, LinkInfo::UNRESOLVED)?;
            b.end_node(node)
        }
        BlockKind::Comment | BlockKind::BlockBreak => {
            let text = comment_text(sk, id);
            let b = h.b();
            let node = b.begin_node(NodeKind::Comment, span)?;
            if sk.blocks[id].kind == BlockKind::BlockBreak {
                b.add_attribute_string(node, AttributeKey::Class, "block_break")?;
            }
            let t = b.add_text(&text)?;
            b.set_text(node, t);
            b.end_node(node)
        }
        BlockKind::MathBlock => super::math::emit_math_block(h, sk, id),
        BlockKind::Table => super::table::emit_table(h, sk, id),
        _ => Ok(()),
    }
}

/// Joins a comment's lines, each trimmed, and trims the whole.
fn comment_text(sk: &Skeleton<'_>, id: Bid) -> String {
    let mut out = String::new();
    let mut wrote = false;
    for k in 0..sk.seg_count(id) {
        let line = trim_space(sk.seg_text(id, k));
        if !wrote && line.is_empty() {
            continue;
        }
        if wrote {
            out.push('\n');
        }
        out.push_str(line);
        wrote = true;
    }
    out.truncate(out.trim_end_matches('\n').len());
    out
}

fn is_definition(sk: &Skeleton<'_>, id: Option<Bid>) -> Option<Bid> {
    id.filter(|&d| sk.blocks[d].kind == BlockKind::Definition)
}

/// Groups terms with their definitions into items.
pub(crate) fn emit_definition_list(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    id: Bid,
) -> Result<(), Fail> {
    let list = open_block(h, NodeKind::DefinitionList, sk.block_span(id))?;
    h.b().add_attribute_string(list, AttributeKey::Class, "simple myst")?;
    let mut c = sk.blocks[id].first;
    while let Some(term) = c {
        if sk.blocks[term].kind != BlockKind::DefinitionTerm {
            c = sk.blocks[term].next;
            continue;
        }
        let mut last = term;
        while let Some(d) = is_definition(sk, sk.blocks[last].next) {
            last = d;
        }
        let span = sk.span(sk.blocks[term].start as usize, sk.blocks[last].end as usize);
        let item = h.b().begin_node(NodeKind::DefinitionItem, span)?;
        let t = h.b().begin_node(NodeKind::Term, sk.block_span(term))?;
        parse_leaf(h, sk, term)?;
        h.b().end_node(t)?;
        let mut d = is_definition(sk, sk.blocks[term].next);
        while let Some(def) = d {
            let node = h.b().begin_node(NodeKind::Definition, sk.block_span(def))?;
            emit_children(h, sk, def)?;
            h.b().end_node(node)?;
            d = is_definition(sk, sk.blocks[def].next);
        }
        h.b().end_node(item)?;
        c = sk.blocks[last].next;
    }
    h.b().end_node(list)
}

/// Emits a field of a field list: its name parsed inline, its body as blocks.
pub(crate) fn emit_field(h: &mut Host<'_, '_>, sk: &Skeleton<'_>, id: Bid) -> Result<(), Fail> {
    let blk = sk.blocks[id];
    let field = h.b().begin_node(NodeKind::Field, sk.block_span(id))?;
    let (a, z) = (blk.name_start as usize, blk.name_end as usize);
    let name = h.b().begin_node(NodeKind::FieldName, sk.span(a, z))?;
    let one = [Line { text: sk.src.get(a..z).unwrap_or(""), offset: a as u32 }];
    inline_lines(h, &one, sk.source)?;
    h.b().end_node(name)?;
    let body = h.b().begin_node(NodeKind::FieldBody, sk.block_span(id))?;
    emit_children(h, sk, id)?;
    h.b().end_node(body)?;
    h.b().end_node(field)
}

/// Expands a paragraph that is only `{{ expression }}` as blocks.
pub(crate) fn block_substitution(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    id: Bid,
) -> Result<bool, Fail> {
    if !h.has(SyntaxExtension::Substitution) || sk.seg_count(id) != 1 {
        return Ok(false);
    }
    let t = trim_space(sk.seg_text(id, 0)).as_bytes();
    let [o, c] = h.settings.sub_delimiters;
    let n = t.len();
    if n < 4 || t[0] != o || t[1] != o || t[n - 1] != c || t[n - 2] != c {
        return Ok(false);
    }
    let inner = &t[2..n - 2];
    if inner.windows(2).any(|w| w[0] == c && w[1] == c) {
        return Ok(false);
    }
    let expr = std::str::from_utf8(inner).unwrap_or("");
    expand_substitution(h, expr, sk.block_span(id), true)?;
    Ok(true)
}

/// Emits a task list item's checkbox, as HTML, and the space after it.
pub(crate) fn task_checkbox(
    h: &mut Host<'_, '_>,
    checked: bool,
    span: SourceSpan,
) -> Result<(), Fail> {
    let html = if checked {
        "<input class=\"task-list-item-checkbox\" checked=\"checked\" disabled=\"disabled\" \
         type=\"checkbox\">"
    } else {
        "<input class=\"task-list-item-checkbox\" disabled=\"disabled\" type=\"checkbox\">"
    };
    let format = h.html_format()?;
    let b = h.b();
    let box_span = SourceSpan::new(span.source, span.start, span.start + 3);
    let node = b.begin_node(NodeKind::RawInline, box_span)?;
    let t = b.add_text(html)?;
    b.set_text(node, t);
    b.add_attribute(node, AttributeKey::Format, format)?;
    b.end_node(node)?;
    let space = SourceSpan::new(span.source, span.start + 3, span.start + 4);
    b.add_text_leaf(NodeKind::Text, space, " ")?;
    Ok(())
}

/// Computes the slugs of the main document's section headings, so links can reach
/// headings that come later.
pub(crate) fn prescan_headings(h: &mut Host<'_, '_>, sk: &Skeleton<'_>) -> Result<(), Fail> {
    let headings = sk.children(0).filter(|&id| sk.blocks[id].kind == BlockKind::Heading);
    for (ordinal, id) in headings.enumerate() {
        if usize::from(sk.blocks[id].level) > h.settings.heading_anchors {
            continue;
        }
        let mut title = String::new();
        leaf_text(h, sk, id, &mut title)?;
        let title = trim_space(&title).to_owned();
        let slug = unique_slug(h, &title);
        h.ms.slug_index.insert(slug.clone(), h.ms.slugs.len());
        h.ms.slugs.push(Slug { slug, title, ordinal, node: None });
    }
    Ok(())
}

/// Makes a GitHub-style slug, "-1", "-2", ... added for repeats.
fn unique_slug(h: &mut Host<'_, '_>, title: &str) -> String {
    let mut base = String::with_capacity(title.len());
    for c in trim_space(title).chars() {
        if c.is_ascii() {
            match c {
                'A'..='Z' => base.push(c.to_ascii_lowercase()),
                'a'..='z' | '0'..='9' | '_' | '-' => base.push(c),
                ' ' => base.push('-'),
                _ => {}
            }
        } else if !is_unicode_punctuation(c) && !is_unicode_whitespace(c) {
            base.push(c);
        }
    }
    let mut n = h.ms.slug_bases.get(&base).copied().unwrap_or(0);
    let mut slug = base.clone();
    while n < 1_000_000 && h.ms.slug_index.contains_key(&slug) {
        n += 1;
        slug = format!("{base}-{n}");
    }
    h.ms.slug_bases.insert(base, n);
    slug
}

/// Tells whether a list is a task list: an item starts with "[ ]" or "[x]".
pub(crate) fn task_list(sk: &Skeleton<'_>, list: Bid) -> bool {
    sk.children(list).any(|item| task_state(sk, item) > 0)
}

/// The task state of a list item: 0 for no task, 1 for "[ ] ", 2 for "[x] ".
pub(crate) fn task_state(sk: &Skeleton<'_>, item: Bid) -> u8 {
    let Some(first) = sk.blocks[item].first else { return 0 };
    if sk.blocks[first].kind != BlockKind::Paragraph {
        return 0;
    }
    let t = sk.content(first).as_bytes();
    if t.len() < 4 || t[0] != b'[' || t[2] != b']' || (t[3] != b' ' && t[3] != b'\n') {
        return 0;
    }
    match t[1] {
        b' ' => 1,
        b'x' | b'X' => 2,
        _ => 0,
    }
}
