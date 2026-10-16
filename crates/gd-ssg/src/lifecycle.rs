//! Publication transitions operate on a single source and never invoke version control.

use std::path::Path;
use std::time::SystemTime;

use gd_env::PageMeta;
use gd_host::{Problem, io_problem};
use guidedoc::{Request, Workspace, default_request, read_document};

use crate::metadata::replace_fields;
use crate::transaction::write_source;

/// An explicit source-level publication transition.
#[derive(Debug, Clone, Copy)]
pub enum Publication<'a> {
    /// Set draft, preserving publication history and schedule.
    Draft,
    /// Clear draft and schedule at the supplied RFC 3339 instant.
    Schedule(&'a str),
    /// Clear draft and schedule; set the date explicitly or to the build's UTC date.
    Publish(Option<&'a str>),
}

/// Reads and validates leading page metadata with the real document reader.
/// Fails for unreadable files, unsupported suffixes or invalid publication metadata.
pub fn read_metadata(path: &Path) -> Result<PageMeta, Problem> {
    metadata(path, &read_source(path)?)
}

fn metadata(path: &Path, text: &str) -> Result<PageMeta, Problem> {
    let markdown = is_markdown(path)?;
    let name = path.display().to_string();
    let text = crate::metadata::leading_metadata(text, markdown);
    let mut request: Request<'_> = default_request(&name, text, "", "");
    request.read.promote_title = false;
    let mut workspace = Workspace::new();
    let reader =
        if markdown { guidedoc_commonmark::MYST_READER } else { guidedoc_sphinx::reader() };
    let doc = read_document(&mut workspace, &reader, &request)
        .map_err(|e| invalid(path, &e.diagnostic.formatted_message()))?;
    let page = gd_env::page_metadata(doc).map_err(|error| invalid(path, &error.to_string()))?;
    if let Some(report) = workspace.reports().items().iter().find(|d| d.code == "myst.topmatter") {
        return Err(invalid(path, &report.formatted_message()));
    }
    Ok(page)
}

/// Atomically creates a draft source; fails rather than replacing an existing file.
/// `date` is a valid calendar date, and `title` and `template` must be single lines.
pub fn create_post(path: &Path, title: &str, template: &str, date: &str) -> Result<(), Problem> {
    let markdown = is_markdown(path)?;
    gd_clock::parse_date(date).map_err(|e| invalid(path, &e.to_string()))?;
    if title.is_empty() || title.contains(['\n', '\r']) || template.contains(['\n', '\r']) {
        return Err(invalid(
            path,
            "A title is required and metadata values must be single lines.",
        ));
    }
    let body = if markdown {
        format!("# {title}\n\n")
    } else {
        // Byte length covers wide Unicode characters without a terminal-width dependency.
        format!("{title}\n{}\n\n", "=".repeat(title.len()))
    };
    let mut fields = vec![("draft", Some("true")), ("date", Some(date))];
    if !template.is_empty() {
        fields.push(("template", Some(template)));
    }
    let source = replace_fields(&body, markdown, &fields)?;
    metadata(path, &source)?;
    write_source(path, None, &source)
}

/// Atomically applies a validated transition, retaining body bytes and file permissions.
/// Publishing a future-dated or expired post resets its release boundary to `now`.
pub fn transition(path: &Path, state: Publication<'_>, now: SystemTime) -> Result<(), Problem> {
    let before = read_source(path)?;
    let page = metadata(path, &before)?;
    let today = utc_date(now);
    let fields = match state {
        Publication::Draft => vec![("draft", Some("true"))],
        Publication::Schedule(at) => {
            gd_clock::parse_timestamp(at).map_err(|e| invalid(path, &e.to_string()))?;
            vec![("draft", Some("false")), ("publish-at", Some(at)), ("expires-at", None)]
        }
        Publication::Publish(date) => {
            let date = date.unwrap_or_else(|| {
                if page.date.is_empty()
                    || gd_env::publication_time(&page.date).is_ok_and(|t| t > now)
                {
                    &today
                } else {
                    &page.date
                }
            });
            let at = gd_env::publication_time(date).map_err(|e| invalid(path, &e.to_string()))?;
            if at > now {
                return Err(invalid(
                    path,
                    "A publish date cannot be in the future; use schedule.",
                ));
            }
            vec![
                ("draft", Some("false")),
                ("publish-at", None),
                ("expires-at", None),
                ("date", Some(date)),
            ]
        }
    };
    let after = replace_fields(&before, is_markdown(path)?, &fields)?;
    metadata(path, &after)?;
    write_source(path, Some(&before), &after)
}

pub(crate) fn utc_date(now: SystemTime) -> String {
    let seconds = match now.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
        Err(e) => {
            -i64::try_from(e.duration().as_secs()).unwrap_or(i64::MAX)
                - i64::from(e.duration().subsec_nanos() != 0)
        }
    };
    gd_clock::DateTime::from_unix(seconds).date.to_string()
}

pub(crate) fn read_source(path: &Path) -> Result<String, Problem> {
    let info = std::fs::symlink_metadata(path).map_err(|e| io_problem(path, &e))?;
    if !info.is_file() {
        return Err(invalid(path, "Expected a regular source file, without a symlink."));
    }
    let bytes = gd_host::read_file(path, None, gd_host::MAX_SOURCE_BYTES)?;
    String::from_utf8(bytes).map_err(|_| invalid(path, "The source must be UTF-8."))
}

fn is_markdown(path: &Path) -> Result<bool, Problem> {
    match path.extension().and_then(|s| s.to_str()) {
        Some("md") => Ok(true),
        Some("rst") => Ok(false),
        _ => Err(invalid(path, "Expected an .rst or .md source file.")),
    }
}

fn invalid(path: &Path, message: &str) -> Problem {
    Problem::new(
        "ssg.metadata",
        "INVALID PUBLICATION SOURCE",
        &path.display().to_string(),
        message,
        "Correct the source metadata, then retry guidedog ssg.",
        2,
    )
}
