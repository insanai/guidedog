//! The body directives: admonitions, topics, sidebars, code, math, containers, and the
//! document-level directives (contents, sectnum, header, footer, meta, title).

use guidedoc::{AttributeKey, Fail, NodeFlags, NodeKind, arg_span, arg_text};

use crate::directives::Directive;
use crate::fields::field_marker;
use crate::lines::{Line, join_lines};
use crate::messages::{DIRECTIVE_CONTEXT, DIRECTIVE_OPTION_INVALID, META_INVALID};
use crate::names::{parse_int, write_id};
use crate::parser::Parser;
use crate::specs::DirKind;

impl Parser<'_, '_> {
    fn context_problem(&mut self, d: &Directive<'_>, why: &'static str) {
        self.report(DIRECTIVE_CONTEXT, d.name_span, arg_span(d.name_span), arg_text(why));
    }

    pub(crate) fn admonition(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let spec = d.standard();
        let id = self.open(NodeKind::Admonition, d.span)?;
        self.ctx.b.set_value(id, spec.map_or(0, |s| s.value as i32));
        self.add_common(id, d, "class")?;
        if spec.is_some_and(|s| s.kind == DirKind::GenericAdmonition) {
            if !d.has_option("class") {
                let mut class = String::from("admonition-");
                write_id(&mut class, &d.argument());
                self.ctx.b.add_attribute_string(id, AttributeKey::Class, &class)?;
            }
            self.inline_node(NodeKind::Title, &d.arg)?;
        }
        self.parse_body(&d.content, false, None)?;
        self.close(id)
    }

    pub(crate) fn topic(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        if !d.sections && self.ctx.b.current_kind() != NodeKind::Sidebar {
            self.context_problem(d, "a topic may only appear at the top level or in a sidebar");
            return Ok(());
        }
        let id = self.open(NodeKind::Topic, d.span)?;
        self.add_common(id, d, "class")?;
        self.inline_node(NodeKind::Title, &d.arg)?;
        self.parse_body(&d.content, false, None)?;
        self.close(id)
    }

    pub(crate) fn sidebar(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        if self.ctx.b.current_kind() == NodeKind::Sidebar {
            self.context_problem(d, "a sidebar may not be nested in a sidebar");
            return Ok(());
        }
        let id = self.open(NodeKind::Sidebar, d.span)?;
        self.add_common(id, d, "class")?;
        self.inline_node(NodeKind::Title, &d.arg)?;
        if let Some(lines) = d.option_lines("subtitle") {
            self.inline_node(NodeKind::Subtitle, lines)?;
        }
        self.parse_body(&d.content, false, None)?;
        self.close(id)
    }

    pub(crate) fn parsed_literal(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let id = self.open(NodeKind::Preformatted, d.span)?;
        self.add_common(id, d, "class")?;
        self.inline(&d.content)?;
        self.close(id)
    }

    /// Sets a code block's first line number from `:number-lines:`.
    pub(crate) fn number_lines(&mut self, id: guidedoc::NodeId, d: &Directive<'_>) {
        if let Some(v) = d.option_value("number-lines") {
            let n = if v.is_empty() { 1 } else { parse_int(&v).clamp(1, i64::from(i32::MAX)) };
            self.ctx.b.set_value(id, n as i32);
        }
    }

    pub(crate) fn code_directive(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let id = self.open(NodeKind::CodeBlock, d.span)?;
        if !d.arg.is_empty() {
            let language = self.pool_lines(&d.arg, " ")?;
            self.ctx.b.add_attribute(id, AttributeKey::Language, language)?;
        }
        self.add_common(id, d, "class")?;
        self.number_lines(id, d);
        let text = self.code_text(&d.content)?;
        self.ctx.b.set_text(id, text);
        self.close(id)
    }

    /// Emits one MathBlock per blank-line separated group; the argument, when present,
    /// is the first group.
    pub(crate) fn math_directive(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let mut first = true;
        if !d.arg.is_empty() {
            let arg = if d.arg_block.is_empty() { &d.arg } else { &d.arg_block };
            self.math_block(d, arg, first)?;
            first = false;
        }
        for group in d.content.split(|l| l.is_blank()).filter(|g| !g.is_empty()) {
            self.math_block(d, group, first)?;
            first = false;
        }
        Ok(())
    }

    fn math_block(
        &mut self,
        d: &Directive<'_>,
        lines: &[Line<'_>],
        first: bool,
    ) -> Result<(), Fail> {
        let id = self.open(NodeKind::MathBlock, self.lines_span(lines))?;
        if let Some(o) = d.option("class")
            && let Some(t) = self.pool_classes(&join_lines(&o.lines, " "))?
        {
            self.ctx.b.add_attribute(id, AttributeKey::Class, t)?;
        }
        if let Some(o) = d.option("name").filter(|_| first) {
            let name = self.pool_name(&join_lines(&o.lines, " "))?;
            self.ctx.b.add_attribute(id, AttributeKey::Name, name)?;
        }
        let text = self.pool_lines(lines, "\n")?;
        self.ctx.b.set_text(id, text);
        self.close(id)
    }

    pub(crate) fn rubric(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let id = self.open(NodeKind::Rubric, d.span)?;
        self.add_common(id, d, "class")?;
        self.inline(&d.arg)?;
        self.close(id)
    }

    pub(crate) fn body_container(
        &mut self,
        d: &Directive<'_>,
        kind: NodeKind,
    ) -> Result<(), Fail> {
        let id = self.open(kind, d.span)?;
        self.add_common(id, d, "class")?;
        self.parse_body(&d.content, false, None)?;
        self.close(id)
    }

    pub(crate) fn container(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let mut classes = None;
        if !d.arg.is_empty() {
            classes = self.pool_classes(&d.argument())?;
            if classes.is_none() {
                let span = self.lines_span(&d.arg);
                let why = arg_text("expected class names");
                self.report(DIRECTIVE_OPTION_INVALID, span, arg_span(span), why);
                return Ok(());
            }
        }
        let id = self.open(NodeKind::Container, d.span)?;
        if let Some(classes) = classes {
            self.ctx.b.add_attribute(id, AttributeKey::Class, classes)?;
        }
        self.add_common(id, d, "class")?;
        self.parse_body(&d.content, false, None)?;
        self.close(id)
    }

    pub(crate) fn contents(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let id = self.open(NodeKind::Contents, d.span)?;
        if !d.arg.is_empty() {
            let title = self.pool_lines(&d.arg, " ")?;
            self.ctx.b.add_attribute(id, AttributeKey::Title, title)?;
        }
        self.add_common(id, d, "class")?;
        if let Some(v) = d.option_value("backlinks") {
            self.custom(id, "backlinks", &v)?;
        }
        if let Some(v) = d.option_value("depth") {
            self.ctx.b.set_value(id, parse_int(&v).clamp(0, i64::from(i32::MAX)) as i32);
        }
        if d.has_option("local") {
            self.ctx.b.add_flags(id, NodeFlags::LOCAL);
        }
        self.close(id)
    }

    pub(crate) fn sectnum(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let id = self.directive_node(d, "sectnum")?;
        for name in ["depth", "prefix", "suffix", "start"] {
            if let Some(v) = d.option_value(name) {
                self.custom(id, name, &v)?;
            }
        }
        self.close(id)
    }

    pub(crate) fn header_footer(&mut self, d: &Directive<'_>, kind: DirKind) -> Result<(), Fail> {
        let name = if kind == DirKind::Header { "header" } else { "footer" };
        let id = self.directive_node(d, name)?;
        self.parse_body(&d.content, false, None)?;
        self.close(id)
    }

    pub(crate) fn target_notes(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let id = self.directive_node(d, "target-notes")?;
        self.add_common(id, d, "class")?;
        self.close(id)
    }

    pub(crate) fn title_directive(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let id = self.directive_node(d, "title")?;
        let title = self.pool_lines(&d.arg, " ")?;
        self.ctx.b.add_attribute(id, AttributeKey::Title, title)?;
        self.close(id)
    }

    /// Reads a field list of meta data into Custom attributes: field name = value.
    pub(crate) fn meta(&mut self, d: &Directive<'_>) -> Result<(), Fail> {
        let fields = meta_fields(&d.content);
        if let Err(bad) = fields {
            let span = self.line_span(&d.content[bad]);
            self.report_at(META_INVALID, span);
            return Ok(());
        }
        let id = self.directive_node(d, "meta")?;
        for (k, e) in fields.unwrap_or_default() {
            let lines = &d.content[k..e];
            let Some((name_end, col)) = field_marker(lines[0].text) else {
                continue;
            };
            let mut value = lines[0].text[col..].to_owned();
            for l in &lines[1..] {
                value.push(' ');
                value.push_str(l.trim().text);
            }
            self.ctx.b.add_custom_attribute(id, &lines[0].text[1..name_end], &value)?;
        }
        self.close(id)
    }
}

/// Splits meta content into fields (line ranges); `Err` with the line that is not a
/// field with a value.
fn meta_fields(lines: &[Line<'_>]) -> Result<Vec<(usize, usize)>, usize> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < lines.len() {
        if lines[k].is_blank() {
            k += 1;
            continue;
        }
        let mut e = k + 1;
        while e < lines.len() && lines[e].is_indented() {
            e += 1;
        }
        match field_marker(lines[k].text) {
            Some((_, col)) if col < lines[k].text.len() || e > k + 1 => out.push((k, e)),
            _ => return Err(k),
        }
        k = e;
    }
    Ok(out)
}
