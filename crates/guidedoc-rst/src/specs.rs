//! The standard directives' specifications: argument, content, and option rules.

use guidedoc::AdmonitionKind;

/// Whether a directive takes an argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArgRule {
    /// No argument; words on the directive line are content.
    #[default]
    None,
    /// An argument may be given.
    Optional,
    /// An argument must be given.
    Required,
}

/// Whether a directive takes content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentRule {
    /// Content is refused.
    #[default]
    None,
    /// Content may be given.
    Optional,
    /// Content must be given.
    Required,
}

/// The type of an option's value, checked before the directive runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptType {
    /// No value.
    #[default]
    Flag,
    /// Any text, possibly empty.
    Text,
    /// Non-empty text.
    TextRequired,
    /// A signed integer.
    Int,
    /// A nonnegative integer.
    NonnegInt,
    /// A positive integer.
    PosInt,
    /// A nonnegative integer or nothing.
    OptInt,
    /// A CSS length (`12px`, `3em`, a bare number).
    Length,
    /// A length or a percentage.
    LengthPercent,
    /// A length, a percentage, or `image`.
    Figwidth,
    /// A percentage (with or without "%").
    Percentage,
    /// Class names.
    Class,
    /// A reference name (non-empty).
    Name,
    /// One of `OptSpec::choices`.
    Choice,
    /// An encoding name.
    Encoding,
    /// One character, `tab`, `space`, or a character code.
    Char,
    /// `auto`, `grid`, or positive integers.
    Widths,
    /// A path (non-empty).
    Path,
}

/// One option a directive accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OptSpec {
    /// The option name, compared exactly.
    pub name: &'static str,
    /// Its value type.
    pub kind: OptType,
    /// Space-separated choices, for `OptType::Choice`.
    pub choices: &'static str,
}

/// Shorthand for an option specification without choices.
pub const fn opt(name: &'static str, kind: OptType) -> OptSpec {
    OptSpec { name, kind, choices: "" }
}

/// Shorthand for a choice option.
pub const fn choice(name: &'static str, choices: &'static str) -> OptSpec {
    OptSpec { name, kind: OptType::Choice, choices }
}

/// What a standard directive does, dispatched by `run_directive`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DirKind {
    Admonition,
    GenericAdmonition,
    Image,
    Figure,
    Topic,
    Sidebar,
    LineBlock,
    ParsedLiteral,
    Code,
    Math,
    Rubric,
    Quote,
    Compound,
    Container,
    Table,
    CsvTable,
    ListTable,
    Contents,
    Sectnum,
    Header,
    Footer,
    TargetNotes,
    Meta,
    Include,
    Raw,
    Class,
    Role,
    DefaultRole,
    Title,
    Replace,
    Unicode,
    Date,
}

/// A standard directive's specification.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DirSpec {
    pub name: &'static str,
    pub kind: DirKind,
    pub arg: ArgRule,
    pub final_ws: bool,
    pub content: ContentRule,
    pub options: &'static [OptSpec],
    pub value: AdmonitionKind,
}

const fn spec(
    name: &'static str,
    kind: DirKind,
    arg: ArgRule,
    final_ws: bool,
    content: ContentRule,
    options: &'static [OptSpec],
) -> DirSpec {
    DirSpec { name, kind, arg, final_ws, content, options, value: AdmonitionKind::Generic }
}

const fn admonition(name: &'static str, value: AdmonitionKind) -> DirSpec {
    DirSpec {
        name,
        kind: DirKind::Admonition,
        arg: ArgRule::None,
        final_ws: false,
        content: ContentRule::Required,
        options: OPTS_CLASS_NAME,
        value,
    }
}

const ALIGN_IMAGE: &str = "top middle bottom left center right";
const ALIGN_BLOCK: &str = "left center right";

use ArgRule as A;
use ContentRule as C;
use DirKind as K;
use OptType as T;

pub(crate) const OPTS_CLASS_NAME: &[OptSpec] = &[opt("class", T::Class), opt("name", T::Name)];
const OPTS_NAME: &[OptSpec] = &[opt("name", T::Name)];
const OPTS_IMAGE: &[OptSpec] = &[
    opt("alt", T::Text),
    opt("height", T::Length),
    opt("width", T::LengthPercent),
    opt("scale", T::Percentage),
    choice("align", ALIGN_IMAGE),
    opt("target", T::TextRequired),
    opt("class", T::Class),
    opt("name", T::Name),
    choice("loading", "embed link lazy"),
];
const OPTS_FIGURE: &[OptSpec] = &[
    opt("alt", T::Text),
    opt("height", T::Length),
    opt("width", T::LengthPercent),
    opt("scale", T::Percentage),
    choice("align", ALIGN_BLOCK),
    opt("target", T::TextRequired),
    opt("class", T::Class),
    opt("name", T::Name),
    choice("loading", "embed link lazy"),
    opt("figwidth", T::Figwidth),
    opt("figclass", T::Class),
];
const OPTS_SIDEBAR: &[OptSpec] =
    &[opt("subtitle", T::TextRequired), opt("class", T::Class), opt("name", T::Name)];
const OPTS_CODE: &[OptSpec] =
    &[opt("class", T::Class), opt("name", T::Name), opt("number-lines", T::OptInt)];
const OPTS_TABLE: &[OptSpec] = &[
    opt("class", T::Class),
    opt("name", T::Name),
    choice("align", ALIGN_BLOCK),
    opt("width", T::LengthPercent),
    opt("widths", T::Widths),
];
const OPTS_CSV: &[OptSpec] = &[
    opt("header-rows", T::NonnegInt),
    opt("stub-columns", T::NonnegInt),
    opt("header", T::Text),
    opt("widths", T::Widths),
    opt("width", T::LengthPercent),
    opt("file", T::Path),
    opt("url", T::TextRequired),
    opt("encoding", T::Encoding),
    opt("delim", T::Char),
    opt("quote", T::Char),
    opt("keepspace", T::Flag),
    opt("escape", T::Char),
    choice("align", ALIGN_BLOCK),
    opt("class", T::Class),
    opt("name", T::Name),
];
const OPTS_LIST: &[OptSpec] = &[
    opt("header-rows", T::NonnegInt),
    opt("stub-columns", T::NonnegInt),
    opt("widths", T::Widths),
    opt("width", T::LengthPercent),
    choice("align", ALIGN_BLOCK),
    opt("class", T::Class),
    opt("name", T::Name),
];
const OPTS_CONTENTS: &[OptSpec] = &[
    opt("depth", T::NonnegInt),
    opt("local", T::Flag),
    choice("backlinks", "entry top none"),
    opt("class", T::Class),
];
const OPTS_SECTNUM: &[OptSpec] =
    &[opt("depth", T::Int), opt("prefix", T::Text), opt("suffix", T::Text), opt("start", T::Int)];
const OPTS_INCLUDE: &[OptSpec] = &[
    opt("literal", T::Flag),
    opt("code", T::Text),
    opt("number-lines", T::OptInt),
    opt("encoding", T::Encoding),
    opt("tab-width", T::Int),
    opt("start-line", T::Int),
    opt("end-line", T::Int),
    opt("start-after", T::TextRequired),
    opt("end-before", T::TextRequired),
    opt("parser", T::Text),
    opt("class", T::Class),
    opt("name", T::Name),
];
const OPTS_RAW: &[OptSpec] = &[
    opt("file", T::Path),
    opt("url", T::TextRequired),
    opt("encoding", T::Encoding),
    opt("class", T::Class),
];
/// The options of the role directive, read from its content.
pub(crate) const OPTS_ROLE: &[OptSpec] =
    &[opt("class", T::Class), opt("format", T::TextRequired), opt("language", T::TextRequired)];
const OPTS_UNICODE: &[OptSpec] =
    &[opt("ltrim", T::Flag), opt("rtrim", T::Flag), opt("trim", T::Flag)];

/// The standard directives, by name.
pub(crate) const DIRECTIVES: &[DirSpec] = &[
    admonition("attention", AdmonitionKind::Attention),
    admonition("caution", AdmonitionKind::Caution),
    admonition("danger", AdmonitionKind::Danger),
    admonition("error", AdmonitionKind::Error),
    admonition("hint", AdmonitionKind::Hint),
    admonition("important", AdmonitionKind::Important),
    admonition("note", AdmonitionKind::Note),
    admonition("tip", AdmonitionKind::Tip),
    admonition("warning", AdmonitionKind::Warning),
    admonition("seealso", AdmonitionKind::Seealso),
    spec("admonition", K::GenericAdmonition, A::Required, true, C::Required, OPTS_CLASS_NAME),
    spec("image", K::Image, A::Required, true, C::None, OPTS_IMAGE),
    spec("figure", K::Figure, A::Required, true, C::Optional, OPTS_FIGURE),
    spec("topic", K::Topic, A::Required, true, C::Required, OPTS_CLASS_NAME),
    spec("sidebar", K::Sidebar, A::Optional, true, C::Required, OPTS_SIDEBAR),
    spec("line-block", K::LineBlock, A::None, false, C::Required, OPTS_CLASS_NAME),
    spec("parsed-literal", K::ParsedLiteral, A::None, false, C::Required, OPTS_CLASS_NAME),
    spec("code", K::Code, A::Optional, false, C::Required, OPTS_CODE),
    spec("code-block", K::Code, A::Optional, false, C::Required, OPTS_CODE),
    spec("sourcecode", K::Code, A::Optional, false, C::Required, OPTS_CODE),
    spec("math", K::Math, A::Optional, true, C::Optional, OPTS_CLASS_NAME),
    spec("rubric", K::Rubric, A::Required, true, C::None, OPTS_CLASS_NAME),
    spec("epigraph", K::Quote, A::None, false, C::Required, &[]),
    spec("highlights", K::Quote, A::None, false, C::Required, &[]),
    spec("pull-quote", K::Quote, A::None, false, C::Required, &[]),
    spec("compound", K::Compound, A::None, false, C::Required, OPTS_CLASS_NAME),
    spec("container", K::Container, A::Optional, true, C::Required, OPTS_NAME),
    spec("table", K::Table, A::Optional, true, C::Required, OPTS_TABLE),
    spec("csv-table", K::CsvTable, A::Optional, true, C::Optional, OPTS_CSV),
    spec("list-table", K::ListTable, A::Optional, true, C::Required, OPTS_LIST),
    spec("contents", K::Contents, A::Optional, true, C::None, OPTS_CONTENTS),
    spec("sectnum", K::Sectnum, A::None, false, C::None, OPTS_SECTNUM),
    spec("section-numbering", K::Sectnum, A::None, false, C::None, OPTS_SECTNUM),
    spec("header", K::Header, A::None, false, C::Required, &[]),
    spec("footer", K::Footer, A::None, false, C::Required, &[]),
    spec("target-notes", K::TargetNotes, A::None, false, C::None, OPTS_CLASS_NAME),
    spec("meta", K::Meta, A::None, false, C::Required, &[]),
    spec("include", K::Include, A::Required, true, C::None, OPTS_INCLUDE),
    spec("raw", K::Raw, A::Required, true, C::Optional, OPTS_RAW),
    spec("class", K::Class, A::Required, true, C::Optional, &[]),
    spec("role", K::Role, A::None, false, C::Required, &[]),
    spec("default-role", K::DefaultRole, A::Optional, false, C::None, &[]),
    spec("title", K::Title, A::Required, true, C::None, &[]),
    spec("replace", K::Replace, A::None, false, C::Required, &[]),
    spec("unicode", K::Unicode, A::Required, true, C::None, OPTS_UNICODE),
    spec("date", K::Date, A::Optional, true, C::None, &[]),
];

/// Compares ASCII case-insensitively, as directive and role names are compared.
pub fn equal_fold(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Returns the standard directive called `name`.
pub(crate) fn find_directive(name: &str) -> Option<&'static DirSpec> {
    DIRECTIVES.iter().find(|s| equal_fold(s.name, name))
}
