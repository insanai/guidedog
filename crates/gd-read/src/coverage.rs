//! sphinx.ext.coverage: the Python objects (and C items) the documents do not
//! describe. The modules are read by the source scan, never imported; `python.txt`
//! and `c.txt` keep Sphinx's layout.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use gd_autodoc::Session;
use gd_conf::{Config, Pattern};
use gd_report::Report;

/// The most modules one coverage run checks.
const MAX_MODULES: usize = 10_000;

/// What the documents describe: `py:` objects by full name, `py:` modules, and C
/// items as (object type, name).
#[derive(Debug, Clone, Default)]
pub struct Documented {
    /// Full names of the Python objects described.
    pub objects: HashSet<String>,
    /// The Python modules described.
    pub modules: BTreeSet<String>,
    /// The C items described, as (object type, name).
    pub c_items: HashSet<(String, String)>,
}

/// What the coverage builder writes and says.
#[derive(Debug, Clone, Default)]
pub struct Coverage {
    /// `python.txt`.
    pub python: String,
    /// `c.txt`.
    pub c: String,
    /// The statistics table's lines, for the terminal when
    /// `coverage_statistics_to_stdout` asks for them.
    pub statistics: Vec<String>,
    /// `coverage.module`, `coverage.undocumented` and `coverage.c` reports.
    pub reports: Vec<Report>,
}

/// One module's findings.
#[derive(Debug, Clone, Default)]
struct ModuleCoverage {
    funcs: Vec<String>,
    /// Class name to its undocumented methods; empty: the class itself is undocumented.
    classes: BTreeMap<String, Vec<String>>,
    documented: BTreeSet<String>,
    undocumented: BTreeSet<String>,
    /// Why the module could not be read.
    error: Option<String>,
}

/// The patterns of the settings, compiled to match as Python's `re.match` (at the
/// start) or `re.search` (anywhere); gd-conf has checked that they compile.
struct Ignores {
    modules: Vec<Pattern>,
    functions: Vec<Pattern>,
    classes: Vec<Pattern>,
    pyobjects: Vec<Pattern>,
}

fn prefix_patterns(patterns: &[String]) -> Vec<Pattern> {
    patterns.iter().filter_map(|p| Pattern::new(&format!("(?:{p}).*")).ok()).collect()
}

fn any_match(patterns: &[Pattern], name: &str) -> bool {
    patterns.iter().any(|p| p.full_match(name))
}

impl Ignores {
    fn new(c: &Config) -> Ignores {
        let k = &c.ext.coverage;
        let search = k.ignore_pyobjects.iter();
        Ignores {
            modules: prefix_patterns(&k.ignore_modules),
            functions: prefix_patterns(&k.ignore_functions),
            classes: prefix_patterns(&k.ignore_classes),
            pyobjects: search.filter_map(|p| Pattern::new(&format!(".*(?:{p}).*")).ok()).collect(),
        }
    }

    fn pyobj(&self, name: &str) -> bool {
        any_match(&self.pyobjects, name)
    }
}

/// Runs the coverage check of `config` over what `documented` describes.
pub fn coverage(config: &Config, documented: &Documented) -> Coverage {
    let mut out = Coverage::default();
    let roots = crate::autodoc::python_roots(config);
    let files = crate::files::SourceFiles::new(&roots);
    let reader = Box::new(move |path: &Path| files.read(path));
    let Ok(project) = gd_pyscan::Project::with_reader(&roots, reader) else { return out };
    let mut session = Session::new(project);
    let ignores = Ignores::new(config);
    let mut found: BTreeMap<String, ModuleCoverage> = BTreeMap::new();
    for name in module_names(config, &mut session, &ignores, documented, &mut out.reports) {
        if any_match(&ignores.modules, &name) || ignores.pyobj(&name) {
            continue;
        }
        let module = match gd_autodoc::module_objects(&mut session, &name) {
            Some(objects) => check_module(config, &ignores, documented, &name, objects),
            None => {
                out.reports.push(module_report(&name, &roots));
                ModuleCoverage { error: Some(not_found(&name)), ..ModuleCoverage::default() }
            }
        };
        found.insert(name, module);
    }
    out.statistics = statistics(&found);
    out.python = python_txt(config, &found, &out.statistics);
    if config.ext.coverage.show_missing_items {
        out.reports.extend(missing_items(&found));
    }
    out.c = crate::coverage_c::c_txt(config, documented, &mut out.reports);
    out
}

fn not_found(name: &str) -> String {
    format!("No module named '{name}' in the Python source")
}

/// `_determine_py_coverage_modules`: `coverage_modules` with their submodules, or the
/// modules the documents describe.
fn module_names(
    c: &Config,
    session: &mut Session,
    ignores: &Ignores,
    documented: &Documented,
    reports: &mut Vec<Report>,
) -> Vec<String> {
    let wanted = &c.ext.coverage.modules;
    if wanted.is_empty() {
        return documented.modules.iter().cloned().collect();
    }
    let ignored = |name: &str| any_match(&ignores.modules, name);
    let mut modules = BTreeSet::new();
    for name in wanted {
        match gd_autodoc::package_modules(session, name, &ignored, MAX_MODULES) {
            Some(found) => modules.extend(found),
            None => reports.push(module_report(name, &crate::autodoc::python_roots(c))),
        }
    }
    let extra: Vec<&String> = documented.modules.difference(&modules).collect();
    if !extra.is_empty() {
        let list = extra.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
        reports.push(warning(
            "coverage.module",
            "MODULES DOCUMENTED BUT NOT CHECKED",
            format!("The documents describe modules coverage_modules does not list: {list}."),
            "Add them to coverage_modules, or ignore them with coverage_ignore_modules.",
        ));
    }
    let missing: Vec<&String> = modules.difference(&documented.modules).collect();
    if !missing.is_empty() {
        let list = missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
        reports.push(warning(
            "coverage.module",
            "MODULES CHECKED BUT NOT DOCUMENTED",
            format!("coverage_modules lists modules no document describes: {list}."),
            "Document them, for example with automodule, or ignore them.",
        ));
    }
    modules.into_iter().collect()
}

/// `build_py_coverage` for one module.
fn check_module(
    c: &Config,
    ignores: &Ignores,
    documented: &Documented,
    name: &str,
    objects: gd_autodoc::ModuleObjects,
) -> ModuleCoverage {
    let skip_undoc = c.ext.coverage.skip_undoc_in_source;
    let seen = |full: &str| documented.objects.contains(full);
    let mut m = ModuleCoverage::default();
    for f in objects.functions {
        let full = format!("{name}.{}", f.name);
        if ignores.pyobj(&full) {
            continue;
        }
        if seen(&full) {
            m.documented.insert(full);
        } else if !any_match(&ignores.functions, &f.name) && !(skip_undoc && !f.has_doc) {
            m.funcs.push(f.name);
            m.undocumented.insert(full);
        }
    }
    for class in objects.classes {
        let full = format!("{name}.{}", class.name);
        if ignores.pyobj(&full) || any_match(&ignores.classes, &class.name) {
            continue;
        }
        if !seen(&full) {
            if !(skip_undoc && !class.has_doc) {
                m.classes.insert(class.name, Vec::new());
            }
            continue;
        }
        let mut attrs = Vec::new();
        for method in class.methods {
            let full_attr = format!("{full}.{}", method.name);
            if (skip_undoc && !method.has_doc) || ignores.pyobj(&full_attr) {
                continue;
            }
            if seen(&full_attr) {
                m.documented.insert(full_attr);
            } else {
                attrs.push(method.name);
                m.undocumented.insert(full_attr);
            }
        }
        if !attrs.is_empty() {
            m.classes.insert(class.name, attrs);
        }
    }
    m
}

/// `_write_table`: a grid table, the first row as its header.
fn table(rows: &[[String; 3]]) -> Vec<String> {
    let sizes: Vec<usize> =
        (0..3).map(|i| rows.iter().map(|r| r[i].chars().count()).max().unwrap_or(0) + 1).collect();
    let line = |sep: char| {
        let cells: String =
            sizes.iter().map(|s| format!("{}+", sep.to_string().repeat(s + 1))).collect();
        format!("+{cells}")
    };
    let mut out = vec![line('-')];
    for (k, row) in rows.iter().enumerate() {
        let cells: String =
            row.iter().zip(&sizes).map(|(cell, w)| format!("| {cell:<w$}")).collect();
        out.push(format!("{cells}|"));
        out.push(line(if k == 0 { '=' } else { '-' }));
    }
    out
}

/// `_write_py_statistics`: per module the share of objects described, then the total.
fn statistics(found: &BTreeMap<String, ModuleCoverage>) -> Vec<String> {
    let mut rows = vec![["Module", "Coverage", "Undocumented"].map(String::from)];
    let (mut all, mut all_documented) = (0usize, 0usize);
    for (name, m) in found.iter().filter(|(_, m)| m.error.is_none()) {
        let total = m.documented.len() + m.undocumented.len();
        let value =
            if total == 0 { 100.0 } else { 100.0 * m.documented.len() as f64 / total as f64 };
        rows.push([name.clone(), format!("{value:.2}%"), m.undocumented.len().to_string()]);
        all += total;
        all_documented += m.documented.len();
    }
    if all > 0 {
        let value = 100.0 * all_documented as f64 / all as f64;
        rows.push(["TOTAL".into(), format!("{value:.2}%"), (all - all_documented).to_string()]);
    } else {
        rows.push(["TOTAL", "100", "0"].map(String::from));
    }
    table(&rows)
}

/// A title and its underline, then a blank line (`write_header`).
fn header(out: &mut String, text: &str, mark: char) {
    out.push_str(text);
    out.push('\n');
    out.push_str(&mark.to_string().repeat(text.chars().count()));
    out.push_str("\n\n");
}

/// `write_py_coverage`.
fn python_txt(c: &Config, found: &BTreeMap<String, ModuleCoverage>, stats: &[String]) -> String {
    let k = &c.ext.coverage;
    let mut out = String::new();
    if k.write_headline {
        header(&mut out, "Undocumented Python objects", '=');
    }
    if k.statistics_to_report {
        header(&mut out, "Statistics", '-');
        for line in stats {
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
    }
    let mut failed = Vec::new();
    for (name, m) in found {
        if let Some(error) = &m.error {
            failed.push((name, error));
            continue;
        }
        if m.classes.is_empty() && m.funcs.is_empty() {
            continue;
        }
        header(&mut out, name, '-');
        if !m.funcs.is_empty() {
            out.push_str("Functions:\n");
            m.funcs.iter().for_each(|f| out.push_str(&format!(" * {f}\n")));
            out.push('\n');
        }
        if !m.classes.is_empty() {
            out.push_str("Classes:\n");
            for (class, methods) in &m.classes {
                if methods.is_empty() {
                    out.push_str(&format!(" * {class}\n"));
                    continue;
                }
                out.push_str(&format!(" * {class} -- missing methods:\n\n"));
                methods.iter().for_each(|x| out.push_str(&format!("   - {x}\n")));
            }
            out.push('\n');
        }
    }
    if !failed.is_empty() {
        header(&mut out, "Modules that failed to import", '-');
        for (name, error) in failed {
            out.push_str(&format!(" * {name} -- {error}\n"));
        }
    }
    out
}

/// `coverage.undocumented` for each object missing (`coverage_show_missing_items`).
fn missing_items(found: &BTreeMap<String, ModuleCoverage>) -> Vec<Report> {
    let mut out = Vec::new();
    for (name, m) in found {
        for f in &m.funcs {
            out.push(undocumented(format!("undocumented python function: {name} :: {f}")));
        }
        for (class, methods) in &m.classes {
            if methods.is_empty() {
                out.push(undocumented(format!("undocumented python class: {name} :: {class}")));
            }
            for x in methods {
                out.push(undocumented(format!(
                    "undocumented python method: {name} :: {class} :: {x}"
                )));
            }
        }
    }
    out
}

fn undocumented(message: String) -> Report {
    let report = Report::note("coverage.undocumented", "UNDOCUMENTED OBJECT", &message)
        .hint("Describe it in a document, or ignore it with a coverage_ignore_* setting.");
    Report { category: "coverage.undocumented".to_owned(), ..report }
}

pub(crate) fn warning(code: &str, title: &str, message: String, hint: &str) -> Report {
    let report = Report::warning(code, title, &message).hint(hint);
    Report { category: code.to_owned(), ..report }
}

fn module_report(name: &str, roots: &[PathBuf]) -> Report {
    let folders: Vec<String> = roots.iter().map(|r| r.display().to_string()).collect();
    let folders = if folders.is_empty() { "none".to_owned() } else { folders.join(", ") };
    warning(
        "coverage.module",
        "COVERAGE MODULE NOT FOUND",
        format!(
            "The module {name} is not in the Python source; the folders searched are {folders}."
        ),
        "Check the name, or add the folder holding its package to autodoc_source_paths.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_statistics_table_is_drawn_as_sphinx_draws_it() {
        let mut found = BTreeMap::new();
        let mut m = ModuleCoverage::default();
        m.documented.insert("pkg.a".into());
        m.undocumented.insert("pkg.b".into());
        found.insert("pkg".to_owned(), m);
        assert_eq!(
            statistics(&found),
            [
                "+--------+----------+--------------+",
                "| Module | Coverage | Undocumented |",
                "+========+==========+==============+",
                "| pkg    | 50.00%   | 1            |",
                "+--------+----------+--------------+",
                "| TOTAL  | 50.00%   | 1            |",
                "+--------+----------+--------------+",
            ]
        );
    }
}
