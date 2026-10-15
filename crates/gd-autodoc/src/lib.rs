//! Generates the reStructuredText that Sphinx 9.1's `sphinx.ext.autodoc` generates, from
//! Python source that [`gd_pyscan`] analyses instead of imported modules: the same
//! `py:` directives (`.. py:class:: Name(signature)` with `:module:`, `:canonical:`,
//! `Bases: ...`), docstrings prepared as Sphinx prepares them, and members chosen,
//! filtered, and ordered as autodoc's options say. What static analysis cannot know
//! (compiled modules, objects made at run time, members of classes outside the source
//! roots) is reported as a [`Message`] naming the object, never guessed silently.
//!
//! ```no_run
//! use gd_autodoc::{Config, DirectiveOption, MemberOrder, Request, Session, document};
//!
//! let project = gd_pyscan::Project::new(&["src"]).expect("the Python grammar loads");
//! let mut session = Session::new(project);
//! let config = Config { autodoc_member_order: MemberOrder::Bysource, ..Config::default() };
//! let request = Request {
//!     directive: "autoclass".into(),
//!     argument: "mypackage.core.Parser".into(),
//!     options: vec![DirectiveOption::flag("members")],
//!     ..Request::default()
//! };
//! let output = document(&mut session, &config, &request);
//! for m in &output.messages {
//!     eprintln!("{m}");
//! }
//! println!("{}", output.lines.join("\n"));
//! ```

#![forbid(unsafe_code)]

mod autosummary;
mod config;
mod dataclasses;
mod docstrings;
mod emit;
mod hints;
mod loader;
mod members;
mod names;
mod napoleon;
mod options;
mod process;
mod props;
mod signatures;
mod typehints;
mod types;
mod typevars;
mod values;

#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::fmt;

use gd_pyscan::Project;

pub use autosummary::{
    Stub, StubOptions, Summary, SummaryItem, SummaryRequest, SummarySignatures, module_docstring,
    stub, summarize, summary_table,
};
pub use config::{ClassContent, ClassSignature, Config, DefaultOption, DescriptionTarget};
pub use config::{MemberOrder, TypeAlias, Typehints, TypehintsFormat};
pub use napoleon::{
    DocstringStyle, Napoleon, NapoleonContext, NapoleonNote, NapoleonOutput, NapoleonSection,
    convert_docstring, includes_member, napoleon_docstring, unknown_custom_sections,
};
pub use options::ALL_OPTIONS;

/// A directive option as written; `value` is `""` for a flag.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirectiveOption {
    /// The option's name, such as `"members"`.
    pub name: String,
    /// Its value, or `""`.
    pub value: String,
}

impl DirectiveOption {
    /// An option given without a value.
    pub fn flag(name: &str) -> DirectiveOption {
        DirectiveOption { name: name.into(), value: String::new() }
    }

    /// An option given with a value.
    pub fn new(name: &str, value: &str) -> DirectiveOption {
        DirectiveOption { name: name.into(), value: value.into() }
    }
}

/// One autodoc directive: its name (`autoclass`, `automodule`, ...), argument, options,
/// content, and the reader's context (the current `py:module` and `py:class`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Request {
    /// The directive, with or without `py:`.
    pub directive: String,
    /// What to document, such as `package.module.Class`.
    pub argument: String,
    /// The options as written.
    pub options: Vec<DirectiveOption>,
    /// The directive's content, added after the docstring.
    pub content: Vec<String>,
    /// The current module of the document, or `""`.
    pub module: String,
    /// The current class of the document, or `""`.
    pub class: String,
}

/// How serious a [`Message`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Severity {
    /// Something static analysis could not know; the output is still useful.
    #[default]
    Note,
    /// Something was left out.
    Warning,
    /// The directive could not be run.
    Error,
}

/// A problem to report where the directive stands, Elm-style: a stable code, a title,
/// a plain explanation, and a hint saying how to fix it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Message {
    /// How serious it is.
    pub severity: Severity,
    /// A stable code such as `"autodoc.import"`.
    pub code: String,
    /// A short upper-case title.
    pub title: String,
    /// What happened.
    pub text: String,
    /// How to fix it.
    pub hint: String,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rule = "-".repeat(80usize.saturating_sub(self.title.len() + 4));
        write!(f, "-- {} {rule}\n\n{}\n\nHint: {}", self.title, self.text, self.hint)
    }
}

impl std::error::Error for Message {}

/// The generated reST of one directive (lines without line breaks; empty when there is
/// nothing to add) and the messages about it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Output {
    /// The reST lines.
    pub lines: Vec<String>,
    /// What to report about them.
    pub messages: Vec<Message>,
}

impl Output {
    /// Whether a message with `code` was reported.
    pub fn has_message(&self, code: &str) -> bool {
        self.messages.iter().any(|m| m.code == code)
    }
}

/// autodoc's state across the directives of one build: the analysed project, and the
/// modules whose analyser annotations autodoc has merged into their classes (as
/// `_ensure_annotations_from_type_comments` does the first time it documents a data or
/// attribute from them), which changes the members found afterwards.
#[derive(Debug)]
pub struct Session {
    project: Project,
    merged: HashSet<String>,
}

impl Session {
    /// A session over a project, which it owns for the session's life.
    pub fn new(project: Project) -> Session {
        Session { project, merged: HashSet::new() }
    }

    /// The analysed project: its modules, and the files read (a build's dependencies).
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// Ends the session, giving the project back.
    pub fn into_project(self) -> Project {
        self.project
    }
}

/// What one autodoc directive documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObjType {
    Module,
    Class,
    Exception,
    Function,
    Decorator,
    Method,
    Property,
    Attribute,
    Data,
    Type,
}

const OBJ_TYPES: [(ObjType, &str); 10] = [
    (ObjType::Module, "module"),
    (ObjType::Class, "class"),
    (ObjType::Exception, "exception"),
    (ObjType::Function, "function"),
    (ObjType::Decorator, "decorator"),
    (ObjType::Method, "method"),
    (ObjType::Property, "property"),
    (ObjType::Attribute, "attribute"),
    (ObjType::Data, "data"),
    (ObjType::Type, "type"),
];

impl ObjType {
    /// The name Sphinx gives the type: `class`, `method`, ...
    pub(crate) fn name(self) -> &'static str {
        OBJ_TYPES.iter().find(|(t, _)| *t == self).map_or("", |(_, name)| name)
    }

    /// The object type a directive name (`autoclass`, `py:autoclass`) documents.
    fn of_directive(directive: &str) -> Option<ObjType> {
        let name = directive.strip_prefix("py:").unwrap_or(directive);
        let name = name.strip_prefix("auto")?;
        OBJ_TYPES.iter().find(|(_, n)| *n == name).map(|(t, _)| *t)
    }

    /// Classes and exceptions, which share most of autodoc's class handling.
    pub(crate) fn is_class(self) -> bool {
        matches!(self, ObjType::Class | ObjType::Exception)
    }
}

/// Bounds the directives one autodoc directive generates. Members nest at most 24 deep,
/// but a class whose nested classes bring in the class itself under its own name
/// documents it again at every level: the output grows exponentially.
pub(crate) const MAX_GENERATED: usize = 10_000;

/// One directive's generation.
pub(crate) struct Generator<'a> {
    pub(crate) project: &'a mut Project,
    pub(crate) merged: &'a mut HashSet<String>,
    pub(crate) config: &'a Config,
    pub(crate) options: options::Options,
    pub(crate) request: &'a Request,
    pub(crate) out: Output,
    /// The module and class members are documented in (`autodoc_module` and
    /// `autodoc_class` of Sphinx's current document), set while members are loaded.
    pub(crate) autodoc_module: String,
    pub(crate) autodoc_class: String,
    pub(crate) depth: usize,
    /// Directives generated so far (`MAX_GENERATED`).
    pub(crate) generated: usize,
}

impl Generator<'_> {
    /// Adds a message about the directive, once.
    pub(crate) fn report(
        &mut self,
        severity: Severity,
        code: &str,
        title: &str,
        text: String,
        hint: &str,
    ) {
        if self.out.messages.iter().any(|m| m.code == code && m.text == text) {
            return;
        }
        self.out.messages.push(Message {
            severity,
            code: code.into(),
            title: title.into(),
            text,
            hint: hint.into(),
        });
    }
}

/// Generates the reST of one autodoc directive, as `AutodocDirective.run` does before
/// parsing it: the lines, and messages about them. An unknown directive or a bad option
/// is an [`Severity::Error`] and no lines; what analysis cannot know is a note naming
/// the object. Modules are analysed through the session's project as needed.
pub fn document(session: &mut Session, config: &Config, request: &Request) -> Output {
    let mut g = Generator {
        project: &mut session.project,
        merged: &mut session.merged,
        config,
        options: options::Options::default(),
        request,
        out: Output::default(),
        autodoc_module: String::new(),
        autodoc_class: String::new(),
        depth: 0,
        generated: 0,
    };
    let Some(obj_type) = ObjType::of_directive(&request.directive) else {
        g.report(
            Severity::Error,
            "autodoc.directive",
            "UNKNOWN AUTODOC DIRECTIVE",
            format!("{} is not one of autodoc's directives.", request.directive),
            "Use one of the autodoc directives: automodule, autoclass, autoexception, \
             autofunction, automethod, autoattribute, autodata, autoproperty, autodecorator, \
             or autotype.",
        );
        return g.out;
    };
    let Some(options) = options::process_options(&mut g, obj_type, &request.options) else {
        return g.out;
    };
    g.options = options;
    let Some(mut props) = props::load_object(&mut g, &request.argument, obj_type, "", false)
    else {
        return g.out;
    };
    emit::generate_directives(&mut g, &mut props, &request.content, "", false, false, "");
    g.out
}
