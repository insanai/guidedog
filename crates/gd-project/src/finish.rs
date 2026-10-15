//! Ending a build: deciding whether it publishes, committing or abandoning its output,
//! and the result.

use gd_conf::Config;
use gd_env::BuildResult;
use gd_host::Problem;
use gd_outputs::{CommitRecords, CommitState};
use gd_report::{Report, Severity};

use crate::build::{Log, Setup, Written};

/// Decides whether the build publishes its output, commits or abandons it, and makes
/// the result. Only a build that did all its work, read every document, and that `-W`
/// does not fail publishes; `suppress_warnings` is applied before `-W` counts.
pub(crate) fn finish(
    s: &Setup<'_>,
    config: &Config,
    log: Log,
    outcome: Result<Written, Problem>,
) -> BuildResult {
    let mut reports = unsuppressed(log.reports, &config.suppress_warnings);
    let mut result = log.result;
    let problem = match outcome {
        Err(problem) => Some(problem),
        Ok(written) => publish(s, written, &mut reports, &mut result),
    };
    result.exit = problem.as_ref().map_or(0, |p| p.exit);
    result.problem = problem;
    result.reports = reports;
    result.peak = s.budget.peak();
    result.budget = s.budget.limit();
    result
}

/// Commits a written build unless something keeps it from publishing; dropping the
/// generation abandons it. Returns the problem that kept it.
fn publish(
    s: &Setup<'_>,
    written: Written,
    reports: &mut Vec<Report>,
    result: &mut BuildResult,
) -> Option<Problem> {
    if let Some(problem) = publication_problem(s, &written, reports) {
        return Some(problem);
    }
    let Written { generation, env, store } = written;
    let outcome = generation.commit(CommitRecords { environment: &env, store: &store });
    reports.extend(outcome.reports);
    result.published = outcome.state == CommitState::Published && outcome.problem.is_none();
    result.unsettled = outcome.state == CommitState::Unsettled;
    outcome.problem
}

/// Why a build that finished its work does not publish it. As sphinx-build does,
/// problems in a document's content are reported as warnings and the build still
/// succeeds; a document that could not be read at all has no page and fails it
/// (`build.read`), any other error fails it (`build.errors`), so a build never
/// succeeds with errors, and `-W` fails a build with any warning (`build.warnings`).
fn publication_problem(s: &Setup<'_>, written: &Written, reports: &[Report]) -> Option<Problem> {
    let unread = written.env.docs.iter().filter(|d| d.failed).count();
    if unread > 0 {
        return Some(Problem::new(
            "build.read",
            "DOCUMENTS NOT READ",
            "",
            format!(
                "{unread} document(s) could not be read, so this build's output was not published."
            ),
            "Fix the errors reported above for each document, then run the build again.",
            1,
        ));
    }
    let count = |severity| reports.iter().filter(|r| r.severity == severity).count();
    let (errors, warnings) = (count(Severity::Error), count(Severity::Warning));
    if errors > 0 && !s.options.warnings_as_errors {
        return Some(Problem::new(
            "build.errors",
            "BUILD HAD ERRORS",
            "",
            format!("The build reported {errors} error(s), so its output was not published."),
            "Fix the errors reported above, then run the build again.",
            1,
        ));
    }
    if !s.options.warnings_as_errors {
        return None;
    }
    if errors + warnings == 0 {
        return None;
    }
    Some(Problem::new(
        "build.warnings",
        "WARNINGS TREATED AS ERRORS",
        "",
        format!(
            "The build produced {warnings} warning(s) and {errors} error(s), and -W is set, \
             so its output was not published."
        ),
        "Fix them, or build without -W.",
        1,
    ))
}

/// `reports` without the warnings `suppress` names, by Sphinx's rule: `type` silences
/// every warning of that category type, `type.subtype` one subtype, `type.*` all.
pub(crate) fn unsuppressed(reports: Vec<Report>, suppress: &[String]) -> Vec<Report> {
    if suppress.is_empty() {
        return reports;
    }
    reports
        .into_iter()
        .filter(|r| r.severity != Severity::Warning || !suppressed(&r.category, suppress))
        .collect()
}

fn suppressed(category: &str, suppress: &[String]) -> bool {
    if category.is_empty() {
        return false;
    }
    let (kind, subtype) = category.split_once('.').unwrap_or((category, ""));
    suppress.iter().any(|s| {
        let (target, sub) = match s.split_once('.') {
            Some((target, sub)) => (target, Some(sub)),
            None => (s.as_str(), None),
        };
        target == kind
            && (subtype.is_empty() || sub.is_none_or(|sub| sub == subtype || sub == "*"))
    })
}
