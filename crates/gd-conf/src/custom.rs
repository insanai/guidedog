//! The settings whose values need a reader of their own: books, extlinks, declared
//! object types, directive aliases, nitpick patterns, intersphinx and autodoc options.
//!
//! Object types a project declares replace what a Sphinx extension's `setup()` does in
//! Python with `app.add_object_type` and `app.add_crossref_type`:
//!
//! ```toml
//! [[object_types]]              # app.add_object_type
//! directive = "django-admin"
//! role = "djadmin"
//! index = "pair: %s; django-admin command"
//! name = "first-word"           # the object's name: "whole" (default) or "first-word"
//! display = "django-admin %s"   # how the signature is shown
//! program = true                # options that follow belong to it
//!
//! [[crossref_types]]            # app.add_crossref_type
//! directive = "setting"
//! role = "setting"
//!
//! [directive_aliases]           # app.add_directive(name, one of Sphinx's own)
//! django-admin-option = "option"
//! ```

use gd_host::Problem;
use gd_toml::{Data, Value};

use crate::config::{
    AutodocDefaultOption, Config, IntersphinxProject, MAX_OBJECT_TYPES, ObjectTypeDecl,
    PdfDocument,
};
use crate::pattern::Pattern;
use crate::settings::SettingKind;
use crate::store::{config_problem, pairs, string_list};

/// Stores a value of one of the kinds with their own reader.
pub(crate) fn store(
    config: &mut Config,
    kind: SettingKind,
    v: &Value,
    path: &str,
) -> Result<(), Problem> {
    match kind {
        SettingKind::PdfDocuments => config.pdf.documents = pdf_documents(v, path)?,
        SettingKind::Extlinks => extlinks(config, v, path)?,
        SettingKind::ObjectTypes => declared_types(config, v, path, false)?,
        SettingKind::CrossrefTypes => declared_types(config, v, path, true)?,
        SettingKind::DirectiveAliases => config.directive_aliases = directive_aliases(v, path)?,
        SettingKind::PatternPairs => config.nitpick_ignore_regex = pattern_pairs(v, path)?,
        SettingKind::Intersphinx => config.intersphinx.mapping = intersphinx_mapping(v, path)?,
        SettingKind::AutodocOptions => {
            config.autodoc.default_options = autodoc_default_options(v, path)?;
        }
        SettingKind::String
        | SettingKind::Bool
        | SettingKind::Int
        | SettingKind::Strings
        | SettingKind::Table
        | SettingKind::Suffixes
        | SettingKind::Pairs
        | SettingKind::Json
        | SettingKind::Values
        | SettingKind::OptionalString
        | SettingKind::FlagOrName => {
            unreachable!("the settings table gives {kind:?} a field of its own")
        }
    }
    Ok(())
}

fn pdf_documents(v: &Value, path: &str) -> Result<Vec<PdfDocument>, Problem> {
    let Data::Array(items) = &v.data else {
        return Err(config_problem(
            path,
            v.line,
            "pdf_documents must be a list of tables.".into(),
            "Write pdf_documents = [{root = \"index\", file = \"project.pdf\"}].",
        ));
    };
    let mut docs = Vec::with_capacity(items.len());
    for item in items {
        let Some(table) = item.as_table() else {
            return Err(config_problem(
                path,
                item.line,
                "pdf_documents must be a list of tables.".into(),
                "Write pdf_documents = [{root = \"index\", file = \"project.pdf\"}].",
            ));
        };
        let mut d = PdfDocument::default();
        for (key, value) in table.iter() {
            let Some(text) = value.as_str() else {
                return Err(config_problem(
                    path,
                    value.line,
                    "pdf_documents values are strings.".into(),
                    "Quote each value.",
                ));
            };
            let field = match key {
                "root" => &mut d.root,
                "file" => &mut d.file,
                "title" => &mut d.title,
                "author" => &mut d.author,
                _ => {
                    return Err(config_problem(
                        path,
                        value.line,
                        format!("pdf_documents has no field {key}."),
                        "Use root, file, title, and author.",
                    ));
                }
            };
            *field = text.to_string();
        }
        docs.push(d);
    }
    Ok(docs)
}

fn extlinks(config: &mut Config, v: &Value, path: &str) -> Result<(), Problem> {
    let Data::Table(table) = &v.data else {
        return Err(config_problem(
            path,
            v.line,
            "extlinks must be a table.".into(),
            "Write [extlinks] issue = [\"https://example.org/%s\", \"issue %s\"].",
        ));
    };
    for (key, item) in table.iter() {
        match string_list(item).as_deref() {
            Some([url, caption]) => {
                config.extlinks.insert(key.to_string(), (url.clone(), caption.clone()));
            }
            _ => {
                return Err(config_problem(
                    path,
                    item.line,
                    format!("extlinks.{key} must be a URL pattern and a caption pattern."),
                    "Write both patterns with %s where the name goes, for example \
                     [\"https://x/%s\", \"#%s\"].",
                ));
            }
        }
    }
    Ok(())
}

fn declared_types(
    config: &mut Config,
    v: &Value,
    path: &str,
    target: bool,
) -> Result<(), Problem> {
    let setting = if target { "crossref_types" } else { "object_types" };
    let Data::Array(items) = &v.data else {
        return Err(config_problem(
            path,
            v.line,
            format!("{setting} must be a list of tables."),
            &format!("Write [[{setting}]] with directive = \"name\"."),
        ));
    };
    for item in items {
        let mut t = ObjectTypeDecl { target, ..ObjectTypeDecl::default() };
        object_type_fields(&mut t, item, path, setting)?;
        if t.directive.is_empty() {
            return Err(config_problem(
                path,
                item.line,
                format!("A {setting} entry has no directive."),
                "Give each entry directive = \"name\".",
            ));
        }
        if config.object_types.len() >= MAX_OBJECT_TYPES {
            return Err(config_problem(
                path,
                item.line,
                format!("More than {MAX_OBJECT_TYPES} object types are declared."),
                "Declare fewer object types.",
            ));
        }
        config.object_types.push(t);
    }
    Ok(())
}

fn object_type_fields(
    t: &mut ObjectTypeDecl,
    item: &Value,
    path: &str,
    setting: &str,
) -> Result<(), Problem> {
    let Some(table) = item.as_table() else {
        return Err(config_problem(
            path,
            item.line,
            format!("{setting} must be a list of tables."),
            &format!("Write [[{setting}]] with directive = \"name\"."),
        ));
    };
    for (key, value) in table.iter() {
        let known = matches!(key, "directive" | "role" | "index")
            || (!t.target && matches!(key, "display" | "name" | "program"));
        if !known {
            let fields = if t.target {
                "directive, role, and index"
            } else {
                "directive, role, index, name, display, and program"
            };
            return Err(config_problem(
                path,
                value.line,
                format!("{setting} has no field {key}."),
                &format!("Use {fields}."),
            ));
        }
        object_type_field(t, key, value, path, setting)?;
    }
    Ok(())
}

fn object_type_field(
    t: &mut ObjectTypeDecl,
    key: &str,
    value: &Value,
    path: &str,
    setting: &str,
) -> Result<(), Problem> {
    if key == "program" {
        let Data::Boolean(b) = value.data else {
            return Err(config_problem(
                path,
                value.line,
                "program is true or false.".into(),
                "Write program = true.",
            ));
        };
        t.program = b;
        return Ok(());
    }
    let Some(text) = value.as_str() else {
        return Err(config_problem(
            path,
            value.line,
            format!("{setting}.{key} is a string."),
            "Quote the value.",
        ));
    };
    match key {
        "directive" => t.directive = text.to_string(),
        "role" => t.role = text.to_string(),
        "index" => t.index = text.to_string(),
        "display" => t.display = text.to_string(),
        _ => {
            if text != "whole" && text != "first-word" {
                return Err(config_problem(
                    path,
                    value.line,
                    "name is \"whole\" or \"first-word\".".into(),
                    "Set name to \"whole\" (the whole signature) or \"first-word\" (the \
                     signature's first word).",
                ));
            }
            t.first_word = text == "first-word";
        }
    }
    Ok(())
}

fn directive_aliases(v: &Value, path: &str) -> Result<Vec<(String, String)>, Problem> {
    let Data::Table(table) = &v.data else {
        return Err(config_problem(
            path,
            v.line,
            "directive_aliases must be a table.".into(),
            "Write [directive_aliases] django-admin-option = \"option\".",
        ));
    };
    let mut out = Vec::with_capacity(table.len());
    for (key, value) in table.iter() {
        let Some(text) = value.as_str() else {
            return Err(config_problem(
                path,
                value.line,
                format!("directive_aliases.{key} names a directive."),
                "Quote the directive name.",
            ));
        };
        out.push((key.to_string(), text.to_string()));
    }
    Ok(out)
}

/// Reads `nitpick_ignore_regex` and compiles it; as Sphinx uses `re.fullmatch`, each
/// pattern must match the whole type or target.
fn pattern_pairs(v: &Value, path: &str) -> Result<Vec<(Pattern, Pattern)>, Problem> {
    let written = pairs(v, path, "nitpick_ignore_regex")?;
    let lines: Vec<usize> = match &v.data {
        Data::Array(items) => items.iter().map(|i| i.line).collect(),
        _ => Vec::new(),
    };
    let mut out = Vec::with_capacity(written.len());
    for (i, (kind, target)) in written.iter().enumerate() {
        let line = lines.get(i).copied().unwrap_or(v.line);
        out.push((compile(kind, path, line)?, compile(target, path, line)?));
    }
    Ok(out)
}

fn compile(pattern: &str, path: &str, line: usize) -> Result<Pattern, Problem> {
    Pattern::new(pattern).map_err(|e| {
        config_problem(
            path,
            line,
            format!("{pattern:?} is not a regular expression I can read ({e})."),
            "Escape special characters with a backslash, as in py:class\\.Name.",
        )
    })
}

/// Reads `intersphinx_mapping` and checks it as Sphinx's `validate_intersphinx_mapping`
/// does: each value is a target URI and a location, or a list of locations; target URIs
/// are non-empty and unique. The result is sorted by name.
fn intersphinx_mapping(v: &Value, path: &str) -> Result<Vec<IntersphinxProject>, Problem> {
    let Data::Table(table) = &v.data else {
        return Err(config_problem(
            path,
            v.line,
            "intersphinx_mapping must be a table.".into(),
            "Write [intersphinx_mapping] python = [\"https://docs.python.org/3/\", \"\"].",
        ));
    };
    let mut out: Vec<IntersphinxProject> = Vec::with_capacity(table.len());
    for (key, value) in table.iter() {
        let project = intersphinx_entry(key, value, path)?;
        if let Some(other) = out.iter().find(|o| o.uri == project.uri) {
            return Err(config_problem(
                path,
                value.line,
                format!(
                    "intersphinx_mapping.{key} repeats the target URI of \
                     intersphinx_mapping.{}.",
                    other.name
                ),
                "Target URIs must be unique; keep one of the two entries.",
            ));
        }
        out.push(project);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn intersphinx_entry(key: &str, value: &Value, path: &str) -> Result<IntersphinxProject, Problem> {
    let (uri, location) = match value.as_array() {
        Some([uri, location]) if uri.as_str().is_some() => (uri.as_str(), location),
        _ => {
            return Err(config_problem(
                path,
                value.line,
                format!(
                    "intersphinx_mapping.{key} must be a target URI and an inventory location."
                ),
                "Write [\"https://example.org/\", \"\"]; \"\" means the objects.inv at that URI.",
            ));
        }
    };
    let uri = uri.unwrap_or_default();
    if uri.is_empty() {
        return Err(config_problem(
            path,
            value.line,
            format!("intersphinx_mapping.{key} has no target URI."),
            "Give the URL of the other project's pages.",
        ));
    }
    let locations = string_list(location).filter(|l| !l.is_empty()).ok_or_else(|| {
        config_problem(
            path,
            value.line,
            format!(
                "The inventory location of intersphinx_mapping.{key} must be a string or a \
                 list of strings."
            ),
            "Use \"\" for the objects.inv at the target URI.",
        )
    })?;
    Ok(IntersphinxProject { name: key.to_string(), uri: uri.to_string(), locations })
}

/// Reads `autodoc_default_options`: each option set to a string or `true` (`false`
/// leaves the option out).
fn autodoc_default_options(v: &Value, path: &str) -> Result<Vec<AutodocDefaultOption>, Problem> {
    let Data::Table(table) = &v.data else {
        return Err(config_problem(
            path,
            v.line,
            "autodoc_default_options must be a table.".into(),
            "Write [autodoc_default_options] with lines such as members = true.",
        ));
    };
    let mut out = Vec::with_capacity(table.len());
    for (key, item) in table.iter() {
        let name = key.to_string();
        match &item.data {
            Data::String(value) => {
                out.push(AutodocDefaultOption { name, value: value.clone(), flag: false });
            }
            Data::Boolean(true) => {
                out.push(AutodocDefaultOption { name, value: String::new(), flag: true });
            }
            Data::Boolean(false) => {}
            _ => {
                return Err(config_problem(
                    path,
                    item.line,
                    format!("autodoc_default_options.{key} must be a string or true."),
                    "Write the option's value as it would follow the option, or true for a \
                     flag.",
                ));
            }
        }
    }
    Ok(out)
}
