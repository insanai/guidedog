//! With `autodoc_typehints = "description"`, Sphinx adds the annotations of an object to
//! the field lists of its description after parsing (`_merge_typehints`): to each field
//! list that stands directly in the description's content, or to a new one at its end.
//! Here the same fields are added to the text: a field list is a run of field items at
//! the content's own indentation, after a blank line.

use std::ops::Range;

use crate::DescriptionTarget;
use crate::options::contains;
use crate::props::Hint;

/// Matches the start of a field item (docutils' field marker) and returns the field
/// name as written.
pub(crate) fn field_name_of(line: &str) -> Option<&str> {
    let b = line.as_bytes();
    if b.len() < 3 || b[0] != b':' || b[1] == b' ' || b[1] == b':' {
        return None;
    }
    let mut i = 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            b':' if b[i - 1] != b' ' && (i + 1 == b.len() || b[i + 1] == b' ') => {
                return Some(&line[1..i]);
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn is_blank(l: &str) -> bool {
    l.trim().is_empty()
}

fn is_indented(l: &str) -> bool {
    l.starts_with([' ', '\t'])
}

/// The field lists at indentation 0 of a body, as line ranges ending after their last
/// non-blank line.
fn field_lists(body: &[String]) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::new();
    let mut previous_blank = true;
    let mut in_list = false;
    for (i, l) in body.iter().enumerate() {
        let blank = is_blank(l);
        if blank {
            // A blank line neither starts nor ends a list.
        } else if is_indented(l) {
            if let Some(list) = out.last_mut().filter(|_| in_list) {
                list.end = i + 1;
            }
        } else if field_name_of(l).is_some() && (previous_blank || in_list) {
            if !in_list {
                out.push(i..i + 1);
            }
            in_list = true;
            if let Some(list) = out.last_mut() {
                list.end = i + 1;
            }
        } else {
            in_list = false;
        }
        previous_blank = blank;
    }
    out
}

/// What the fields of a list already say about a name.
#[derive(Debug, Clone, Default)]
struct FieldSeen {
    name: String,
    param: bool,
    has_type: bool,
}

fn seen_entry<'s>(seen: &'s mut Vec<FieldSeen>, name: &str) -> &'s mut FieldSeen {
    match seen.iter().position(|s| s.name == name) {
        Some(i) => &mut seen[i],
        None => {
            seen.push(FieldSeen { name: name.to_string(), ..FieldSeen::default() });
            let last = seen.len() - 1;
            &mut seen[last]
        }
    }
}

/// What the fields of a list already say (`modify_field_list`'s arguments): the names
/// with `:param:` or `:type:` fields, and those described.
fn fields_of(lines: &[String]) -> (Vec<FieldSeen>, Vec<String>) {
    let mut seen = Vec::new();
    let mut described = Vec::new();
    for raw in lines.iter().filter_map(|l| field_name_of(l)) {
        let name = unescape_field(raw);
        let parts: Vec<&str> = name.split_whitespace().collect();
        match parts.as_slice() {
            ["param", arg] => {
                seen_entry(&mut seen, arg).param = true;
                described.push(arg.to_string());
            }
            ["param", _, rest @ ..] => {
                let arg = rest.join(" ");
                let e = seen_entry(&mut seen, &arg);
                (e.param, e.has_type) = (true, true);
                described.push(arg);
            }
            ["type", rest @ ..] => seen_entry(&mut seen, &rest.join(" ")).has_type = true,
            ["rtype", ..] => seen_entry(&mut seen, "return").has_type = true,
            ["return" | "returns", ..] => described.push("return".into()),
            _ => {}
        }
    }
    (seen, described)
}

fn unescape_field(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.extend(chars.next()),
            _ => out.push(c),
        }
    }
    out
}

/// Escapes reST inline markup, so an annotation reads as the plain text the field
/// Sphinx builds holds.
fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        if matches!(c, '\\' | '*' | '`' | '|' | '_' | '[' | ']' | ':') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Escapes a field name's stars, which reST would read as emphasis.
fn escape_field(s: &str) -> String {
    s.replace('*', "\\*")
}

/// Adds `:type:`, `:param:`, and `:rtype:` fields for recorded annotations to a body:
/// to each field list in it, or to a new one at its end. `target` is
/// `autodoc_typehints_description_target`; `suppress_rtype` leaves out `:rtype: None`
/// (for classes).
pub(crate) fn merge_typehints(
    hints: &[Hint],
    target: DescriptionTarget,
    suppress_rtype: bool,
    body: &mut Vec<String>,
) {
    let lists = field_lists(body);
    if lists.is_empty() {
        if body.last().is_some_and(|l| !is_blank(l)) {
            body.push(String::new());
        }
        let added = hint_fields(hints, &[], target, suppress_rtype);
        if !added.is_empty() {
            body.extend(added);
            body.push(String::new());
        }
        return;
    }
    // Insert from the last list back, so earlier positions stay valid.
    for list in lists.iter().rev() {
        let start = list_start(body, list.end);
        let added = hint_fields(hints, &body[start..list.end], target, suppress_rtype);
        body.splice(list.end..list.end, added);
    }
}

/// The first line of the field list that ends at `end`.
fn list_start(body: &[String], end: usize) -> usize {
    let mut start = end;
    for i in (0..end).rev() {
        let l = &body[i];
        if is_blank(l) || is_indented(l) {
            continue;
        }
        if field_name_of(l).is_none() {
            break;
        }
        start = i;
    }
    start
}

fn hint_fields(
    hints: &[Hint],
    list: &[String],
    target: DescriptionTarget,
    suppress_rtype: bool,
) -> Vec<String> {
    let (seen, described) = fields_of(list);
    let find = |name: &str| seen.iter().find(|s| s.name == name);
    let returns = hints.iter().find(|h| h.name == "return").map(|h| h.text.as_str());
    let mut out = Vec::new();
    if target == DescriptionTarget::All {
        for h in hints.iter().filter(|h| h.name != "return") {
            let star = format!("*{}", h.name);
            let stars = format!("**{}", h.name);
            let (name, entry) = match (find(&star), find(&stars)) {
                (Some(e), _) => (star.as_str(), Some(e)),
                (None, Some(e)) => (stars.as_str(), Some(e)),
                (None, None) => (h.name.as_str(), find(&h.name)),
            };
            if !entry.is_some_and(|e| e.has_type) {
                out.push(format!(":type {}: {}", escape_field(name), escape_text(&h.text)));
            }
            if !entry.is_some_and(|e| e.param) {
                out.push(format!(":param {}:", escape_field(name)));
            }
        }
        let rtype =
            returns.filter(|r| find("return").is_none() && !(*r == "None" && suppress_rtype));
        if let Some(r) = rtype {
            out.push(format!(":rtype: {}", escape_text(r)));
        }
        return out;
    }
    for h in hints.iter().filter(|h| h.name != "return") {
        let star = format!("*{}", h.name);
        let stars = format!("**{}", h.name);
        let name = if contains(&described, &star) {
            star.as_str()
        } else if contains(&described, &stars) {
            stars.as_str()
        } else {
            h.name.as_str()
        };
        let typed = find(name).is_some_and(|e| e.has_type);
        if contains(&described, name) && !typed {
            out.push(format!(":type {}: {}", escape_field(name), escape_text(&h.text)));
        }
    }
    if let Some(r) = returns {
        let typed = find("return").is_some_and(|e| e.has_type);
        let force = target == DescriptionTarget::DocumentedParams && r != "None";
        if !typed && (contains(&described, "return") || force) {
            out.push(format!(":rtype: {}", escape_text(r)));
        }
    }
    out
}
