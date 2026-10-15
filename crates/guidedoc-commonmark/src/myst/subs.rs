//! Substitutions ("substitution" syntax): `{{ expression }}` is replaced by the Markdown
//! its expression gives, parsed inline, or as blocks when it fills a paragraph. Values
//! come from `Settings::substitutions` and the front matter's substitutions (which win).
//! Expressions other than plain names go to `eval.rs`.

use guidedoc::{Fail, SourceId, SourceSpan, arg_static};

use super::eval::evaluate;
use super::trim_space;
use super::yaml::{Yaml, YamlKind};
use crate::blocks::{parse_lines, split_source};
use crate::host::Host;
use crate::messages::{SUBSTITUTION_CIRCULAR, SUBSTITUTION_EXPRESSION, SUBSTITUTION_UNKNOWN};

/// How deeply substitutions may expand into each other.
const MAX_SUB_DEPTH: usize = 16;

/// A substitution's name and value, and where the value is written when it is in a
/// source (registered on first use otherwise).
#[derive(Debug, Clone, Default)]
pub(crate) struct SubEntry {
    pub name: String,
    pub value: String,
    pub source: Option<SourceId>,
    pub offset: u32,
}

/// The substitutions of a read, and the names being expanded.
#[derive(Debug, Clone, Default)]
pub(crate) struct Substitutions {
    pub items: Vec<SubEntry>,
    active: Vec<String>,
}

impl Substitutions {
    pub(crate) fn add(&mut self, name: &str, value: &str, source: Option<SourceId>, offset: u32) {
        self.items.push(SubEntry {
            name: name.to_owned(),
            value: value.to_owned(),
            source,
            offset,
        });
    }

    /// The index of a substitution; the last definition wins.
    pub(crate) fn find(&self, name: &str) -> Option<usize> {
        self.items.iter().rposition(|e| e.name == name)
    }
}

/// Adds a front matter mapping, nested mappings under dotted names.
pub(crate) fn add_substitutions(h: &mut Host<'_, '_>, y: &Yaml, mapping: usize, prefix: &str) {
    if y.nodes[mapping].kind != YamlKind::Mapping {
        return;
    }
    let main = h.ctx.source_of(h.ctx.source);
    for c in y.children(mapping) {
        let n = &y.nodes[c];
        let name = if prefix.is_empty() { n.key.clone() } else { format!("{prefix}.{}", n.key) };
        match n.kind {
            YamlKind::Mapping => add_substitutions(h, y, c, &name),
            YamlKind::Scalar => {
                let at = n.offset as usize;
                let in_source = main.get(at..at + n.text.len()) == Some(n.text.as_str());
                let source = in_source.then_some(h.ctx.source);
                h.ms.subs.add(&name, &n.text, source, n.offset);
            }
            YamlKind::Null => h.ms.subs.add(&name, "", None, 0),
            YamlKind::Sequence => {}
        }
    }
}

/// Emits the substitution `{{ expr }}` at `span`; `block` is set when it fills a
/// paragraph, whose place it takes with blocks.
pub(crate) fn expand_substitution(
    h: &mut Host<'_, '_>,
    expr: &str,
    span: SourceSpan,
    block: bool,
) -> Result<(), Fail> {
    let name = trim_space(expr);
    let (value, source, offset) = match h.ms.subs.find(name) {
        Some(i) => registered(h, i),
        None => match evaluate(h, name) {
            Ok(Some(value)) => match h.ctx.add_source("<substitution>", &value) {
                Some(id) => (value, Some(id), 0),
                None => return Ok(()),
            },
            Ok(None) => {
                h.report_span(SUBSTITUTION_UNKNOWN, span);
                return Ok(());
            }
            Err(why) => {
                h.report(SUBSTITUTION_EXPRESSION, span, guidedoc::arg_span(span), arg_static(why));
                return Ok(());
            }
        },
    };
    let Some(source) = source else { return Ok(()) };
    if h.ms.subs.active.iter().any(|a| a == name) {
        h.report_span(SUBSTITUTION_CIRCULAR, span);
        return Ok(());
    }
    if h.ms.subs.active.len() >= MAX_SUB_DEPTH {
        return Ok(());
    }
    let lines = split_source(&value, offset);
    h.ms.subs.active.push(name.to_owned());
    let result = if block {
        parse_lines(h, &lines, source, false, false)
    } else {
        super::inline_lines(h, &lines, source)
    };
    h.ms.subs.active.pop();
    result
}

/// The value of substitution `i`, registering it as a source when it is not in one.
fn registered(h: &mut Host<'_, '_>, i: usize) -> (String, Option<SourceId>, u32) {
    let e = h.ms.subs.items[i].clone();
    if let Some(source) = e.source {
        return (e.value, Some(source), e.offset);
    }
    let id = h.ctx.add_source("<substitution>", &e.value);
    let entry = &mut h.ms.subs.items[i];
    entry.source = id;
    entry.offset = 0;
    (e.value, id, 0)
}
