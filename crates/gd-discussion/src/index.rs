//! The generated index: the registry of records and states, and the reading order.

use crate::catalog::{Catalog, Format, catalog_format};
use crate::metadata::{Record, quote};
use crate::problem::shown;
use crate::transaction::{Change, Plan};
use crate::{MAX_INDEX_BYTES, Prefix, Problem, Result, exists, read_text};
use std::fmt::Write;

/// Plans the regenerated registry and reading order of `catalog`: `registry.rst` and
/// `bundle.rst`, or `registry.typ` and `bundle.typ` in an older Typst catalog. Only the
/// region between the `PREFIX-generated-start` and `-end` markers is generated; the
/// prose around it stays. The plan has no changes when the index is current.
/// Unbalanced markers are a problem.
pub fn index_plan(catalog: &Catalog) -> Result<Plan> {
    let records: Vec<&Record> = catalog.records.iter().collect();
    plan_index(catalog, &records)
}

/// Plans the index of `catalog` as if it held `records`, sorted, instead of its own: the
/// index a planned change leaves.
pub(crate) fn plan_index(catalog: &Catalog, records: &[&Record]) -> Result<Plan> {
    let format = catalog_format(&catalog.dir);
    let mut plan = Plan {
        summary: "Refresh the discussion registry and reading order".to_string(),
        ..Plan::default()
    };
    for name in format.index_files() {
        let path = catalog.dir.join(name);
        let old = if exists(&path) {
            Some(read_text(catalog.prefix.as_str(), &path, MAX_INDEX_BYTES)?)
        } else {
            None
        };
        let text = old.as_deref().unwrap_or("");
        let next = match format {
            Format::Rst => rst_index_text(catalog, records, name, text),
            Format::Typst => typst_index_text(catalog, records, name, text),
        }
        .map_err(|p| p.with_path(shown(&path)))?;
        if old.as_deref() != Some(next.as_str()) {
            plan.changes.push(Change { path: name.to_string(), before: old, after: Some(next) });
        }
    }
    Ok(plan)
}

/// Checks that the index of `catalog` is what [`index_plan`] would write;
/// `PREFIX.index-drift` when it is not.
pub fn check_index(catalog: &Catalog) -> Result<()> {
    if index_plan(catalog)?.changes.is_empty() {
        return Ok(());
    }
    Err(Problem::new(catalog.prefix.code("index-drift"), "The discussion index needs refreshing",
        shown(&catalog.dir), "Registry or bundle metadata differs from the records.",
        "Run the index command to regenerate the registry and reading order."))
}

/// The comment lines around the generated region, in RST or Typst.
fn markers(prefix: &Prefix, rst: bool) -> (String, String) {
    let lead = if rst { ".." } else { "//" };
    (format!("{lead} {prefix}-generated-start"), format!("{lead} {prefix}-generated-end"))
}

/// Replaces the generated region of `old`, or starts a new file with `lead` and the
/// region when `old` has none. Stray or repeated markers are a problem.
pub(crate) fn managed_text(old: &str, generated: &str, lead: &str, prefix: &Prefix, rst: bool)
    -> Result<String> {
    let (start, end) = markers(prefix, rst);
    let Some((before, rest)) = old.split_once(&start) else {
        if old.contains(&end) {
            return Err(markers_problem(prefix, rst));
        }
        return Ok(format!("{lead}{start}\n{generated}{end}\n"));
    };
    let Some((inside, after)) = rest.split_once(&end) else {
        return Err(markers_problem(prefix, rst));
    };
    if inside.contains(&start) || after.contains(&start) || after.contains(&end) {
        return Err(markers_problem(prefix, rst));
    }
    Ok(format!("{before}{start}\n{generated}{end}{after}"))
}

fn markers_problem(prefix: &Prefix, rst: bool) -> Problem {
    let (start, end) = markers(prefix, rst);
    Problem::new(prefix.code("markers"), "Generated markers are unbalanced", "",
        "The generated region needs one start marker followed by one end marker.",
        format!("Restore the lines {start} and {end} around the generated region."))
}

/// Tells whether `text` has one generated region, as [`index_plan`] writes it.
pub(crate) fn is_balanced(text: &str, prefix: &Prefix, rst: bool) -> bool {
    let (start, end) = markers(prefix, rst);
    text.matches(&start).count() == 1
        && text.matches(&end).count() == 1
        && text.find(&start) < text.find(&end)
}

fn rst_index_text(c: &Catalog, records: &[&Record], name: &str, old: &str) -> Result<String> {
    let registry = name == "registry.rst";
    let mut out = String::with_capacity(128 + records.len() * 96);
    out.push('\n');
    if registry {
        let _ = write!(out, ".. list-table:: Records and review states\n   :header-rows: 1\n   \
            :widths: 15 55 30\n\n   * - {}\n     - Discussion\n     - State\n", c.prefix.upper());
    } else {
        out.push_str(".. toctree::\n   :maxdepth: 1\n\n");
    }
    for r in records.iter().filter(|r| !r.historical) {
        if registry {
            let _ = write!(out, "   * - {}\n     - :doc:`records/{}`\n     - {}\n", r.number,
                r.stem(), r.state);
        } else {
            let _ = writeln!(out, "   records/{}", r.stem());
        }
    }
    out.push('\n');
    let lead = if registry {
        format!(":{}-reserved-through: {}\n\nDiscussion registry\n===================\n\n",
            c.prefix, c.reserved)
    } else {
        "Discussion reading order\n========================\n\n".to_string()
    };
    managed_text(old, &out, &lead, &c.prefix, true)
}

fn typst_index_text(c: &Catalog, records: &[&Record], name: &str, old: &str)
    -> Result<String> {
    let prefix = &c.prefix;
    let mut out = String::new();
    let mut lead = "// Managed by guidedog gds index.\n".to_string();
    let records = records.iter().filter(|r| !r.historical);
    if name == "registry.typ" {
        let _ = writeln!(out, "#let {prefix}-documents = (");
        for r in records {
            out.push_str("  (\n");
            let source = format!("records/{}", r.file_name());
            for (key, value) in [("number", &r.number), ("slug", &r.slug), ("state", &r.state),
                ("type", &r.kind), ("title", &r.title), ("source", &source),
                ("summary", &r.title)] {
                let _ = writeln!(out, "    {key}: {},", quote(value));
            }
            out.push_str("  ),\n");
        }
        out.push_str(")\n");
        let documents = format!("#let {prefix}-documents");
        lead = match old.split_once(&documents) {
            Some((before, _)) => before.to_string(),
            None => format!("#let {prefix}-reserved-through = {}\n", c.reserved),
        };
    } else {
        out.push_str("#include \"index.typ\"\n");
        for r in records {
            let _ = writeln!(out, "#pagebreak()\n#include {}",
                quote(&format!("records/{}", r.file_name())));
        }
        if let Some((before, _)) = old.split_once("#include") {
            lead = before.to_string();
        }
    }
    managed_text(old, &out, &lead, prefix, false)
}
