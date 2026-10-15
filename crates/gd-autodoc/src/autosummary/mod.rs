//! `sphinx.ext.autosummary` from the source scan: the summary table of an
//! `autosummary` directive (each object's name, shortened signature and first
//! sentence), and what a stub page's template is given about an object.
//!
//! ```no_run
//! use gd_autodoc::{Config, Session, SummaryRequest, summarize, summary_table};
//!
//! let mut session = Session::new(gd_pyscan::Project::new(&["src"]).expect("grammar"));
//! let names = vec!["shapes.Square".to_owned(), "shapes.area".to_owned()];
//! let summary = summarize(&mut session, &Config::default(), &SummaryRequest {
//!     names: &names,
//!     ..SummaryRequest::default()
//! });
//! println!("{}", summary_table(&summary.items, &[]).join("\n"));
//! ```

mod find;
mod signature;
mod stub;
mod summary;

use std::collections::HashSet;

pub(crate) use stub::package_entries;
pub use stub::{Stub, StubOptions, stub};

use crate::options::Options;
use crate::props::load_object;
use crate::{Config, Generator, Message, ObjType, Request, Session, Severity};
use find::{Found, find_object, import_prefixes, object_type};

/// How an autosummary table shows signatures (`:signatures:`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SummarySignatures {
    /// `long`: the parameters, shortened to fit.
    #[default]
    Long,
    /// `short`: `()` or `(…)`.
    Short,
    /// `none` (`:nosignatures:`): no signature.
    Omitted,
}

impl SummarySignatures {
    /// Reads the option's value; `None` for one Sphinx does not know.
    pub fn parse(s: &str) -> Option<SummarySignatures> {
        match s {
            "long" => Some(SummarySignatures::Long),
            "short" => Some(SummarySignatures::Short),
            "none" => Some(SummarySignatures::Omitted),
            _ => None,
        }
    }
}

/// One autosummary directive's entries and the document's context.
#[derive(Debug, Clone, Copy, Default)]
pub struct SummaryRequest<'a> {
    /// The names, as written (`~` shows only the last part).
    pub names: &'a [String],
    /// The current `py:module`, or `""`.
    pub module: &'a str,
    /// The current `py:class`, or `""`.
    pub class: &'a str,
    /// How signatures show.
    pub signatures: SummarySignatures,
}

/// One row of a summary table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SummaryItem {
    /// The name shown.
    pub display_name: String,
    /// The shortened signature; `None` without signatures.
    pub signature: Option<String>,
    /// The first sentence of the docstring.
    pub summary: String,
    /// The full name the row links to.
    pub real_name: String,
}

/// The rows of a table and what to report about them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Summary {
    /// The rows, in the order named; names that were not found have none.
    pub items: Vec<SummaryItem>,
    /// `autosummary.import` and `autosummary.import_cycle` messages.
    pub messages: Vec<Message>,
}

/// The characters Sphinx's `rst.escape` escapes: ASCII punctuation but `.`.
fn rst_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for (i, c) in s.chars().enumerate() {
        if (c.is_ascii_punctuation() && c != '.') || (i == 0 && c == '.') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A generator for work outside a directive: no options, no document context.
pub(crate) fn generator<'a>(
    session: &'a mut Session,
    config: &'a Config,
    request: &'a Request,
) -> Generator<'a> {
    Generator {
        project: &mut session.project,
        merged: &mut session.merged,
        config,
        options: Options::default(),
        request,
        out: crate::Output::default(),
        autodoc_module: String::new(),
        autodoc_class: String::new(),
        depth: 0,
        generated: 0,
    }
}

/// The full name autodoc loads a found object by: a module's own name, else
/// `module::path`, so members of nested classes resolve.
fn load_name(found: &Found, obj_type: ObjType) -> String {
    if obj_type == ObjType::Module {
        return found.real_name.clone();
    }
    let rest = found.real_name.strip_prefix(&format!("{}.", found.modname));
    format!("{}::{}", found.modname, rest.unwrap_or(&found.real_name))
}

/// `Autosummary.get_items`: the rows for `request.names`, found in the source scan.
pub fn summarize(session: &mut Session, config: &Config, request: &SummaryRequest<'_>) -> Summary {
    let req = Request::default();
    let mut g = generator(session, config, &req);
    let prefixes = import_prefixes(request.module, request.class);
    let mut summary = Summary::default();
    let mut cycles = Vec::new();
    for written in request.names {
        let (name, display) = match written.strip_prefix('~') {
            Some(name) => (name, name.rsplit('.').next().unwrap_or(name)),
            None => (written.as_str(), written.as_str()),
        };
        let Some(found) = find_object(&mut g, name, &prefixes, &mut cycles) else {
            report_missing(&mut g, name, &prefixes);
            continue;
        };
        summary.items.push(item(&mut g, &found, display, request.signatures));
    }
    report_cycles(&mut g, &cycles);
    summary.messages = g.out.messages;
    summary
}

/// `autosummary.import`: `name` is in no module of the source folders.
pub(crate) fn report_missing(g: &mut Generator<'_>, name: &str, prefixes: &[String]) {
    let tried: Vec<String> = prefixes
        .iter()
        .map(|p| if p.is_empty() { name.to_owned() } else { format!("{p}.{name}") })
        .collect();
    let roots: Vec<String> = g.project.roots().iter().map(|r| r.display().to_string()).collect();
    let folders = if roots.is_empty() { "no folders".to_owned() } else { roots.join(", ") };
    g.report(
        Severity::Warning,
        "autosummary.import",
        "SUMMARY ENTRY NOT FOUND",
        format!(
            "autosummary: {} is not in the Python source; the folders searched are {folders}.",
            tried.join(" or ")
        ),
        "Check the name, or add the folder holding its package to autodoc_source_paths in \
         conf.toml.",
    );
}

fn report_cycles(g: &mut Generator<'_>, cycles: &[String]) {
    let mut seen = HashSet::new();
    for prefix in cycles.iter().filter(|p| seen.insert(p.as_str())) {
        g.report(
            Severity::Warning,
            "autosummary.import_cycle",
            "SUMMARY ENTRY REPEATS ITS MODULE",
            format!("Summarised items should not include the current module {prefix}."),
            "Write the entries relative to the current module.",
        );
    }
}

/// One row: the object loaded as autodoc loads it, its signature and summary.
fn item(
    g: &mut Generator<'_>,
    found: &Found,
    display: &str,
    sigs: SummarySignatures,
) -> SummaryItem {
    let obj_type = object_type(g, found);
    let mut row = SummaryItem {
        display_name: display.to_owned(),
        real_name: found.real_name.clone(),
        ..SummaryItem::default()
    };
    let Some(props) = load_object(g, &load_name(found, obj_type), obj_type, "", false) else {
        row.signature = Some(String::new());
        return row;
    };
    row.signature = match sigs {
        SummarySignatures::Omitted => None,
        _ if props.signatures.is_empty() => Some(String::new()),
        SummarySignatures::Short if props.signatures == ["()"] => Some("()".to_owned()),
        SummarySignatures::Short => Some("(\u{2026})".to_owned()),
        SummarySignatures::Long => {
            let max_chars = 50usize.saturating_sub(display.chars().count()).max(10);
            Some(signature::mangle_signature(&props.signatures.join("\n"), max_chars))
        }
    };
    row.summary = summary::extract_summary(&props.docstring_lines);
    row
}

/// `Autosummary.get_table` as reStructuredText: a two-column `list-table` with the
/// classes `autosummary longtable` and `classes`; nothing for no rows.
pub fn summary_table(items: &[SummaryItem], classes: &[String]) -> Vec<String> {
    if items.is_empty() {
        return Vec::new();
    }
    let mut class = vec!["autosummary".to_owned(), "longtable".to_owned()];
    class.extend(classes.iter().cloned());
    let mut out = vec![
        ".. list-table::".to_owned(),
        format!("   :class: {}", class.join(" ")),
        "   :widths: 10 90".to_owned(),
        String::new(),
    ];
    for it in items {
        let link = format!(":py:obj:`{} <{}>`", it.display_name, it.real_name);
        let first = match &it.signature {
            Some(sig) => format!("{link}\\ {}", rst_escape(sig)),
            None => link,
        };
        out.push(format!("   * - {first}"));
        out.push(if it.summary.is_empty() {
            "     -".to_owned()
        } else {
            format!("     - {}", it.summary)
        });
    }
    out
}

/// The docstring of the module `name`, as lines; `None` when it is not found.
pub fn module_docstring(session: &mut Session, name: &str) -> Option<Vec<String>> {
    let id = session.project.load_module(name)?;
    let doc = session.project.module(id).doc.clone()?;
    Some(gd_pyscan::prepare_docstring(&doc, 8))
}
