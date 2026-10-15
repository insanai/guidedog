//! Checking the settings of the extensions GDS 0011 adds: placeholders, patterns,
//! folders and numbers that would only fail later, in the middle of a build.

use gd_host::Problem;
use gd_report::Report;

use crate::check::{check_format_fields, invalid};
use crate::config::Config;
use crate::extensions::LINKCODE_FIELDS;
use crate::pattern::Pattern;
use crate::store::warning;

/// Checks the napoleon, apidoc, linkcode, inheritance, coverage and duration settings.
/// A linkcode extension without a URL is a warning; the rest fail with
/// `config.invalid`.
pub(crate) fn check_extensions(
    config: &Config,
    path: &str,
    warnings: &mut Vec<Report>,
) -> Result<(), Problem> {
    check_apidoc(config, path)?;
    check_linkcode(config, path, warnings)?;
    check_inheritance(config, path)?;
    check_coverage(config, path)?;
    check_duration(config, path)
}

/// Whether `folder` names a folder inside the one it is relative to.
fn is_inside(folder: &str) -> bool {
    let folder = folder.replace('\\', "/");
    !folder.is_empty()
        && !folder.starts_with('/')
        && !folder.contains(':')
        && !folder.split('/').any(|part| part == "..")
}

fn check_apidoc(config: &Config, path: &str) -> Result<(), Problem> {
    let a = &config.ext.apidoc;
    let depths = a.modules.iter().filter_map(|m| m.max_depth).chain([a.max_depth]);
    if let Some(depth) = depths.into_iter().find(|d| *d < 0) {
        return Err(invalid(
            path,
            format!("apidoc_max_depth is {depth}; a depth cannot be negative."),
            "Use 4, Sphinx's default, or 0 for no submodule entries.",
        ));
    }
    if let Some(m) = a.modules.iter().find(|m| !is_inside(&m.destination)) {
        return Err(invalid(
            path,
            format!(
                "apidoc_modules has the destination {:?}, which is not a folder inside the \
                 source folder.",
                m.destination
            ),
            "Give a folder relative to the source folder, such as \"api\".",
        ));
    }
    Ok(())
}

fn check_linkcode(config: &Config, path: &str, warnings: &mut Vec<Report>) -> Result<(), Problem> {
    let url = &config.ext.linkcode.url;
    if let Err(why) = check_format_fields(url, LINKCODE_FIELDS, "linkcode_url") {
        return Err(invalid(
            path,
            format!("linkcode_url: {why}."),
            "Use the fields {domain}, {module}, {fullname}, {path}, {line}, {end_line} and \
             {branch}; write {{ and }} for a brace.",
        ));
    }
    if url.is_empty() && config.has_extension("sphinx.ext.linkcode") {
        warnings.push(warning(
            path,
            0,
            "config.ext.linkcode",
            "LINKCODE HAS NO URL",
            "sphinx.ext.linkcode is listed, but linkcode_url is not set, so no source links \
             are added."
                .into(),
            "Set linkcode_url, for example \
             \"https://github.com/o/r/blob/{branch}/{path}#L{line}-L{end_line}\".",
        ));
    }
    Ok(())
}

/// Whether `name` is a Graphviz identifier, so it cannot change the DOT around it.
fn is_graphviz_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn check_inheritance(config: &Config, path: &str) -> Result<(), Problem> {
    let i = &config.ext.inheritance;
    let tables = [
        ("inheritance_graph_attrs", &i.graph_attrs),
        ("inheritance_node_attrs", &i.node_attrs),
        ("inheritance_edge_attrs", &i.edge_attrs),
    ];
    for (setting, table) in tables {
        if let Some(name) = table.keys().find(|k| !is_graphviz_name(k)) {
            return Err(invalid(
                path,
                format!("{setting} has the attribute {name:?}, which is not a Graphviz name."),
                "Use Graphviz attribute names, such as rankdir, fontsize or color.",
            ));
        }
    }
    Ok(())
}

fn check_coverage(config: &Config, path: &str) -> Result<(), Problem> {
    let c = &config.ext.coverage;
    let lists = [
        ("coverage_ignore_modules", &c.ignore_modules),
        ("coverage_ignore_functions", &c.ignore_functions),
        ("coverage_ignore_classes", &c.ignore_classes),
        ("coverage_ignore_pyobjects", &c.ignore_pyobjects),
    ];
    let named =
        lists.into_iter().flat_map(|(setting, list)| list.iter().map(move |p| (setting, p)));
    let c_regexes = c.c_regexes.values().map(|p| ("coverage_c_regexes", p));
    let c_items = c.ignore_c_items.values().flatten().map(|p| ("coverage_ignore_c_items", p));
    for (setting, pattern) in named.chain(c_regexes).chain(c_items) {
        if let Err(e) = Pattern::new(pattern) {
            return Err(invalid(
                path,
                format!("{setting} has the pattern {pattern:?}, which does not compile: {e}."),
                "Write a regular expression in the subset nitpick_ignore_regex uses, and \
                 escape dots as \\\\. in names.",
            ));
        }
    }
    Ok(())
}

fn check_duration(config: &Config, path: &str) -> Result<(), Problem> {
    let d = &config.ext.duration;
    if d.n_slowest < 0 {
        return Err(invalid(
            path,
            format!("duration_n_slowest is {}; it cannot be negative.", d.n_slowest),
            "Use 5, Sphinx's default, or 0 to list every document.",
        ));
    }
    if let Some(limit) = d.limit
        && !(limit.is_finite() && limit > 0.0)
    {
        return Err(invalid(
            path,
            format!("duration_limit is {limit}; it must be a positive number of seconds."),
            "Use a number such as 2.5, or remove the setting for no limit.",
        ));
    }
    if !d.write_json.is_empty() && !is_inside(&d.write_json) {
        return Err(invalid(
            path,
            format!(
                "duration_write_json is {:?}, which leads outside the output folder.",
                d.write_json
            ),
            "Give a file name relative to the output folder, or \"\" to write no file.",
        ));
    }
    Ok(())
}
