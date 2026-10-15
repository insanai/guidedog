//! The image and figure directives.

use guidedoc::{
    AttributeKey, Fail, INVALID_NODE, LinkInfo, LinkKind, NodeId, NodeKind, SourceSpan, Text,
};
use guidedoc::{arg_span, arg_text};

use crate::directives::Directive;
use crate::lines::{Line, is_space, join_lines, text_end, trim_blank_start, trim_space};
use crate::messages::{DIRECTIVE_OPTION_INVALID, FIGURE_CAPTION};
use crate::names::{is_reference, uri_string};
use crate::parser::{Construct, Parser, detect, is_adornment};

/// What class directives and internal targets before a figure's caption give it: they
/// apply to the caption, as they would to any next element.
#[derive(Debug, Clone, Copy, Default)]
struct CaptionPrefix<'s> {
    skip: usize,
    classes: &'s str,
    name: &'s str,
}

/// Reads one-line ".. class:: names" and ".. _name:" lines (and blank lines between
/// them) at the start of a figure's content.
fn caption_prefix<'s>(content: &[Line<'s>]) -> CaptionPrefix<'s> {
    let mut c = CaptionPrefix::default();
    for (k, l) in content.iter().enumerate() {
        let t = l.text;
        if l.is_blank() {
            // Blank lines between them belong to the prefix.
        } else if let Some(rest) = t.strip_prefix(".. class:: ").filter(|r| !r.is_empty()) {
            if !c.classes.is_empty() {
                return c;
            }
            c.classes = trim_space(rest);
        } else if t.len() > 5
            && t.starts_with(".. _")
            && t.ends_with(':')
            && !t[4..].starts_with('_')
        {
            if !c.name.is_empty() {
                return c;
            }
            let name = &t[4..t.len() - 1];
            c.name = match name.strip_prefix('`').and_then(|n| n.strip_suffix('`')) {
                Some(inner) if name.len() > 1 => inner,
                _ => name,
            };
        } else {
            return c;
        }
        c.skip = k + 1;
    }
    c
}

fn caption_ok(content: &[Line<'_>]) -> bool {
    if detect(content, 0) != Construct::Text {
        return false;
    }
    match content.get(1) {
        Some(next) if !next.is_blank() => {
            !next.is_indented() && !(is_adornment(next.text) && next.text.len() >= 4)
        }
        _ => true,
    }
}

impl Parser<'_, '_> {
    /// Opens the Link (URI) or Reference (name_) that `:target:` wraps an image in;
    /// `None` without the option.
    pub(crate) fn open_image_target(
        &mut self,
        d: &Directive<'_>,
        span: SourceSpan,
    ) -> Result<Option<NodeId>, Fail> {
        let Some(o) = d.option("target") else {
            return Ok(None);
        };
        let mut info = LinkInfo { resolved: INVALID_NODE, ..LinkInfo::default() };
        let joined = join_lines(&o.lines, " ");
        let kind = if let Some(name) = is_reference(&joined) {
            info.kind = LinkKind::Unresolved;
            info.name = self.pool_name(name)?;
            NodeKind::Reference
        } else {
            info.kind = LinkKind::External;
            info.destination = self.ctx.b.add_text(&uri_string(&o.lines))?;
            NodeKind::Link
        };
        let id = self.ctx.b.begin_node(kind, span)?;
        self.ctx.b.set_link(id, info)?;
        Ok(Some(id))
    }

    /// Emits an Image with its attributes and a link row holding the URI. In a figure,
    /// `:align:` and `:name:` belong to the figure instead.
    pub(crate) fn emit_image(
        &mut self,
        d: &Directive<'_>,
        span: SourceSpan,
        in_figure: bool,
    ) -> Result<(), Fail> {
        let id = self.open(NodeKind::Image, span)?;
        if let Some(lines) = d.option_lines("alt") {
            let alt = self.pool_lines(lines, " ")?;
            self.ctx.b.add_attribute(id, AttributeKey::Alt, alt)?;
        } else if d.subst && !d.subst_name.is_empty() {
            // An image in a substitution definition is described by the name.
            self.ctx.b.add_attribute_string(id, AttributeKey::Alt, &d.subst_name)?;
        }
        for (name, key) in [("width", AttributeKey::Width), ("height", AttributeKey::Height)] {
            if let Some(v) = d.option_value(name) {
                let t = self.length_text(&v, "")?;
                self.ctx.b.add_attribute(id, key, t)?;
            }
        }
        if let Some(v) = d.option_value("scale") {
            let v = v.strip_suffix('%').map_or(&*v, trim_space).to_owned();
            self.ctx.b.add_attribute_string(id, AttributeKey::Scale, &v)?;
        }
        if let Some(v) = d.option_value("align").filter(|_| !in_figure) {
            self.ctx.b.add_attribute_string(id, AttributeKey::Align, &v)?;
        }
        if let Some(v) = d.option_value("loading") {
            self.custom(id, "loading", &v)?;
        }
        if let Some(o) = d.option("class")
            && let Some(t) = self.pool_classes(&join_lines(&o.lines, " "))?
        {
            self.ctx.b.add_attribute(id, AttributeKey::Class, t)?;
        }
        if let Some(o) = d.option("name").filter(|_| !in_figure) {
            let name = self.pool_name(&join_lines(&o.lines, " "))?;
            self.ctx.b.add_attribute(id, AttributeKey::Name, name)?;
        }
        let destination = self.ctx.b.add_text(&uri_string(&d.arg))?;
        let info = LinkInfo {
            destination,
            resolved: INVALID_NODE,
            kind: LinkKind::External,
            ..LinkInfo::default()
        };
        self.ctx.b.set_link(id, info)?;
        self.close(id)
    }

    /// Rejects vertical alignments outside substitution definitions, and horizontal
    /// ones inside them.
    pub(crate) fn check_image_align(&mut self, d: &Directive<'_>) -> bool {
        let (Some(v), Some(o)) = (d.option_value("align"), d.option("align")) else {
            return true;
        };
        let vertical = matches!(&*v, "top" | "middle" | "bottom");
        if vertical == d.subst {
            return true;
        }
        let why = if vertical {
            "top, middle, and bottom apply only to images in substitutions"
        } else {
            "images in substitutions align top, middle, or bottom"
        };
        self.report(DIRECTIVE_OPTION_INVALID, o.span, arg_span(o.span), arg_text(why));
        false
    }

    /// Emits a block image. A `:target:` link cannot stand at block level, so a
    /// targeted image is wrapped in a paragraph.
    pub(crate) fn image_directive(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        if !self.check_image_align(d) {
            return Ok(());
        }
        if !d.has_option("target") {
            return self.emit_image(d, d.span, false);
        }
        let para = self.ctx.b.begin_node(NodeKind::Paragraph, d.span)?;
        let link = self.open_image_target(d, d.span)?;
        self.emit_image(d, d.span, false)?;
        if let Some(link) = link {
            self.close(link)?;
        }
        self.close(para)
    }

    pub(crate) fn figure(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        if !self.check_image_align(d) {
            return Ok(());
        }
        let prefix = caption_prefix(&d.content);
        let mut content: &[Line<'_>] = &d.content;
        if !prefix.classes.is_empty() || !prefix.name.is_empty() {
            content = &content[prefix.skip..];
        }
        let (mut caption, mut legend): (&[Line<'_>], &[Line<'_>]) = (&[], &[]);
        if !content.is_empty() {
            if content[0].text == ".." && content.get(1).is_none_or(|l| l.is_blank()) {
                legend = trim_blank_start(&content[1..]);
            } else if caption_ok(content) {
                let (end, _) = text_end(content, 0, false);
                caption = &content[..end];
                legend = trim_blank_start(&content[end..]);
            } else {
                let span = self.line_span(&content[0]);
                self.report_at(FIGURE_CAPTION, span);
                return Ok(());
            }
        }
        let fig = self.open(NodeKind::Figure, d.span)?;
        self.figure_attributes(fig, d)?;
        let target = self.open_image_target(d, d.span)?;
        self.emit_image(d, d.span, true)?;
        if let Some(target) = target {
            self.close(target)?;
        }
        self.emit_caption(caption, prefix)?;
        if !legend.is_empty() {
            let leg = self.ctx.b.begin_node(NodeKind::Legend, self.lines_span(legend))?;
            self.parse_body(legend, false, None)?;
            self.close(leg)?;
        }
        self.close(fig)
    }

    fn figure_attributes(&mut self, fig: NodeId, d: &Directive<'_>) -> Result<(), Fail> {
        match d.option_value("figwidth") {
            Some(v) if v != "image" => {
                let t = self.length_text(&v, "px")?;
                self.ctx.b.add_attribute(fig, AttributeKey::Width, t)?;
            }
            // The figure takes the width of its image; renderers size it from the image.
            Some(_) => self.custom(fig, "figwidth", "image")?,
            None => {}
        }
        self.add_common(fig, d, "figclass")?;
        if let Some(v) = d.option_value("align") {
            self.ctx.b.add_attribute_string(fig, AttributeKey::Align, &v)?;
        }
        Ok(())
    }

    fn emit_caption(
        &mut self,
        caption: &[Line<'_>],
        prefix: CaptionPrefix<'_>,
    ) -> Result<(), Fail> {
        if caption.is_empty() {
            return Ok(());
        }
        let id = self.ctx.b.begin_node(NodeKind::Caption, self.lines_span(caption))?;
        if !prefix.classes.is_empty()
            && let Some(t) = self.pool_classes(prefix.classes)?
        {
            self.ctx.b.add_attribute(id, AttributeKey::Class, t)?;
        }
        if !prefix.name.is_empty() {
            let name = self.pool_name(prefix.name)?;
            self.ctx.b.add_attribute(id, AttributeKey::Name, name)?;
        }
        self.inline(caption)?;
        self.close(id)
    }

    /// Writes a length without the white space between number and unit, as the
    /// specification normalizes it; a number without a unit gets `unit`.
    pub(crate) fn length_text(&mut self, v: &str, unit: &str) -> Result<Text, Fail> {
        let mut out: String =
            v.chars().filter(|&c| !(c.is_ascii() && is_space(c as u8))).collect();
        let last = v.as_bytes().last().copied().unwrap_or(0);
        if !unit.is_empty() && (last.is_ascii_digit() || last == b'.') {
            out.push_str(unit);
        }
        self.ctx.b.add_text(&out)
    }
}
