//! Storing conf.toml's values: `apply_table` reads every key of a parsed file into a
//! `Config`, by the settings table.

use std::collections::BTreeMap;

use gd_host::Problem;
use gd_report::{Report, Severity};
use gd_toml::{Data, Table, Value};

use crate::check::check_config;
use crate::config::{Config, FlagOrName};
use crate::custom;
use crate::settings::{Setting, SettingKind, Store, find_setting, is_foreign_setting, suggestion};

/// The problem of a setting that is not right: `config.invalid`, exit status 2, at
/// `path` and `line` (0: no line).
pub(crate) fn config_problem(path: &str, line: usize, message: String, hint: &str) -> Problem {
    let location = if line == 0 { path.to_string() } else { format!("{path}:{line}") };
    Problem::new("config.invalid", "INVALID CONFIGURATION", &location, message, hint, 2)
}

/// A configuration warning at `path` and `line` (0: no line).
pub(crate) fn warning(
    path: &str,
    line: usize,
    code: &str,
    title: &str,
    message: String,
    hint: &str,
) -> Report {
    Report {
        code: code.to_string(),
        severity: Severity::Warning,
        title: title.to_string(),
        message,
        hints: vec![hint.to_string()],
        path: path.to_string(),
        line,
        ..Report::default()
    }
}

/// Stores every key of `table`, a parsed conf.toml (or a `-D` override, `path` `"-D"`),
/// into `config`, adding warnings for ignored Sphinx settings to `warnings`. The first
/// error stops it. With `check`, the settings are then checked together
/// ([`check_config`]); a caller with `-D` overrides still to apply checks once they are.
///
/// Fails with `config.invalid` (exit 2) for an unknown key, with a "Did you mean" hint,
/// or a value of the wrong type.
pub fn apply_table(
    config: &mut Config,
    table: &Table,
    path: &str,
    warnings: &mut Vec<Report>,
    check: bool,
) -> Result<(), Problem> {
    for (key, value) in table.iter() {
        let Some(setting) = find_setting(key) else {
            if is_foreign_setting(key) {
                warnings.push(warning(
                    path,
                    value.line,
                    "config.ignored",
                    "SETTING IGNORED",
                    format!(
                        "{key} configures a Sphinx builder or extension Guidedog does not \
                         have; it is ignored."
                    ),
                    "Remove it, or keep it for a Sphinx build of the same sources.",
                ));
                continue;
            }
            let message = format!("Unknown setting {key}.");
            return Err(config_problem(path, value.line, message, &suggestion(key)));
        };
        store(config, setting, value, path)?;
    }
    if check {
        check_config(config, path, warnings)?;
    }
    Ok(())
}

fn wrong(path: &str, v: &Value, name: &str, expected: &str) -> Problem {
    config_problem(
        path,
        v.line,
        format!("{name} must be {expected}."),
        "Compare with the example in the file guidedog quickstart writes.",
    )
}

/// Stores one value by its setting's kind.
pub(crate) fn store(
    config: &mut Config,
    setting: &Setting,
    v: &Value,
    path: &str,
) -> Result<(), Problem> {
    let name = setting.name;
    match setting.store {
        Store::Custom => custom::store(config, setting.kind, v, path),
        Store::String(field) => {
            *field(config) = string_value(setting.kind, v, path, name)?;
            Ok(())
        }
        Store::Bool(field) => {
            let Data::Boolean(b) = v.data else {
                return Err(wrong(path, v, name, "true or false"));
            };
            *field(config) = b;
            Ok(())
        }
        Store::Int(field) => {
            let Data::Integer(n) = v.data else {
                return Err(wrong(path, v, name, "a whole number"));
            };
            if name == "plugin_timeout" {
                check_plugin_timeout(path, v.line, n)?;
            }
            *field(config) = n;
            Ok(())
        }
        Store::Strings(field) => {
            *field(config) = strings_value(setting.kind, v, path, name)?;
            Ok(())
        }
        Store::Table(field) => {
            // Keys add to the table, so numfig_format = {figure = ".."} keeps the
            // other kinds' formats, as in Sphinx.
            let table =
                string_table(v).ok_or_else(|| wrong(path, v, name, "a table of strings"))?;
            field(config).extend(table);
            Ok(())
        }
        Store::Pairs(field) => {
            *field(config) = pairs(v, path, name)?;
            Ok(())
        }
        Store::Values(field) => {
            let Data::Table(t) = &v.data else { return Err(wrong(path, v, name, "a table")) };
            *field(config) = t.clone();
            Ok(())
        }
        Store::OptionalString(field) => {
            let Data::String(s) = &v.data else {
                return Err(wrong(path, v, name, "a string in quotes"));
            };
            *field(config) = Some(s.clone());
            Ok(())
        }
        Store::FlagOrName(field) => {
            *field(config) = match &v.data {
                Data::Boolean(b) => FlagOrName::Flag(*b),
                Data::String(s) => FlagOrName::Name(s.clone()),
                _ => return Err(wrong(path, v, name, "true, false, or a name in quotes")),
            };
            Ok(())
        }
    }
}

fn string_value(kind: SettingKind, v: &Value, path: &str, name: &str) -> Result<String, Problem> {
    if kind == SettingKind::Json {
        if !matches!(v.data, Data::Table(_)) {
            return Err(wrong(path, v, name, "a table"));
        }
        return Ok(crate::json::json_dumps(v));
    }
    match &v.data {
        Data::String(s) => Ok(s.clone()),
        _ => Err(wrong(path, v, name, "a string in quotes")),
    }
}

fn strings_value(
    kind: SettingKind,
    v: &Value,
    path: &str,
    name: &str,
) -> Result<Vec<String>, Problem> {
    if kind == SettingKind::Suffixes {
        return suffixes(v).ok_or_else(|| wrong(path, v, name, "a suffix or a list of suffixes"));
    }
    string_list(v).ok_or_else(|| wrong(path, v, name, "a list of strings, such as [\"a\", \"b\"]"))
}

/// A string, or a list of strings; `None` for anything else.
pub(crate) fn string_list(v: &Value) -> Option<Vec<String>> {
    match &v.data {
        Data::String(s) => Some(vec![s.clone()]),
        Data::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect(),
        _ => None,
    }
}

/// `".rst"`, `[".rst", ".md"]`, or Sphinx's `{".rst" = "restructuredtext"}` (its keys).
fn suffixes(v: &Value) -> Option<Vec<String>> {
    match &v.data {
        Data::Table(t) => Some(t.keys().map(str::to_string).collect()),
        _ => string_list(v),
    }
}

/// A table of strings; booleans and integers are kept as their text.
fn string_table(v: &Value) -> Option<BTreeMap<String, String>> {
    let Data::Table(t) = &v.data else { return None };
    let mut out = BTreeMap::new();
    for (key, item) in t.iter() {
        let text = match &item.data {
            Data::String(s) => s.clone(),
            Data::Boolean(b) => b.to_string(),
            Data::Integer(n) => n.to_string(),
            _ => return None,
        };
        out.insert(key.to_string(), text);
    }
    Some(out)
}

/// A list of string pairs, such as `nitpick_ignore`.
pub(crate) fn pairs(v: &Value, path: &str, name: &str) -> Result<Vec<(String, String)>, Problem> {
    let Data::Array(items) = &v.data else {
        return Err(config_problem(
            path,
            v.line,
            format!("{name} must be a list of pairs."),
            &format!("Write {name} = [[\"py:class\", \"MyType\"]]."),
        ));
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match string_list(item).as_deref() {
            Some([a, b]) => out.push((a.clone(), b.clone())),
            _ => {
                return Err(config_problem(
                    path,
                    item.line,
                    format!("Each {name} entry is a pair."),
                    "Write [\"py:class\", \"MyType\"].",
                ));
            }
        }
    }
    Ok(out)
}

/// Refuses a `plugin_timeout` outside 1 to 86400 seconds.
pub(crate) fn check_plugin_timeout(path: &str, line: usize, seconds: i64) -> Result<(), Problem> {
    if (1..=86400).contains(&seconds) {
        return Ok(());
    }
    Err(config_problem(
        path,
        line,
        "plugin_timeout must be between 1 and 86400 seconds.".to_string(),
        "Set plugin_timeout = 60 for one minute per plugin request.",
    ))
}
