//! `find_autosummary_in_lines`: the entries of a document's `autosummary` directives,
//! read from its text before the document is parsed, as Sphinx reads them, so stub
//! pages exist by the time the document is read.

/// One entry of an `autosummary` directive.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Entry {
    /// The object's full name.
    pub(crate) name: String,
    /// The folder of its stub page, relative to the source folder; `None` without
    /// `:toctree:` (no stub page).
    pub(crate) folder: Option<String>,
    /// `:template:`, or `""`.
    pub(crate) template: String,
    /// `:recursive:`.
    pub(crate) recursive: bool,
}

/// What a line of a document is to the scanner.
enum Line<'a> {
    /// `.. autosummary::` with its indent.
    Autosummary(&'a str),
    /// `.. automodule:: name`.
    Automodule(&'a str),
    /// `.. module:: name` or `.. currentmodule:: name`.
    Module(&'a str),
    Other,
}

/// The name after `.. NAME::` when the line is that directive.
fn directive_argument<'a>(line: &'a str, names: &[&str]) -> Option<&'a str> {
    let rest = line.trim_start().strip_prefix("..")?;
    let rest = rest.trim_start();
    if rest.len() == line.trim_start().len() - 2 {
        return None;
    }
    let (name, arg) = rest.split_once("::")?;
    names.contains(&name).then_some(arg)
}

/// `[A-Za-z0-9_.]+` and nothing else around it but spaces.
fn dotted_name(arg: &str) -> Option<&str> {
    let arg = arg.trim();
    let ok =
        !arg.is_empty() && arg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
    ok.then_some(arg)
}

fn classify(line: &str) -> Line<'_> {
    if directive_argument(line, &["autosummary"]).is_some() {
        let indent = &line[..line.len() - line.trim_start().len()];
        return Line::Autosummary(indent);
    }
    if let Some(name) = directive_argument(line, &["automodule"]).and_then(dotted_name) {
        return Line::Automodule(name);
    }
    match directive_argument(line, &["module", "currentmodule"]).and_then(dotted_name) {
        Some(name) => Line::Module(name),
        None => Line::Other,
    }
}

/// The scanner's state inside one `autosummary` directive.
struct Directive {
    indent: String,
    folder: Option<String>,
    template: String,
    recursive: bool,
}

/// The entries in `lines`, a document in `folder` (relative to the source folder; `""`
/// for the top), whose current module starts as `module`. `docstring` gives the lines
/// of a module's docstring, which an `automodule` directive's own autosummary
/// directives are read from.
pub(crate) fn find_entries(
    lines: &[&str],
    module: Option<&str>,
    folder: &str,
    docstring: &mut dyn FnMut(&str) -> Option<Vec<String>>,
) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut current = module.map(str::to_owned);
    let mut inside: Option<Directive> = None;
    for line in lines {
        if let Some(d) = inside.as_mut() {
            match directive_line(d, line, folder, current.as_deref()) {
                Some(Some(entry)) => {
                    out.push(entry);
                    continue;
                }
                Some(None) => continue,
                None => inside = None,
            }
        }
        match classify(line) {
            Line::Autosummary(indent) => {
                inside = Some(Directive {
                    indent: indent.to_owned(),
                    folder: None,
                    template: String::new(),
                    recursive: false,
                });
            }
            Line::Automodule(name) => {
                current = Some(name.to_owned());
                if let Some(doc) = docstring(name) {
                    let doc: Vec<&str> = doc.iter().map(String::as_str).collect();
                    out.extend(find_entries(&doc, Some(name), folder, &mut |_| None));
                }
            }
            Line::Module(name) => current = Some(name.to_owned()),
            Line::Other => {}
        }
    }
    out
}

/// One line inside an `autosummary` directive: `Some(Some(entry))` for an entry,
/// `Some(None)` for an option or a blank line, `None` when the directive ended.
fn directive_line(
    d: &mut Directive,
    line: &str,
    folder: &str,
    module: Option<&str>,
) -> Option<Option<Entry>> {
    let t = line.trim();
    let indented = line.starts_with(char::is_whitespace);
    if indented && t == ":recursive:" {
        d.recursive = true;
        return Some(None);
    }
    if indented && let Some(value) = t.strip_prefix(":toctree:") {
        let parts = [folder, value.trim()];
        d.folder =
            Some(parts.iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join("/"));
        return Some(None);
    }
    if indented && let Some(value) = t.strip_prefix(":template:") {
        d.template = value.trim().to_owned();
        return Some(None);
    }
    if t.starts_with(':') {
        return Some(None);
    }
    if indented && let Some(name) = entry_name(t) {
        let name = name.strip_prefix('~').unwrap_or(name);
        let name = match module {
            Some(m) if !name.starts_with(&format!("{m}.")) => format!("{m}.{name}"),
            _ => name.to_owned(),
        };
        let entry = Entry {
            name,
            folder: d.folder.clone(),
            template: d.template.clone(),
            recursive: d.recursive,
        };
        return Some(Some(entry));
    }
    if t.is_empty() || line.starts_with(&format!("{} ", d.indent)) {
        return Some(None);
    }
    None
}

/// `~?[_a-zA-Z][a-zA-Z0-9_.]*` at the start of a stripped line.
fn entry_name(t: &str) -> Option<&str> {
    let body = t.strip_prefix('~').unwrap_or(t);
    if !body.starts_with(|c: char| c == '_' || c.is_ascii_alphabetic()) {
        return None;
    }
    let n = body
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
        .unwrap_or(body.len());
    Some(&t[..t.len() - body.len() + n])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_follow_options_and_the_current_module() {
        let text = "\
.. currentmodule:: pkg

.. autosummary::
   :toctree: generated
   :recursive:

   mod
   ~pkg.Square
   other.thing  description

Text after.

.. autosummary::

   no_stub
";
        let lines: Vec<&str> = text.lines().collect();
        let entries = find_entries(&lines, None, "api", &mut |_| None);
        let names: Vec<(&str, Option<&str>, bool)> =
            entries.iter().map(|e| (e.name.as_str(), e.folder.as_deref(), e.recursive)).collect();
        assert_eq!(
            names,
            [
                ("pkg.mod", Some("api/generated"), true),
                ("pkg.Square", Some("api/generated"), true),
                ("pkg.other.thing", Some("api/generated"), true),
                ("pkg.no_stub", None, false),
            ]
        );
    }

    #[test]
    fn automodule_docstrings_are_read() {
        let lines = [".. automodule:: pkg.mod"];
        let mut docstring = |name: &str| {
            assert_eq!(name, "pkg.mod");
            Some(vec![
                ".. autosummary::".into(),
                "   :toctree: gen".into(),
                "".into(),
                "   f".into(),
            ])
        };
        let entries = find_entries(&lines, None, "", &mut docstring);
        assert_eq!(entries[0].name, "pkg.mod.f");
        assert_eq!(entries[0].folder.as_deref(), Some("gen"));
    }
}
