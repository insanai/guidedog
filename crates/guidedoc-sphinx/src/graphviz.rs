//! sphinx.ext.graphviz is built in. graphviz, graph, and digraph become a Code_Block
//! with Language "dot" whose text is the complete DOT source: the content, the file the
//! argument names (read through `include_source`, like literalinclude), or for graph and
//! digraph the content wrapped as "graph NAME {...}". Custom "graphviz" = "1", "layout"
//! (the engine; "" means dot), "alt", "align", and "caption"; :name: and :class: as for
//! code blocks. The project host renders the graph.

use guidedoc::{AttributeKey, Fail, NodeKind, SourceUse, arg_span, arg_static};
use guidedoc_rst::{Directive, OptSpec, OptType, Parser, choice, opt, path_of};

use crate::code::caption_child;
use crate::directives::{class_attribute, value_attribute};
use crate::messages::GRAPHVIZ_SOURCE;
use crate::text::trim_space;

/// The extensions conf.toml may name: those this reader implements natively.
pub const EXTENSIONS: [&str; 7] = [
    "sphinx.ext.mathjax",
    "sphinx.ext.todo",
    "sphinx.ext.githubpages",
    "sphinx.ext.ifconfig",
    "sphinx.ext.extlinks",
    "sphinx.ext.graphviz",
    "sphinx.ext.doctest",
];

/// Tells whether conf.toml's extensions may name `ext`: a built-in extension this
/// reader implements natively.
pub fn extension_supported(ext: &str) -> bool {
    EXTENSIONS.contains(&ext)
}

/// The options of the graphviz directives.
pub(crate) const OPTS_GRAPHVIZ: &[OptSpec] = &[
    opt("alt", OptType::Text),
    choice("align", "left center right"),
    opt("caption", OptType::Text),
    opt("layout", OptType::Text),
    opt("graphviz_dot", OptType::Text),
    opt("name", OptType::Text),
    opt("class", OptType::Class),
];

/// Runs graphviz, graph, or digraph (`name`).
pub(crate) fn graphviz_directive(
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
    name: &str,
) -> Result<(), Fail> {
    let wrap = name != "graphviz";
    let mut file = None;
    if !wrap && !d.arg.is_empty() {
        if !d.content.is_empty() {
            let why = arg_static("it has both content and a file argument");
            p.report(GRAPHVIZ_SOURCE, d.name_span, arg_span(d.name_span), why);
            return Ok(());
        }
        let from = p.src();
        let Some(id) = p.ctx.include_source(from, &path_of(&d.arg), d.span, SourceUse::Literal)
        else {
            return Ok(());
        };
        file = Some(p.ctx.source_of(id));
    } else if d.content.is_empty() {
        let why = arg_static("it has neither content nor a file argument");
        p.report(GRAPHVIZ_SOURCE, d.name_span, arg_span(d.name_span), why);
        return Ok(());
    }
    let id = p.open(NodeKind::CodeBlock, d.span)?;
    p.ctx.b.add_attribute_string(id, AttributeKey::Language, "dot")?;
    class_attribute(p, id, d)?;
    p.custom(id, "graphviz", "1")?;
    let layout = d.option_value("layout").or_else(|| d.option_value("graphviz_dot"));
    p.custom(id, "layout", layout.as_deref().unwrap_or(""))?;
    for option in ["alt", "align", "caption", "name"] {
        value_attribute(p, id, d, option)?;
    }
    caption_child(p, d, &[])?;
    let start = p.ctx.b.text_open();
    match &file {
        _ if wrap => {
            let arg = d.argument();
            p.ctx.b.text_push(&format!("{name} {} {{\n", trim_space(&arg)))?;
            p.push_lines(&d.content, "\n")?;
            p.ctx.b.text_push("\n}\n")?;
        }
        Some(text) if !text.is_empty() => {
            let text = text.strip_prefix('\u{feff}').unwrap_or(text);
            p.ctx.b.text_push(text)?;
            if !text.is_empty() && !text.ends_with('\n') {
                p.ctx.b.text_push("\n")?;
            }
        }
        _ => {
            p.push_lines(&d.content, "\n")?;
            p.ctx.b.text_push("\n")?;
        }
    }
    let text = p.ctx.b.text_close(start);
    p.ctx.b.set_text(id, text);
    p.close(id)
}
