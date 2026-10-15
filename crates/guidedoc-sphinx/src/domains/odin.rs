//! The odin domain describes Odin APIs as the Python domain describes Python ones (GDS
//! 0005). A package is the context, as a Python module is: `odin:package` sets it, and
//! the objects described after it are named "<package>.<name>", the package being its
//! import path ("core:fmt", or "readers/sphinx" below a project's roots). Fields and
//! enumerators nested in a type are "<Type>.<member>". Ids are Sphinx's make_id of
//! "odin-" and that full name ("odin-core-fmt.println"): case and dots are kept, and the
//! ':' and '/' of the import path become hyphens. A package's id is
//! "odin-package-<path>".
//!
//! Signatures are read as Odin writes declarations, one line each:
//!
//! ```text
//! [@(attributes)] name :: [#force_inline ]proc["c"](params)[ -> results][ tags][ where ...]
//! name :: proc{a, b}                  Name :: struct($T: typeid) #packed
//! Name :: union #no_nil {A, B}        Name :: enum u8       Name :: bit_set[E; u8]
//! Name :: distinct int                NAME :: 64            name: T = value
//! name: T `tag`   (field)             Name = 3   (enumerator)
//! ```
//!
//! "name(params) -> results" is short for a procedure's "name :: proc(params) ->
//! results". Names in type positions become Xref{odin:type} with the package as context,
//! and an import alias the package declares (`:imports: gd=core`) is followed to the
//! package it names; built-in types, keywords, and polymorphic names ($T, and T after
//! it) stay text. Default values, constants, tags, and where clauses are shown as
//! written.

mod render;
mod scan;

use guidedoc::{Builder, Fail, Label, NodeId, NodeKind, SourceSpan};
use guidedoc_rst::{ArgRule, ContentRule, Directive, OptSpec, OptType, Parser, opt};

use super::{
    Cx, DocField, Domain, DomainDirective, FieldShape, Object, RoleSpec, TocNames, XrefFlags,
    XrefInfo,
};
use crate::config::{Config, TocParents};
use crate::extension::Sphinx;
use crate::state::{NameStack, State};
use crate::text::{last_part, trim_space};
use render::SigCx;

/// The odin domain's per-document state (`State::odin`).
#[derive(Debug, Clone, Default)]
pub(crate) struct OdinState {
    /// The current package's import path.
    package: String,
    /// Its import aliases: ("gd", "core").
    imports: Vec<(String, String)>,
    /// Its private names, which stay text: "State", "Rules".
    private: Vec<String>,
    /// The types members are nested in.
    types: NameStack,
    /// Their polymorphic names, separated by spaces.
    type_polys: NameStack,
    /// Packages saved by a `:package:` option.
    packages: NameStack,
}

/// The odin domain.
#[derive(Debug)]
pub(crate) struct OdinDomain;

/// The value `domains::DOMAINS` registers.
pub(crate) static DOMAIN: OdinDomain = OdinDomain;

static OPTS_OBJECT: &[OptSpec] = &[
    opt("no-index", OptType::Flag),
    opt("noindex", OptType::Flag),
    opt("no-index-entry", OptType::Flag),
    opt("noindexentry", OptType::Flag),
    opt("no-contents-entry", OptType::Flag),
    opt("nocontentsentry", OptType::Flag),
    opt("no-typesetting", OptType::Flag),
    opt("single-line-parameter-list", OptType::Flag),
    opt("package", OptType::Text),
    opt("private", OptType::Flag),
    opt("deprecated", OptType::Text),
    opt("availability", OptType::Text),
    opt("foreign", OptType::Text),
];

static OPTS_PACKAGE: &[OptSpec] = &[
    opt("platform", OptType::Text),
    opt("synopsis", OptType::Text),
    opt("imports", OptType::Text),
    opt("private", OptType::Text),
    opt("no-index", OptType::Flag),
    opt("noindex", OptType::Flag),
    opt("no-index-entry", OptType::Flag),
    opt("noindexentry", OptType::Flag),
    opt("no-contents-entry", OptType::Flag),
    opt("nocontentsentry", OptType::Flag),
    opt("no-typesetting", OptType::Flag),
    opt("deprecated", OptType::Flag),
];

static OPTS_CURRENT: &[OptSpec] = &[opt("imports", OptType::Text), opt("private", OptType::Text)];

/// The options of the generating directives, as text; the host checks them.
static OPTS_AUTO: &[OptSpec] = &[
    opt("members", OptType::Text),
    opt("undoc-members", OptType::Text),
    opt("private-members", OptType::Text),
    opt("exclude-members", OptType::Text),
    opt("member-order", OptType::Text),
    opt("no-index", OptType::Text),
    opt("noindex", OptType::Text),
    opt("synopsis", OptType::Text),
    opt("platform", OptType::Text),
];

const PACKAGE_TAG: u16 = 0;
const CURRENTPACKAGE_TAG: u16 = 1;

static DIRECTIVES: &[DomainDirective] = &[
    DomainDirective::run(
        "odin:package",
        PACKAGE_TAG,
        (ArgRule::Required, false, ContentRule::Optional),
        OPTS_PACKAGE,
    ),
    DomainDirective::run(
        "odin:currentpackage",
        CURRENTPACKAGE_TAG,
        (ArgRule::Required, false, ContentRule::None),
        OPTS_CURRENT,
    ),
    DomainDirective::object("odin:procedure", OPTS_OBJECT),
    DomainDirective::object("odin:proc", OPTS_OBJECT),
    DomainDirective::object("odin:procgroup", OPTS_OBJECT),
    DomainDirective::object("odin:struct", OPTS_OBJECT),
    DomainDirective::object("odin:union", OPTS_OBJECT),
    DomainDirective::object("odin:enum", OPTS_OBJECT),
    DomainDirective::object("odin:enumerator", OPTS_OBJECT),
    DomainDirective::object("odin:field", OPTS_OBJECT),
    DomainDirective::object("odin:bitset", OPTS_OBJECT),
    DomainDirective::object("odin:bitfield", OPTS_OBJECT),
    DomainDirective::object("odin:type", OPTS_OBJECT),
    DomainDirective::object("odin:const", OPTS_OBJECT),
    DomainDirective::object("odin:var", OPTS_OBJECT),
    DomainDirective::object("odin:foreign", OPTS_OBJECT),
    DomainDirective::autodoc("odin:autopackage", OPTS_AUTO),
    DomainDirective::autodoc("odin:autoproc", OPTS_AUTO),
    DomainDirective::autodoc("odin:autoprocgroup", OPTS_AUTO),
    DomainDirective::autodoc("odin:autotype", OPTS_AUTO),
    DomainDirective::autodoc("odin:autoconst", OPTS_AUTO),
    DomainDirective::autodoc("odin:autovar", OPTS_AUTO),
];

const CODE_DOTTED: XrefFlags = XrefFlags::CODE.with(XrefFlags::DOTTED);

static ROLES: &[RoleSpec] = &[
    RoleSpec::xref("pkg", CODE_DOTTED),
    RoleSpec::xref("proc", CODE_DOTTED),
    RoleSpec::xref("type", CODE_DOTTED),
    RoleSpec::xref("const", CODE_DOTTED),
    RoleSpec::xref("var", CODE_DOTTED),
    RoleSpec::xref("field", CODE_DOTTED),
    RoleSpec::xref("enumerator", CODE_DOTTED),
    RoleSpec::xref("obj", CODE_DOTTED),
];

/// Every Odin object's info fields: parameters with their types, a procedure's named
/// results (`:result name:`, as Odin's core library documents them), and its returns.
static FIELDS: &[DocField] = &[
    DocField {
        name: "parameter",
        shape: FieldShape::Typed,
        label: Label::Parameters,
        has_arg: true,
        names: "param|parameter|arg|argument",
        typenames: "type",
        typerole: "type",
        collapse: true,
        ..DocField::DEFAULT
    },
    DocField {
        name: "result",
        shape: FieldShape::Grouped,
        label: Label::Returns,
        has_arg: true,
        names: "result",
        collapse: true,
        ..DocField::DEFAULT
    },
    DocField {
        name: "returnvalue",
        shape: FieldShape::Plain,
        label: Label::Returns,
        names: "returns|return",
        ..DocField::DEFAULT
    },
    DocField {
        name: "returntype",
        shape: FieldShape::Plain,
        label: Label::ReturnType,
        names: "rtype",
        bodyrole: "type",
        ..DocField::DEFAULT
    },
];

impl Domain for OdinDomain {
    fn name(&self) -> &'static str {
        "odin"
    }

    fn directives(&self) -> &'static [DomainDirective] {
        DIRECTIVES
    }

    fn roles(&self) -> &'static [RoleSpec] {
        ROLES
    }

    fn fields(&self, _objtype: &str) -> &'static [DocField] {
        FIELDS
    }

    fn signature(
        &self,
        cx: &mut Cx<'_, '_, '_>,
        o: &mut Object<'_>,
        sig: &str,
        span: SourceSpan,
    ) -> Result<(), Fail> {
        signature(cx, o, sig, span)
    }

    fn run(
        &self,
        sx: &Sphinx<'_>,
        p: &mut Parser<'_, '_>,
        d: &Directive<'_>,
        tag: u16,
    ) -> Result<(), Fail> {
        let name = trim_space(&d.argument()).to_owned();
        if tag == CURRENTPACKAGE_TAG {
            let mut st = sx.state();
            st.odin.package = if name == "None" { String::new() } else { name };
            package_options(&mut st.odin, d);
            return Ok(());
        }
        {
            let mut st = sx.state();
            st.odin.package.clone_from(&name);
            package_options(&mut st.odin, d);
        }
        sx.with_cx(p, |cx| package_target(cx, d, &name))?;
        p.parse_body(&d.content, d.sections, None)
    }

    fn content_begin(&self, cx: &mut Cx<'_, '_, '_>, o: &Object<'_>) -> Result<(), Fail> {
        availability(cx, o)
    }

    fn before_content(&self, cx: &mut Cx<'_, '_, '_>, o: &Object<'_>) -> u32 {
        let odin = &mut cx.st.odin;
        let mut pushed = 0;
        if nests(o.objtype) && odin.types.push(&o.fullname) {
            odin.type_polys.push(&o.extra);
            pushed |= 1;
        }
        if let Some(v) = o.option("package") {
            let current = std::mem::take(&mut odin.package);
            if odin.packages.push(&current) {
                pushed |= 2;
            }
            odin.package = v.into_owned();
        }
        pushed
    }

    fn after_content(&self, st: &mut State, _o: &Object<'_>, pushed: u32) {
        let odin = &mut st.odin;
        if pushed & 1 != 0 {
            odin.types.pop();
            odin.type_polys.pop();
        }
        if pushed & 2 != 0 {
            odin.package = odin.packages.top().to_owned();
            odin.packages.pop();
        }
    }

    fn toc_name(
        &self,
        config: &Config,
        _st: &State,
        _o: &Object<'_>,
        names: TocNames<'_>,
    ) -> Option<String> {
        let fullname = names.fullname;
        Some(match config.toc_object_entries_show_parents {
            TocParents::Domain => fullname.to_owned(),
            TocParents::Hide => last_part(fullname, ".").to_owned(),
            TocParents::All if !names.module.is_empty() => format!("{}.{fullname}", names.module),
            TocParents::All => fullname.to_owned(),
        })
    }

    fn map_target(&self, st: &State, target: &str) -> Option<String> {
        mapped(st, target)
    }

    fn xref_context(
        &self,
        b: &mut Builder<'_>,
        st: &State,
        id: NodeId,
        _x: &XrefInfo<'_>,
    ) -> Result<(), Fail> {
        if !st.odin.package.is_empty() {
            b.add_custom_attribute(id, "odin:package", &st.odin.package)?;
        }
        let top = st.odin.types.top();
        if !top.is_empty() {
            b.add_custom_attribute(id, "odin:type", top)?;
        }
        Ok(())
    }

    fn autodoc_context(&self, st: &State) -> (String, String) {
        (st.odin.package.clone(), st.odin.types.top().to_owned())
    }
}

/// Returns an object type's full name: odin:proc is odin:procedure.
fn full_objtype(objtype: &str) -> &str {
    if objtype == "proc" { "procedure" } else { objtype }
}

/// Tells whether an object type's content describes its members.
fn nests(objtype: &str) -> bool {
    matches!(objtype, "struct" | "union" | "enum" | "bitfield" | "bitset" | "type")
}

/// Returns how index entries name an object type.
fn kind_word(objtype: &str) -> &str {
    match objtype {
        "procgroup" => "procedure group",
        "bitset" => "bit set",
        "bitfield" => "bit field",
        "const" => "constant",
        "var" => "variable",
        "foreign" => "foreign import",
        other => other,
    }
}

/// Emits an Odin object's signature, its ids, and its index entry. Only the first
/// signature of a directive (the others are the same object for other targets) gets
/// the id and the entry.
fn signature(
    cx: &mut Cx<'_, '_, '_>,
    o: &mut Object<'_>,
    sig: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let objtype = full_objtype(o.objtype);
    let Some(os) = scan::parse_sig(sig, objtype == "foreign") else {
        return cx.plain_signature(sig, span);
    };
    let pkg = o.option("package").map_or_else(|| cx.st.odin.package.clone(), |v| v.into_owned());
    let parent = cx.st.odin.types.top().to_owned();
    let fullname = if parent.is_empty()
        || (os.prefix.starts_with(parent.as_str()) && os.prefix[parent.len()..].starts_with('.'))
    {
        format!("{}{}", os.prefix, os.name)
    } else {
        format!("{parent}.{}{}", os.prefix, os.name)
    };
    let full = if pkg.is_empty() { fullname.clone() } else { format!("{pkg}.{fullname}") };
    let first = o.signatures == 0;
    let id = cx.b().begin_node(NodeKind::DescSignature, span)?;
    if !o.noindex && first {
        cx.add_ids(id, "odin", &full, "")?;
    }
    cx.custom(id, "fullname", &fullname)?;
    if !pkg.is_empty() {
        cx.custom(id, "module", &pkg)?;
    }
    cx.toc_entry_name(o, id, TocNames { fullname: &fullname, module: &pkg, sig })?;
    let c = SigCx::new(sig, span, cx.st.odin.type_polys.top().to_owned());
    signature_head(cx, o, &os, &pkg, !parent.is_empty(), span)?;
    render::rest(cx, &c, objtype, os.rest)?;
    cx.b().end_node(id)?;
    o.extra = c.polys_text();
    o.prefix = os.prefix.to_owned();
    if first {
        let kind = kind_word(objtype);
        if pkg.is_empty() {
            o.entry("single", &[&fullname, " (", kind, ")"]);
        } else {
            o.entry("single", &[&fullname, " (", kind, " in ", &pkg, ")"]);
        }
    }
    o.fullname = fullname;
    o.signatures += 1;
    Ok(())
}

/// Emits the attributes, the options shown as attributes, the package or written
/// prefix, and the name.
fn signature_head(
    cx: &mut Cx<'_, '_, '_>,
    o: &Object<'_>,
    os: &scan::OdinSig<'_>,
    pkg: &str,
    nested: bool,
    span: SourceSpan,
) -> Result<(), Fail> {
    let mut annotation = String::new();
    if !os.attrs.is_empty() {
        annotation.push_str(os.attrs);
        annotation.push(' ');
    }
    if o.flag("private") && !os.attrs.contains("private") {
        annotation.push_str("@(private) ");
    }
    if let Some(v) = o.option("deprecated")
        && !os.attrs.contains("deprecated")
    {
        if v.is_empty() {
            annotation.push_str("@(deprecated) ");
        } else {
            annotation.push_str(&format!("@(deprecated=\"{v}\") "));
        }
    }
    if let Some(v) = o.option("foreign")
        && !v.is_empty()
    {
        annotation.push_str(&format!("foreign {v} "));
    }
    if full_objtype(o.objtype) == "foreign" {
        annotation.push_str("foreign import ");
    }
    if !annotation.is_empty() {
        cx.leaf_in(NodeKind::DescAnnotation, span, &annotation)?;
    }
    if !os.prefix.is_empty() {
        cx.leaf_in(NodeKind::DescAddname, span, os.prefix)?;
    } else if !pkg.is_empty() && !nested && cx.config.add_module_names {
        let addname = format!("{}.", scan::short_name(pkg));
        cx.leaf_in(NodeKind::DescAddname, span, &addname)?;
    }
    cx.leaf_in(NodeKind::DescName, span, os.name)
}

/// odin:package's target: Index{Id "odin-package-<path>"; entries "single: <path>
/// (package)"; module, domain "odin", platform, synopsis, deprecated}.
fn package_target(cx: &mut Cx<'_, '_, '_>, d: &Directive<'_>, name: &str) -> Result<(), Fail> {
    if d.has_option("no-index") || d.has_option("noindex") {
        return Ok(());
    }
    let id = cx.p.open(NodeKind::Index, d.span)?;
    cx.add_ids(id, "odin-package", name, "")?;
    if !d.has_option("no-index-entry") && !d.has_option("noindexentry") {
        cx.custom(id, "entries", &format!("single: {name} (package)"))?;
    }
    cx.custom(id, "module", name)?;
    cx.custom(id, "domain", "odin")?;
    for option in ["platform", "synopsis"] {
        if let Some(v) = d.option_value(option) {
            cx.custom(id, option, &v)?;
        }
    }
    if d.has_option("deprecated") {
        cx.custom(id, "deprecated", "1")?;
    }
    cx.p.close(id)
}

/// Reads what a package's directive says about its names: the import aliases its
/// signatures use (`:imports: gd=core`) and its private declarations (`:private: State
/// Rules`), which have no descriptions and so stay text in signatures.
fn package_options(odin: &mut OdinState, d: &Directive<'_>) {
    odin.imports.clear();
    odin.private.clear();
    if let Some(v) = d.option_value("imports") {
        for item in v.split_ascii_whitespace() {
            if let Some((alias, pkg)) = item.split_once('=')
                && !alias.is_empty()
                && !pkg.is_empty()
            {
                odin.imports.push((alias.to_owned(), pkg.to_owned()));
            }
        }
    }
    if let Some(v) = d.option_value("private") {
        odin.private.extend(v.split_ascii_whitespace().map(str::to_owned));
    }
}

/// Tells whether `word` is one of the current package's private names.
fn is_private(st: &State, word: &str) -> bool {
    st.odin.private.iter().any(|name| name == word)
}

/// Replaces an import alias before the first dot of a name with the package the
/// current package's `:imports:` gives it; `None` when there is none.
fn mapped(st: &State, name: &str) -> Option<String> {
    let dot = name.find('.').filter(|&k| k > 0)?;
    let (alias, rest) = name.split_at(dot);
    let (_, pkg) = st.odin.imports.iter().find(|(a, _)| a == alias)?;
    Some(format!("{pkg}{rest}"))
}

/// Puts `:availability:` first in an object's content, as a paragraph "Availability:
/// Windows, Linux." whose label is the document's language's.
fn availability(cx: &mut Cx<'_, '_, '_>, o: &Object<'_>) -> Result<(), Fail> {
    let Some(v) = o.option("availability").filter(|v| !v.is_empty()) else { return Ok(()) };
    let span = o.d.span;
    let para = cx.b().begin_node(NodeKind::Paragraph, span)?;
    let first = cx.node_count();
    let config = cx.config;
    let label = config.label(Label::Availability);
    let strong = cx.b().begin_node(NodeKind::Strong, span)?;
    cx.text_leaf(span, label)?;
    cx.b().end_node(strong)?;
    cx.text_leaf(span, ": ")?;
    cx.generated(first);
    cx.text_leaf(span, &v)?;
    if !v.ends_with('.') {
        cx.text_leaf(span, ".")?;
    }
    cx.b().end_node(para)
}
