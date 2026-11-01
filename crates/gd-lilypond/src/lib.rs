//! Reads notation into a music model. Never runs Scheme, external programs or file I/O.
//! Includes are lexical and require an explicit caller-supplied policy callback.

#![forbid(unsafe_code)]

mod bindings;
mod commands;
mod contexts;
mod cursor;
mod files;
mod finish;
mod interpret;
mod lexer;
mod limits;
mod lyrics;
mod memory;
mod names;
mod notation;
mod notes;
mod ornaments;
mod parser;
mod repeats;
mod syntax;

pub use commands::{COMMANDS, Command, CommandStatus, command};
pub use files::{Failed, SourceFile, SourceFiles, error_report};
pub use gd_score::{Error, ErrorKind, FileId, Severity, Span};
pub use interpret::interpret_syntax;
pub use limits::Limits;
pub use parser::parse_lilypond;
pub use syntax::{Expr, ExprKind, Syntax, SyntaxId, Token, TokenKind};

use std::fmt;

/// An include name and the file in which it appeared.
#[derive(Clone, Copy, Debug)]
pub struct IncludeRequest<'a> {
    /// File whose directory is the first search location.
    pub from: FileId,
    /// Name written in the include command.
    pub name: &'a str,
}

/// Source text returned by a confined caller-supplied include reader.
#[derive(Clone, Debug)]
pub struct IncludedFile {
    /// Resolved name retained in diagnostics and dependency tracking.
    pub name: String,
    /// UTF-8 file contents.
    pub text: String,
}

/// A caller's refusal to read an included file.
#[derive(Clone, Debug)]
pub struct IncludeRefusal {
    /// Why the path could not be read.
    pub message: String,
    /// How to make the include valid.
    pub hint: String,
}

/// Decides a lexical include. Called synchronously; never stored after a load.
pub type Include<'a> = dyn FnMut(IncludeRequest<'_>) -> Result<IncludedFile, IncludeRefusal> + 'a;

/// Reader bounds and explicit file policy. The default refuses filesystem includes.
#[derive(Default)]
pub struct Options<'a> {
    /// Finite resource bounds.
    pub limits: Limits,
    /// Caller-owned include reader, borrowed only during parsing.
    pub include: Option<&'a mut Include<'a>>,
    /// Reserves the complete conservative reader allowance before expansion.
    pub reserve: Option<&'a dyn Fn(usize) -> bool>,
}

impl fmt::Debug for Options<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Options")
            .field("limits", &self.limits)
            .field("include", &self.include.is_some())
            .finish()
    }
}

/// A successful load, including retained files and nonfatal diagnostics.
#[derive(Clone, Debug)]
pub struct Loaded {
    /// Reader-independent music model.
    pub score: gd_score::Score,
    /// All files read, indexed by FileId.
    pub files: SourceFiles,
    /// Failed bar checks, ignored metadata and unused lyrics.
    pub warnings: Vec<Error>,
}

/// Parses and interprets borrowed source under the supplied bounds and include policy.
/// Returns an owned score or a diagnostic with all files read before the failure.
pub fn load_score(source: &str, name: &str, options: &mut Options<'_>) -> Result<Loaded, Failed> {
    if options.reserve.is_some_and(|reserve| !reserve(options.limits.memory_bytes)) {
        return Err(Failed {
            error: limits::limit("memory ledger", Span::default()),
            files: SourceFiles::default(),
        });
    }
    let syntax = parse_lilypond(source, name, options)?;
    interpret_syntax(&syntax, &options.limits)
}

#[cfg(test)]
mod tests;
