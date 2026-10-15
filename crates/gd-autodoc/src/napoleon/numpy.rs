//! What `NumpyDocstring` does differently: `name : type` fields with the description
//! on the lines below, and numpydoc's `See Also` entries.

use super::docstring::{
    Field, Parser, dedent, format_admonition, has_spaced_name, indent_of, partition_on_colon,
};
use super::patterns::match_name;

/// One `See Also` entry: the name, its description, and its role when written.
struct SeeAlsoItem {
    name: String,
    desc: Vec<String>,
    role: Option<String>,
}

impl Parser<'_> {
    /// `NumpyDocstring._consume_field`.
    pub(crate) fn consume_numpy_field(&mut self, parse_type: bool, prefer_type: bool) -> Field {
        let at = self.position();
        let line = self.next_line();
        let (name, ty, colon) = if parse_type {
            let (name, colon, ty) = partition_on_colon(&line);
            (name, ty, colon)
        } else {
            (line.trim().to_owned(), String::new(), "")
        };
        if parse_type && !prefer_type && colon.is_empty() && has_spaced_name(&name) {
            self.note_field(at, &line);
        }
        let mut name = self.escape_args(&name);
        let mut ty = ty;
        if parse_type && ty.is_empty() {
            ty = self.lookup_annotation(&name);
        }
        if prefer_type && ty.is_empty() {
            std::mem::swap(&mut ty, &mut name);
        }
        if self.shared.settings.preprocess_types {
            ty = self.preprocess_type(&ty, at);
        }
        let block = self.consume_indented_block(indent_of(&line) + 1);
        let desc = self.nested(dedent(block));
        Field { name, ty, desc }
    }

    /// `_parse_numpydoc_see_also_section`: `None` when an entry is not a name, and the
    /// section is then an ordinary `seealso` admonition.
    pub(crate) fn numpy_see_also(&self, content: &[String]) -> Option<Vec<String>> {
        let mut items: Vec<SeeAlsoItem> = Vec::new();
        let mut current: Option<String> = None;
        let mut rest: Vec<String> = Vec::new();
        for line in content {
            if line.trim().is_empty() {
                continue;
            }
            let m = match_name(line).filter(|m| line[m.end..].trim().starts_with(':'));
            if let Some(m) = m {
                push_item(&mut items, current.take(), &mut rest)?;
                current = Some(line[..m.end].to_owned());
                let after = &line[m.end..];
                let desc = after.split_once(':').map_or("", |(_, d)| d).trim();
                rest = if desc.is_empty() { Vec::new() } else { vec![desc.to_owned()] };
            } else if !line.starts_with(' ') {
                push_item(&mut items, current.take(), &mut rest)?;
                if line.contains(',') {
                    for func in line.split(',').filter(|f| !f.trim().is_empty()) {
                        push_item(&mut items, Some(func.to_owned()), &mut Vec::new())?;
                    }
                } else if !line.trim().is_empty() {
                    current = Some(line.clone());
                }
            } else if current.is_some() {
                rest.push(line.trim().to_owned());
            }
        }
        push_item(&mut items, current, &mut rest)?;
        if items.is_empty() {
            return Some(Vec::new());
        }
        let items: Vec<SeeAlsoItem> = items.into_iter().map(|i| self.translate(i)).collect();
        Some(format_admonition("seealso", see_also_lines(items)))
    }

    /// `translate`: an entry without a role, renamed by `napoleon_type_aliases`.
    fn translate(&self, item: SeeAlsoItem) -> SeeAlsoItem {
        let aliases = &self.shared.settings.type_aliases;
        if item.role.is_some() || aliases.is_empty() {
            return item;
        }
        let translated = aliases.iter().find(|(k, _)| *k == item.name).map(|(_, v)| v.clone());
        let translated = translated.unwrap_or_else(|| item.name.clone());
        match match_name(&translated) {
            Some(m) => SeeAlsoItem {
                name: m.name.to_owned(),
                desc: item.desc,
                role: m.role.map(str::to_owned),
            },
            None => SeeAlsoItem { name: translated, ..item },
        }
    }
}

/// `push_item`: adds the entry `name` with the description lines in `rest`, which it
/// empties; `None` when `name` is not a name.
fn push_item(
    items: &mut Vec<SeeAlsoItem>,
    name: Option<String>,
    rest: &mut Vec<String>,
) -> Option<()> {
    let Some(name) = name.filter(|n| !n.is_empty()) else { return Some(()) };
    let m = match_name(&name)?;
    items.push(SeeAlsoItem {
        name: m.name.to_owned(),
        desc: std::mem::take(rest),
        role: m.role.map(str::to_owned),
    });
    Some(())
}

/// The links of the entries, those without a description joined on one line.
fn see_also_lines(items: Vec<SeeAlsoItem>) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut last_had_desc = true;
    for item in items {
        let link = match &item.role {
            Some(role) => format!(":{role}:`{}`", item.name),
            None => format!(":py:obj:`{}`", item.name),
        };
        if !item.desc.is_empty() || last_had_desc {
            lines.push(String::new());
            lines.push(link);
        } else if let Some(last) = lines.last_mut() {
            last.push_str(", ");
            last.push_str(&link);
        }
        last_had_desc = !item.desc.is_empty();
        if last_had_desc {
            lines.push(format!("    {}", item.desc.join(" ")));
        }
    }
    lines.push(String::new());
    lines
}
