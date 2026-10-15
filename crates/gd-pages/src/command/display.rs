//! What the command line prints: problems as Elm-style reports, progress, plans and
//! outcomes, as text or JSON lines.

use std::io::{IsTerminal, Write};

use gd_json::Value;
use gd_plugins::{Outcome, Plan, Problem, State};
use gd_report::{Style, escape_terminal};

use super::Options;
use crate::target::short;

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

/// A JSON object of `fields`, in their order.
fn object<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(fields.into_iter().map(|(key, value)| (key.to_string(), value)).collect())
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

    /// Writes `value` as one JSON line on standard output.
    fn json_line(&mut self, value: Value) {
        let _ = writeln!(self.out, "{}", gd_json::to_string(&value));
    }

    /// Confirms a finished step on standard output.
    pub(crate) fn success(&mut self, message: &str) {
        if self.json {
            self.json_line(object([("status", "ok".into()), ("message", message.into())]));
            return;
        }
        let mark = styled(gd_report::terminal_symbols().check, GREEN, self.color_out);
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
                object([("path", c.path.as_str().into()), ("kind", c.kind.name().into())])
            });
            self.json_line(object([
                ("target", plan.target.as_str().into()),
                ("baseline", plan.baseline.as_str().into()),
                ("summary", plan.summary.as_str().into()),
                ("generation", plan.generation_hex().into()),
                ("unchanged", plan.unchanged.into()),
                ("changes", Value::Array(changes.collect())),
            ]));
            return;
        }
        let _ = writeln!(self.out);
        for line in crate::guide::plan_lines(plan, verbose) {
            let _ = writeln!(self.out, "  {}", escape_terminal(&line));
        }
        let _ = writeln!(self.out);
    }

    /// Writes an outcome and returns the command's exit status for it.
    pub(crate) fn show_result(&mut self, r: &Outcome) -> i32 {
        let status = r.exit_status();
        if self.json {
            let problem = r.problem.as_ref().map_or(Value::Null, |p| {
                gd_json::parse(&gd_report::render_json(&p.report())).unwrap_or(Value::Null)
            });
            self.json_line(object([
                ("state", r.state.name().into()),
                ("operation", r.operation.as_str().into()),
                ("generation", r.generation.as_str().into()),
                ("url", r.url.as_str().into()),
                ("problem", problem),
            ]));
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

/// The guided publish's questions on the terminal; its lines go to standard error.
pub(crate) struct ConsolePrompt<'c, 'a> {
    pub console: &'c mut Console<'a>,
}

/// Asks `question` on standard error and reads one line of standard input.
fn ask_line(question: &str) -> Option<String> {
    eprint!("  {} ", escape_terminal(question));
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).ok()?;
    Some(answer.trim().to_string())
}

impl crate::Prompt for ConsolePrompt<'_, '_> {
    fn show(&mut self, text: &str) {
        self.console.say(text);
    }

    fn confirm(&mut self, question: &str) -> bool {
        let answer = ask_line(&format!("{question} [y/N]")).unwrap_or_default();
        matches!(answer.to_lowercase().as_str(), "y" | "yes")
    }

    fn choose(&mut self, question: &str, choices: &[&str]) -> Option<usize> {
        self.console.say(question);
        for (i, choice) in choices.iter().enumerate() {
            self.console.say(&format!("  {}. {choice}", i + 1));
        }
        let answer = ask_line(&format!("Choose 1 to {} (empty to stop):", choices.len()))?;
        let n = answer.parse::<usize>().ok()?;
        (1..=choices.len()).contains(&n).then(|| n - 1)
    }
}
