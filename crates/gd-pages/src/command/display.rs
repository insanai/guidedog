//! What the command line prints: problems as Elm-style reports, progress, plans and
//! outcomes, as text or JSON lines.

use std::io::{IsTerminal, Write};

use gd_json::{Object, Value};
use gd_plugins::{ChangeKind, Outcome, Plan, Problem, State};
use gd_report::{Style, escape_terminal};

use super::Options;

const GREEN: &str = "32";

/// Where a command writes, and how.
pub(crate) struct Console<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    color_out: bool,
    color_err: bool,
    /// `--diagnostics=json`: problems and confirmations as JSON lines.
    json: bool,
}

fn use_color(choice: &str, terminal: bool) -> bool {
    match choice {
        "always" => true,
        "never" => false,
        _ => {
            terminal
                && std::env::var_os("NO_COLOR").is_none()
                && std::env::var("TERM").map_or(true, |t| t != "dumb")
        }
    }
}

fn styled(text: &str, code: &str, color: bool) -> String {
    if color { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
}

/// The first 12 characters of an ID.
pub(crate) fn short(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

impl<'a> Console<'a> {
    pub(crate) fn new(out: &'a mut dyn Write, err: &'a mut dyn Write, o: &Options) -> Console<'a> {
        Console {
            out,
            err,
            color_out: use_color(&o.color, std::io::stdout().is_terminal()),
            color_err: use_color(&o.color, std::io::stderr().is_terminal()),
            json: o.json,
        }
    }

    /// Writes `text` to standard output as it is.
    pub(crate) fn print(&mut self, text: &str) {
        let _ = self.out.write_all(text.as_bytes());
    }

    /// Reports `p` on standard error, as text or JSON, and returns its exit status.
    pub(crate) fn report(&mut self, p: &Problem) -> i32 {
        let report = p.report();
        let text = if self.json {
            format!("{}\n", gd_report::render_json(&report))
        } else {
            let style = Style { color: self.color_err, ..Style::default() };
            format!("\n{}\n", gd_report::render_text(&report, &style))
        };
        let _ = self.err.write_all(text.as_bytes());
        p.exit.max(1)
    }

    /// Writes one line of progress to standard error, which a script's output skips.
    pub(crate) fn say(&mut self, message: &str) {
        if !self.json {
            let _ = writeln!(self.err, "  {}", escape_terminal(message));
        }
    }

    /// Confirms a finished step on standard output.
    pub(crate) fn success(&mut self, message: &str) {
        if self.json {
            let mut o = Object::new();
            o.insert("status".into(), "ok".into());
            o.insert("message".into(), message.into());
            let _ = writeln!(self.out, "{}", gd_json::to_string(&Value::Object(o)));
            return;
        }
        let mark = styled("✓", GREEN, self.color_out);
        let _ = writeln!(self.out, "  {mark} {}", escape_terminal(message));
    }

    /// Writes a line of a review (account, repository, ...) on standard output.
    pub(crate) fn line(&mut self, text: &str) {
        if !self.json {
            let _ = writeln!(self.out, "  {}", escape_terminal(text));
        }
    }

    /// Writes what a plan publishes: the target and account, and the changes (the first
    /// 20 unless `verbose`).
    pub(crate) fn show_plan(&mut self, plan: &Plan, verbose: bool) {
        if self.json {
            let changes = plan.changes.iter().map(|c| {
                let mut o = Object::new();
                o.insert("path".into(), c.path.as_str().into());
                o.insert("kind".into(), c.kind.name().into());
                Value::Object(o)
            });
            let mut o = Object::new();
            o.insert("target".into(), plan.target.as_str().into());
            o.insert("baseline".into(), plan.baseline.as_str().into());
            o.insert("summary".into(), plan.summary.as_str().into());
            o.insert("generation".into(), plan.generation_hex().into());
            o.insert("unchanged".into(), plan.unchanged.into());
            o.insert("changes".into(), Value::Array(changes.collect()));
            let _ = writeln!(self.out, "{}", gd_json::to_string(&Value::Object(o)));
            return;
        }
        let _ = writeln!(self.out, "\n  {}", escape_terminal(&plan.summary));
        if plan.unchanged {
            let _ =
                writeln!(self.out, "  The branch already holds this site: nothing to publish.\n");
            return;
        }
        let count = |k: ChangeKind| plan.changes.iter().filter(|c| c.kind == k).count();
        let _ = writeln!(
            self.out,
            "  {} added, {} changed, {} removed",
            count(ChangeKind::Added),
            count(ChangeKind::Changed),
            count(ChangeKind::Removed)
        );
        for (i, c) in plan.changes.iter().enumerate() {
            if i == 20 && !verbose {
                let _ = writeln!(
                    self.out,
                    "    ... and {} more (--verbose lists them all)",
                    plan.changes.len() - 20
                );
                break;
            }
            let verb = match c.kind {
                ChangeKind::Added => "add",
                ChangeKind::Changed => "change",
                ChangeKind::Removed => "remove",
            };
            let _ = writeln!(self.out, "    {verb:<6} {}", escape_terminal(&c.path));
        }
        let _ = writeln!(self.out);
    }

    /// Writes an outcome and returns the command's exit status for it.
    pub(crate) fn show_result(&mut self, r: &Outcome) -> i32 {
        let status = r.exit_status();
        if self.json {
            let mut o = Object::new();
            o.insert("state".into(), r.state.name().into());
            o.insert("operation".into(), r.operation.as_str().into());
            o.insert("generation".into(), r.generation.as_str().into());
            o.insert("url".into(), r.url.as_str().into());
            let problem = r.problem.as_ref().map_or(Value::Null, |p| {
                gd_json::parse(&gd_report::render_json(&p.report())).unwrap_or(Value::Null)
            });
            o.insert("problem".into(), problem);
            let _ = writeln!(self.out, "{}", gd_json::to_string(&Value::Object(o)));
            return status;
        }
        match r.state {
            State::Live => self.success(&format!(
                "Published {} (commit {}): live at {}",
                short(&r.generation),
                short(&r.operation),
                r.url
            )),
            State::Unchanged => {
                self.success(&format!("Already published: {} holds this site", r.url));
            }
            State::Accepted | State::Pending => {
                self.success(&format!(
                    "Published commit {}; GitHub Pages has not confirmed it \
                    live yet",
                    short(&r.operation)
                ));
                if let Some(p) = &r.problem {
                    self.report(p);
                }
            }
            State::Failed | State::Unknown | State::NotStarted => {
                if let Some(p) = &r.problem {
                    self.report(p);
                }
            }
        }
        status
    }
}

/// Asks `question` on the terminal and returns the answer. Without a terminal, or with
/// `--yes`, it does not ask: `--yes` is yes, and no terminal is no.
pub(crate) fn confirm(o: &Options, question: &str) -> bool {
    if o.yes {
        return true;
    }
    if !std::io::stdin().is_terminal() {
        return false;
    }
    eprint!("  {} [y/N] ", escape_terminal(question));
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
}

/// `pages.unconfirmed`: the user did not confirm `what`.
pub(crate) fn not_confirmed(what: &str) -> Problem {
    crate::problem::problem(
        "pages.unconfirmed",
        "Not confirmed",
        what,
        &format!("Nothing was changed: {what} needs your confirmation."),
        "Run the command in a terminal and answer y, or add --yes once you have checked what \
         it shows.",
    )
}
