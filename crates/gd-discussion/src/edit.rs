//! Planning record changes: a new draft, a promotion, and a lifecycle decision.

use crate::catalog::{Catalog, catalog_format, resolve, sort_by_number};
use crate::config::Process;
use crate::index::plan_index;
use crate::metadata::{FieldValue, Record, field_line, is_valid_slug, label_problem, read_record};
use crate::problem::shown;
use crate::record_plan::record_plan;
use crate::transaction::{Change, Plan};
use crate::{CONFIG_FILE, DRAFT_NUMBER, MAX_RECORD_BYTES, Prefix, Problem, Result};
use crate::{contains, exists, read_text, rst, today};
use std::path::Path;
use std::process::Command;

/// What a new draft is made from. Empty fields take their defaults.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewDraft {
    /// The title; required.
    pub title: String,
    /// The slug; empty derives it from the title (`Cache policy` → `cache-policy`).
    pub slug: String,
    /// The author; empty takes Git's `user.name` in the catalog.
    pub author: String,
    /// The labels; empty is `documentation`, when the catalog allows it.
    pub labels: Vec<String>,
}

/// A promotion or a lifecycle decision about one record.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Decision {
    /// The record: its number, slug, title, or file name.
    pub identifier: String,
    /// The state to move to ([`state_plan`] only; promotion opens the draft).
    pub to: String,
    /// The number promotion gives; `None` takes the next free one.
    pub number: Option<u32>,
    /// Where the record is discussed, written to its discussion field when given.
    pub discussion: String,
    /// Why; a state change needs one, a promotion has a default.
    pub reason: String,
    /// The review or validation, when the target state asks for evidence.
    pub evidence: String,
}

/// Plans a new draft in `catalog`: its file, made from the catalog's template, and the
/// refreshed index. It changes nothing; [`crate::apply`] writes the plan. A bad or
/// taken slug, an unknown label, or no author is a problem.
pub fn new_plan(catalog: &Catalog, draft: &NewDraft) -> Result<Plan> {
    let (dir, prefix) = (catalog.dir.as_path(), &catalog.prefix);
    let slug = if draft.slug.is_empty() { make_slug(&draft.title) } else { draft.slug.clone() };
    if !is_valid_slug(&slug) {
        return Err(Problem::new(prefix.code("slug"), "Choose a valid slug", &slug,
            "Slugs use lowercase letters, digits, and separating hyphens.",
            "Pass --slug=short-descriptive-name.")
        .exit(2));
    }
    if let Some(r) = catalog.records.iter().find(|r| r.slug == slug) {
        return Err(Problem::new(prefix.code("slug-taken"), "That discussion already exists",
            shown(&r.path), "Slugs identify drafts as well as numbered records.",
            "Choose another title or pass --slug with a unique name.")
        .exit(2));
    }
    let author = if draft.author.is_empty() { git_author(dir) } else { draft.author.clone() };
    if author.is_empty() {
        return Err(Problem::new(prefix.code("author"), "An author is needed", shown(dir),
            "No Git author identity is configured.",
            "Pass --author='Your name' when creating the record.")
        .exit(2));
    }
    let labels = draft_labels(catalog, &draft.labels)?;
    let suffix = catalog_format(dir).suffix();
    let template_path = dir.join(format!("template/rfc-template{suffix}"));
    let template = read_text(prefix.as_str(), &template_path, MAX_RECORD_BYTES)?;
    let mut source = fill_draft(&template, &draft.title, &author, &labels, prefix,
        &catalog.process)?;
    if suffix == ".rst" {
        source = rst::draft_title(&source, &draft.title, prefix);
    }
    let path = format!("records/{DRAFT_NUMBER}-{slug}{suffix}");
    if exists(&dir.join(&path)) {
        return Err(Problem::new(prefix.code("exists"), "File already exists", &path,
            "Creating a draft would overwrite a file.", "Use a different --slug.")
        .exit(2));
    }
    let r = read_record(catalog, &dir.join(&path), &source)?;
    let mut records: Vec<&Record> = catalog.records.iter().chain([&r]).collect();
    sort_by_number(&mut records);
    let mut plan = plan_index(catalog, &records)?;
    plan.changes.push(Change { path: path.clone(), before: None, after: Some(source) });
    plan.summary = format!("Create {path}");
    Ok(plan)
}

/// The labels of a new draft: those given, or `documentation`.
fn draft_labels(c: &Catalog, given: &[String]) -> Result<Vec<String>> {
    let labels: Vec<String> = if !given.is_empty() {
        given.iter().map(|l| l.trim().to_string()).collect()
    } else if let Some(allowed) = c.vocabulary().filter(|v| !v.contains(&"documentation")) {
        return Err(Problem::new(c.prefix.code("label"), "Choose the draft's labels",
            shown(&c.dir), "This catalog has no documentation label, the default of a new draft.",
            format!("Pass --labels with labels from: {}.", allowed.join(", ")))
        .exit(2));
    } else {
        vec!["documentation".to_string()]
    };
    if let Some(bad) = labels.iter().find(|l| !c.allows_label(l)) {
        return Err(label_problem(c, bad, bad).exit(2));
    }
    Ok(labels)
}

/// Fills a template's metadata for a new draft dated today.
pub(crate) fn fill_draft(template: &str, title: &str, author: &str, labels: &[String],
    prefix: &Prefix, process: &Process) -> Result<String> {
    let date = today();
    let authors = [author.to_string()];
    let fields = [
        ("number", FieldValue::Text(DRAFT_NUMBER)),
        ("title", FieldValue::Text(title)),
        ("state", FieldValue::Text(&process.draft)),
        ("authors", FieldValue::List(&authors)),
        ("created", FieldValue::Text(&date)),
        ("updated", FieldValue::Text(&date)),
        ("labels", FieldValue::List(labels)),
    ];
    let mut source = template.to_string();
    for (key, value) in fields {
        source = crate::replace_field(&source, prefix, key, value).map_err(|p| {
            Problem::new(prefix.code("template"), "The record template is incomplete",
                "template", p.message,
                format!("Add the {prefix}-{key} field to the metadata at the start of the \
                    template."))
            .exit(2)
        })?;
    }
    let status = process.state(&process.draft).map_or("", |s| s.status.as_str());
    if !status.is_empty() && field_line(&source, prefix, "status", true).is_some() {
        source = crate::replace_field(&source, prefix, "status", FieldValue::Text(status))?;
    }
    Ok(source)
}

/// A slug from a title: lowercase ASCII letters and digits, other runs as one hyphen.
pub(crate) fn make_slug(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    let mut separator = false;
    for c in title.chars().map(|c| c.to_ascii_lowercase()) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(c);
            separator = false;
        } else {
            separator = true;
        }
    }
    slug
}

/// Git's `user.name` in `dir`, or "" when Git or the setting is missing.
pub(crate) fn git_author(dir: &Path) -> String {
    let output = Command::new("git").arg("-C").arg(dir).args(["config", "user.name"]).output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().into(),
        _ => String::new(),
    }
}

/// The number a promoted draft takes: `requested` when it is free, else one past the
/// highest that a record, the registry's reservation, or a `PREFIX/NUMBER` branch of
/// the catalog's Git repository holds, and at least the numbering's start. A number
/// already reserved, or above the numbering's limit, is `PREFIX.number-taken`.
pub fn next_number(catalog: &Catalog, requested: Option<u32>) -> Result<u32> {
    let (prefix, process) = (&catalog.prefix, &catalog.process);
    let records = catalog.records.iter().filter(|r| !r.historical);
    let held = records.filter_map(|r| process.number_value(&r.number)).max().unwrap_or(0);
    let reserved = catalog.reserved.max(held).max(branch_numbers(&catalog.dir, prefix));
    let n = requested.unwrap_or_else(|| reserved.saturating_add(1).max(process.numbering.start));
    let limit = process.number_limit();
    if n <= reserved || n > limit || n < process.numbering.start {
        return Err(Problem::new(prefix.code("number-taken"),
            "Number is reserved or out of range", shown(&catalog.dir),
            format!("Numbers through {reserved} are taken; this catalog numbers from {} to \
                {limit}.", process.numbering.start),
            format!("Omit --number to take the next free number, or change numbering.limit \
                in {CONFIG_FILE}."))
        .exit(2));
    }
    Ok(n)
}

/// The highest number a `PREFIX/NUMBER` branch reserves, or 0 outside Git.
fn branch_numbers(dir: &Path, prefix: &Prefix) -> u32 {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["for-each-ref", "--format=%(refname:short)", &format!("refs/heads/{prefix}/")])
        .output();
    let Ok(out) = output else { return 0 };
    if !out.status.success() {
        return 0;
    }
    let lead = format!("{prefix}/");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.strip_prefix(&lead)?.parse::<u32>().ok())
        .filter(|&n| n <= crate::MAX_NUMBER)
        .max()
        .unwrap_or(0)
}

/// Plans the promotion of the draft `decision.identifier` names: it takes the next
/// number (or `decision.number`) and moves to the workflow's open state, with the index
/// refreshed. A historical record or one that is not a draft is a problem.
pub fn promote_plan(catalog: &Catalog, decision: &Decision) -> Result<Plan> {
    let (r, text) = changeable_record(catalog, &decision.identifier)?;
    let (prefix, process) = (&catalog.prefix, &catalog.process);
    if !r.is_draft() || r.state != process.draft {
        return Err(Problem::new(prefix.code("promote"), "Only a draft can be promoted",
            shown(&r.path),
            format!("This record is {}, {}; a draft is {DRAFT_NUMBER} and {}.", r.number,
                r.state, process.draft),
            "Use the state command for a record that is not a draft.")
        .exit(2));
    }
    let n = next_number(catalog, decision.number)?;
    check_decision(decision, catalog, &r, &process.open, false)?;
    record_plan(catalog, decision, &r, &text, &process.open, &process.format_number(n))
}

/// Plans the move of the record `decision.identifier` names to `decision.to`: a move
/// the workflow allows, with its reason, and evidence where the target state needs it;
/// with the index refreshed.
pub fn state_plan(catalog: &Catalog, decision: &Decision) -> Result<Plan> {
    let (r, text) = changeable_record(catalog, &decision.identifier)?;
    check_move(decision, catalog, &r)?;
    record_plan(catalog, decision, &r, &text, &decision.to, &r.number)
}

/// The record `id` names, read whole as it is now, with its text. A catalog keeps only
/// metadata, so the one record to change is read again.
fn changeable_record(c: &Catalog, id: &str) -> Result<(Record, String)> {
    let r = resolve(c, id)?;
    if r.historical {
        return Err(Problem::new(c.prefix.code("historical"), "Historical record is read-only",
            shown(&r.path), "Inherited records cannot be promoted or rewritten.",
            "Create a new proposal with the new command instead.")
        .exit(2));
    }
    let text = read_text(c.prefix.as_str(), &r.path, MAX_RECORD_BYTES)?;
    Ok((read_record(c, &r.path, &text)?, text))
}

/// Checks a state change against the workflow.
fn check_move(d: &Decision, c: &Catalog, r: &Record) -> Result<()> {
    let (prefix, process) = (&c.prefix, &c.process);
    let next: &[String] = process.state(&r.state).map_or(&[], |s| &s.next);
    if process.state(&d.to).is_none() || !contains(next, &d.to) {
        let moves = if next.is_empty() { "none".to_string() } else { next.join(", ") };
        return Err(Problem::new(prefix.code("transition"), "That lifecycle move is not allowed",
            shown(&r.path), format!("A record in {} may move to: {moves}.", r.state),
            format!("Choose one of those states, or add the move to workflow.states.{}.next \
                in {CONFIG_FILE}.", r.state))
        .exit(2)
        .suggest(gd_report::suggest(&d.to, next.iter().map(String::as_str))));
    }
    if r.is_draft() && !process.is_unnumbered(&d.to) {
        return Err(Problem::new(prefix.code("transition"), "A draft needs a number first",
            shown(&r.path), format!("{} is a state for numbered records.", d.to),
            "Use the promote command, which numbers the draft and opens it.")
        .exit(2));
    }
    check_decision(d, c, r, &d.to, true)
}

/// Every move into an evidence-bearing state needs evidence, including promotion. A
/// promotion has a default reason; a state change needs its own.
fn check_decision(d: &Decision, c: &Catalog, r: &Record, target: &str, needs_reason: bool)
    -> Result<()> {
    let reason = |message: String, hint: &str| {
        Err(Problem::new(c.prefix.code("reason"), "Record the decision", shown(&r.path),
            message, hint)
        .exit(2))
    };
    if needs_reason && d.reason.trim().is_empty() {
        return reason("State changes need a reason.".into(),
            "Pass --reason='Explain why this decision is appropriate'.");
    }
    if c.process.state(target).is_some_and(|s| s.evidence) && d.evidence.trim().is_empty() {
        return reason(format!("Moving into {target} needs evidence, including by promotion."),
            "Pass --evidence='path/to/review-notes.rst' naming the review or validation.")
            .map_err(|p| Problem { title: "Record the evidence".into(), ..p });
    }
    Ok(())
}
