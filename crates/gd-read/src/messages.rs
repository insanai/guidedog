//! The messages the autodoc and odindoc hosts report at a directive. The engine reports
//! static `guidedoc::Message`s; gd-autodoc and gd-odindoc build theirs at run time, so
//! each is reported through a message with its code and title from the tables below and
//! its text and hint as arguments (`{0}`, `{1}`), which are never read as a template.

use guidedoc::{Message, Severity, Status, arg_text};
use guidedoc_sphinx::AutodocMessage;

/// Every code gd-autodoc and gd-odindoc report (a test checks the list against their
/// sources).
const CODES: &[&str] = &[
    "autodoc.all",
    "autodoc.decorator",
    "autodoc.default",
    "autodoc.directive",
    "autodoc.docstring",
    "autodoc.import",
    "autodoc.kind",
    "autodoc.limit",
    "autodoc.member",
    "autodoc.members",
    "autodoc.module",
    "autodoc.native",
    "autodoc.option",
    "autodoc.outside",
    "autodoc.signature",
    "autodoc.too_deep",
    "autodoc.value",
    "autosummary.import",
    "autosummary.import_cycle",
    "autosummary.option",
    "autosummary.toctree",
    "napoleon.field",
    "napoleon.section",
    "napoleon.type",
    "odin.autodoc.directive",
    "odin.autodoc.kind",
    "odin.autodoc.memory",
    "odin.autodoc.nesting",
    "odin.autodoc.object",
    "odin.autodoc.option",
    "odin.autodoc.order",
    "odin.autodoc.package",
    "odin.autodoc.syntax",
];

/// Every title they report with (checked as `CODES` is).
const TITLES: &[&str] = &[
    "__ALL__ IS COMPUTED",
    "AUTODOC OPTION NOT ACCEPTED",
    "AUTOMODULE TAKES NO SIGNATURE",
    "CAPTION WITHOUT TOCTREE",
    "COMPILED MODULE CANNOT BE READ",
    "CONSTRUCTOR OUTSIDE THE SOURCE PATHS",
    "DECORATOR OUTSIDE THE SOURCE PATHS",
    "DEFAULT KNOWN ONLY AT RUN TIME",
    "DEFAULT VALUE NOT PRESERVED",
    "DOCSTRING KNOWN ONLY AT RUN TIME",
    "DOCSTRING MAY BE INHERITED",
    "FIELD NOT READ",
    "FUNCTION MADE AT RUN TIME",
    "INVALID AUTODOC SIGNATURE",
    "MALFORMED TYPE",
    "MEMBER NOT FOUND",
    "MEMBERS OUTSIDE THE SOURCE PATHS",
    "NO MODULE FOR AUTODOC",
    "NOT A CLASS",
    "NOT A FUNCTION",
    "OBJECT OUTSIDE THE SOURCE PATHS",
    "ODIN DECLARATION NOT FOUND",
    "ODIN DECLARATION OF ANOTHER KIND",
    "ODIN PACKAGE NOT FOUND",
    "ODIN SOURCE DOES NOT PARSE",
    "ODIN SOURCE NESTS TOO DEEPLY",
    "ODIN SOURCE TOO LARGE",
    "PYTHON MODULE NOT FOUND",
    "PYTHON OBJECT NOT FOUND",
    "SECTION READ AS TEXT",
    "SOURCE NESTS TOO DEEPLY",
    "SUMMARY ENTRY NOT FOUND",
    "SUMMARY ENTRY REPEATS ITS MODULE",
    "TOO MANY MEMBERS",
    "TYPE OUTSIDE THE SOURCE PATHS",
    "UNKNOWN AUTODOC DIRECTIVE",
    "UNKNOWN MEMBER ORDER",
    "UNKNOWN SIGNATURE STYLE",
    "UNKNOWN ODIN AUTODOC DIRECTIVE",
    "UNKNOWN ODIN AUTODOC OPTION",
    "VALUE KNOWN ONLY AT RUN TIME",
];

/// sphinx.ext.autodoc without `autodoc_source_paths`.
pub(crate) const NO_AUTODOC_PATHS: Message = Message {
    code: "autodoc.paths",
    severity: Severity::Warning,
    status: Status::Ok,
    title: "NO AUTODOC SOURCE PATHS",
    message: "autodoc reads Python source from autodoc_source_paths, and conf.toml sets none.",
    hint: "Add the folder holding your package, relative to the source folder, such as \
           autodoc_source_paths = [\"../src\"].",
};

/// An odin generating directive without `odin_autoapi_dirs` or `odin_collections`.
pub(crate) const NO_ODIN_ROOTS: Message = Message {
    code: "odin.autodoc.paths",
    severity: Severity::Warning,
    status: Status::Ok,
    title: "NO ODIN SOURCE FOLDERS",
    message: "The odin domain's generating directives read Odin source from \
              odin_autoapi_dirs and odin_collections, and conf.toml sets neither.",
    hint: "Add the folder holding your packages, relative to the source folder, such as \
           odin_autoapi_dirs = [\"../src\"].",
};

/// The Python grammar could not be loaded (a broken build of Guidedog).
pub(crate) const PYTHON_GRAMMAR: Message = Message {
    code: "autodoc.grammar",
    severity: Severity::Error,
    status: Status::Internal,
    title: "PYTHON GRAMMAR NOT LOADED",
    message: "The Python parser could not start: {0}",
    hint: "Report this to the Guidedog developers with the output of guidedog --version.",
};

/// A host message: its code and title, how serious it is, its text and hint.
pub(crate) fn host_message(
    severity: Severity,
    code: &str,
    title: &str,
    text: &str,
    hint: &str,
) -> AutodocMessage {
    let odin = code.starts_with("odin.");
    let code = CODES.iter().find(|c| **c == code).copied();
    let title = TITLES.iter().find(|t| **t == title).copied();
    let message = Message {
        code: code.unwrap_or(if odin { "odin.autodoc" } else { "autodoc" }),
        severity,
        status: if severity == Severity::Error { Status::InvalidInput } else { Status::Ok },
        title: title.unwrap_or(if odin { "ODIN AUTODOC PROBLEM" } else { "AUTODOC PROBLEM" }),
        message: "{0}",
        hint: "{1}",
    };
    AutodocMessage { message, args: [arg_text(text), arg_text(hint)] }
}

/// A static message without arguments, or with the one given.
pub(crate) fn static_message(message: Message, arg: Option<&str>) -> AutodocMessage {
    let first = arg.map_or(guidedoc::Arg::None, arg_text);
    AutodocMessage { message, args: [first, guidedoc::Arg::None] }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every quoted code and upper-case title in a crate's sources.
    fn quoted_in(dir: &str, prefix: &str) -> (Vec<String>, Vec<String>) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(dir);
        let (mut codes, mut titles) = (Vec::new(), Vec::new());
        let mut dirs = vec![root.join("src")];
        let mut files = Vec::new();
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() { dirs.push(path) } else { files.push(path) }
            }
        }
        for path in files {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            for piece in text.split('"').skip(1).step_by(2) {
                if piece.starts_with(prefix)
                    && piece[prefix.len()..].chars().all(|c| c.is_ascii_lowercase() || c == '_')
                    && piece.len() > prefix.len()
                {
                    codes.push(piece.to_owned());
                }
                let upper = piece.chars().all(|c| c.is_ascii_uppercase() || " ,'_-".contains(c));
                if upper && piece.contains(' ') && piece.len() > 6 {
                    titles.push(piece.to_owned());
                }
            }
        }
        (codes, titles)
    }

    #[test]
    fn every_code_and_title_of_the_generators_is_known() {
        let generators = [
            ("gd-autodoc", "autodoc."),
            ("gd-autodoc", "napoleon."),
            ("gd-autodoc", "autosummary."),
            ("gd-odindoc", "odin.autodoc."),
        ];
        for (dir, prefix) in generators {
            let (codes, titles) = quoted_in(dir, prefix);
            assert!(!codes.is_empty(), "{dir}: no codes found");
            for code in codes {
                assert!(CODES.contains(&code.as_str()), "{dir}: unknown code {code}");
            }
            for title in titles {
                assert!(TITLES.contains(&title.as_str()), "{dir}: unknown title {title}");
            }
        }
    }

    #[test]
    fn host_messages_keep_their_code_and_title() {
        let m =
            host_message(Severity::Warning, "autodoc.import", "PYTHON OBJECT NOT FOUND", "t", "h");
        assert_eq!(
            (m.message.code, m.message.title),
            ("autodoc.import", "PYTHON OBJECT NOT FOUND")
        );
        let m = host_message(Severity::Error, "odin.autodoc.new", "SOMETHING NEW", "t", "h");
        assert_eq!((m.message.code, m.message.title), ("odin.autodoc", "ODIN AUTODOC PROBLEM"));
        assert_eq!(m.message.status, Status::InvalidInput);
    }
}
