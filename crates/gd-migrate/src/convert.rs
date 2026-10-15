//! Writing conf.py's bindings as conf.toml: renamed and adapted settings, the check
//! that each value has the form its setting takes, and the final assembly.

use std::fmt::Write as _;
use std::path::Path;

use gd_conf::{
    CONFIG_FILE, Config, KNOWN_THEMES, NATIVE_EXTENSIONS, SettingKind, find_setting,
    is_foreign_setting, suggestion,
};
use gd_host::Problem;

use crate::migrate::{Conversion, Migration};
use crate::pyconf::{Scope, Value};
use crate::read::Binding;

/// Whether `name` is a conf.toml setting, a Sphinx setting Guidedog ignores, or a LaTeX
/// setting that maps to a PDF one.
pub(crate) fn is_setting_name(name: &str) -> bool {
    find_setting(name).is_some() || is_foreign_setting(name) || name.starts_with("latex_")
}

/// Writes one conf.py assignment into conf.toml, or explains why not.
pub(crate) fn convert_binding(m: &mut Migration<'_>, scope: &Scope, b: &Binding<'_>) {
    if b.name == "linkcode_resolve" {
        report_linkcode_resolve(m, b.line);
        return;
    }
    if !is_setting_name(&b.name) {
        let private = b.name.starts_with('_');
        if !scope.used.contains(&b.name) && !private && b.failure.is_none() {
            m.tell(
                b.line,
                "migrate.unknown",
                "NOT A SETTING",
                format!("{} is not a setting Guidedog knows, so it is left out.", b.name),
                &suggestion(&b.name),
            );
        }
        return;
    }
    if let Some(failure) = &b.failure {
        m.warn(
            b.line,
            "migrate.computed",
            "SETTING NOT CONVERTED",
            format!("{} is not converted. {failure}", b.name),
            "Write its value in conf.toml by hand; the original line is kept there as a \
             comment.",
        );
        keep_comment(m, b.source);
        return;
    }
    let Some(v) = scope.values.get(&b.name) else { return };
    let name = match b.name.as_str() {
        "master_doc" if scope.values.contains_key("root_doc") => return,
        "master_doc" => "root_doc",
        "latex_documents" => return pdf_from_latex(m, b, v),
        "latex_elements" => return paper_from_latex(m, b, v),
        "latex_logo" => "pdf_logo",
        "latex_toplevel_sectioning" => "pdf_toplevel_sectioning",
        "latex_show_urls" => "pdf_show_urls",
        "viewcode_import" => "viewcode_follow_imported_members",
        other => other,
    };
    if *v == Value::None {
        if name == "duration_write_json" {
            // None turns the file off, and conf.toml says that with "".
            write_setting(m, name, &Value::Str(String::new()));
            m.converted += 1;
        }
        return; // Sphinx's default, which conf.toml leaves out.
    }
    convert_value(m, b, name, v.clone());
}

/// Writes the value of a known setting `name`, once adapted, checked for its form and
/// loaded alone; otherwise keeps the line as a comment and says why.
fn convert_value(m: &mut Migration<'_>, b: &Binding<'_>, name: &str, v: Value) {
    let Some(setting) = find_setting(name) else {
        m.sphinx_only.push(name.to_string());
        keep_comment(m, b.source);
        return;
    };
    let v = adapt(m, b, name, v);
    let refused = if !fits(setting.kind, &v) {
        let message =
            format!("{name} holds a kind of value Guidedog's {name} setting does not take.");
        Some(("migrate.form", "SETTING HAS AN UNUSABLE FORM", message))
    } else {
        loads_alone(m, name, &v).err().map(|why| {
            ("migrate.value", "SETTING VALUE REFUSED", format!("{name} is not converted: {why}"))
        })
    };
    if let Some((code, title, message)) = refused {
        m.warn(b.line, code, title, message, "Adjust the commented copy in conf.toml by hand.");
        keep_comment(m, b.source);
        return;
    }
    write_setting(m, name, &v);
    m.converted += 1;
}

/// Loads `name = v` alone, as a build would, so a value Guidedog checks and refuses (a
/// pattern it cannot compile, a folder outside the project) is reported and kept as a
/// comment instead of making the whole conf.toml fail to load. Fails with the reason.
fn loads_alone(m: &Migration<'_>, name: &str, v: &Value) -> Result<(), String> {
    let mut scratch = Migration::new("", &m.path);
    write_setting(&mut scratch, name, v);
    scratch.plain.push_str(&scratch.tables);
    let table = gd_toml::parse(&scratch.plain).map_err(|e| e.message)?;
    let mut config = Config::new(Path::new(""), Path::new(""));
    let mut warnings = Vec::new();
    gd_conf::apply_table(&mut config, &table, CONFIG_FILE, &mut warnings, true)
        .map_err(|p| p.message)
}

/// Rewrites values whose Sphinx form Guidedog reads differently.
fn adapt(m: &mut Migration<'_>, b: &Binding<'_>, name: &str, v: Value) -> Value {
    match (name, v) {
        ("intersphinx_mapping", v) => none_as_empty(v),
        ("extensions", Value::List(items)) => Value::List(keep_native(m, b, items)),
        ("html_theme", Value::Str(theme)) if !KNOWN_THEMES.contains(&theme.as_str()) => {
            m.tell(
                b.line,
                "migrate.theme",
                "THEME REPLACED",
                format!(
                    "The {theme} theme is not available; the project uses Guidedog's own theme."
                ),
                "Put the styles you need in _static/guidedog.css.",
            );
            Value::Str("guidedog".into())
        }
        // Sphinx accepts (filename, attributes) pairs; Guidedog takes the file names.
        ("html_css_files" | "html_js_files", Value::List(items)) => Value::List(
            items
                .into_iter()
                .map(|item| match item {
                    Value::List(mut pair) if !pair.is_empty() => pair.swap_remove(0),
                    other => other,
                })
                .collect(),
        ),
        // A caption of None shows the whole URL, which is the URL pattern itself.
        ("extlinks", Value::Dict(entries)) => Value::Dict(
            entries
                .into_iter()
                .map(|(k, item)| match item {
                    Value::List(pair) if pair.len() == 2 && pair[1] == Value::None => {
                        (k, Value::List(vec![pair[0].clone(), pair[0].clone()]))
                    }
                    other => (k, other),
                })
                .collect(),
        ),
        (_, v) => v,
    }
}

/// Keeps the built-in extensions and reports the others, remembering them so their
/// modules can be read for object types.
fn keep_native(m: &mut Migration<'_>, b: &Binding<'_>, items: Vec<Value>) -> Vec<Value> {
    let mut kept = Vec::with_capacity(items.len());
    for item in items {
        if let Value::Str(name) = &item
            && !NATIVE_EXTENSIONS.contains(&name.as_str())
        {
            m.left_out.push(name.clone());
            m.warn(
                b.line,
                "migrate.extension",
                "EXTENSION NOT BUILT IN",
                format!("{name} is not built into Guidedog, so it is left out."),
                "A build reports the directives and roles it adds as unknown. Replace them \
                 with built-in ones, or remove them.",
            );
            continue;
        }
        kept.push(item);
    }
    kept
}

/// Reports a `linkcode_resolve` function: Python that Guidedog does not run, whose work
/// a `linkcode_url` template does.
pub(crate) fn report_linkcode_resolve(m: &mut Migration<'_>, line: usize) {
    m.warn(
        line,
        "migrate.linkcode",
        "LINKCODE_RESOLVE NOT RUN",
        "linkcode_resolve is a Python function, which Guidedog does not run, so the source \
         links it makes are not added."
            .into(),
        "Set linkcode_url to a URL template instead, such as \
         linkcode_url = \"https://github.com/o/r/blob/{branch}/{path}#L{line}-L{end_line}\".",
    );
}

/// Writes `intersphinx_mapping`'s None locations as `""`, which conf.toml reads as None:
/// the objects.inv at the target URI.
fn none_as_empty(v: Value) -> Value {
    let empty = |v: Value| if v == Value::None { Value::Str(String::new()) } else { v };
    let Value::Dict(entries) = v else { return v };
    let entries = entries.into_iter().map(|(k, item)| match item {
        Value::List(mut pair) if pair.len() == 2 => {
            pair[1] = match std::mem::take(&mut pair[1]) {
                Value::List(list) => Value::List(list.into_iter().map(empty).collect()),
                other => empty(other),
            };
            (k, Value::List(pair))
        }
        other => (k, other),
    });
    Value::Dict(entries.collect())
}

/// Whether conf.toml's loader accepts `v` for a setting of this kind.
fn fits(kind: SettingKind, v: &Value) -> bool {
    let string_keys =
        |entries: &[(Value, Value)]| entries.iter().all(|(k, _)| k.as_str().is_some());
    let string_pairs = |items: &[Value]| {
        items.iter().all(|i| i.is_string_list() && matches!(i, Value::List(p) if p.len() == 2))
    };
    match (kind, v) {
        (SettingKind::String | SettingKind::OptionalString, Value::Str(_)) => true,
        (SettingKind::Bool, Value::Bool(_)) => true,
        (SettingKind::Int, Value::Int(_)) => true,
        (SettingKind::Strings | SettingKind::Suffixes, Value::Str(_)) => true,
        (SettingKind::Strings | SettingKind::Suffixes, v) if v.is_string_list() => true,
        (SettingKind::Suffixes, Value::Dict(entries)) => string_keys(entries),
        (SettingKind::Table, Value::Dict(entries)) => {
            string_keys(entries)
                && entries
                    .iter()
                    .all(|(_, i)| matches!(i, Value::Str(_) | Value::Bool(_) | Value::Int(_)))
        }
        (SettingKind::PdfDocuments, Value::List(items)) => items
            .iter()
            .all(|i| matches!(i, Value::Dict(e) if e.iter().all(|(_, v)| v.as_str().is_some()))),
        (SettingKind::Extlinks, Value::Dict(entries)) => {
            let items: Vec<Value> = entries.iter().map(|(_, i)| i.clone()).collect();
            string_pairs(&items)
        }
        (SettingKind::Pairs | SettingKind::PatternPairs, Value::List(items)) => {
            string_pairs(items)
        }
        (SettingKind::FlagOrName, Value::Bool(_) | Value::Str(_)) => true,
        (SettingKind::AutodocOptions, Value::Dict(entries)) => {
            string_keys(entries)
                && entries.iter().all(|(_, i)| matches!(i, Value::Str(_) | Value::Bool(_)))
        }
        (SettingKind::Json | SettingKind::Values, Value::Dict(_)) => true,
        (SettingKind::FlagOrList, Value::Bool(_)) => true,
        (SettingKind::FlagOrList, v) => v.is_string_list(),
        (SettingKind::StringLists, Value::Dict(entries)) => {
            string_keys(entries)
                && entries.iter().all(|(_, i)| matches!(i, Value::Str(_)) || i.is_string_list())
        }
        (SettingKind::Number, Value::Int(_) | Value::Float(_)) => true,
        (SettingKind::NapoleonSections, Value::List(items)) => items.iter().all(|i| {
            matches!(i, Value::Str(_))
                || matches!(i, Value::List(p) if p.len() == 2 && i.is_string_list())
        }),
        (SettingKind::ApidocModules, Value::List(items)) => {
            items.iter().all(|i| matches!(i, Value::Dict(e) if string_keys(e)))
        }
        (SettingKind::Intersphinx, Value::Dict(entries)) => {
            string_keys(entries) && entries.iter().all(|(_, i)| intersphinx_entry(i))
        }
        // Declared in Python by an extension's setup(), never in conf.py.
        (
            SettingKind::ObjectTypes | SettingKind::CrossrefTypes | SettingKind::DirectiveAliases,
            _,
        ) => false,
        _ => false,
    }
}

/// `(uri, location)` where location is a string or a list of strings.
fn intersphinx_entry(item: &Value) -> bool {
    let Value::List(pair) = item else { return false };
    match pair.as_slice() {
        [Value::Str(_), Value::Str(_)] => true,
        [Value::Str(_), locations] => locations.is_string_list(),
        _ => false,
    }
}

/// Turns `latex_documents` tuples (root, target, title, author, ...) into
/// `pdf_documents`, one book each.
fn pdf_from_latex(m: &mut Migration<'_>, b: &Binding<'_>, v: &Value) {
    let Value::List(items) = v else { return };
    let mut books = Vec::with_capacity(items.len());
    for item in items {
        let fields: Option<Vec<&str>> = match item {
            Value::List(t) if t.len() >= 2 => t.iter().take(4).map(Value::as_str).collect(),
            _ => None,
        };
        let Some(fields) = fields else {
            m.warn(
                b.line,
                "migrate.form",
                "LATEX DOCUMENT NOT CONVERTED",
                "A latex_documents entry is not a (root, file, title, author) tuple.".into(),
                "Add the book to pdf_documents in conf.toml by hand.",
            );
            keep_comment(m, b.source);
            return;
        };
        let file = fields[1].strip_suffix(".tex").unwrap_or(fields[1]);
        let file = format!("{file}.pdf");
        let values =
            [fields[0], &file, fields.get(2).unwrap_or(&""), fields.get(3).unwrap_or(&"")];
        let book = ["root", "file", "title", "author"]
            .iter()
            .zip(values)
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| (Value::Str((*k).into()), Value::Str(v.into())))
            .collect();
        books.push(Value::Dict(book));
    }
    write_setting(m, "pdf_documents", &Value::List(books));
    m.converted += 1;
}

/// Keeps `latex_elements`' papersize as `pdf_paper_size`; the rest is kept as a comment.
fn paper_from_latex(m: &mut Migration<'_>, b: &Binding<'_>, v: &Value) {
    m.sphinx_only.push("latex_elements".into());
    keep_comment(m, b.source);
    let Value::Dict(entries) = v else { return };
    for (key, value) in entries {
        if let (Some("papersize"), Value::Str(paper)) = (key.as_str(), value) {
            let paper = paper.strip_suffix("paper").unwrap_or(paper);
            write_setting(m, "pdf_paper_size", &Value::Str(paper.into()));
            m.converted += 1;
        }
    }
}

/// Keeps a statement as comment lines at the end of conf.toml.
pub(crate) fn keep_comment(m: &mut Migration<'_>, source: &str) {
    for line in source.trim_end().lines() {
        let _ = writeln!(m.comments, "# {line}");
    }
}

/// Writes one setting: a key in the top section, a `[table]`, or an array of tables
/// for `pdf_documents`.
pub(crate) fn write_setting(m: &mut Migration<'_>, name: &str, v: &Value) {
    match v {
        Value::List(books) if name == "pdf_documents" => {
            for book in books {
                let _ = write!(m.tables, "\n[[{name}]]\n");
                write_pairs(&mut m.tables, book);
            }
        }
        Value::Dict(entries) if !entries.is_empty() => {
            let _ = write!(m.tables, "\n[{name}]\n");
            write_pairs(&mut m.tables, v);
        }
        _ => {
            gd_toml::write_key(&mut m.plain, name);
            m.plain.push_str(" = ");
            write_value(&mut m.plain, v, true);
            m.plain.push('\n');
        }
    }
}

fn write_pairs(out: &mut String, dict: &Value) {
    let Value::Dict(entries) = dict else { return };
    for (key, value) in entries {
        gd_toml::write_key(out, &key.to_text().unwrap_or_default());
        out.push_str(" = ");
        write_value(out, value, true);
        out.push('\n');
    }
}

/// Writes `v` as a TOML value. Long top-level lists get one item per line.
fn write_value(out: &mut String, v: &Value, top: bool) {
    match v {
        Value::Str(s) => gd_toml::write_string(out, s),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(n) => {
            let _ = write!(out, "{n}");
        }
        Value::Float(f) => out.push_str(f),
        Value::None => out.push_str("\"\""),
        Value::List(items) => write_list(out, items, top),
        Value::Dict(entries) => {
            out.push('{');
            for (i, (key, value)) in entries.iter().enumerate() {
                out.push_str(if i > 0 { ", " } else { " " });
                gd_toml::write_key(out, &key.to_text().unwrap_or_default());
                out.push_str(" = ");
                write_value(out, value, false);
            }
            out.push_str(if entries.is_empty() { "}" } else { " }" });
        }
    }
}

fn write_list(out: &mut String, items: &[Value], top: bool) {
    let mut inline = String::from("[");
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            inline.push_str(", ");
        }
        write_value(&mut inline, item, false);
    }
    inline.push(']');
    if !top || inline.len() <= 72 {
        out.push_str(&inline);
        return;
    }
    out.push_str("[\n");
    for item in items {
        out.push_str("    ");
        write_value(out, item, false);
        out.push_str(",\n");
    }
    out.push(']');
}

/// Assembles conf.toml: settings, tables, then what was kept as comments.
pub(crate) fn finish(mut m: Migration<'_>) -> Conversion {
    if !m.sphinx_only.is_empty() {
        let message = format!(
            "{} configure Sphinx builders or extensions Guidedog does not have.",
            m.sphinx_only.join(", ")
        );
        m.tell(
            0,
            "migrate.sphinx-only",
            "SPHINX-ONLY SETTINGS KEPT AS COMMENTS",
            message,
            "They are at the end of conf.toml as comments; delete them when you no longer \
             build with Sphinx.",
        );
    }
    let base = m.path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    let mut toml = format!(
        "# Guidedog configuration, migrated from {base} by guidedog migrate.\n\
         # Setting names are Sphinx's conf.py names.\n\n"
    );
    toml.push_str(&m.plain);
    toml.push_str(&m.tables);
    if !m.comments.is_empty() {
        toml.push_str(
            "\n# Kept from the original for reference; Guidedog does not read these lines.\n",
        );
        toml.push_str(&m.comments);
    }
    Conversion { toml, notes: m.notes, converted: m.converted, attention: m.attention }
}

/// Loads `toml` as a build would (Sphinx's defaults, then the settings, then the checks),
/// so a migration never writes a conf.toml that does not load. Paths in it are relative
/// to the working folder.
///
/// Fails with `migrate.internal` (exit 1) naming what the loader refused.
pub fn check_conf_toml(toml: &str) -> Result<(), Problem> {
    let refused = match gd_toml::parse(toml) {
        Err(e) => e.message,
        Ok(table) => {
            let mut config = Config::new(Path::new(""), Path::new(""));
            let mut warnings = Vec::new();
            match gd_conf::apply_table(&mut config, &table, CONFIG_FILE, &mut warnings, true) {
                Ok(()) => return Ok(()),
                Err(p) => p.message,
            }
        }
    };
    Err(Problem::new(
        "migrate.internal",
        "THE CONVERTED CONF.TOML DOES NOT LOAD",
        CONFIG_FILE,
        format!("Guidedog wrote a conf.toml it cannot read: {refused}"),
        "This is a bug in guidedog migrate. Run it with -o - to see the output and report it \
         with your conf.py.",
        1,
    ))
}
