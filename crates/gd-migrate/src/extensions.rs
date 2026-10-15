//! A project's own Sphinx extension often only declares object types in its `setup()`:
//! `app.add_crossref_type`, `app.add_object_type`, and `app.add_directive` with one of
//! Sphinx's own directive classes. These calls are read, without running the extension,
//! and written as `[[crossref_types]]`, `[[object_types]]` and `[directive_aliases]`;
//! what else the extension registers is reported, since it needs Python.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::migrate::Migration;
use crate::pyconf::{self, Scope, Value, group_end, name_char};

/// Sphinx's standard-domain directive classes an extension may register under a name
/// of its own, and the directive each is.
const STD_DIRECTIVE_CLASSES: [(&str, &str); 6] = [
    ("Cmdoption", "option"),
    ("Program", "program"),
    ("OptionXRef", "option"),
    ("EnvVar", "envvar"),
    ("Glossary", "glossary"),
    ("ProductionList", "productionlist"),
];

/// How many folders below conf.py's folder an extension module is looked for.
const MAX_MODULE_DEPTH: usize = 3;

/// The largest extension module read.
const MAX_MODULE_BYTES: u64 = 1 << 20;

/// Reads the `setup()` of each extension conf.py lists that is a module of the project,
/// found below `conf_dir` (conf.py's folder).
pub(crate) fn harvest_extensions(m: &mut Migration<'_>, conf_dir: &Path) {
    let conf_dir = if conf_dir.as_os_str().is_empty() { Path::new(".") } else { conf_dir };
    let names = std::mem::take(&mut m.left_out);
    let mut aliases = Vec::new();
    for name in &names {
        let Some(path) = extension_module(conf_dir, name) else { continue };
        let Some(text) = read_module(&path) else { continue };
        let mut declared = harvest_types(m, &text, name);
        declared += harvest_aliases(&text, &mut aliases);
        report_left(m, &text, name, declared);
    }
    m.left_out = names;
    if aliases.is_empty() {
        return;
    }
    m.tables.push_str("\n[directive_aliases]\n");
    for (alias, directive) in aliases {
        gd_toml::write_key(&mut m.tables, &alias);
        m.tables.push_str(" = ");
        gd_toml::write_string(&mut m.tables, directive);
        m.tables.push('\n');
    }
}

fn read_module(path: &Path) -> Option<String> {
    let size = std::fs::metadata(path).ok()?.len();
    if size > MAX_MODULE_BYTES {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// The file of a module named in `extensions`: its dots as folders, below `dir`.
fn extension_module(dir: &Path, name: &str) -> Option<PathBuf> {
    let want = format!("{}.py", name.replace('.', "/"));
    find_module(dir, &want, 0)
}

fn find_module(dir: &Path, want: &str, depth: usize) -> Option<PathBuf> {
    let candidate = dir.join(want);
    if candidate.is_file() {
        return Some(candidate);
    }
    if depth >= MAX_MODULE_DEPTH {
        return None;
    }
    let mut folders: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| {
            let n = e.file_name();
            let n = n.to_string_lossy();
            !n.starts_with('.') && !matches!(n.as_ref(), "_build" | "node_modules" | "__pycache__")
        })
        .map(|e| e.path())
        .collect();
    folders.sort(); // the same module wins on every system.
    folders.iter().find_map(|f| find_module(f, want, depth + 1))
}

/// Turns `add_crossref_type` and `add_object_type` calls into tables.
fn harvest_types(m: &mut Migration<'_>, text: &str, extension: &str) -> usize {
    let mut count = 0;
    for (method, table) in
        [("add_crossref_type", "crossref_types"), ("add_object_type", "object_types")]
    {
        let target = table == "crossref_types";
        let fourth = if target { "ref_nodeclass" } else { "parse_node" };
        for args in calls(text, method) {
            let [directive, role, index, parse] =
                call_fields(args, ["directivename", "rolename", "indextemplate", fourth]);
            if directive.is_empty() {
                continue;
            }
            let _ = write!(m.tables, "\n[[{table}]]\ndirective = ");
            gd_toml::write_string(&mut m.tables, &directive);
            m.tables.push('\n');
            for (key, value) in [("role", &role), ("index", &index)] {
                if !value.is_empty() {
                    let _ = write!(m.tables, "{key} = ");
                    gd_toml::write_string(&mut m.tables, value);
                    m.tables.push('\n');
                }
            }
            count += 1;
            if !target && !parse.is_empty() && parse != "None" {
                m.attention += 1;
                m.tell(
                    0,
                    "migrate.extension.parse_node",
                    "SIGNATURE READER LEFT",
                    format!(
                        "{extension} reads {directive} signatures with {parse}, which runs Python."
                    ),
                    "Set name, display, and program in its [[object_types]] entry to match \
                     what that function does.",
                );
            }
        }
    }
    count
}

/// Turns `add_directive` calls with a standard directive class into aliases.
fn harvest_aliases(text: &str, aliases: &mut Vec<(String, &'static str)>) -> usize {
    let mut count = 0;
    for args in calls(text, "add_directive") {
        let [name, class] = call_fields(args, ["name", "cls"]);
        if name.is_empty() {
            continue;
        }
        if let Some(directive) = standard_directive(text, &class) {
            aliases.push((name, directive));
            count += 1;
        }
    }
    count
}

/// The directive a standard-domain class is, when the module imports it from Sphinx.
fn standard_directive(text: &str, class: &str) -> Option<&'static str> {
    STD_DIRECTIVE_CLASSES
        .iter()
        .find(|(c, _)| *c == class && imports_from_std(text, class))
        .map(|(_, directive)| *directive)
}

/// Tells what an extension registers that could not be declared.
fn report_left(m: &mut Migration<'_>, text: &str, extension: &str, declared: usize) {
    let mut left = Vec::new();
    for method in [
        "add_directive",
        "add_role",
        "add_node",
        "connect",
        "add_directive_to_domain",
        "add_role_to_domain",
        "add_transform",
    ] {
        for args in calls(text, method) {
            if method == "add_directive" {
                let [_, class] = call_fields(args, ["name", "cls"]);
                if standard_directive(text, &class).is_some() {
                    continue;
                }
            }
            let [first] = call_fields(args, ["name"]);
            if !first.is_empty() {
                left.push(format!("{method}({first})"));
            }
        }
    }
    let mut message = format!(
        "{extension} declares {declared} object type(s) and alias(es), converted to conf.toml."
    );
    if !left.is_empty() {
        let _ = write!(message, " It also registers {}, which need Python.", left.join(", "));
    }
    m.tell(
        0,
        "migrate.extension.read",
        "EXTENSION READ",
        message,
        "Replace the directives and roles that need Python with built-in ones; until then a \
         build reports them as unknown.",
    );
}

/// Whether the module imports `name` from Sphinx's standard domain.
fn imports_from_std(text: &str, name: &str) -> bool {
    const LEAD: &str = "from sphinx.domains.std import";
    text.match_indices(LEAD).any(|(i, _)| {
        let rest = &text[i + LEAD.len()..];
        let end = if rest.trim_start().starts_with('(') {
            rest.find('(').map_or(rest.len(), |open| group_end(rest, open))
        } else {
            rest.find('\n').unwrap_or(rest.len())
        };
        rest[..end].split_whitespace().any(|w| w.trim_matches(['(', ')', ',']) == name)
    })
}

/// The argument texts of every `.method(...)` call in `text`.
fn calls<'t>(text: &'t str, method: &str) -> Vec<&'t str> {
    let needle = format!(".{method}(");
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(&needle) {
        let open = i + needle.len() - 1;
        let end = group_end(rest, open);
        found.push(&rest[open + 1..end.saturating_sub(1).max(open + 1)]);
        rest = &rest[end..];
    }
    found
}

/// Reads a call's arguments by position or keyword, in the order of `names`: strings as
/// their value, other expressions as written; missing ones are empty.
fn call_fields<const N: usize>(args: &str, names: [&str; N]) -> [String; N] {
    let mut out: [String; N] = std::array::from_fn(|_| String::new());
    let mut position = 0;
    for arg in pyconf::split_top(args, b',') {
        let a = arg.trim();
        if a.is_empty() {
            continue;
        }
        let (slot, value) = match keyword_of(a) {
            Some((k, value)) => (names.iter().position(|n| *n == k), value),
            None => {
                position += 1;
                (Some(position - 1), a)
            }
        };
        let Some(slot) = slot.filter(|&s| s < N) else { continue };
        out[slot] = match pyconf::eval(value, &mut Scope::default()) {
            Ok(Value::Str(s)) => s,
            _ => value.to_string(),
        };
    }
    out
}

/// The name and value of a keyword argument (`name=value`), or `None`.
fn keyword_of(arg: &str) -> Option<(&str, &str)> {
    let bytes = arg.as_bytes();
    for (i, &c) in bytes.iter().enumerate() {
        if c == b'=' {
            return (bytes.get(i + 1) != Some(&b'='))
                .then(|| (arg[..i].trim(), arg[i + 1..].trim()));
        }
        if !name_char(c, i == 0) && c != b' ' {
            return None;
        }
    }
    None
}
