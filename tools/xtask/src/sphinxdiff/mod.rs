//! `cargo xtask sphinxdiff`: compares a project's HTML built by Sphinx with the same
//! project built by Guidedog. It compares the pages each wrote; per page the ids, the
//! internal link targets, the numbers of the document's body (theme chrome left out)
//! and the texts of Sphinx's `:numref:` links; the entries of objects.inv; and, given
//! both builds' logs, where each warned. Sphinx's output is a reference made in a
//! separate verification environment; Guidedog never runs Python.
//!
//! `cargo xtask sphinxdiff-roots` ([`roots`]) runs the comparison over Sphinx's own
//! test roots and writes a record.

mod html;
mod roots;
mod warnings;

#[cfg(test)]
mod tests;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::args::{self, Spec};
use crate::fsutil::{absolute, read_text};
use crate::{Result, usage};
use html::{Side, attribute_values, counted, link_targets, link_texts, numbers, numref_texts};

pub(crate) use roots::main as roots_main;

const HELP: &str = "\
Compares a project's HTML built by Sphinx with the same project built by Guidedog: the
pages, and per page the ids, internal link targets, body numbers and :numref: texts; the
entries of objects.inv; and, with --warnings, where each builder warned.

Usage: cargo xtask sphinxdiff SPHINX_HTML GUIDEDOG_HTML [-v]
                              [--warnings SPHINX_LOG GUIDEDOG_JSON_LOG SOURCE_DIR]

  -v          list every difference, not only those of pages, inventory and warnings
  --warnings  compare the warnings of sphinx-build's log with those of Guidedog's
              --diagnostics=json log; SOURCE_DIR is the project both read

Sphinx's build should use the basic theme (-D html_theme=basic), whose body is
<div class=\"body\">. The last line is a summary for scripts: `summary:` followed by
name=compared/differing pairs, or `summary: sphinx=failed` when Sphinx wrote no pages.
";

/// Generated pages both builders write; they are compared as pages, not by content.
const GENERATED: [&str; 3] = ["genindex.html", "search.html", "py-modindex.html"];

/// What is compared, in the order the summary names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Measure {
    Pages,
    Ids,
    Links,
    Numbers,
    Numrefs,
    Inventory,
    Warnings,
}

impl Measure {
    const ALL: [Measure; 7] = [
        Measure::Pages,
        Measure::Ids,
        Measure::Links,
        Measure::Numbers,
        Measure::Numrefs,
        Measure::Inventory,
        Measure::Warnings,
    ];

    fn name(self) -> &'static str {
        match self {
            Measure::Pages => "pages",
            Measure::Ids => "ids",
            Measure::Links => "links",
            Measure::Numbers => "numbers",
            Measure::Numrefs => "numrefs",
            Measure::Inventory => "inventory",
            Measure::Warnings => "warnings",
        }
    }
}

/// The two logs and the project folder `--warnings` names.
#[derive(Debug, Clone, Default)]
pub(crate) struct Logs {
    /// sphinx-build's standard error.
    pub sphinx: PathBuf,
    /// Guidedog's `--diagnostics=json` output.
    pub guidedog: PathBuf,
    /// The project both builders read.
    pub source: PathBuf,
}

/// What `sphinxdiff` compares.
#[derive(Debug, Clone, Default)]
pub(crate) struct Options {
    /// Sphinx's HTML output folder.
    pub sphinx: PathBuf,
    /// Guidedog's HTML output folder.
    pub guidedog: PathBuf,
    /// Whether to list every difference.
    pub verbose: bool,
    /// The logs to compare warnings from, if any.
    pub logs: Option<Logs>,
}

/// The result of a comparison: the report to print, its last line being the summary.
#[derive(Debug, Default)]
pub(crate) struct Comparison {
    /// The whole report, one line per difference and the totals.
    pub report: String,
}

impl Comparison {
    /// The summary line without its `summary: ` label, for a record.
    pub fn summary(&self) -> &str {
        let last = self.report.lines().last().unwrap_or_default();
        last.strip_prefix("summary: ").unwrap_or(last)
    }
}

/// Runs `cargo xtask sphinxdiff`.
pub(crate) fn main(args: &[String]) -> Result<()> {
    let spec = Spec { command: "sphinxdiff", flags: &[], options: &[], help: HELP };
    let (rest, verbose, logs) = take_options(args, &spec)?;
    let Some(parsed) = args::parse(&rest, &spec)? else { return Ok(()) };
    parsed.at_most(2, &spec)?;
    let options = Options {
        sphinx: absolute(Path::new(parsed.positional(0, "Sphinx's HTML folder", &spec)?)),
        guidedog: absolute(Path::new(parsed.positional(1, "Guidedog's HTML folder", &spec)?)),
        verbose,
        logs,
    };
    print!("{}", compare(&options)?.report);
    Ok(())
}

/// Takes `-v` and `--warnings` with its three values out of `args`, which the shared
/// argument reader does not know: a short flag, and an option with three values.
fn take_options(args: &[String], spec: &Spec<'_>) -> Result<(Vec<String>, bool, Option<Logs>)> {
    let mut rest = Vec::with_capacity(args.len());
    let (mut verbose, mut logs) = (false, None);
    let mut items = args.iter();
    while let Some(arg) = items.next() {
        match arg.as_str() {
            "-v" => verbose = true,
            "--warnings" => {
                let mut value = || -> Result<PathBuf> {
                    let value = items.next().filter(|v| !v.starts_with('-'));
                    value.map(|v| absolute(Path::new(v))).ok_or_else(|| {
                        usage(
                            "The option --warnings needs three values.",
                            &format!(
                                "Write `--warnings SPHINX_LOG GUIDEDOG_JSON_LOG SOURCE_DIR`; \
                                 `cargo xtask {} --help` explains them.",
                                spec.command
                            ),
                        )
                    })
                };
                logs = Some(Logs { sphinx: value()?, guidedog: value()?, source: value()? });
            }
            _ => rest.push(arg.clone()),
        }
    }
    Ok((rest, verbose, logs))
}

/// Counts per measure: how many entries Sphinx's side has, and how many differ.
#[derive(Debug, Default)]
struct Totals {
    count: [usize; 7],
    differ: [usize; 7],
    identical_pages: usize,
}

impl Totals {
    fn add(&mut self, measure: Measure, count: usize, differ: usize) {
        self.count[measure as usize] += count;
        self.differ[measure as usize] += differ;
    }
}

/// Compares the two builds as [`Options`] says and returns the report. Fails only when a
/// log named with `--warnings` cannot be read; a missing page or inventory is a
/// difference, and a Sphinx build without pages is reported as failed.
pub(crate) fn compare(options: &Options) -> Result<Comparison> {
    let mut out = String::new();
    let sphinx_pages = list_pages(&options.sphinx);
    let guidedog_pages = list_pages(&options.guidedog);
    if sphinx_pages.is_empty() {
        out.push_str("Sphinx wrote no pages: its build failed, so there is nothing to compare\n");
        out.push_str("summary: sphinx=failed\n");
        return Ok(Comparison { report: out });
    }
    let mut totals = Totals::default();
    totals.differ[Measure::Pages as usize] =
        report_set(&mut out, "pages", &sphinx_pages, &guidedog_pages, true);
    for page in &sphinx_pages {
        if guidedog_pages.contains(page) && !GENERATED.contains(&page.as_str()) {
            compare_page(&mut out, &mut totals, options, page);
        }
    }
    let sphinx_entries = inventory_entries(&options.sphinx);
    let guidedog_entries = inventory_entries(&options.guidedog);
    let differ = report_set(&mut out, "objects.inv", &sphinx_entries, &guidedog_entries, true);
    totals.add(Measure::Inventory, sphinx_entries.len(), differ);
    if let Some(logs) = &options.logs {
        compare_warnings(&mut out, &mut totals, logs, options.verbose)?;
    }
    write_totals(&mut out, &totals);
    Ok(Comparison { report: out })
}

/// Compares one page's ids, internal links, numbers and numref texts.
fn compare_page(out: &mut String, totals: &mut Totals, options: &Options, page: &str) {
    let sphinx_html = std::fs::read_to_string(options.sphinx.join(page)).unwrap_or_default();
    let guidedog_html = std::fs::read_to_string(options.guidedog.join(page)).unwrap_or_default();
    let a = html::body(&sphinx_html, Side::Sphinx);
    let b = html::body(&guidedog_html, Side::Guidedog);
    totals.add(Measure::Pages, 1, 0);
    let measured = [
        (Measure::Ids, counted(attribute_values(a, "id")), counted(attribute_values(b, "id"))),
        (Measure::Links, link_targets(a, page), link_targets(b, page)),
        (Measure::Numbers, numbers(a), numbers(b)),
    ];
    let mut same = true;
    for (measure, sphinx, guidedog) in &measured {
        let what = format!("{page} {}", measure.name());
        let differ = report_set(out, &what, sphinx, guidedog, options.verbose);
        totals.add(*measure, sphinx.len(), differ);
        same &= differ == 0;
    }
    // Each :numref: text Sphinx shows should be the text of a Guidedog link to the same
    // target; Guidedog's links carry no role class, so only Sphinx's side is listed.
    let refs = numref_texts(a, page);
    let texts = link_texts(b, page);
    let missing = html::only(&refs, &texts);
    totals.add(Measure::Numrefs, refs.len(), missing.len());
    if !missing.is_empty() {
        same = false;
        if options.verbose {
            let _ = writeln!(out, "{page} numref texts:");
            for text in missing {
                let _ = writeln!(out, "  only Sphinx:   {text}");
            }
        }
    }
    if same {
        totals.identical_pages += 1;
    }
}

/// Compares where the builders warned, per document; the lines are listed in verbose
/// mode but not counted.
fn compare_warnings(
    out: &mut String,
    totals: &mut Totals,
    logs: &Logs,
    verbose: bool,
) -> Result<()> {
    let sphinx = warnings::sphinx_warnings(&read_text(&logs.sphinx)?, &logs.source);
    let guidedog = warnings::guidedog_warnings(&read_text(&logs.guidedog)?, &logs.source);
    let sphinx_docs = warnings::per_document(&sphinx);
    let guidedog_docs = warnings::per_document(&guidedog);
    let what = "warnings per document";
    let differ = report_set(out, what, &sphinx_docs, &guidedog_docs, true);
    totals.add(Measure::Warnings, sphinx_docs.len(), differ);
    let (a, b) = (warnings::located(&sphinx), warnings::located(&guidedog));
    report_set(out, "warning lines (not counted)", &a, &b, verbose);
    Ok(())
}

/// Writes the totals in words, then the summary line for scripts.
fn write_totals(out: &mut String, totals: &Totals) {
    let pair = |m: Measure| (totals.count[m as usize], totals.differ[m as usize]);
    let _ = write!(
        out,
        "pages compared {}, identical {}",
        totals.count[Measure::Pages as usize],
        totals.identical_pages
    );
    let words = [
        (Measure::Ids, "ids"),
        (Measure::Links, "internal links"),
        (Measure::Numbers, "numbers"),
        (Measure::Numrefs, "numref texts"),
        (Measure::Inventory, "inventory entries"),
        (Measure::Warnings, "warnings"),
    ];
    for (i, (measure, name)) in words.into_iter().enumerate() {
        let (count, differ) = pair(measure);
        let separator = if i == 0 { "; " } else { ", " };
        let _ = write!(out, "{separator}{name} {count} ({differ} differ)");
    }
    out.push_str("\nsummary:");
    for measure in Measure::ALL {
        let (count, differ) = pair(measure);
        let _ = write!(out, " {}={count}/{differ}", measure.name());
    }
    let _ = writeln!(out, " identical={}", totals.identical_pages);
}

/// The HTML pages below `dir`, relative with `/`, sorted; static files and copied
/// sources (names starting with `_` or `.`) are left out. A missing folder has none.
fn list_pages(dir: &Path) -> Vec<String> {
    let mut pages = Vec::new();
    walk(dir, "", &mut pages);
    pages.sort();
    pages
}

fn walk(dir: &Path, prefix: &str, pages: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('_') || name.starts_with('.') {
            continue;
        }
        let relative = format!("{prefix}{name}");
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            walk(&entry.path(), &format!("{relative}/"), pages);
        } else if name.ends_with(".html") {
            pages.push(relative);
        }
    }
}

/// The entries of a folder's objects.inv as `type name uri display` lines, sorted; none
/// when it has no readable inventory.
fn inventory_entries(dir: &Path) -> Vec<String> {
    let Ok(data) = std::fs::read(dir.join("objects.inv")) else { return Vec::new() };
    let Ok(inventory) = gd_inventory::load_inventory(&data, "") else { return Vec::new() };
    let items = inventory.items().iter();
    let mut entries: Vec<String> =
        items.map(|it| format!("{} {} {} {}", it.kind, it.name, it.uri, it.display)).collect();
    entries.sort();
    entries
}

/// Writes what only one side has, under `what`, when `show` is set; returns how many
/// entries differ.
fn report_set(
    out: &mut String,
    what: &str,
    sphinx: &[String],
    guidedog: &[String],
    show: bool,
) -> usize {
    let only_sphinx = html::only(sphinx, guidedog);
    let only_guidedog = html::only(guidedog, sphinx);
    let differ = only_sphinx.len() + only_guidedog.len();
    if differ > 0 && show {
        let _ = writeln!(out, "{what}:");
        for item in only_sphinx {
            let _ = writeln!(out, "  only Sphinx:   {item}");
        }
        for item in only_guidedog {
            let _ = writeln!(out, "  only Guidedog: {item}");
        }
    }
    differ
}
