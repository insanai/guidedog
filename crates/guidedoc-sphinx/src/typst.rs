//! The Guidedog Typst diagram directive. Parsing never executes Typst.

use guidedoc::{AttributeKey, Fail, NodeKind, SourceUse, arg_span, arg_static};
use guidedoc_rst::{Directive, OptSpec, OptType, Parser, choice, opt, path_of};

use crate::code::caption_child;
use crate::directives::{class_attribute, value_attribute};
use crate::messages::TYPST_SOURCE;

pub(crate) const OPTIONS: &[OptSpec] = &[
    opt("alt", OptType::Text),
    choice("align", "left center right"),
    opt("caption", OptType::Text),
    opt("name", OptType::Text),
    opt("class", OptType::Class),
    choice("format", "svg png"),
];

pub(crate) fn diagram(p: &mut Parser<'_, '_>, d: &Directive<'_>) -> Result<(), Fail> {
    if d.arg.is_empty() == d.content.is_empty() {
        p.report(TYPST_SOURCE, d.name_span, arg_span(d.name_span), arg_static(""));
        return Ok(());
    }
    let mut source = p.src();
    let mut file = None;
    if !d.arg.is_empty() {
        let from = p.src();
        let Some(id) = p.ctx.include_source(from, &path_of(&d.arg), d.span, SourceUse::Literal)
        else {
            return Ok(());
        };
        source = id;
        file = Some(p.ctx.source_of(id));
    }
    let id = p.open(NodeKind::CodeBlock, d.span)?;
    p.ctx.b.add_attribute_string(id, AttributeKey::Language, "typst")?;
    p.custom(id, "typst-diagram", "1")?;
    // The source table survives object caching, but the node span names the directive.
    let origin = p.ctx.source_entry(source).map(|entry| entry.name.clone()).unwrap_or_default();
    p.custom(id, "typst-source", &origin)?;
    if !d.arg.is_empty() {
        p.custom(id, "typst-file", &path_of(&d.arg))?;
    }
    class_attribute(p, id, d)?;
    for option in ["alt", "align", "caption", "name", "format"] {
        value_attribute(p, id, d, option)?;
    }
    caption_child(p, d, &[])?;
    let start = p.ctx.b.text_open();
    match file {
        Some(text) => p.ctx.b.text_push(text.strip_prefix('\u{feff}').unwrap_or(&text))?,
        None => p.push_lines(&d.content, "\n")?,
    }
    p.ctx.b.text_push("\n")?;
    let text = p.ctx.b.text_close(start);
    p.ctx.b.set_text(id, text);
    p.close(id)
}
