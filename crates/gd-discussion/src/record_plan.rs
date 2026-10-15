//! The plan of one record's promotion or state change: its fields, its lifecycle event,
//! its file (renamed when it takes a number), and the refreshed index.

use crate::catalog::{Catalog, sort_by_number};
use crate::edit::Decision;
use crate::index::plan_index;
use crate::metadata::{FieldValue, Record, is_rst, read_record};
use crate::transaction::{Change, Plan};
use crate::{Problem, Result, exists, replace_field, rst, today, typst};

/// Plans `r`, whose whole text is `text`, moving to `target` with `number`.
pub(crate) fn record_plan(
    c: &Catalog,
    d: &Decision,
    r: &Record,
    original: &str,
    target: &str,
    number: &str,
) -> Result<Plan> {
    let prefix = &c.prefix;
    // Calendar dates can differ between contributors' time zones: keep the latest
    // recorded date when the local clock is behind it.
    let date = today().max(r.updated.clone());
    let status = c.process.state(target).map_or("", |s| s.status.as_str());
    let mut edits = vec![("state", target), ("number", number), ("updated", date.as_str())];
    if !d.discussion.is_empty() {
        edits.push(("discussion", &d.discussion));
    }
    if !r.status.is_empty() && !status.is_empty() {
        edits.push(("status", status));
    }
    let mut text = original.to_string();
    for (key, value) in edits {
        text = replace_field(&text, prefix, key, FieldValue::Text(value))?;
    }
    let reason = if d.reason.is_empty() {
        "Assigned a permanent number and opened for discussion."
    } else {
        &d.reason
    };
    let event = format!("{date}: {} → {target}. {reason} {}", r.state, d.evidence);
    if is_rst(&r.path) {
        text.push_str(&rst::history(event.trim()));
    } else {
        text.push_str(&typst::history(event.trim()));
    }
    let suffix = if is_rst(&r.path) { ".rst" } else { ".typ" };
    let path = format!("records/{number}-{}{suffix}", r.slug);
    let updated = read_record(c, &c.dir.join(&path), &text)?;
    let mut records: Vec<&Record> =
        c.records.iter().map(|row| if row.path == r.path { &updated } else { row }).collect();
    sort_by_number(&mut records);
    let mut plan = plan_index(c, &records)?;
    let old_path = format!("records/{}", r.file_name());
    let before = Some(original.to_string());
    if old_path == path {
        plan.changes.push(Change { path: path.clone(), before, after: Some(text) });
    } else {
        if exists(&c.dir.join(&path)) {
            return Err(Problem::new(
                prefix.code("exists"),
                "Destination exists",
                &path,
                "Promotion would overwrite a file.",
                "Choose an unreserved number with --number.",
            )
            .exit(2));
        }
        plan.changes.push(Change { path: old_path.clone(), before, after: None });
        plan.changes.push(Change { path: path.clone(), before: None, after: Some(text) });
    }
    plan.summary = format!("{old_path} → {path} ({target})");
    Ok(plan)
}
