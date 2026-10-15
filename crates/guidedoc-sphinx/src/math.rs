//! The math directive: one Math_Block for the whole directive, as Sphinx makes one
//! displaymath node; the argument, when present, is the first equation group, and groups
//! stay separated by blank lines in the text. A `:label:` (or `:name:`) gives Id
//! "equation-<label>" in Docutils' id form; an empty `:label:`, and math_number_all for
//! every equation without one, labels it "docname:N".

use guidedoc::{AttributeKey, Fail, NodeKind};
use guidedoc_rst::{Directive, Parser, trim_blank_end};

use crate::directives::class_attribute;
use crate::extension::Sphinx;
use crate::ids::docutils_id;

/// Runs the math directive.
pub(crate) fn math_directive(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
) -> Result<(), Fail> {
    let given = d.option_value("label").or_else(|| d.option_value("name"));
    let id = p.open(NodeKind::MathBlock, d.span)?;
    class_attribute(p, id, d)?;
    let mut label = given.as_deref().unwrap_or("").to_owned();
    if label.is_empty() && (given.is_some() || sx.config.math_number_all) {
        let mut st = sx.state();
        label = format!("{}:{}", sx.config.docname, st.equation_serial);
        st.equation_serial += 1;
    }
    if !label.is_empty() {
        let eq = docutils_id("equation-", &label);
        p.ctx.b.add_attribute_string(id, AttributeKey::Id, &eq)?;
        sx.state().note_id(&eq);
        p.custom(id, "label", &label)?;
    }
    if let Some(v) = d.option_value("name").filter(|v| !v.is_empty()) {
        p.ctx.b.add_attribute_string(id, AttributeKey::Name, &v)?;
    }
    if d.has_option("nowrap") || d.has_option("no-wrap") {
        p.custom(id, "nowrap", "1")?;
    }
    let start = p.ctx.b.text_open();
    p.push_lines(&d.arg, "\n")?;
    let content = trim_blank_end(&d.content);
    if !d.arg.is_empty() && !content.is_empty() {
        p.ctx.b.text_push("\n\n")?;
    }
    p.push_lines(content, "\n")?;
    let text = p.ctx.b.text_close(start);
    p.ctx.b.set_text(id, text);
    p.close(id)
}
