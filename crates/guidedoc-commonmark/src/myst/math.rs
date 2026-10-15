//! Display math: `$$ ... $$ (label)` blocks and amsmath environments, emitted through
//! the math directive when there is an extension (so labels, numbering, and ids are
//! Sphinx's), else as a MathBlock.

use guidedoc::{AttributeKey, Fail, NodeKind};

use super::blocks::{amsmath_begin, math_label};
use super::directive::{Call, run_directive};
use super::{trim_right, trim_space};
use crate::blocks::{Bid, Skeleton};
use crate::emit::open_block;
use crate::extension::{DirectiveOption, Line};
use crate::host::Host;

/// Emits a math block.
pub(crate) fn emit_math_block(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    id: Bid,
) -> Result<(), Fail> {
    let blk = sk.blocks[id];
    let dollar = blk.level == 1;
    let mut lines = sk.block_lines(id, 0);
    let mut label = "";
    if let Some(last) = lines.last_mut().filter(|_| dollar) {
        let (body, l) = math_label(h.settings.dmath_allow_labels, trim_right(last.text));
        if body.ends_with("$$") {
            last.text = &last.text[..body.len() - 2];
            label = l;
        }
    }
    let mut lines = &lines[..];
    while lines.first().is_some_and(|l| trim_space(l.text).is_empty()) {
        lines = &lines[1..];
    }
    let mut owned = lines.to_vec();
    if dollar && !owned.is_empty() {
        let first = &mut owned[0];
        let lead = first.text.len() - first.text.trim_start_matches(' ').len();
        first.text = &first.text[lead..];
        first.offset += lead as u32;
        if let Some(last) = owned.last_mut() {
            last.text = trim_right(last.text);
        }
    }
    while owned.last().is_some_and(|l| trim_space(l.text).is_empty()) {
        owned.pop();
    }
    let span = sk.block_span(id);
    if h.ext().is_some() {
        return math_directive(h, sk, id, &owned, label);
    }
    let node = open_block(h, NodeKind::MathBlock, span)?;
    let b = h.b();
    if !dollar {
        b.add_attribute_string(node, AttributeKey::Class, "amsmath")?;
        b.add_custom_attribute(node, "nowrap", "1")?;
    }
    if !label.is_empty() {
        b.add_custom_attribute(node, "label", label)?;
    }
    let start = b.text_open();
    for (k, l) in owned.iter().enumerate() {
        if k > 0 {
            b.text_push_byte(b'\n')?;
        }
        b.text_push(l.text)?;
    }
    let text = b.text_close(start);
    b.set_text(node, text);
    b.end_node(node)
}

/// Runs the math directive for a math block: a label for `$$...$$ (label)`; for
/// amsmath, nowrap, class amsmath, and a label for numbered environments.
fn math_directive(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    id: Bid,
    lines: &[Line<'_>],
    label: &str,
) -> Result<(), Fail> {
    let span = sk.block_span(id);
    let at = sk.blocks[id].start;
    let mut name = label.to_owned();
    let mut extra: Vec<DirectiveOption<'_>> = Vec::new();
    if sk.blocks[id].level == 2 {
        let env = lines.first().and_then(|l| amsmath_begin(l.text)).unwrap_or("");
        if !env.is_empty() && !env.ends_with('*') {
            h.ms.equations += 1;
            name = format!("amsmath-{}", h.ms.equations);
        }
        extra.push(DirectiveOption { name: "nowrap", value: Vec::new(), span });
        let class = vec![Line { text: "amsmath", offset: at }];
        extra.push(DirectiveOption { name: "class", value: class, span });
    }
    if !name.is_empty() {
        let value = vec![Line { text: &name, offset: at }];
        extra.push(DirectiveOption { name: "label", value, span });
    }
    let call =
        Call { name: "math", name_span: sk.span(at as usize, at as usize), span, top: false };
    let none = Line { text: "", offset: at };
    run_directive(h, sk, &call, none, lines, extra)
}
