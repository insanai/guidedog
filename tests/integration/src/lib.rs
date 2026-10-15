//! Support for Guidedog's end-to-end tests: private scratch folders, the repository's
//! own files, and the built programs (`guidedog`, `dsc`, `dscc`, ...) run as a user runs
//! them.
//!
//! The programs are found beside the test executable, in the profile folder cargo builds
//! into (`target/<profile>/`). `cargo test --workspace` builds them; running only this
//! package needs `cargo build --workspace` first, which the panic message says.

#![forbid(unsafe_code)]
#![allow(clippy::unwrap_used)] // test support: a failure to set up is a failed test.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The repository's root folder.
pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// A file or folder of this crate's fixtures (`tests/integration/data/<rel>`).
pub fn data_path(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join(rel)
}

/// The text of a repository file (`rel` is relative to the root); panics when unreadable.
pub fn repository_text(rel: &str) -> String {
    read(&repository_root().join(rel))
}

/// The folder cargo built the programs into: the test executable's profile folder.
pub fn profile_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    // target/<profile>/deps/<test>-<hash>: two folders up.
    exe.parent().and_then(Path::parent).unwrap().to_path_buf()
}

/// The path of a built program of the workspace (`"guidedog"`, `"dsc"`, ...). Panics
/// with directions when cargo has not built it.
pub fn program(name: &str) -> PathBuf {
    let path = profile_dir().join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    assert!(
        path.is_file(),
        "{} is not built. Run `cargo build --workspace` (or `cargo test --workspace`) first.",
        path.display()
    );
    path
}

/// A private, empty folder for one test, removed when dropped.
#[derive(Debug)]
pub struct Scratch {
    /// The folder's path.
    pub path: PathBuf,
}

impl Scratch {
    /// Makes a fresh folder named after `name`, the process and a counter, below the
    /// system's temporary folder.
    pub fn new(name: &str) -> Scratch {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("gd-it-{name}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch { path: dir.canonicalize().unwrap() }
    }

    /// A path below the folder.
    pub fn join(&self, rel: &str) -> PathBuf {
        self.path.join(rel)
    }

    /// Writes `text` to `rel`, making its folders.
    pub fn write(&self, rel: &str, text: &str) {
        write(&self.join(rel), text);
    }

    /// The text of `rel`; panics when unreadable.
    pub fn read(&self, rel: &str) -> String {
        read(&self.join(rel))
    }

    /// Whether `rel` names a file, a folder or a link.
    pub fn exists(&self, rel: &str) -> bool {
        fs::symlink_metadata(self.join(rel)).is_ok()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Writes `text` to `path`, making its folders; panics on failure.
pub fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, text).unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
}

/// The text of `path`; panics with the path when unreadable.
pub fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// What a program run ended with.
#[derive(Debug, Clone)]
pub struct Run {
    /// The exit status (-1 when a signal ended it).
    pub status: i32,
    /// Standard output.
    pub out: String,
    /// Standard error.
    pub err: String,
}

impl Run {
    /// Panics, showing the output, unless the run ended with `status`.
    pub fn expect(self, status: i32) -> Run {
        assert_eq!(
            self.status, status,
            "unexpected exit status\nstdout:\n{}\nstderr:\n{}",
            self.out, self.err
        );
        self
    }
}

/// Runs a built program of the workspace in `dir` with `args`, colors off and a private
/// configuration folder (`GUIDEDOG_CONFIG_DIR` inside `dir`'s parent is not touched: it
/// is set to `<dir>/.gd-config` only when the caller has not set one).
pub fn run_program(name: &str, dir: &Path, args: &[&str]) -> Run {
    let mut command = Command::new(program(name));
    command.args(args).current_dir(dir).env("NO_COLOR", "1");
    if std::env::var_os("GUIDEDOG_CONFIG_DIR").is_none() {
        command.env("GUIDEDOG_CONFIG_DIR", dir.join(".gd-config"));
    }
    run_command(&mut command)
}

/// Runs `command` to completion and collects its status and output.
pub fn run_command(command: &mut Command) -> Run {
    let output = command.output().unwrap_or_else(|e| panic!("cannot run {command:?}: {e}"));
    Run {
        status: output.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&output.stdout).into_owned(),
        err: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Runs `guidedog args` in `dir` and expects `status`.
pub fn guidedog(dir: &Path, status: i32, args: &[&str]) -> Run {
    run_program("guidedog", dir, args).expect(status)
}

/// Panics unless `text` contains `needle`, showing both.
pub fn has(text: &str, needle: &str) {
    assert!(text.contains(needle), "expected {needle:?} in:\n{text}");
}

/// Panics if `text` contains `needle`, showing both.
pub fn lacks(text: &str, needle: &str) {
    assert!(!text.contains(needle), "did not expect {needle:?} in:\n{text}");
}

/// A code block of an RST document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock {
    /// The 1-based line of its directive.
    pub line: usize,
    /// Its option lines, trimmed (`:caption: x`).
    pub options: Vec<String>,
    /// Its body, with the common indentation removed.
    pub code: String,
}

/// The bodies of the `.. code-block:: <language>` (and `.. code::`) directives in an RST
/// document, in order, with their indentation removed.
pub fn rst_code_blocks(text: &str, language: &str) -> Vec<String> {
    rst_code_block_list(text, language).into_iter().map(|b| b.code).collect()
}

/// The `.. code-block:: <language>` (and `.. code::`, `.. sourcecode::`) directives in an
/// RST document, in order, with their lines and options.
pub fn rst_code_block_list(text: &str, language: &str) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();
    let mut lines = text.lines().enumerate().peekable();
    while let Some((n, line)) = lines.next() {
        let trimmed = line.trim_start();
        let directive = trimmed
            .strip_prefix(".. code-block::")
            .or_else(|| trimmed.strip_prefix(".. code::"))
            .or_else(|| trimmed.strip_prefix(".. sourcecode::"));
        if directive.map(str::trim) != Some(language) {
            continue;
        }
        let outer = line.len() - trimmed.len();
        let mut rest = std::iter::from_fn(|| lines.next_if(|(_, l)| is_inside(l, outer)))
            .map(|(_, l)| l)
            .peekable();
        let (options, code) = directive_body(&mut rest);
        blocks.push(CodeBlock { line: n + 1, options, code });
    }
    blocks
}

/// Whether `line` belongs to a directive indented by `outer`: blank, or indented more.
fn is_inside(line: &str, outer: usize) -> bool {
    line.trim().is_empty() || line.len() - line.trim_start().len() > outer
}

/// Splits a directive's lines into its options and its body, the body without its
/// common indentation.
fn directive_body<'a>(lines: &mut impl Iterator<Item = &'a str>) -> (Vec<String>, String) {
    let mut options = Vec::new();
    let mut body: Vec<&str> = Vec::new();
    let mut in_options = true;
    for line in lines {
        if in_options && line.trim_start().starts_with(':') {
            options.push(line.trim().to_owned());
            continue;
        }
        if in_options && line.trim().is_empty() && body.is_empty() {
            in_options = false;
            continue;
        }
        in_options = false;
        body.push(line);
    }
    while body.last().is_some_and(|l| l.trim().is_empty()) {
        body.pop();
    }
    let common = body
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut out = String::new();
    for line in body {
        out.push_str(line.get(common..).unwrap_or(""));
        out.push('\n');
    }
    (options, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_blocks_are_found_with_their_options_and_indentation_removed() {
        let text = "Intro.\n\n.. code-block:: rust\n   :caption: x\n\n   fn main() {\n       \
            go();\n   }\n\nAfter.\n\n.. code-block:: toml\n\n   a = 1\n\n.. code:: rust\n\n   \
            let x = 1;\n";
        let blocks = rst_code_blocks(text, "rust");
        assert_eq!(blocks, ["fn main() {\n    go();\n}\n", "let x = 1;\n"]);
        assert_eq!(rst_code_blocks(text, "toml"), ["a = 1\n"]);
        let list = rst_code_block_list(text, "rust");
        assert_eq!((list[0].line, list[0].options.clone()), (3, vec![":caption: x".to_string()]));
        assert_eq!(list[1].line, 16);
    }
}
