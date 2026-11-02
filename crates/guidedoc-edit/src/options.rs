//! Setting and removing directive options, checked against the directive's rules in
//! the Sphinx layer's catalog.

use guidedoc_rst::{OptSpec, OptType};

use crate::edit::{Plan, TextEdit};
use crate::error::EditError;
use crate::lines::{indent_of, line_at, line_end, line_end_trimmed, line_start, next_line};
use crate::map::{Block, BlockId, DirectiveParts, SourceMap};
use crate::ops::Cx;
use crate::shape;
use crate::{Format, Span};

/// Sets option `name` of block `id` to `value`, or removes it when `value` is `None`.
pub(crate) fn set_option(
    cx: &Cx<'_>,
    id: BlockId,
    name: &str,
    value: Option<&str>,
) -> Result<Plan, EditError> {
    let block = cx.map.block(id).ok_or_else(EditError::unknown_block)?;
    let parts = block.directive.as_ref().ok_or_else(|| {
        EditError::unsupported(&format!("options of a {}", block.kind.name()), block.span)
    })?;
    let text = cx.text;
    let existing = parts.options.iter().find(|o| o.name.text(text) == name);
    let edit = match (value, existing) {
        (None, Some(option)) => removal(text, option.span),
        (None, None) => return Err(missing_option(name, block.span)),
        (Some(value), _) => {
            let config = default_or(cx.options.sphinx);
            check_option(&config, parts.name.text(text), name, value, block.span)?;
            let line = option_line(name, value, parts.yaml_options);
            match existing {
                Some(option) => {
                    TextEdit { start: option.span.start, end: option.span.end, insert: line }
                }
                None => addition(cx, block, parts, &line)?,
            }
        }
    };
    let (want, owned_name) = (value.map(str::to_string), name.to_string());
    let index = id.index();
    let mut plan = Plan::new(vec![edit], shape::shape(cx.map), block.span);
    plan.check = Box::new(move |after: &SourceMap, new_text: &str| {
        let Some(parts) = after.blocks.get(index).and_then(|b| b.directive.as_ref()) else {
            return false;
        };
        let found = parts.options.iter().find(|o| o.name.text(new_text) == owned_name);
        match (&want, found) {
            (None, None) => true,
            (Some(v), Some(o)) => o.value.map_or("", |s| s.text(new_text)) == v.as_str(),
            _ => false,
        }
    });
    Ok(plan)
}

fn default_or(config: Option<&guidedoc_sphinx::Config>) -> guidedoc_sphinx::Config {
    config.cloned().unwrap_or_default()
}

fn option_line(name: &str, value: &str, yaml: bool) -> String {
    match (yaml, value.is_empty()) {
        (true, _) => format!("{name}: {value}").trim_end().to_string(),
        (false, true) => format!(":{name}:"),
        (false, false) => format!(":{name}: {value}"),
    }
}

/// The edit that removes an option's lines with their line break.
fn removal(text: &str, span: Span) -> TextEdit {
    let start = line_start(text, span.start);
    let end = line_end(text, span.end);
    if end < text.len() {
        TextEdit { start, end: end + 1, insert: String::new() }
    } else {
        // The last line of the text: remove the line break before it instead.
        TextEdit { start: start.saturating_sub(1), end, insert: String::new() }
    }
}

/// The edit that adds option line `line`: after the last option, or after the marker
/// line and the arguments, at the indentation options take.
fn addition(
    cx: &Cx<'_>,
    block: &Block,
    parts: &DirectiveParts,
    line: &str,
) -> Result<TextEdit, EditError> {
    let text = cx.text;
    let marker_line = line_start(text, block.span.start);
    if let Some(last) = parts.options.last() {
        let indent = &text[line_start(text, last.span.start)..last.span.start];
        let at = last.span.end;
        return Ok(TextEdit { start: at, end: at, insert: format!("\n{indent}{line}") });
    }
    let content_on_marker =
        parts.content.is_some_and(|c| line_start(text, c.start) == marker_line);
    if content_on_marker {
        return Err(EditError::unsupported(
            "an option of a directive whose content starts on its first line",
            block.span,
        ));
    }
    let after = match (cx.map.format, parts.yaml_options) {
        (Format::Markdown, true) => next_line(text, block.span.start),
        _ => parts.arguments.map_or(block.span.start, |a| a.end),
    };
    let at = line_end_trimmed(text, after);
    let marker_indent = indent_of(line_at(text, marker_line));
    let indent = match cx.map.format {
        Format::Markdown => " ".repeat(marker_indent),
        Format::Rst => match parts.content {
            Some(content) => text[line_start(text, content.start)..content.start].to_string(),
            None => " ".repeat(marker_indent + 3),
        },
    };
    Ok(TextEdit { start: at, end: at, insert: format!("\n{indent}{line}") })
}

/// Refuses an option the directive does not take, or a value its type refuses. A
/// directive the catalog does not know takes any option.
fn check_option(
    config: &guidedoc_sphinx::Config,
    directive: &str,
    name: &str,
    value: &str,
    span: Span,
) -> Result<(), EditError> {
    if value.contains(['\n', '\r']) {
        return Err(option_value(name, "must be one line", span));
    }
    let Some(info) = guidedoc_sphinx::find_directive(config, directive) else { return Ok(()) };
    let Some(spec) = info.options.iter().find(|o| o.name == name) else {
        let names = info.options.iter().map(|o| o.name);
        let suggestion = gd_report::suggest(name, names);
        return Err(unknown_option(directive, name, suggestion, &info.options, span));
    };
    check_value(spec, value).map_err(|why| option_value(name, &why, span))
}

/// Checks `value` against the option's type; the explanation of a refusal.
fn check_value(spec: &OptSpec, value: &str) -> Result<(), String> {
    let number = |min: i64| value.parse::<i64>().ok().filter(|&n| n >= min).is_some();
    let ok = match spec.kind {
        OptType::Flag => value.is_empty(),
        OptType::Text | OptType::OptInt if value.is_empty() => true,
        OptType::Int => number(i64::MIN),
        OptType::NonnegInt | OptType::OptInt => number(0),
        OptType::PosInt => number(1),
        OptType::Choice => spec.choices.split_whitespace().any(|c| c.eq_ignore_ascii_case(value)),
        _ => !value.is_empty() || spec.kind == OptType::Text,
    };
    if ok {
        return Ok(());
    }
    Err(match spec.kind {
        OptType::Flag => "is a flag and takes no value".to_string(),
        OptType::Choice => format!("must be one of: {}", spec.choices),
        OptType::Int | OptType::NonnegInt | OptType::PosInt | OptType::OptInt => {
            "must be a whole number in its range".to_string()
        }
        _ => "must not be empty".to_string(),
    })
}

fn unknown_option(
    directive: &str,
    name: &str,
    suggestion: Option<&str>,
    options: &[OptSpec],
    span: Span,
) -> EditError {
    let known: Vec<&str> = options.iter().map(|o| o.name).collect();
    let mut hint = if known.is_empty() {
        format!("`{directive}` takes no options.")
    } else {
        format!("`{directive}` takes {}.", known.join(", "))
    };
    if let Some(s) = suggestion {
        hint.push_str(&format!(" Did you mean `{s}`?"));
    }
    EditError::new(
        "edit.unknown-option",
        "UNKNOWN OPTION",
        format!("The `{directive}` directive has no option `{name}`."),
        hint,
    )
    .at(span)
}

fn option_value(name: &str, why: &str, span: Span) -> EditError {
    EditError::new(
        "edit.option-value",
        "INVALID OPTION VALUE",
        format!("The value of `{name}` {why}."),
        "Correct the value, or remove the option.",
    )
    .at(span)
}

fn missing_option(name: &str, span: Span) -> EditError {
    EditError::new(
        "edit.missing-option",
        "NO SUCH OPTION",
        format!("The directive has no option `{name}` to remove."),
        "Choose one of the options the directive has.",
    )
    .at(span)
}
