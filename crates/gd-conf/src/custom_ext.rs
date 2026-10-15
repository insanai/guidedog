//! The readers of the extension settings whose values need more than one field:
//! `napoleon_custom_sections` and `apidoc_modules` (GDS 0011).
//!
//! ```toml
//! napoleon_custom_sections = ["Side Effects", ["Arguments", "params_style"]]
//!
//! [[apidoc_modules]]
//! path = "../src/shapes"      # the package folder, relative to conf.toml
//! destination = "api"         # the pages' folder, inside the source folder
//! separate_modules = true     # any apidoc_* setting, by its short name
//! ```

use gd_host::Problem;
use gd_toml::{Data, Value};

use crate::extensions::{ApidocModule, NapoleonSection};
use crate::store::{config_problem, string_list};

/// The keys an `apidoc_modules` entry may have.
pub const APIDOC_MODULE_KEYS: &[&str] = &[
    "path",
    "destination",
    "exclude_patterns",
    "max_depth",
    "follow_links",
    "separate_modules",
    "include_private",
    "no_headings",
    "module_first",
    "implicit_namespaces",
    "automodule_options",
];

/// Reads `napoleon_custom_sections`: each item a name, or a name and the section it
/// behaves like.
pub(crate) fn napoleon_sections(v: &Value, path: &str) -> Result<Vec<NapoleonSection>, Problem> {
    let Data::Array(items) = &v.data else {
        return Err(config_problem(
            path,
            v.line,
            "napoleon_custom_sections must be a list.".into(),
            "Write napoleon_custom_sections = [\"Side Effects\", [\"Arguments\", \
             \"params_style\"]].",
        ));
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let section = match (&item.data, string_list(item).as_deref()) {
            (Data::String(name), _) => NapoleonSection { name: name.clone(), like: String::new() },
            (Data::Array(_), Some([name, like])) => {
                NapoleonSection { name: name.clone(), like: like.clone() }
            }
            _ => {
                return Err(config_problem(
                    path,
                    item.line,
                    "Each napoleon_custom_sections entry is a section name, or a name and the \
                     section it behaves like."
                        .into(),
                    "Write \"Side Effects\", or [\"Arguments\", \"params_style\"].",
                ));
            }
        };
        if section.name.trim().is_empty() {
            return Err(config_problem(
                path,
                item.line,
                "A napoleon_custom_sections entry has an empty name.".into(),
                "Write the section's header as docstrings spell it, such as \"Side Effects\".",
            ));
        }
        out.push(section);
    }
    Ok(out)
}

/// Reads `apidoc_modules`: a list of tables, each with `path` and `destination` and any
/// of the general settings by their short names.
pub(crate) fn apidoc_modules(v: &Value, path: &str) -> Result<Vec<ApidocModule>, Problem> {
    let Data::Array(items) = &v.data else {
        return Err(not_tables(path, v));
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let Some(table) = item.as_table() else { return Err(not_tables(path, item)) };
        let mut module = ApidocModule::default();
        for (key, value) in table.iter() {
            apidoc_field(&mut module, key, value, path)?;
        }
        for (field, given) in [("path", &module.path), ("destination", &module.destination)] {
            if given.is_empty() {
                return Err(config_problem(
                    path,
                    item.line,
                    format!("An apidoc_modules entry has no {field}."),
                    "Give both: {path = \"../src/package\", destination = \"api\"}.",
                ));
            }
        }
        out.push(module);
    }
    Ok(out)
}

fn not_tables(path: &str, v: &Value) -> Problem {
    config_problem(
        path,
        v.line,
        "apidoc_modules must be a list of tables.".into(),
        "Write [[apidoc_modules]] with path = \"../src/package\" and destination = \"api\".",
    )
}

/// Stores one key of an `apidoc_modules` entry.
fn apidoc_field(m: &mut ApidocModule, key: &str, v: &Value, path: &str) -> Result<(), Problem> {
    let wrong = |expected: &str| {
        config_problem(
            path,
            v.line,
            format!("apidoc_modules: {key} must be {expected}."),
            &format!("Write it as apidoc_{key} is written."),
        )
    };
    let flag = || match v.data {
        Data::Boolean(b) => Ok(Some(b)),
        _ => Err(wrong("true or false")),
    };
    match key {
        "path" | "destination" => {
            let text = v.as_str().ok_or_else(|| wrong("a string in quotes"))?.to_string();
            if key == "path" { m.path = text } else { m.destination = text }
        }
        "exclude_patterns" | "automodule_options" => {
            let list = string_list(v).ok_or_else(|| wrong("a list of strings"))?;
            if key == "exclude_patterns" {
                m.exclude_patterns = Some(list);
            } else {
                m.automodule_options = Some(list);
            }
        }
        "max_depth" => {
            let Data::Integer(n) = v.data else { return Err(wrong("a whole number")) };
            m.max_depth = Some(n);
        }
        "follow_links" => m.follow_links = flag()?,
        "separate_modules" => m.separate_modules = flag()?,
        "include_private" => m.include_private = flag()?,
        "no_headings" => m.no_headings = flag()?,
        "module_first" => m.module_first = flag()?,
        "implicit_namespaces" => m.implicit_namespaces = flag()?,
        _ => {
            let best = APIDOC_MODULE_KEYS
                .iter()
                .map(|k| (gd_report::edit_distance(key, k), *k))
                .min()
                .filter(|(d, _)| *d <= 3);
            let hint = match best {
                Some((_, k)) => format!("Did you mean {k}?"),
                None => format!("Use {}.", APIDOC_MODULE_KEYS.join(", ")),
            };
            return Err(config_problem(
                path,
                v.line,
                format!("apidoc_modules has no key {key}."),
                &hint,
            ));
        }
    }
    Ok(())
}
