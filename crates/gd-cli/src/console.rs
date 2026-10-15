//! Where a command writes: standard output for results, standard error for reports, and
//! standard input for the answers to questions. Tests give it buffers instead.

use std::io::{BufRead, Write};

use gd_host::Problem;
use gd_report::ansi::{BOLD_CYAN, FAINT, GREEN, RED, styled};
use gd_report::{Report, Severity, Style};

use crate::command::{ColorMode, Diagnostics, Output};

/// Whether each standard stream is a terminal: colors (`--color=auto`) and questions
/// (only a person at a terminal is asked) depend on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Terminals {
    /// Standard input is a terminal.
    pub stdin: bool,
    /// Standard output is a terminal.
    pub stdout: bool,
    /// Standard error is a terminal.
    pub stderr: bool,
}

impl Terminals {
    /// The process's own streams.
    pub fn detect() -> Terminals {
        use std::io::IsTerminal;
        Terminals {
            stdin: std::io::stdin().is_terminal(),
            stdout: std::io::stdout().is_terminal(),
            stderr: std::io::stderr().is_terminal(),
        }
    }
}

/// The streams a command line runs with, borrowed for the run.
pub struct Streams<'a> {
    /// Results: tables, converted output, success messages.
    pub out: &'a mut dyn Write,
    /// Reports, problems, progress, and questions.
    pub err: &'a mut dyn Write,
    /// Answers to questions, one per line.
    pub input: &'a mut dyn BufRead,
    /// Which streams are terminals.
    pub terminals: Terminals,
}

impl std::fmt::Debug for Streams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Streams").field("terminals", &self.terminals).finish_non_exhaustive()
    }
}

/// A command's streams with its reporting options applied.
pub(crate) struct Console<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub input: &'a mut dyn BufRead,
    pub color_out: bool,
    pub color_err: bool,
    pub json: bool,
    pub quiet: bool,
    pub terminals: Terminals,
}

/// Whether `mode` styles a stream that is a terminal or not (`tty`).
fn use_color(mode: ColorMode, tty: bool) -> bool {
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => gd_report::ansi::terminal_color(tty),
    }
}

impl<'a> Console<'a> {
    pub fn new(streams: Streams<'a>, output: Output) -> Console<'a> {
        Console {
            out: streams.out,
            err: streams.err,
            input: streams.input,
            color_out: use_color(output.color, streams.terminals.stdout),
            color_err: use_color(output.color, streams.terminals.stderr),
            json: output.diagnostics == Diagnostics::Json,
            quiet: output.quiet,
            terminals: streams.terminals,
        }
    }

    /// Whether a question may be asked: a person at a terminal reading text reports.
    pub fn interactive(&self) -> bool {
        !self.json && self.terminals.stdin && self.terminals.stderr
    }

    /// Writes `text` to standard output. A closed output is not this command's problem.
    pub fn print(&mut self, text: &str) {
        let _ = self.out.write_all(text.as_bytes());
    }

    /// Writes `text` to standard error.
    pub fn eprint(&mut self, text: &str) {
        let _ = self.err.write_all(text.as_bytes());
    }

    /// Reports `problem` on standard error and returns its exit status.
    pub fn problem(&mut self, problem: &Problem) -> i32 {
        self.report(&problem.report(), "");
        problem.exit
    }

    /// Reports `report` on standard error: as JSON, or explained. `consequence` is a
    /// sentence added to an error's message (what the failure means for the output).
    /// `-q` keeps errors only.
    pub fn report(&mut self, report: &Report, consequence: &str) {
        if self.quiet && report.severity != Severity::Error {
            return;
        }
        let mut r = report.clone();
        if let Some(hint) = limit_hint(&r.code) {
            r.hints = vec![hint.to_string()];
        }
        if !consequence.is_empty() {
            r.message = format!("{} {consequence}", r.message);
        }
        if self.json {
            let line = gd_report::render_json(&r);
            self.eprint(&format!("{line}\n"));
        } else {
            let text = gd_report::render_text(&r, &Style { color: self.color_err, width: 80 });
            self.eprint(&text);
        }
    }

    /// A success message on standard output, as text or as a JSON object; nothing with
    /// `-q`.
    pub fn success(&mut self, message: &str) {
        if self.quiet {
            return;
        }
        if self.json {
            let mut line = String::from("{\"status\":\"ok\",\"message\":");
            gd_report::escape_json(&mut line, message);
            line.push_str("}\n");
            self.print(&line);
        } else {
            let tick = styled(gd_report::terminal_symbols().check, GREEN, self.color_out);
            let text = format!("  {tick} {}\n", gd_report::escape_terminal(message));
            self.print(&text);
        }
    }

    /// A failure line in the verdict's style, for the build summary.
    pub fn verdict(&self, ok: bool) -> String {
        let symbols = gd_report::terminal_symbols();
        if ok {
            styled(&format!("{} build succeeded", symbols.check), GREEN, self.color_out)
        } else {
            styled(&format!("{} build failed", symbols.cross), RED, self.color_out)
        }
    }

    /// `guidedog TEXT`, the line a project command starts with; nothing when quiet or
    /// reporting JSON.
    pub fn heading(&mut self, text: &str) {
        if self.quiet || self.json {
            return;
        }
        let name = styled("guidedog", BOLD_CYAN, self.color_out);
        let line = format!("{name} {}\n", gd_report::escape_terminal(text));
        self.print(&line);
    }

    /// One step of a command's progress: a faint label and its text.
    pub fn step(&mut self, label: &str, text: &str) {
        if self.quiet || self.json {
            return;
        }
        let label = styled(&format!("{label:<8}"), FAINT, self.color_out);
        let line = format!("  {label} {}\n", gd_report::escape_terminal(text));
        self.print(&line);
    }

    /// A section heading of a table (`INPUT FORMATS`).
    pub fn section(&self, text: &str) -> String {
        styled(text, BOLD_CYAN, self.color_out)
    }

    /// Shows `question` on standard output and reads the answer: one line of standard
    /// input, trimmed; `None` at the end of input.
    pub fn prompt(&mut self, question: &str) -> Option<String> {
        self.print(question);
        let _ = self.out.flush();
        let mut line = String::new();
        match self.input.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim().to_string()),
        }
    }
}

/// The hints of the engine's limit reports as the command line gives them: the engine
/// names the `Limits` field to raise, which the command line sets with a flag.
fn limit_hint(code: &str) -> Option<&'static str> {
    Some(match code {
        "core.limit.stack" => {
            "Flatten the nesting, or raise --stack-kib=N; the thread's stack bounds how far."
        }
        "core.limit.depth" | "text.limit.depth" | "typst.limit.depth" => {
            "Flatten the nesting, or raise --max-depth=N."
        }
        "commonmark.nesting" => "Flatten the nesting of quotes and lists, or raise --max-depth=N.",
        "core.limit.nodes" => "Split the document, or raise --max-nodes=N.",
        // One conversion's text and output may use at most the memory budget.
        "core.capacity" => "Split the document, or raise --budget=N (MiB).",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_follow_the_mode_and_the_terminal() {
        assert!(use_color(ColorMode::Always, false));
        assert!(!use_color(ColorMode::Never, true));
        assert!(!use_color(ColorMode::Auto, false));
    }

    #[test]
    fn limit_reports_name_the_flag_to_raise() {
        assert!(limit_hint("core.limit.nodes").is_some_and(|h| h.contains("--max-nodes")));
        assert!(limit_hint("core.capacity").is_some_and(|h| h.contains("--budget")));
        assert_eq!(limit_hint("rst.unknown"), None);
    }
}
