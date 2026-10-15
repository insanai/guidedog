//! HTML the optional syntax converts: `<div class="admonition ...">` blocks
//! ("html_admonition") become admonitions whose title and content are Markdown, and
//! `<img>` tags ("html_image") become images, with the image directive's attributes
//! src, alt, class, width, height, and name.

use guidedoc::{AdmonitionKind, AttributeKey, Fail, LinkInfo, NodeKind, SourceSpan};

use crate::blocks::{Bid, Skeleton, is_blank, parse_lines};
use crate::emit::open_block;
use crate::extension::Line;
use crate::host::Host;
use crate::inline::html_scan::is_html_space;
use crate::inline::{Inline, Item};
use crate::settings::SyntaxExtension;

/// The most attributes of a tag kept.
const MAX_ATTRS: usize = 16;

/// A scanned tag: its name, attributes, where it ends, and whether it closes.
#[derive(Debug, Default)]
struct Tag<'s> {
    name: &'s str,
    attrs: Vec<(&'s str, &'s str)>,
    /// Offset after ">".
    end: usize,
    close: bool,
}

impl<'s> Tag<'s> {
    fn attr(&self, name: &str) -> Option<&'s str> {
        self.attrs.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
    }

    fn is(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
    }
}

/// Reads the tag at `s[i]`, which is '<'.
fn scan_tag(s: &str, i: usize) -> Option<Tag<'_>> {
    let b = s.as_bytes();
    let mut t = Tag::default();
    let mut k = i + 1;
    if b.get(k) == Some(&b'/') {
        t.close = true;
        k += 1;
    }
    let a = k;
    while k < b.len() && (b[k].is_ascii_alphanumeric() || b[k] == b'-') {
        k += 1;
    }
    if k == a {
        return None;
    }
    t.name = &s[a..k];
    loop {
        while k < b.len() && is_html_space(b[k]) {
            k += 1;
        }
        match b.get(k)? {
            b'>' => {
                t.end = k + 1;
                return Some(t);
            }
            b'/' if b.get(k + 1) == Some(&b'>') => {
                t.end = k + 2;
                return Some(t);
            }
            _ => {}
        }
        let n = k;
        while k < b.len() && !is_html_space(b[k]) && !b"=>/".contains(&b[k]) {
            k += 1;
        }
        if k == n {
            return None;
        }
        let name = &s[n..k];
        while k < b.len() && is_html_space(b[k]) {
            k += 1;
        }
        let mut value = "";
        if b.get(k) == Some(&b'=') {
            k += 1;
            while k < b.len() && is_html_space(b[k]) {
                k += 1;
            }
            if let Some(&q) = b.get(k).filter(|&&q| q == b'"' || q == b'\'') {
                let v = k + 1;
                k = v + b[v..].iter().position(|&c| c == q)?;
                value = &s[v..k];
                k += 1;
            } else {
                let v = k;
                while k < b.len() && !is_html_space(b[k]) && b[k] != b'>' {
                    k += 1;
                }
                value = &s[v..k];
            }
        }
        if t.attrs.len() < MAX_ATTRS {
            t.attrs.push((name, value));
        }
    }
}

fn has_class(classes: &str, want: &str) -> bool {
    classes.split(' ').any(|c| c == want)
}

/// Converts an HTML block the optional syntax recognizes; false for other HTML.
pub(crate) fn emit_html_extension(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    id: Bid,
) -> Result<bool, Fail> {
    if h.has(SyntaxExtension::HtmlAdmonition) && html_admonition(h, sk, id)? {
        return Ok(true);
    }
    if h.has(SyntaxExtension::HtmlImage) {
        return html_images(h, sk, id);
    }
    Ok(false)
}

/// Emits a block of only `<img>` tags as images; false when it holds anything else.
pub(crate) fn html_images(h: &mut Host<'_, '_>, sk: &Skeleton<'_>, id: Bid) -> Result<bool, Fail> {
    let s = sk.content(id);
    let mut tags = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if s.as_bytes()[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        match scan_tag(s, i) {
            Some(t) if !t.close && t.is("img") => {
                let end = t.end;
                tags.push((i, t));
                i = end;
            }
            _ => return Ok(false),
        }
    }
    if tags.is_empty() {
        return Ok(false);
    }
    let start = sk.blocks[id].content_start;
    for (i, t) in &tags {
        let at = sk.source_offset(id, start + i);
        emit_img(h, t, sk.span(at, at + t.end - i))?;
    }
    Ok(true)
}

impl Inline<'_, '_, '_, '_> {
    /// Emits an inline `<img>` as an image; false for other raw HTML.
    pub(crate) fn html_inline_image(&mut self, it: Item) -> Result<bool, Fail> {
        if !self.h.has(SyntaxExtension::HtmlImage) {
            return Ok(false);
        }
        let s = self.str(it.text).to_owned();
        let Some(t) = scan_tag(&s, 0).filter(|t| !t.close && t.is("img") && t.end == s.len())
        else {
            return Ok(false);
        };
        let span = self.item_span(it.start, it.end);
        emit_img(self.h, &t, span)?;
        Ok(true)
    }
}

fn emit_img(h: &mut Host<'_, '_>, t: &Tag<'_>, span: SourceSpan) -> Result<(), Fail> {
    let name = match t.attr("name").filter(|v| !v.is_empty()) {
        Some(v) => Some(h.normalized_name(v)?),
        None => None,
    };
    let b = h.b();
    let id = b.begin_node(NodeKind::Image, span)?;
    for (key, attr) in [
        (AttributeKey::Alt, "alt"),
        (AttributeKey::Width, "width"),
        (AttributeKey::Height, "height"),
    ] {
        if let Some(v) = t.attr(attr) {
            b.add_attribute_string(id, key, v)?;
        }
    }
    if let Some(v) = t.attr("class").filter(|v| !v.is_empty()) {
        b.add_attribute_string(id, AttributeKey::Class, v)?;
    }
    if let Some(name) = name {
        b.add_attribute(id, AttributeKey::Name, name)?;
    }
    let destination = b.add_text(t.attr("src").unwrap_or(""))?;
    b.set_link(id, LinkInfo { destination, ..LinkInfo::default() })?;
    b.end_node(id)
}

/// Finds the `</name>` that closes the tag opened before `from`, counting nested tags
/// of the same name; returns the offsets of the closing tag.
fn matching_close(s: &str, name: &str, from: usize) -> Option<(usize, usize)> {
    let mut depth = 1;
    let mut i = from;
    while i < s.len() {
        if s.as_bytes()[i] != b'<' {
            i += 1;
            continue;
        }
        let Some(t) = scan_tag(s, i) else {
            i += 1;
            continue;
        };
        if t.is(name) {
            depth = if t.close { depth - 1 } else { depth + 1 };
            if depth == 0 {
                return Some((i, t.end));
            }
        }
        i = t.end;
    }
    None
}

fn html_admonition(h: &mut Host<'_, '_>, sk: &Skeleton<'_>, id: Bid) -> Result<bool, Fail> {
    let s = sk.content(id);
    let i = s.len() - s.trim_start_matches(|c: char| c.is_ascii_whitespace()).len();
    if !s[i..].starts_with('<') {
        return Ok(false);
    }
    let Some(open) = scan_tag(s, i) else { return Ok(false) };
    let classes = open.attr("class").unwrap_or("");
    if open.close || !open.is("div") || !has_class(classes, "admonition") {
        return Ok(false);
    }
    let Some((a, z)) = matching_close(s, "div", open.end) else { return Ok(false) };
    if !is_blank(&s[z..]) {
        return Ok(false);
    }
    emit_html_admonition(h, sk, id, &open, a)?;
    Ok(true)
}

/// The class an admonition kind's div carries.
fn kind_class(k: AdmonitionKind) -> &'static str {
    match k {
        AdmonitionKind::Generic => "",
        AdmonitionKind::Attention => "attention",
        AdmonitionKind::Caution => "caution",
        AdmonitionKind::Danger => "danger",
        AdmonitionKind::Error => "error",
        AdmonitionKind::Hint => "hint",
        AdmonitionKind::Important => "important",
        AdmonitionKind::Note => "note",
        AdmonitionKind::Tip => "tip",
        AdmonitionKind::Warning => "warning",
        AdmonitionKind::Seealso => "seealso",
        AdmonitionKind::Todo => "todo",
    }
}

/// A title element (`<div class="title">` or `<p class="title">`) at the start of the
/// body `[a, z)`: the title's range and where the body continues.
fn title_element(s: &str, a: usize, z: usize) -> Option<(usize, usize, usize)> {
    let k =
        a + s[a..z].len() - s[a..z].trim_start_matches(|c: char| c.is_ascii_whitespace()).len();
    if !s[k..z].starts_with('<') {
        return None;
    }
    let t = scan_tag(s, k).filter(|t| !t.close)?;
    if !(t.is("div") || t.is("p")) || !has_class(t.attr("class").unwrap_or(""), "title") {
        return None;
    }
    let (ca, cz) = matching_close(s, t.name, t.end).filter(|&(_, cz)| cz <= z)?;
    Some((t.end, ca, cz))
}

fn emit_html_admonition(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    id: Bid,
    open: &Tag<'_>,
    z: usize,
) -> Result<(), Fail> {
    let s = sk.content(id);
    let classes = open.attr("class").unwrap_or("");
    let mut kind = AdmonitionKind::Generic;
    for k in AdmonitionKind::ALL {
        if k != AdmonitionKind::Generic && has_class(classes, kind_class(k)) {
            kind = k;
        }
    }
    let title = title_element(s, open.end, z).filter(|(ta, tz, _)| tz > ta);
    if title.is_some() {
        kind = AdmonitionKind::Generic;
    }
    let body_start = title.map_or(open.end, |(_, _, cz)| cz);
    let at = sk.source_offset(id, sk.blocks[id].content_start);
    let node = open_block(h, NodeKind::Admonition, sk.block_span(id))?;
    h.b().set_value(node, kind as i32);
    let rest = other_classes(classes, kind == AdmonitionKind::Generic);
    if !rest.is_empty() {
        h.b().add_attribute_string(node, AttributeKey::Class, &rest)?;
    }
    if let Some(v) = open.attr("name").filter(|v| !v.is_empty()) {
        let name = h.normalized_name(v)?;
        h.b().add_attribute(node, AttributeKey::Name, name)?;
    }
    if let Some((ta, tz, _)) = title {
        let t = h.b().begin_node(NodeKind::Title, sk.span(at, at))?;
        super::inline_lines(h, &arena_lines(sk, id, ta, tz), sk.source)?;
        h.b().end_node(t)?;
    }
    parse_lines(h, &arena_lines(sk, id, body_start, z), sk.source, false, false)?;
    h.b().end_node(node)
}

/// Keeps the classes other than "admonition" and, unless `keep_kinds` (a titled
/// admonition, which is generic), the kinds' names.
fn other_classes(classes: &str, keep_kinds: bool) -> String {
    let kinds = AdmonitionKind::ALL.map(kind_class);
    classes
        .split(' ')
        .filter(|c| !c.is_empty() && *c != "admonition")
        .filter(|c| keep_kinds || !kinds.contains(c))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `[a, z)` of a block's content as lines with source offsets.
fn arena_lines<'k>(sk: &'k Skeleton<'_>, id: Bid, a: usize, z: usize) -> Vec<Line<'k>> {
    let s = sk.content(id);
    let start = sk.blocks[id].content_start;
    let mut lines = Vec::new();
    let mut from = a;
    for (i, c) in s[a..z].bytes().enumerate().map(|(k, c)| (a + k, c)).chain([(z, b'\n')]) {
        if c != b'\n' {
            continue;
        }
        let at = sk.source_offset(id, start + from);
        lines.push(Line { text: &s[from..i], offset: at as u32 });
        from = i + 1;
    }
    lines
}
