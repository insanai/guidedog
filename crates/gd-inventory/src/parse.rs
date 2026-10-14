//! Reading inventory headers and lines.

use crate::{Error, ErrorKind, Inventory, Item};

/// Splits `data` at its first line break: the line without it, and the rest.
pub(crate) fn next_line(data: &[u8]) -> (&[u8], &[u8]) {
    match data.iter().position(|&b| b == b'\n') {
        Some(i) => (&data[..i], &data[i + 1..]),
        None => (data, &[]),
    }
}

/// Reads the `# Project:` and `# Version:` lines; returns them and the data after.
pub(crate) fn header(data: &[u8]) -> Result<(String, String, &[u8]), Error> {
    let (project, after) = next_line(data);
    let (version, rest) = next_line(after);
    let field = |line: &[u8], prefix: &[u8]| {
        line.strip_prefix(prefix).map(|v| String::from_utf8_lossy(v.trim_ascii_end()).into_owned())
    };
    match (field(project, b"# Project: "), field(version, b"# Version: ")) {
        (Some(p), Some(v)) => Ok((p, v, rest)),
        _ => Err(Error { kind: ErrorKind::Truncated }),
    }
}

/// Reads version 1 lines, `name type location`: modules and Python objects.
pub(crate) fn load_v1(inventory: &mut Inventory, text: &str, base: &str) {
    for line in text.lines() {
        let fields: Vec<&str> = line.split_ascii_whitespace().collect();
        let [name, kind, location] = fields[..] else { continue };
        let (kind, location) = if kind == "mod" {
            ("py:module".to_string(), format!("{location}#module-{name}"))
        } else {
            (format!("py:{kind}"), format!("{location}#{name}"))
        };
        let uri = join_base(base, &location);
        inventory.add(Item {
            name: name.to_string(),
            kind,
            priority: 1,
            uri,
            display: "-".into(),
        });
    }
}

/// Reads version 2 lines, `name domain:type priority location display`. A location
/// ending in `$` ends in the name, and the first module entry of a name wins, as in
/// Sphinx.
pub(crate) fn load_v2(inventory: &mut Inventory, text: &str, base: &str) {
    for line in text.split('\n') {
        let Some(mut item) = parse_line(line.trim_end()) else { continue };
        if !item.kind.contains(':') {
            continue;
        }
        if item.kind == "py:module" && inventory.find("py:module", &item.name).is_some() {
            continue;
        }
        if let Some(stem) = item.uri.strip_suffix('$') {
            item.uri = format!("{stem}{}", item.name);
        }
        item.uri = join_base(base, &item.uri);
        inventory.add(item);
    }
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n' | b'\x0c' | b'\x0b')
}

/// Splits `s` at the end of its first word.
fn word(s: &str) -> (&str, &str) {
    let end = s.bytes().position(is_space).unwrap_or(s.len());
    s.split_at(end)
}

fn trim_space_start(s: &str) -> &str {
    s.trim_start_matches(|c: char| c.is_ascii() && is_space(c as u8))
}

/// Matches Sphinx's pattern `(.+?)\s+(\S+)\s+(-?\d+)\s+?(\S*)\s+(.*)`: the name is the
/// shortest prefix after which a type, priority, location and display fit.
fn parse_line(line: &str) -> Option<Item> {
    let bytes = line.as_bytes();
    // Try every place the name could end, first to last, as the lazy (.+?) does.
    for end in 1..bytes.len() {
        if !is_space(bytes[end]) || is_space(bytes[end - 1]) {
            continue;
        }
        if let Some(item) = parse_after_name(&line[..end], &line[end..]) {
            return Some(item);
        }
    }
    None
}

/// Reads the type, priority, location and display after a candidate name.
fn parse_after_name(name: &str, rest: &str) -> Option<Item> {
    let (kind, after_kind) = word(trim_space_start(rest));
    if kind.is_empty() || !after_kind.bytes().next().is_some_and(is_space) {
        return None;
    }
    let (priority_text, after_priority) = word(trim_space_start(after_kind));
    let digits = priority_text.strip_prefix('-').unwrap_or(priority_text);
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let priority = priority_text.parse::<i64>().ok()?;
    if !after_priority.bytes().next().is_some_and(is_space) {
        return None;
    }
    // \s+? takes one space; the location may be empty, then \s+ and the display.
    let (location, after_location) = word(&after_priority[1..]);
    if !after_location.bytes().next().is_some_and(is_space) {
        return None;
    }
    Some(Item {
        name: name.to_string(),
        kind: kind.to_string(),
        priority,
        uri: location.to_string(),
        display: trim_space_start(after_location).to_string(),
    })
}

/// Joins `location` onto `base` as Sphinx does with `posixpath.join`: a location
/// starting with `/` stays as it is, and so does any location when `base` is empty.
fn join_base(base: &str, location: &str) -> String {
    if base.is_empty() || location.starts_with('/') {
        location.to_string()
    } else if base.ends_with('/') {
        format!("{base}{location}")
    } else {
        format!("{base}/{location}")
    }
}
