//! Inline markup: emphasis, literals, references and links, notes, substitutions,
//! targets, math, raw text, and roles, each appended to the inline buffer whole.

use std::borrow::Cow;
use std::fmt::{self, Write as _};

use guidedoc::{AttributeKey, LinkInfo, LinkKind, NodeFlag, NodeId, NodeKind, normalize_label};

use crate::escape::{is_simple_name, push_markup_text};
use crate::roles::SPHINX_ROLES;
use crate::state::Writer;

/// Returns `label` normalized as reference names are: white space collapsed,
/// lowercase.
pub(crate) fn normalized(label: &str) -> String {
    let mut out = String::new();
    normalize_label(&mut out, label);
    out
}

/// Undoes smart quotes: curly quotation marks become straight ones.
fn unsmarten(s: &str) -> String {
    s.replace(['’', '‘'], "'").replace(['“', '”'], "\"")
}

/// Text escaped for the inside of interpreted text or a phrase, written as it is
/// formatted (`push_markup_text` without a buffer).
struct Escaped<'s>(&'s str, bool);

impl fmt::Display for Escaped<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.0.chars() {
            if c == '\\' || c == '`' || c == '*' || (self.1 && c == '<') {
                f.write_char('\\')?;
            }
            f.write_char(c)?;
        }
        Ok(())
    }
}

/// A URI as an embedded target writes it: backslashes doubled, spaces and backquotes
/// percent-encoded.
struct EmbeddedUri<'s>(&'s str);

impl fmt::Display for EmbeddedUri<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.0.chars() {
            match c {
                '\\' => f.write_str("\\\\")?,
                ' ' => f.write_str("%20")?,
                '`' => f.write_str("%60")?,
                _ => f.write_char(c)?,
            }
        }
        Ok(())
    }
}

/// Escapes text for the inside of interpreted text or a phrase.
fn escaped(s: &str, angle: bool) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    push_markup_text(&mut out, s, angle);
    out
}

impl Writer<'_, '_> {
    /// A Text node, less what an `:index:` role before it already wrote.
    pub fn text_node(&mut self, id: NodeId) {
        let text = self.doc.node_text(id);
        let skip = std::mem::take(&mut self.skip_text).min(text.len());
        self.inline.text(text.get(skip..).unwrap_or(text));
    }

    /// The text of an inline node whose content markup cannot nest: its Text
    /// children; other markup inside is reported and written as its text.
    /// Give the text back with `recycle` when done.
    pub fn markup_text(&mut self, id: NodeId) -> String {
        let mut out = self.buffer();
        let mut nested = false;
        for child in self.doc.children(id) {
            match self.kind(child) {
                NodeKind::Text => out.push_str(self.doc.node_text(child)),
                NodeKind::SoftBreak | NodeKind::HardBreak => out.push(' '),
                _ => {
                    nested = true;
                    self.doc.plain_text(child, &mut out);
                }
            }
        }
        if nested {
            self.lossy(id, "markup nested in markup", "the inner markup's text");
        }
        out
    }

    /// Writes `open content close`, moving white space at the content's ends outside
    /// the markup, where reStructuredText requires it.
    fn wrapped_markup(&mut self, id: NodeId, open: &str, content: &str, close: &str) {
        self.markup_with(id, open, content, close, true);
    }

    /// Writes `open content close`. White space at the content's ends, which may not
    /// touch the start- or end-string, is guarded by escaped spaces (`\ `) when the
    /// markup processes escapes, and moved outside the markup otherwise.
    fn markup_with(&mut self, id: NodeId, open: &str, content: &str, close: &str, escapes: bool) {
        let inner = content.trim();
        if inner.is_empty() {
            self.lossy(id, "empty inline markup", "nothing");
            self.inline.text(content);
            return;
        }
        if inner.len() == content.len() {
            return self.inline.markup_fmt(format_args!("{open}{content}{close}"));
        }
        if escapes {
            let lead = if content.starts_with(char::is_whitespace) { "\\ " } else { "" };
            let trail = if content.ends_with(char::is_whitespace) { "\\ " } else { "" };
            return self.inline.markup_fmt(format_args!("{open}{lead}{content}{trail}{close}"));
        }
        self.lossy(id, "white space at the ends of inline markup", "it outside the markup");
        let lead = &content[..content.len() - content.trim_start().len()];
        let trail = &content[content.trim_end().len()..];
        self.inline.text(lead);
        self.inline.markup_fmt(format_args!("{open}{inner}{close}"));
        self.inline.text(trail);
    }

    /// `*text*` or `**text**`; with classes, a role based on emphasis or strong.
    pub fn emphasis(&mut self, id: NodeId, delimiter: &str, base: &str) {
        let text = self.markup_text(id);
        let classes = self.classes(id, &[]);
        if classes.is_empty() {
            let mut content = self.buffer();
            push_markup_text(&mut content, &text, false);
            self.wrapped_markup(id, delimiter, &content, delimiter);
            self.recycle(content);
        } else {
            let role = self.roles.role(&classes, base, &[("class", &classes)]);
            self.interpreted(id, &role, &escaped(&text, false));
        }
        self.recycle(text);
    }

    /// `:role:`content``, the content already escaped.
    fn interpreted(&mut self, id: NodeId, role: &str, content: &str) {
        let mut open = self.buffer();
        let _ = write!(open, ":{role}:`");
        self.wrapped_markup(id, &open, content, "`");
        self.recycle(open);
    }

    /// Tells whether `text` normalizes to the reference name `name`.
    fn normalizes_to(&mut self, text: &str, name: &str) -> bool {
        let mut normal = self.buffer();
        normalize_label(&mut normal, text);
        let same = normal == name;
        self.recycle(normal);
        same
    }

    /// An inline literal: ``` ``text`` ```, `:literal:` when the text cannot stand
    /// between double backquotes, or a code role for a language.
    pub fn code(&mut self, id: NodeId) {
        let text = self.doc.node_text(id);
        if let Some(lang) = self.attr(id, AttributeKey::Language) {
            let classes = self.classes(id, &["code", lang]);
            let mut options = vec![("language", lang)];
            if !classes.is_empty() {
                options.push(("class", &classes));
            }
            let role = self.roles.role(lang, "code", &options);
            return self.interpreted(id, &role, &escaped(text, false));
        }
        let classes = self.classes(id, &[]);
        if classes == "code" {
            return self.interpreted(id, "code", &escaped(text, false));
        }
        if !classes.is_empty() {
            let role = self.roles.role(&classes, "literal", &[("class", &classes)]);
            return self.interpreted(id, &role, &escaped(text, false));
        }
        let plain = !text.contains("``")
            && !text.starts_with(char::is_whitespace)
            && !text.ends_with(char::is_whitespace)
            && !text.ends_with('`')
            && !text.starts_with('`');
        if plain && !text.is_empty() {
            self.inline.markup_fmt(format_args!("``{text}``"));
        } else {
            self.interpreted(id, "literal", &escaped(text, false));
        }
    }

    /// A reference, link, or image link.
    pub fn link(&mut self, id: NodeId) {
        if !self.in_inline {
            match self.image_in(id) {
                Some(image) => self.image_directive("image", image, Some(id)),
                None => {
                    self.lossy(id, "a link outside text", "a paragraph holding it");
                    let lines = self.collect(self.doc.parent_of(id).unwrap_or(id), false, |_| {});
                    self.write_block_lines(&lines);
                    self.give_lines(lines);
                }
            }
            return;
        }
        let link = self.doc.link_info(id).unwrap_or(LinkInfo::UNRESOLVED);
        let mut children = self.doc.children(id);
        let only = children.next().filter(|_| children.next().is_none());
        if let Some(c) = only {
            match self.kind(c) {
                NodeKind::Image => return self.inline_image(c, Some(id)),
                NodeKind::SubstitutionReference => {
                    let anon = self.doc.node(id).flags.has(NodeFlag::Anonymous);
                    let tail = if anon { "__" } else { "_" };
                    let text = self.doc.node_text(c);
                    return self.inline.markup_fmt(format_args!("|{text}|{tail}"));
                }
                _ => {}
            }
        }
        let text = self.markup_text(id);
        if self.kind(id) == NodeKind::Reference {
            self.reference(id, &text, link);
        } else {
            self.hyperlink(id, &text, link);
        }
        self.recycle(text);
    }

    fn reference(&mut self, id: NodeId, text: &str, link: LinkInfo) {
        let anonymous = self.doc.node(id).flags.has(NodeFlag::Anonymous);
        let name = self.doc.text(link.name);
        let tail = if anonymous { "__" } else { "_" };
        let (text_e, target) = (Escaped(text, true), Escaped(name, false));
        if self.custom(id, "target-name").is_some() {
            return self.inline.markup_fmt(format_args!("`{text_e} <{target}_>`_"));
        }
        if anonymous || name.is_empty() || self.normalizes_to(text, name) {
            if is_simple_name(text) {
                return self.inline.markup_fmt(format_args!("{text}{tail}"));
            }
            return self.inline.markup_fmt(format_args!("`{text_e}`{tail}"));
        }
        self.inline.markup_fmt(format_args!("`{text_e} <{target}_>`__"));
    }

    fn hyperlink(&mut self, id: NodeId, text: &str, link: LinkInfo) {
        let destination = self.doc.text(link.destination);
        let name = self.doc.text(link.name);
        if !self.doc.text(link.title).is_empty() {
            self.lossy(id, "a link title", "the link without it");
        }
        let uri = match link.kind {
            LinkKind::Email if text == destination => {
                return self.inline.markup(destination);
            }
            LinkKind::Email => Cow::Owned(format!("mailto:{destination}")),
            LinkKind::Internal => return self.internal_link(id, text, link),
            LinkKind::External | LinkKind::Project | LinkKind::Unresolved => {
                Cow::Borrowed(destination)
            }
        };
        if text == uri && self.bare_uri_fits(id, &uri) {
            return self.inline.markup(&uri);
        }
        let mut text = Cow::Borrowed(text);
        if !name.is_empty()
            && text.contains(['’', '‘', '“', '”'])
            && !self.normalizes_to(&text, name)
            && name == normalized(&unsmarten(&text))
        {
            // Smart quotes changed the text after its name was taken; write it as it
            // was, and the reader changes it again.
            text = Cow::Owned(unsmarten(&text));
        }
        let tail = if !name.is_empty() && self.normalizes_to(&text, name) { "_" } else { "__" };
        let (text, uri) = (Escaped(&text, true), EmbeddedUri(&uri));
        self.inline.markup_fmt(format_args!("`{text} <{uri}>`{tail}"));
    }

    /// Tells whether a URI written bare would be read back whole: it has a scheme and
    /// what follows it cannot extend it.
    fn bare_uri_fits(&self, id: NodeId, uri: &str) -> bool {
        let scheme = uri.split_once(':').is_some_and(|(s, rest)| {
            !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric()) && !rest.is_empty()
        });
        if !scheme || uri.ends_with(['.', ',', ';', ':', '!', '?', ')', '>', '\'', '"']) {
            return false;
        }
        let Some(next) = self.inline_next(id) else { return true };
        if self.kind(next) != NodeKind::Text {
            return self.kind(next) == NodeKind::SoftBreak;
        }
        let t = self.doc.node_text(next);
        let mut chars = t.chars();
        match chars.next() {
            None => true,
            Some(c) if c.is_whitespace() => true,
            Some(c) if ".,;:!?)".contains(c) => chars.next().is_none_or(char::is_whitespace),
            _ => false,
        }
    }

    /// The next sibling of an inline node, inside its parent.
    fn inline_next(&self, id: NodeId) -> Option<NodeId> {
        let next = self.doc.subtree_end(id);
        let parent = self.doc.parent_of(id)?;
        (next < self.doc.subtree_end(parent)).then_some(next)
    }

    /// A link to a node of this document (a Markdown heading anchor): a reference to
    /// the section's title.
    fn internal_link(&mut self, id: NodeId, text: &str, link: LinkInfo) {
        let target = link.resolved;
        let mut title = String::new();
        if target.index() < self.doc.node_count() {
            let heading = self
                .doc
                .first_child(target)
                .filter(|&h| matches!(self.kind(h), NodeKind::Heading | NodeKind::Title));
            if let Some(h) = heading {
                self.doc.plain_text(h, &mut title);
            } else if let Some(n) = self.attr(target, AttributeKey::Name) {
                title = n.to_owned();
            }
        }
        if title.is_empty() {
            self.lossy(id, "a link to an element without a name", "its text");
            return self.inline.text(text);
        }
        self.lossy(id, "a link to a heading anchor", "a reference to the section title");
        let target = escaped(&normalized(&title), false);
        self.inline.markup_fmt(format_args!("`{} <{target}_>`__", escaped(text, true)));
    }

    /// `[label]_`.
    pub fn note_reference(&mut self, id: NodeId) {
        let label = self.doc.node_text(id);
        self.inline.markup_fmt(format_args!("[{label}]_"));
    }

    /// `|name|`.
    pub fn substitution_reference(&mut self, id: NodeId) {
        let name = self.doc.node_text(id);
        self.inline.markup_fmt(format_args!("|{name}|"));
    }

    /// ``_`text` ``.
    pub fn inline_target(&mut self, id: NodeId) {
        let text = self.markup_text(id);
        self.wrapped_markup(id, "_`", &escaped(&text, false), "`");
        self.recycle(text);
    }

    /// `:math:`latex``.
    pub fn math(&mut self, id: NodeId) {
        let text = self.doc.node_text(id).to_owned();
        if text.contains('`') {
            self.lossy(id, "a backquote in inline math", "the math with `\\textasciigrave`");
        }
        let text = text.replace('`', "\\textasciigrave ");
        let classes = self.classes(id, &[]);
        let role = if classes.is_empty() {
            "math".to_owned()
        } else {
            self.roles.role(&classes, "math", &[("class", &classes)])
        };
        self.markup_with(id, &format!(":{role}:`"), &text, "`", false);
    }

    /// A raw role for the format: `:raw-html:`text``.
    pub fn raw_inline(&mut self, id: NodeId) {
        let format = self.attr(id, AttributeKey::Format).unwrap_or("html").to_owned();
        let classes = self.classes(id, &[]);
        let mut options = vec![("format", format.as_str())];
        if !classes.is_empty() {
            options.push(("class", &classes));
        }
        let role = self.roles.role(&format!("raw-{format}"), "raw", &options);
        let text = self.doc.node_text(id).replace('`', "\\`");
        self.markup_with(id, &format!(":{role}:`"), &text, "`", false);
    }

    /// `:sub:` and `:sup:`, or a role based on them for classes.
    pub fn simple_role(&mut self, id: NodeId, role: &str) {
        let text = self.markup_text(id);
        let classes = self.classes(id, &[]);
        let role = if classes.is_empty() {
            role.to_owned()
        } else {
            self.roles.role(&classes, role, &[("class", &classes)])
        };
        self.interpreted(id, &role, &escaped(&text, false));
        self.recycle(text);
    }

    /// `` `title` `` with the default role.
    pub fn title_reference(&mut self, id: NodeId) {
        let text = self.markup_text(id);
        let classes = self.classes(id, &[]);
        if classes.is_empty() {
            self.wrapped_markup(id, "`", &escaped(&text, false), "`");
        } else {
            let role = self.roles.role(&classes, "title-reference", &[("class", &classes)]);
            self.interpreted(id, &role, &escaped(&text, false));
        }
        self.recycle(text);
    }

    /// `:abbr:`LIFO (last in, first out)`` (Sphinx) or `:abbreviation:`.
    pub fn abbreviation(&mut self, id: NodeId) {
        let plain = self.markup_text(id);
        let text = escaped(&plain, false);
        self.recycle(plain);
        match self.attr(id, AttributeKey::Title) {
            Some(title) => {
                let content = format!("{text} ({})", escaped(title, false));
                self.interpreted(id, "abbr", &content);
            }
            None => self.interpreted(id, if self.sphinx { "abbr" } else { "abbreviation" }, &text),
        }
    }

    /// A custom or Sphinx role, kept by name; roles the reader would not know are
    /// declared.
    pub fn role(&mut self, id: NodeId) {
        let name = self.doc.node_text(id).to_owned();
        if name == "xref" {
            self.lossy(id, "a cross-reference that was not resolved", "its text");
            return self.inline_children(id);
        }
        let known = self.sphinx && SPHINX_ROLES.contains(&name.as_str());
        if !known {
            let classes = self.classes(id, &[]);
            if classes.is_empty() || classes == name {
                self.roles.exact(&name, &[]);
            } else {
                self.roles.exact(&name, &[("class", &classes)]);
            }
        }
        let content = self.role_content(id);
        self.interpreted(id, &name, &content);
    }

    /// The source text of a role's content: Sphinx's processing undone (`{var}` of
    /// samp and file, `&` accelerators, the menu arrow).
    fn role_content(&mut self, id: NodeId) -> String {
        let mut out = String::new();
        for child in self.doc.children(id) {
            match self.kind(child) {
                NodeKind::Text => out.push_str(&escaped(self.doc.node_text(child), false)),
                NodeKind::SoftBreak => out.push(' '),
                NodeKind::Emphasis => {
                    let mut text = String::new();
                    self.doc.plain_text(child, &mut text);
                    out.push('{');
                    out.push_str(&escaped(&text, false));
                    out.push('}');
                }
                NodeKind::Role if self.doc.node_text(child) == "accelerator" => {
                    out.push('&');
                    out.push_str(&self.role_content(child));
                }
                _ => {
                    let mut text = String::new();
                    self.doc.plain_text(child, &mut text);
                    out.push_str(&escaped(&text, false));
                }
            }
        }
        if self.doc.node_text(id) == "menuselection" {
            out = out.replace('‣', "-->");
        }
        out
    }

    /// A Sphinx cross-reference: `:reftype:`target`` or `:reftype:`title <target>``.
    pub fn xref(&mut self, id: NodeId) {
        let link = self.doc.link_info(id).unwrap_or(LinkInfo::UNRESOLVED);
        let reftype = self.custom(id, "reftype").unwrap_or("any").to_owned();
        let mut target = self.doc.text(link.name).to_owned();
        if self.custom(id, "refspecific").is_some() {
            target.insert(0, '.');
        }
        if let Some(anchor) = self.custom(id, "anchor").filter(|a| !a.is_empty()) {
            self.lossy(id, "an anchor in a link to a document", "a link to the document");
            let _ = anchor;
        }
        let mut display = String::new();
        self.doc.plain_text(id, &mut display);
        let content = if self.custom(id, "refexplicit").is_some() {
            format!("{} <{}>", escaped(&display, true), escaped(&target, false))
        } else {
            implicit_xref(&display, &target)
        };
        let role = if reftype == "any" && !self.sphinx { "any".to_owned() } else { reftype };
        self.interpreted(id, &role, &content);
    }
}

/// The content of a cross-reference without an explicit title: the target, with `~`
/// when the display is its last component.
fn implicit_xref(display: &str, target: &str) -> String {
    let bare = target.trim_start_matches('.');
    let shown = display.strip_suffix("()").unwrap_or(display);
    if shown == bare || display == bare || shown == target {
        return escaped(target, false);
    }
    let last = bare.rsplit(['.', ':']).next().unwrap_or(bare);
    if shown == last {
        return format!("~{}", escaped(target, false));
    }
    format!("{} <{}>", escaped(display, true), escaped(target, false))
}
