//! The development backend: an installed `typst` program run with an argument vector,
//! never a shell. The typst CLI has no offline switch; offline builds are enforced by the
//! embedded backend only.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Compiled, Diagnostic, Error, Options, Severity, Target};

/// The most compiler output a result keeps.
const DETAIL_LIMIT: usize = 8192;

/// What the program compiles: a file, or generated text fed on stdin.
pub(crate) enum Input<'a> {
    File(&'a Path),
    Text(&'a str),
}

/// Runs the typst program; generated text goes through a temporary file fed on stdin, so
/// no file is written beside the user's sources and no pipe can deadlock.
pub(crate) fn run(
    program: &Path,
    input: Input<'_>,
    root: &Path,
    output: &Path,
    options: &Options<'_>,
) -> Result<Compiled, Error> {
    let mut command = Command::new(program);
    command.arg("compile").arg("--root").arg(root).args(["--diagnostic-format", "short"]);
    for path in &options.font_paths {
        command.arg("--font-path").arg(path);
    }
    if options.embedded_fonts_only {
        command.arg("--ignore-system-fonts");
    }
    if let Some(cache) = &options.package_cache {
        command.arg("--package-cache-path").arg(cache);
    }
    match options.target {
        Target::Pdf => command.args(["--format", "pdf"]),
        Target::Html => command.args(["--features", "html", "--format", "html"]),
    };
    let _temp = match input {
        Input::File(source) => {
            command.arg(source).stdin(Stdio::null());
            None
        }
        Input::Text(text) => {
            let (temp, file) = TempFile::create(text)?;
            command.arg("-").stdin(Stdio::from(file));
            Some(temp)
        }
    };
    command.arg(output);
    let result = command.output().map_err(|err| Error::Launch {
        program: program.display().to_string(),
        message: err.to_string(),
    })?;
    let errors = String::from_utf8_lossy(&result.stderr);
    let text = match errors.trim() {
        "" => String::from_utf8_lossy(&result.stdout).trim().to_string(),
        trimmed => trimmed.to_string(),
    };
    let diagnostics = parse_diagnostics(&text);
    let detail = bounded(text);
    if !result.status.success() {
        let exit_code = result.status.code();
        return Err(Error::Compile { exit_code, diagnostics, detail });
    }
    Ok(Compiled { diagnostics, detail })
}

fn bounded(mut text: String) -> String {
    if text.len() > DETAIL_LIMIT {
        let mut end = DETAIL_LIMIT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("\n[compiler output truncated]");
    }
    text
}

/// Reads Typst's short format, `file:line:col: severity: message`; a `hint: ` line joins
/// the diagnostic before it, and other lines are skipped.
pub(crate) fn parse_diagnostics(text: &str) -> Vec<Diagnostic> {
    let mut found: Vec<Diagnostic> = Vec::new();
    for line in text.lines() {
        if let Some(hint) = hint_of(line) {
            if let Some(last) = found.last_mut() {
                last.hints.push(hint.to_string());
            }
            continue;
        }
        let Some((at, severity)) = severity_at(line) else { continue };
        let word = severity.as_str();
        let mut d = Diagnostic {
            severity,
            message: line[at + word.len() + 4..].to_string(),
            ..Diagnostic::default()
        };
        let location = &line[..at];
        let mut parts = location.rsplitn(3, ':');
        match (parts.next(), parts.next(), parts.next()) {
            (Some(column), Some(row), Some(file)) => {
                d.line = row.parse().unwrap_or(0);
                d.column = column.parse().unwrap_or(0);
                d.file = file.to_string();
            }
            _ => d.file = location.to_string(),
        }
        d.stdin = d.file.ends_with("<stdin>");
        found.push(d);
    }
    found
}

fn severity_at(line: &str) -> Option<(usize, Severity)> {
    [(": error: ", Severity::Error), (": warning: ", Severity::Warning)]
        .into_iter()
        .find_map(|(pattern, s)| line.find(pattern).map(|at| (at, s)))
}

fn hint_of(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if let Some(hint) = trimmed.strip_prefix("hint: ") {
        return Some(hint);
    }
    line.find(": hint: ").map(|at| &line[at + 8..])
}

/// A temporary file removed when dropped.
struct TempFile(PathBuf);

impl TempFile {
    /// Writes `text` to a new temporary file and opens it for reading.
    fn create(text: &str) -> Result<(TempFile, File), Error> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = format!("guidedog-{}-{n}.typ", std::process::id());
        let path = std::env::temp_dir().join(name);
        let io = |path: &Path, err: std::io::Error| Error::Io {
            path: path.to_path_buf(),
            message: err.to_string(),
        };
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|err| io(&path, err))?;
        let temp = TempFile(path);
        file.write_all(text.as_bytes()).map_err(|err| io(&temp.0, err))?;
        drop(file);
        let reader = File::open(&temp.0).map_err(|err| io(&temp.0, err))?;
        Ok((temp, reader))
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
