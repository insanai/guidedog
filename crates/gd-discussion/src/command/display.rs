//! What the command line prints: reports, confirmations, previews, lists, and records.

use super::options::Options;
use crate::{Catalog, Plan, Problem, Record};
use gd_json::{Object, Value};
use gd_report::{Style, escape_terminal};
use std::io::Write;

const BOLD_CYAN: &str = "1;36";
const GREEN: &str = "32";
const FAINT: &str = "2";

/// Where a command writes, and how.
pub(crate) struct Console<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    /// Whether standard output and standard error take colors.
    pub color_out: bool,
    pub color_err: bool,
    /// `--diagnostics=json`: problems and confirmations as JSON lines.
    pub json: bool,
    /// `--quiet`: no confirmations.
    pub quiet: bool,
}

impl std::fmt::Debug for Console<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Console").field("json", &self.json).finish_non_exhaustive()
    }
}

fn styled(text: &str, code: &str, color: bool) -> String {
    if color { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
}

impl Console<'_> {
    /// Reports `p` on standard error and returns its exit status.
    pub fn report(&mut self, p: &Problem) -> i32 {
        let report = p.report();
        let text = if self.json {
            let mut line = gd_report::render_json(&report);
            line.push('\n');
            line
        } else {
            let style = Style { color: self.color_err, ..Style::default() };
            format!("\n{}\n", gd_report::render_text(&report, &style))
        };
        let _ = self.err.write_all(text.as_bytes());
        p.exit.max(1)
    }

    /// Confirms a finished command, unless quiet.
    pub fn success(&mut self, message: &str) {
        if self.quiet {
            return;
        }
        if self.json {
            let mut o = Object::new();
            o.insert("status".into(), "ok".into());
            o.insert("message".into(), message.into());
            let _ = writeln!(self.out, "{}", gd_json::to_string(&Value::Object(o)));
        } else {
            let mark = styled("✓", GREEN, self.color_out);
            let _ = writeln!(self.out, "  {mark} {}", escape_terminal(message));
        }
    }

    /// Shows what a plan would change, and changes nothing.
    pub fn preview(&mut self, plan: &Plan, json: bool) -> i32 {
        if json {
            let mut o = Object::new();
            o.insert("summary".into(), plan.summary.as_str().into());
            o.insert("changes".into(), crate::transaction::changes_json(&plan.changes));
            let _ = writeln!(self.out, "{}", gd_json::to_string(&Value::Object(o)));
            return 0;
        }
        let _ = writeln!(self.out, "\n  Preview · {}", escape_terminal(&plan.summary));
        for change in &plan.changes {
            let _ = writeln!(self.out, "    {}", escape_terminal(&change.path));
        }
        let _ = writeln!(self.out, "\n  No files changed.\n");
        0
    }

    /// Lists the records `o` selects (by state, label, and whether historical ones show).
    pub fn list(&mut self, o: &Options, c: &Catalog, heading: &str) {
        let rows: Vec<&Record> = c.records.iter().filter(|r| {
            (o.historical || !r.historical)
                && (o.state.is_empty() || r.state == o.state)
                && (o.label.is_empty() || r.labels.contains(&o.label))
        })
        .collect();
        if o.json_format {
            let items = rows.iter().map(|r| record_json(r)).collect();
            let _ = writeln!(self.out, "{}", gd_json::to_string(&Value::Array(items)));
            return;
        }
        let color = self.color_out;
        let _ = writeln!(self.out, "\n  {}\n", styled(heading, BOLD_CYAN, color));
        let _ = writeln!(self.out, "  {:<7} {:<14} TITLE", c.prefix.upper(), "STATE");
        for r in &rows {
            let _ = writeln!(self.out, "  {:<7} {:<14} {}", r.number, r.state,
                escape_terminal(&r.title));
            let _ = writeln!(self.out, "  {:<7} {:<14} {}", "", "", styled(&r.slug, FAINT, color));
        }
        if rows.is_empty() {
            let _ = writeln!(self.out, "  No discussions match these filters.");
        }
        let _ = writeln!(self.out, "\n  {} discussion(s) · {}\n", rows.len(),
            escape_terminal(&c.dir.display().to_string()));
    }

    /// Shows one record's metadata.
    pub fn show(&mut self, c: &Catalog, r: &Record, json: bool) {
        if json {
            let _ = writeln!(self.out, "{}", gd_json::to_string(&record_json(r)));
            return;
        }
        let _ = writeln!(self.out, "\n  {} {} · {}\n", c.prefix.upper(), r.number,
            escape_terminal(&r.title));
        let source = r.path.display().to_string();
        let fields = [
            ("State", r.state.clone()),
            ("Type", r.kind.clone()),
            ("Authors", r.authors.join(", ")),
            ("Labels", r.labels.join(", ")),
            ("Created", r.created.clone()),
            ("Updated", r.updated.clone()),
            ("Category", r.category.clone()),
            ("Status", r.status.clone()),
            ("Summary", r.summary.clone()),
            ("Source", source),
            ("Discussion", r.discussion.clone()),
        ];
        for (name, value) in fields.iter().filter(|(_, v)| !v.is_empty()) {
            let _ = writeln!(self.out, "  {name:<12} {}", escape_terminal(value));
        }
        let _ = writeln!(self.out);
    }

    /// Writes `text` as it is to standard output.
    pub fn print(&mut self, text: &str) {
        let _ = self.out.write_all(text.as_bytes());
    }
}

/// A record as `list` and `show` write it in JSON.
fn record_json(r: &Record) -> Value {
    let mut o = Object::new();
    let texts = [
        ("number", &r.number),
        ("slug", &r.slug),
        ("title", &r.title),
        ("state", &r.state),
        ("kind", &r.kind),
    ];
    for (key, value) in texts {
        o.insert(key.into(), value.as_str().into());
    }
    o.insert("source".into(), r.path.display().to_string().into());
    o.insert("authors".into(), r.authors.clone().into());
    o.insert("labels".into(), r.labels.clone().into());
    let more = [
        ("created", &r.created),
        ("updated", &r.updated),
        ("discussion", &r.discussion),
        ("category", &r.category),
        ("status", &r.status),
        ("summary", &r.summary),
    ];
    for (key, value) in more {
        o.insert(key.into(), value.as_str().into());
    }
    o.insert("historical".into(), r.historical.into());
    Value::Object(o)
}
