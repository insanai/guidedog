//! The Sphinx layer's own directives (those without a domain): their table, dispatch,
//! and the helpers every handler shares.

mod body;

use guidedoc::{AttributeKey, Fail, Message, NodeId};
use guidedoc_rst::{ArgRule, ContentRule, Directive, OptSpec, OptType, Parser, choice, opt};

use crate::autodoc::{
    AUTODOC_OPTIONS, INHERITANCE_DIAGRAM, autodoc_directive, provided_directive,
};
use crate::extension::Sphinx;
use crate::messages::INDEX_ENTRY;
use crate::text::{equal_fold, parse_int, trim_space};
use crate::{code, doctest, graphviz, literalinclude, math};

/// What a core directive does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Core {
    Toctree,
    Only,
    Index,
    Centered,
    Hlist,
    Rubric,
    Version,
    Author,
    Ignored,
    Highlight,
    Code,
    Literalinclude,
    Math,
    DefaultDomain,
    DefaultRole,
    Todo,
    Todolist,
    Postlist,
    More,
    Acks,
    RstClass,
    Graphviz,
    Doctest,
    Autodoc,
    Inheritance,
}

/// One core directive and its rules.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CoreDirective {
    pub name: &'static str,
    pub kind: Core,
    pub arg: ArgRule,
    pub final_ws: bool,
    pub content: ContentRule,
    pub options: &'static [OptSpec],
}

const fn core(
    name: &'static str,
    kind: Core,
    rules: (ArgRule, bool, ContentRule),
    options: &'static [OptSpec],
) -> CoreDirective {
    CoreDirective { name, kind, arg: rules.0, final_ws: rules.1, content: rules.2, options }
}

use ArgRule as A;
use ContentRule as C;

const OPTS_NAME: &[OptSpec] = &[opt("name", OptType::Text)];
const OPTS_POSTLIST: &[OptSpec] = &[
    opt("path", OptType::Text),
    opt("category", OptType::Text),
    opt("tags", OptType::Text),
    opt("author", OptType::Text),
    opt("series", OptType::Text),
    opt("year", OptType::Text),
    choice("sort", "date -date"),
    choice("style", "list cards"),
    opt("excerpts", OptType::Flag),
];
const OPTS_CLASS_NAME: &[OptSpec] = &[opt("class", OptType::Class), opt("name", OptType::Name)];
const OPTS_TOCTREE: &[OptSpec] = &[
    opt("maxdepth", OptType::Int),
    opt("name", OptType::Name),
    opt("class", OptType::Class),
    opt("caption", OptType::TextRequired),
    opt("glob", OptType::Flag),
    opt("hidden", OptType::Flag),
    opt("includehidden", OptType::Flag),
    opt("numbered", OptType::OptInt),
    opt("titlesonly", OptType::Flag),
    opt("reversed", OptType::Flag),
];
const OPTS_HLIST: &[OptSpec] = &[opt("columns", OptType::PosInt)];
const OPTS_RUBRIC: &[OptSpec] = &[
    opt("class", OptType::Class),
    opt("name", OptType::Name),
    choice("heading-level", "1 2 3 4 5 6"),
];
const OPTS_HIGHLIGHT: &[OptSpec] =
    &[opt("force", OptType::Flag), opt("linenothreshold", OptType::PosInt)];
const OPTS_CODE: &[OptSpec] = &[
    opt("force", OptType::Flag),
    opt("linenos", OptType::Flag),
    opt("dedent", OptType::OptInt),
    opt("lineno-start", OptType::Int),
    opt("emphasize-lines", OptType::TextRequired),
    opt("caption", OptType::TextRequired),
    opt("class", OptType::Class),
    opt("name", OptType::Text),
];
const OPTS_CODE_DOCUTILS: &[OptSpec] = &[
    opt("force", OptType::Flag),
    opt("linenos", OptType::Flag),
    opt("dedent", OptType::OptInt),
    opt("lineno-start", OptType::Int),
    opt("emphasize-lines", OptType::TextRequired),
    opt("caption", OptType::TextRequired),
    opt("class", OptType::Class),
    opt("name", OptType::Text),
    opt("number-lines", OptType::OptInt),
];
const OPTS_LITERALINCLUDE: &[OptSpec] = &[
    opt("dedent", OptType::OptInt),
    opt("linenos", OptType::Flag),
    opt("lineno-start", OptType::Int),
    opt("lineno-match", OptType::Flag),
    opt("tab-width", OptType::Int),
    opt("language", OptType::TextRequired),
    opt("force", OptType::Flag),
    opt("encoding", OptType::Encoding),
    opt("pyobject", OptType::TextRequired),
    opt("lines", OptType::TextRequired),
    opt("start-after", OptType::TextRequired),
    opt("end-before", OptType::TextRequired),
    opt("start-at", OptType::TextRequired),
    opt("end-at", OptType::TextRequired),
    opt("prepend", OptType::TextRequired),
    opt("append", OptType::TextRequired),
    opt("emphasize-lines", OptType::TextRequired),
    opt("caption", OptType::Text),
    opt("class", OptType::Class),
    opt("name", OptType::Text),
    opt("diff", OptType::TextRequired),
];
/// sphinx.ext.inheritance_diagram's options; the provider checks their values.
const OPTS_INHERITANCE: &[OptSpec] = &[
    opt("parts", OptType::Int),
    opt("private-bases", OptType::Flag),
    opt("caption", OptType::Text),
    opt("top-classes", OptType::TextRequired),
    opt("include-subclasses", OptType::Flag),
];
/// sphinx.ext.autosummary's options; the provider checks their values.
const OPTS_AUTOSUMMARY: &[OptSpec] = &[
    opt("caption", OptType::TextRequired),
    opt("class", OptType::Class),
    opt("toctree", OptType::Text),
    opt("nosignatures", OptType::Flag),
    opt("recursive", OptType::Flag),
    opt("signatures", OptType::Text),
    opt("template", OptType::Text),
];
const OPTS_MATH: &[OptSpec] = &[
    opt("label", OptType::Text),
    opt("name", OptType::Text),
    opt("class", OptType::Class),
    opt("nowrap", OptType::Flag),
    opt("no-wrap", OptType::Flag),
];

/// The core directives. Names with a domain live in the domains' tables.
static CORE: &[CoreDirective] = &[
    core("toctree", Core::Toctree, (A::None, false, C::Optional), OPTS_TOCTREE),
    core("only", Core::Only, (A::Required, true, C::Optional), &[]),
    core("ifconfig", Core::Only, (A::Required, true, C::Required), &[]),
    core("index", Core::Index, (A::Required, true, C::None), OPTS_NAME),
    core("centered", Core::Centered, (A::Required, true, C::None), &[]),
    core("hlist", Core::Hlist, (A::None, false, C::Required), OPTS_HLIST),
    core("rubric", Core::Rubric, (A::Required, true, C::None), OPTS_RUBRIC),
    core("versionadded", Core::Version, (A::Required, true, C::Optional), &[]),
    core("versionchanged", Core::Version, (A::Required, true, C::Optional), &[]),
    core("deprecated", Core::Version, (A::Required, true, C::Optional), &[]),
    core("versionremoved", Core::Version, (A::Required, true, C::Optional), &[]),
    core("sectionauthor", Core::Author, (A::Required, true, C::None), &[]),
    core("moduleauthor", Core::Author, (A::Required, true, C::None), &[]),
    core("codeauthor", Core::Author, (A::Required, true, C::None), &[]),
    core("tabularcolumns", Core::Ignored, (A::Required, true, C::None), &[]),
    core("highlight", Core::Highlight, (A::Required, false, C::None), OPTS_HIGHLIGHT),
    core("code-block", Core::Code, (A::Optional, false, C::Optional), OPTS_CODE),
    core("sourcecode", Core::Code, (A::Optional, false, C::Optional), OPTS_CODE),
    core("code", Core::Code, (A::Optional, false, C::Required), OPTS_CODE_DOCUTILS),
    core(
        "literalinclude",
        Core::Literalinclude,
        (A::Required, true, C::None),
        OPTS_LITERALINCLUDE,
    ),
    core("math", Core::Math, (A::Optional, true, C::Optional), OPTS_MATH),
    core("default-domain", Core::DefaultDomain, (A::Required, false, C::None), &[]),
    core("default-role", Core::DefaultRole, (A::Optional, false, C::None), &[]),
    core("todo", Core::Todo, (A::None, false, C::Required), OPTS_CLASS_NAME),
    core("todolist", Core::Todolist, (A::None, false, C::None), &[]),
    core("postlist", Core::Postlist, (A::Optional, false, C::None), OPTS_POSTLIST),
    core("more", Core::More, (A::None, false, C::None), &[]),
    core("acks", Core::Acks, (A::None, false, C::Required), &[]),
    core("rst-class", Core::RstClass, (A::Required, true, C::Optional), &[]),
    core("graphviz", Core::Graphviz, (A::Optional, true, C::Optional), graphviz::OPTS_GRAPHVIZ),
    core("graph", Core::Graphviz, (A::Required, false, C::Required), graphviz::OPTS_GRAPHVIZ),
    core("digraph", Core::Graphviz, (A::Required, false, C::Required), graphviz::OPTS_GRAPHVIZ),
    core("testsetup", Core::Doctest, (A::Optional, true, C::Optional), doctest::OPTS_DOCTEST),
    core("testcleanup", Core::Doctest, (A::Optional, true, C::Optional), doctest::OPTS_DOCTEST),
    core("doctest", Core::Doctest, (A::Optional, true, C::Optional), doctest::OPTS_DOCTEST),
    core("testcode", Core::Doctest, (A::Optional, true, C::Optional), doctest::OPTS_DOCTEST),
    core("testoutput", Core::Doctest, (A::Optional, true, C::Optional), doctest::OPTS_DOCTEST),
    core("automodule", Core::Autodoc, (A::Required, true, C::Optional), AUTODOC_OPTIONS),
    core("autoclass", Core::Autodoc, (A::Required, true, C::Optional), AUTODOC_OPTIONS),
    core("autoexception", Core::Autodoc, (A::Required, true, C::Optional), AUTODOC_OPTIONS),
    core("autofunction", Core::Autodoc, (A::Required, true, C::Optional), AUTODOC_OPTIONS),
    core("autodecorator", Core::Autodoc, (A::Required, true, C::Optional), AUTODOC_OPTIONS),
    core("automethod", Core::Autodoc, (A::Required, true, C::Optional), AUTODOC_OPTIONS),
    core("autoproperty", Core::Autodoc, (A::Required, true, C::Optional), AUTODOC_OPTIONS),
    core("autoattribute", Core::Autodoc, (A::Required, true, C::Optional), AUTODOC_OPTIONS),
    core("autodata", Core::Autodoc, (A::Required, true, C::Optional), AUTODOC_OPTIONS),
    core("autotype", Core::Autodoc, (A::Required, true, C::Optional), AUTODOC_OPTIONS),
    core("autosummary", Core::Autodoc, (A::None, false, C::Optional), OPTS_AUTOSUMMARY),
    core("inheritance-diagram", Core::Inheritance, (A::Required, true, C::None), OPTS_INHERITANCE),
];

/// Finds a core directive by name, case-insensitively.
pub(crate) fn find_core(name: &str) -> Option<(usize, &'static CoreDirective)> {
    CORE.iter().enumerate().find(|(_, c)| equal_fold(c.name, name))
}

/// Runs core directive `k`.
pub(crate) fn run_core(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
    k: usize,
) -> Result<(), Fail> {
    let Some(c) = CORE.get(k) else { return Ok(()) };
    match c.kind {
        Core::Toctree => body::toctree_directive(p, d),
        Core::Only => body::only_directive(p, d, c.name),
        Core::Index => body::index_directive(sx, p, d),
        Core::Centered => body::centered_directive(p, d),
        Core::Hlist => body::hlist_directive(p, d),
        Core::Rubric => body::rubric_directive(p, d),
        Core::Version => body::version_directive(p, d, c.name),
        Core::Author => body::author_directive(sx, p, d, c.name),
        Core::Ignored => Ok(()),
        Core::Highlight => body::highlight_directive(sx, d),
        Core::Code => code::code_directive(sx, p, d),
        Core::Literalinclude => literalinclude::literalinclude(sx, p, d),
        Core::Math => math::math_directive(sx, p, d),
        Core::DefaultDomain => body::default_domain_directive(sx, d),
        Core::DefaultRole => body::default_role_directive(sx, p, d),
        Core::Todo => body::todo_directive(sx, p, d),
        Core::Todolist => {
            let id = p.directive_node(d, "todolist")?;
            p.close(id)
        }
        Core::More => {
            let id = p.directive_node(d, "more")?;
            p.b().end_node(id)
        }
        Core::Postlist => {
            let id = p.directive_node(d, "postlist")?;
            p.custom(id, "limit", &d.argument())?;
            for name in ["path", "category", "tags", "author", "series", "sort", "style"] {
                value_attribute(p, id, d, name)?;
            }
            flag_attribute(p, id, d, "excerpts")?;
            p.close(id)
        }
        Core::Acks => body::acks_directive(p, d),
        Core::RstClass => p.class_directive(d),
        Core::Graphviz => graphviz::graphviz_directive(p, d, c.name),
        Core::Doctest => doctest::doctest_directive(sx, p, d, c.name),
        Core::Autodoc => autodoc_directive(sx, p, d, "py"),
        Core::Inheritance => provided_directive(sx, p, d, INHERITANCE_DIAGRAM, "py"),
    }
}

/// Adds Custom `name` = the option's value when the option is given.
pub(crate) fn value_attribute(
    p: &mut Parser<'_, '_>,
    id: NodeId,
    d: &Directive<'_>,
    name: &str,
) -> Result<(), Fail> {
    match d.option_value(name) {
        Some(v) => p.custom(id, name, &v),
        None => Ok(()),
    }
}

/// Adds Custom `name` = "1" when the directive has the flag option.
pub(crate) fn flag_attribute(
    p: &mut Parser<'_, '_>,
    id: NodeId,
    d: &Directive<'_>,
    name: &str,
) -> Result<(), Fail> {
    if d.has_option(name) {
        return p.custom(id, name, "1");
    }
    Ok(())
}

/// Adds the :class: option as a Class attribute.
pub(crate) fn class_attribute(
    p: &mut Parser<'_, '_>,
    id: NodeId,
    d: &Directive<'_>,
) -> Result<(), Fail> {
    if let Some(v) = d.option_value("class")
        && let Some(t) = p.pool_classes(&v)?
    {
        p.ctx.b.add_attribute(id, AttributeKey::Class, t)?;
    }
    Ok(())
}

/// Reads an optionally signed integer, trimmed.
pub(crate) fn parse_signed(v: &str) -> i64 {
    parse_int(trim_space(v))
}

/// Returns the warning for a typed index entry that lacks the parts its type needs.
pub(crate) fn check_index_entry(entry: &str) -> Option<Message> {
    let e = entry.strip_prefix('!').map_or(entry, trim_space);
    let types =
        [("single", 1), ("pair", 2), ("double", 2), ("triple", 3), ("see", 2), ("seealso", 2)];
    let (need, value) = types.iter().find_map(|&(t, need)| {
        let rest = e.strip_prefix(t)?;
        rest.strip_prefix(':').filter(|_| !rest.is_empty()).map(|v| (need, v))
    })?;
    let parts = value.matches(';').count() + 1;
    (need > 1 && parts != need).then_some(INDEX_ENTRY)
}
