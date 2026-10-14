//! gd-odindoc reads Odin packages by static analysis and writes the reStructuredText of
//! Guidedog's `odin` domain that documents them, as sphinx.ext.autodoc writes the Python
//! domain's: packages, procedures, procedure groups, types and their fields or
//! enumerators, constants, variables, and foreign imports, each with its doc comment
//! turned into reST.
//!
//! It never runs the compiler: types are shown as written. A package's files are read
//! the way Odin builds them for every target at once: file-name suffixes (`_windows`,
//! `_linux_arm64`) and `#+build` tags choose the targets a file builds for, and
//! file-scope `when ODIN_OS == ...` blocks are evaluated when their conditions are that
//! simple. A declaration made for every target the package builds for is documented
//! once; one made for some targets only says which.
//!
//! The crate has its own Odin tokenizer and a declaration-level parser (procedure
//! bodies are skipped), with a nesting limit instead of Odin's unbounded recursion.
//!
//! ```no_run
//! use gd_odindoc::{Config, DirectiveOption, Project, Request, Root, document};
//!
//! let mut project = Project::from_dirs(vec![Root { dir: "lib".into(), ..Root::default() }]);
//! let request = Request {
//!     directive: "odin:autopackage".into(),
//!     argument: "gettext".into(),
//!     options: vec![DirectiveOption::flag("members")],
//!     ..Request::default()
//! };
//! let output = document(&mut project, &Config::default(), &request);
//! for line in &output.lines {
//!     println!("{line}");
//! }
//! ```

mod analyse;
mod ast;
mod comments;
mod emit;
mod parser;
mod platform;
mod project;
mod tags;
mod text;
mod tokenizer;
mod values;

pub use emit::document;
pub use platform::{Arch, Os, Targets};
pub use project::{
    FileRead, FileReader, FsReader, Limits, MemoryReader, PackageRef, Project, Root,
};

/// What a package-level declaration declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Kind {
    #[default]
    Proc,
    ProcGroup,
    Struct,
    Union,
    Enum,
    BitSet,
    BitField,
    Type,
    Const,
    Var,
    ForeignImport,
}

impl Kind {
    /// The odin domain's directive name for the kind ("procedure", "struct", ...).
    pub fn objtype(self) -> &'static str {
        match self {
            Kind::Proc => "procedure",
            Kind::ProcGroup => "procgroup",
            Kind::Struct => "struct",
            Kind::Union => "union",
            Kind::Enum => "enum",
            Kind::BitSet => "bitset",
            Kind::BitField => "bitfield",
            Kind::Type => "type",
            Kind::Const => "const",
            Kind::Var => "var",
            Kind::ForeignImport => "foreign",
        }
    }
}

/// How private a declaration is; the order is from least to most private.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub enum Private {
    #[default]
    Public,
    /// `@(private)`, or a `#+private` file.
    Package,
    /// `@(private="file")`, or a `#+private file` file.
    File,
}

/// What a member of a type is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum MemberKind {
    /// Of a struct: "[using ]name: T[ `tag`]".
    #[default]
    Field,
    /// Of an enum: "Name[ = value]".
    Enumerator,
    /// Of a bit_field: "name: T | bits".
    BitField,
}

/// A field or enumerator of a type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    pub kind: MemberKind,
    pub signature: String,
    /// The comment above it and the one after it on its line.
    pub docs: String,
}

/// A package-level declaration. `signature` is one line, without attributes, as the
/// odin domain's directive takes it ("clone :: proc(s: string) -> string").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Decl {
    pub name: String,
    pub kind: Kind,
    pub signature: String,
    /// Attributes as written, in order: "@(require_results)".
    pub attributes: Vec<String>,
    pub docs: String,
    pub private: Private,
    pub members: Vec<Member>,
    /// The library of a foreign block it is declared in.
    pub foreign_lib: String,
    pub targets: Targets,
    /// Its targets for people, when not all of the package's.
    pub availability: String,
    /// 0; 1, 2, ... for later declarations of the name that differ.
    pub variant: usize,
    /// The base name of its file.
    pub file: String,
    pub line: u32,
    /// Its position in the package: files by name, then position.
    pub order: usize,
}

/// A package another package imports: the name it is used by in the source (its alias)
/// and the id of the package it names.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Import {
    pub alias: String,
    pub id: String,
}

/// An analysed package. `doc` is its package comment (from doc.odin first, then the
/// first file by name that has one) with comment markers stripped; `synopsis` is that
/// comment's first sentence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub dir: String,
    pub doc: String,
    pub synopsis: String,
    pub decls: Vec<Decl>,
    pub imports: Vec<Import>,
    /// Every target some file of the package builds for.
    pub targets: Targets,
    /// The targets shown for people; "" when it builds for all.
    pub platform: String,
    /// The package's files that were read, sorted.
    pub files: Vec<String>,
    /// Syntax errors the parser reported.
    pub errors: usize,
    /// Files nested too deeply to parse.
    pub too_deep: Vec<String>,
    /// Files larger than `Limits::max_file_bytes`.
    pub too_large: Vec<String>,
}

/// The order `:members:` lists a package's declarations in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemberOrder {
    /// As declared: files by name, then position.
    #[default]
    Source,
    Alphabetical,
    /// Alphabetical within kinds: types, procedures, groups, constants, variables.
    Groupwise,
}

/// Project-wide settings (`odin_autoapi_options`, `odin_autoapi_member_order`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// Options a directive gets when it does not give them itself.
    pub default_options: Vec<DirectiveOption>,
    pub member_order: MemberOrder,
}

/// A directive option, `:name: value`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectiveOption {
    pub name: String,
    pub value: String,
}

impl DirectiveOption {
    /// An option with a value (`:members: a, b`).
    pub fn new(name: &str, value: &str) -> Self {
        DirectiveOption { name: name.to_string(), value: value.to_string() }
    }

    /// An option without a value (`:undoc-members:`).
    pub fn flag(name: &str) -> Self {
        DirectiveOption::new(name, "")
    }
}

/// One generating directive: "odin:autopackage", "odin:autoproc", "odin:autoprocgroup",
/// "odin:autotype", "odin:autoconst", or "odin:autovar", with its argument and options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Request {
    pub directive: String,
    pub argument: String,
    pub options: Vec<DirectiveOption>,
    /// The reader's current odin package, or "".
    pub current: String,
}

/// How serious a message is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub enum Severity {
    #[default]
    Note,
    Warning,
    Error,
}

/// A problem met while documenting, in Elm style: a code, a title, what happened, and
/// how to fix it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Message {
    pub severity: Severity,
    pub code: String,
    pub title: String,
    pub text: String,
    pub hint: String,
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "-- {} --\n\n{}\n\nHint: {}", self.title, self.text, self.hint)
    }
}

impl std::error::Error for Message {}

/// What a directive generates: reST lines, and messages about what went wrong.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
    pub lines: Vec<String>,
    pub messages: Vec<Message>,
}
