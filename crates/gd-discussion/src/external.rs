//! Building with an installed `guidedog` program, so `dsc` stays small: it manages
//! catalogs itself and asks Guidedog only to build them. Guidedog's report goes
//! straight to the terminal.

use crate::build::{Book, BuildRequest, Builder};
use crate::problem::shown;
use crate::{Problem, Result};
use std::ffi::OsString;
use std::fmt::Write;
use std::path::PathBuf;
use std::process::Command;

/// The environment variable naming the `guidedog` program when the command line does not.
pub const ENGINE_VARIABLE: &str = "DSC_GUIDEDOG";

/// An installed `guidedog` as a [`Builder`]: `program`, else `$DSC_GUIDEDOG`, else
/// `guidedog` on `PATH`. It runs the program directly, never through a shell, and passes
/// titles and names as quoted data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuidedogProgram {
    /// The program to run, as `--guidedog` names it; `None` to look further.
    pub program: Option<PathBuf>,
}

impl GuidedogProgram {
    /// The program this builder runs.
    pub fn program(&self) -> PathBuf {
        if let Some(program) = &self.program {
            return program.clone();
        }
        match std::env::var_os(ENGINE_VARIABLE) {
            Some(value) if !value.is_empty() => PathBuf::from(value),
            _ => PathBuf::from("guidedog"),
        }
    }

    /// The arguments after the program that make `r`: a build of the catalog, warnings as
    /// errors, with its books as the `pdf_documents` setting.
    pub fn arguments(&self, r: &BuildRequest) -> Vec<OsString> {
        let mut args: Vec<OsString> = Vec::with_capacity(9);
        args.push("build".into());
        match &r.selection {
            Some(selection) => {
                args.extend(["-b".into(), r.target.name().into(), r.dir.clone().into()]);
                args.push(selected_output(r, selection).into());
            }
            None => args.extend([r.target.name().into(), r.dir.clone().into()]),
        }
        args.push("-W".into());
        if !r.books.is_empty() {
            args.extend(["-D".into(), books_setting(&r.books).into()]);
        }
        args
    }
}

impl Builder for GuidedogProgram {
    fn name(&self) -> &str {
        "external"
    }

    fn build(&self, r: &BuildRequest) -> Result<PathBuf> {
        let program = self.program();
        let shown_program = shown(&program);
        let status = Command::new(&program).args(self.arguments(r)).status().map_err(|e| {
            Problem::new(r.prefix.code("engine-missing"), "Guidedog was not found",
                &shown_program, format!("Cannot start {shown_program}: {e}."),
                "Install Guidedog and put guidedog on PATH, pass --guidedog=PATH, or use the \
                 complete program, dscc, which has Guidedog built in.")
            .exit(4)
        })?;
        if !status.success() {
            let code = status.code().filter(|&c| c != 0).unwrap_or(4);
            return Err(Problem::new(r.prefix.code("render"),
                "Guidedog could not build the catalog", shown(&r.dir),
                format!("{shown_program} stopped with status {code}; its report is above."),
                "Fix the problems Guidedog reported above, then build again.")
            .exit(code));
        }
        Ok(match &r.selection {
            Some(selection) => selected_output(r, selection),
            None => r.dir.join("_build").join(r.target.name()),
        })
    }
}

/// Where a selected record's preview goes: `_build/selected/SELECTION/pdf`.
fn selected_output(r: &BuildRequest, selection: &str) -> PathBuf {
    r.dir.join("_build").join("selected").join(selection).join(r.target.name())
}

/// The `pdf_documents` setting of `books`, as TOML for `-D`. Every value is a quoted
/// string, so titles and names are data, never settings.
pub(crate) fn books_setting(books: &[Book]) -> String {
    let mut out = String::from("pdf_documents=[");
    for (i, book) in books.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let _ = write!(out, "{{root={}, file={}, title={}, author={}}}", toml(&book.root),
            toml(&book.file), toml(&book.title), toml(&book.author));
    }
    out.push(']');
    out
}

/// `value` as a one-line TOML basic string.
fn toml(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 32 || c == '\u{7f}' => _ = write!(out, "\\u{:04x}", c as u32),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Prefix, Target};

    /// dsc asks guidedog to build, and nothing else: Typst, Graphviz, and the rest are
    /// Guidedog's to find. Titles and names reach guidedog as quoted data.
    #[test]
    fn the_command_line_names_only_guidedog_and_quotes_its_data() {
        let books = vec![Book {
            root: "records/0001-cache".into(),
            file: "eds-0001-cache.pdf".into(),
            title: "Quotes \" and \\ slash".into(),
            author: "René, Ada".into(),
        }];
        let mut r = BuildRequest {
            dir: PathBuf::from("docs/eds"),
            prefix: Prefix::new("eds").unwrap_or_default(),
            target: Target::Pdf,
            books,
            selection: None,
        };
        let builder = GuidedogProgram { program: Some("/opt/guidedog".into()) };
        assert_eq!(builder.program(), PathBuf::from("/opt/guidedog"));
        let args = builder.arguments(&r);
        let text: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(text[..4].join(" "), "build pdf docs/eds -W");
        assert_eq!(text[4], "-D");
        assert!(text[5].contains(r#"title="Quotes \" and \\ slash""#), "{}", text[5]);
        assert!(text.iter().all(|a| !a.contains("typst") && !a.contains("dot")));
        r.selection = Some("eds-0001-cache".into());
        let text: Vec<String> =
            builder.arguments(&r).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(text[1], "-b");
        let selected = PathBuf::from("docs/eds/_build/selected/eds-0001-cache/pdf");
        assert_eq!(PathBuf::from(&text[4]), selected);
    }
}
