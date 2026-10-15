//! `guidedog gds`: the discussion command on Guidedog's own catalog (prefix `gds`,
//! folder `docs/gds`), and the builder that publishes catalogs with this program's
//! engine, which `dscc` uses too.

use std::fmt::Write;
use std::path::PathBuf;

use gd_conf::LoadOptions;
use gd_discussion::{Book, BuildRequest, Builder, Catalog, Problem, Record, Target};
use gd_env::{BuildOptions, BuildResult, BuilderKind};

use crate::console::Console;
use crate::ops;

/// Runs `guidedog gds ARGS`.
pub(crate) fn gds(console: &mut Console<'_>, args: &[String]) -> i32 {
    let version = crate::version::version_text();
    let program = gd_discussion::command::Program {
        name: "guidedog gds",
        version: &version,
        tool: "typst",
        make_builder: discussion_builder,
        prefix: gd_discussion::Prefix::new("gds").ok(),
        dir: Some(PathBuf::from("docs/gds")),
    };
    let terminals = (console.terminals.stdout, console.terminals.stderr);
    gd_discussion::command::run_with(&program, args, console.out, console.err, terminals)
}

/// The discussion builder with Guidedog's engine linked in: catalogs build as Guidedog
/// projects in this process, warnings as errors; older Typst catalogs compile with the
/// Typst compiler (embedded, or `typst`, the program `--typst` names). For
/// `gd_discussion::command::Program::make_builder`.
pub fn discussion_builder(typst: Option<PathBuf>) -> Box<dyn Builder> {
    Box::new(ProjectBuilder { typst })
}

/// Builds catalogs as Guidedog projects with the in-process project builder.
#[derive(Debug, Clone, Default)]
struct ProjectBuilder {
    typst: Option<PathBuf>,
}

impl Builder for ProjectBuilder {
    fn name(&self) -> &str {
        "embedded"
    }

    fn build(&self, r: &BuildRequest) -> gd_discussion::Result<PathBuf> {
        let mut load = LoadOptions::default();
        if !r.books.is_empty() {
            load.overrides.push(books_setting(&r.books));
        }
        let linked = gd_plugins::Registry::default();
        let mut project = ops::load(&r.dir, &load, &linked).map_err(|p| from_host(&p))?;
        if let Some(typst) = &self.typst {
            project.config.pdf.typst = typst.to_string_lossy().into_owned();
        }
        let kind = if r.target == Target::Html { BuilderKind::Html } else { BuilderKind::Pdf };
        // A selected preview has its own output, so it cannot remove the other books
        // from the catalog's complete PDF output.
        let output_dir = r.selection.as_ref().map(|selection| {
            project.config.build_dir.join("selected").join(selection).join("pdf")
        });
        let options = BuildOptions { warnings_as_errors: true, output_dir, ..Default::default() };
        let result = ops::build(&project, kind, &options);
        if result.exit != 0 {
            return Err(build_problem(&result));
        }
        if let Some(report) = result.reports.iter().find(|r| is_serious(r)) {
            let hint = report.hints.join("\n");
            return Err(Problem::new(
                &*report.code,
                &report.title,
                &*report.path,
                &*report.message,
                hint,
            ));
        }
        Ok(result.output)
    }

    fn build_typst_catalog(
        &self,
        catalog: &Catalog,
        selected: &[&Record],
        targets: &[Target],
    ) -> gd_discussion::Result<Vec<PathBuf>> {
        crate::legacy::build(catalog, selected, targets, self.typst.as_deref())
    }
}

fn is_serious(report: &gd_report::Report) -> bool {
    report.severity != gd_report::Severity::Note
}

/// The problem of a failed catalog build: the first source diagnostic with its place,
/// so the record can be fixed, beside the build's own summary.
fn build_problem(r: &BuildResult) -> Problem {
    let Some(problem) = &r.problem else {
        return Problem::new("build.failed", "The catalog did not build", "", "", "").exit(r.exit);
    };
    let summary_only = matches!(problem.code, "build.warnings" | "build.read" | "typst.compile");
    let first = r.reports.iter().find(|report| is_serious(report));
    match first.filter(|_| summary_only) {
        Some(report) => {
            let mut place = report.path.clone();
            if report.line > 0 {
                let _ = write!(place, ":{}:{}", report.line, report.column.max(1));
            }
            Problem::new(
                &*report.code,
                &report.title,
                place,
                format!("{}\n\n{}", report.message, problem.message),
                format!(
                    "{}\nRun guidedog build html PROJECT -W to see every source diagnostic \
                     (use pdf for a PDF build).",
                    report.hints.join("\n")
                ),
            )
            .exit(r.exit)
        }
        None => from_host(problem),
    }
}

/// A host problem as a discussion problem.
fn from_host(p: &gd_host::Problem) -> Problem {
    Problem::new(p.code, p.title, &*p.path, &*p.message, &*p.hint).exit(p.exit)
}

/// The `pdf_documents` setting of `books`, as TOML for `-D`. Every value is a quoted
/// string, so titles and names are data, never settings.
fn books_setting(books: &[Book]) -> String {
    let mut out = String::from("pdf_documents=[");
    for (i, book) in books.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let fields = [
            ("root", &book.root),
            ("file", &book.file),
            ("title", &book.title),
            ("author", &book.author),
        ];
        out.push('{');
        for (j, (name, value)) in fields.into_iter().enumerate() {
            if j > 0 {
                out.push_str(", ");
            }
            out.push_str(name);
            out.push('=');
            gd_toml::write_basic_string(&mut out, value);
        }
        out.push('}');
    }
    out.push(']');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn books_reach_the_build_as_quoted_data() {
        let books = vec![Book {
            root: "records/0001-cache".into(),
            file: "gds-0001-cache.pdf".into(),
            title: "Quotes \" and \\ slash".into(),
            author: "René, Ada".into(),
        }];
        assert_eq!(
            books_setting(&books),
            "pdf_documents=[{root=\"records/0001-cache\", file=\"gds-0001-cache.pdf\", \
             title=\"Quotes \\\" and \\\\ slash\", author=\"René, Ada\"}]"
        );
    }

    #[test]
    fn a_failed_build_names_its_first_source_diagnostic() {
        let report = gd_report::Report {
            path: "records/0001-cache.rst".into(),
            line: 3,
            column: 0,
            ..gd_report::Report::warning("rst.title", "TITLE UNDERLINE TOO SHORT", "Too short.")
                .hint("Lengthen it.")
        };
        let problem = gd_host::Problem::new("build.warnings", "WARNINGS", "", "1 warning.", "", 1);
        let result = BuildResult {
            exit: 1,
            problem: Some(problem),
            reports: vec![report],
            ..Default::default()
        };
        let p = build_problem(&result);
        assert_eq!(p.path, "records/0001-cache.rst:3:1");
        assert!(p.message.contains("Too short.") && p.message.contains("1 warning."));
        assert!(p.hint.starts_with("Lengthen it.") && p.hint.contains("-W"));
        assert_eq!(p.exit, 1);
    }
}
