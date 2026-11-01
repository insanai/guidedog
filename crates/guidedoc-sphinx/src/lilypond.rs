//! Retains music notation and exact body-to-document byte mappings. No engraving runs here.

use std::fmt::Write;

use guidedoc::{AttributeKey, Fail, NodeKind, arg_span, arg_static};
use guidedoc_rst::{Directive, OptSpec, OptType, Parser, choice, opt, path_of};

use crate::code::caption_child;
use crate::directives::{class_attribute, value_attribute};
use crate::messages::LILYPOND_SOURCE;

pub(crate) const OPTIONS: &[OptSpec] = &[
    opt("alt", OptType::Text),
    choice("align", "left center right"),
    opt("caption", OptType::Text),
    opt("name", OptType::Text),
    opt("class", OptType::Class),
    choice("format", "svg png"),
    opt("staff-size", OptType::Length),
    opt("width", OptType::LengthPercent),
    choice("justify", "auto all none"),
];

pub(crate) fn music(p: &mut Parser<'_, '_>, d: &Directive<'_>) -> Result<(), Fail> {
    if d.arg.is_empty() == d.content.is_empty() {
        p.report(LILYPOND_SOURCE, d.name_span, arg_span(d.name_span), arg_static(""));
        return Ok(());
    }
    let id = p.open(NodeKind::CodeBlock, d.span)?;
    p.ctx.b.add_attribute_string(id, AttributeKey::Language, "lilypond")?;
    p.custom(id, "lilypond", "1")?;
    let origin = p.ctx.source_entry(p.src()).map(|s| s.name.clone()).unwrap_or_default();
    p.custom(id, "lilypond-source", &origin)?;
    class_attribute(p, id, d)?;
    for name in ["alt", "align", "caption", "name", "format", "staff-size", "width", "justify"] {
        value_attribute(p, id, d, name)?;
    }
    let mut body = String::new();
    if d.arg.is_empty() {
        let mut mapping = String::new();
        for line in &d.content {
            // The raw view retains tabs and Unicode bytes after the reader's dedent.
            let view = line.view(p.tab());
            if !mapping.is_empty() {
                mapping.push(',');
            }
            let _ = write!(mapping, "{}", view.offset);
            body.push_str(view.text);
            body.push('\n');
        }
        let offset = d.content.first().map(|l| l.start(p.tab())).unwrap_or(0) as usize;
        let source = p.ctx.source_of(p.src());
        let prefix = source.get(..offset).unwrap_or("");
        let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
        let indent = prefix.rsplit('\n').next().map(str::chars).map(Iterator::count).unwrap_or(0);
        p.custom(id, "lilypond-line", &line.to_string())?;
        p.custom(id, "lilypond-indent", &indent.to_string())?;
        p.custom(id, "lilypond-map", &mapping)?;
    } else {
        // The trusted link pass opens the file. An untrusted parse never reads it.
        let file = path_of(&d.arg);
        p.custom(id, guidedoc::SCORE_FILE_ATTRIBUTE, &file)?;
        body.push_str(&file);
        body.push('\n');
    }
    caption_child(p, d, &[])?;
    let start = p.ctx.b.text_open();
    p.ctx.b.text_push(&body)?;
    let text = p.ctx.b.text_close(start);
    p.ctx.b.set_text(id, text);
    p.close(id)
}
