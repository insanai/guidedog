//! The rust domain describes Rust APIs as the Odin domain describes Odin ones (see
//! gd-rustdoc's README for the vocabulary, and docs/manual/internals/rust-domain.rst).
//! `rust:crate` and `rust:module` set the current module; the objects described after
//! them are named by their path in it ("Point::new"), and their full path is the
//! module's and that ("my_crate::geometry::Point::new"). Members nest in the content of
//! their type's directive (fields, variants, methods, associated items, impls); an
//! impl's members belong to the type the impl is for.
//!
//! Ids are Sphinx's make_id of "rust-" and the full path with `::` written `.`
//! ("rust-my_crate.geometry.Point.new"); a module's is "rust-module-<path>". Impls and
//! re-exports (`rust:use`) are never targets, and neither are the members of trait
//! impls, which are documented on the trait.
//!
//! Signatures are read as Rust writes declarations, on one line. Paths in type
//! positions become Xref{rust:any} with the module and type as context; `crate::`,
//! `self::`, `super::` and `Self::` are resolved here, other paths are left to the
//! project's resolver (the current module first, then inventories). Keywords,
//! primitive types, generic parameters, lifetimes and values stay text.

mod render;
mod scan;

use std::borrow::Cow;

use guidedoc::{Builder, Fail, Label, NodeId, NodeKind, SourceSpan};
use guidedoc_rst::{ArgRule, ContentRule, Directive, OptSpec, OptType, Parser, opt};

use super::{
    Cx, DocField, Domain, DomainDirective, FieldShape, Object, RoleSpec, TocNames, XrefFlags,
    XrefInfo, XrefParts,
};
use crate::config::{Config, TocParents};
use crate::extension::Sphinx;
use crate::state::{MAX_NEST, NameStack, State};
use crate::text::{last_part, trim_space};
use render::SigCx;
use scan::Head;

/// A type or impl whose content is being read: its members are named after it.
#[derive(Debug, Clone, Default)]
struct Scope {
    /// Its path within the module ("Kind::Rect"); an impl's is its type's.
    fullname: String,
    /// The generic names its members see, separated by spaces.
    generics: String,
    /// A trait impl: its members are documented on the trait and are no targets.
    trait_impl: bool,
}

/// The rust domain's per-document state (`State::rust`).
#[derive(Debug, Clone, Default)]
pub(crate) struct RustState {
    /// The current module's path ("my_crate::geometry").
    module: String,
    /// Modules saved by a `:module:` option.
    modules: NameStack,
    /// The types and impls members are nested in, outermost first.
    scopes: Vec<Scope>,
}

/// The rust domain.
#[derive(Debug)]
pub(crate) struct RustDomain;

/// The value `domains::DOMAINS` registers.
pub(crate) static DOMAIN: RustDomain = RustDomain;

static OPTS_OBJECT: &[OptSpec] = &[
    opt("no-index", OptType::Flag),
    opt("noindex", OptType::Flag),
    opt("no-index-entry", OptType::Flag),
    opt("noindexentry", OptType::Flag),
    opt("no-contents-entry", OptType::Flag),
    opt("nocontentsentry", OptType::Flag),
    opt("no-typesetting", OptType::Flag),
    opt("module", OptType::Text),
    opt("visibility", OptType::Text),
    opt("cfg", OptType::Text),
    opt("unsafe", OptType::Flag),
    opt("async", OptType::Flag),
    opt("const", OptType::Flag),
    opt("provided", OptType::Flag),
    opt("attributes", OptType::Text),
    opt("deprecated", OptType::Text),
    opt("for", OptType::Text),
    opt("trait", OptType::Text),
    opt("target", OptType::Text),
];

static OPTS_MODULE: &[OptSpec] = &[
    opt("synopsis", OptType::Text),
    opt("platform", OptType::Text),
    opt("no-index", OptType::Flag),
    opt("noindex", OptType::Flag),
    opt("no-index-entry", OptType::Flag),
    opt("noindexentry", OptType::Flag),
    opt("no-contents-entry", OptType::Flag),
    opt("nocontentsentry", OptType::Flag),
    opt("no-typesetting", OptType::Flag),
    opt("visibility", OptType::Text),
    opt("cfg", OptType::Text),
    opt("unsafe", OptType::Flag),
    opt("async", OptType::Flag),
    opt("const", OptType::Flag),
    opt("attributes", OptType::Text),
    opt("deprecated", OptType::Text),
];

const CRATE_TAG: u16 = 0;
const MODULE_TAG: u16 = 1;

static DIRECTIVES: &[DomainDirective] = &[
    DomainDirective::run(
        "rust:crate",
        CRATE_TAG,
        (ArgRule::Required, false, ContentRule::Optional),
        OPTS_MODULE,
    ),
    DomainDirective::run(
        "rust:module",
        MODULE_TAG,
        (ArgRule::Required, false, ContentRule::Optional),
        OPTS_MODULE,
    ),
    DomainDirective::object("rust:function", OPTS_OBJECT),
    DomainDirective::object("rust:method", OPTS_OBJECT),
    DomainDirective::object("rust:struct", OPTS_OBJECT),
    DomainDirective::object("rust:enum", OPTS_OBJECT),
    DomainDirective::object("rust:variant", OPTS_OBJECT),
    DomainDirective::object("rust:union", OPTS_OBJECT),
    DomainDirective::object("rust:trait", OPTS_OBJECT),
    DomainDirective::object("rust:impl", OPTS_OBJECT),
    DomainDirective::object("rust:type", OPTS_OBJECT),
    DomainDirective::object("rust:const", OPTS_OBJECT),
    DomainDirective::object("rust:static", OPTS_OBJECT),
    DomainDirective::object("rust:macro", OPTS_OBJECT),
    DomainDirective::object("rust:field", OPTS_OBJECT),
    DomainDirective::object("rust:associatedtype", OPTS_OBJECT),
    DomainDirective::object("rust:associatedconst", OPTS_OBJECT),
    DomainDirective::object("rust:use", OPTS_OBJECT),
];

const CODE_DOTTED: XrefFlags = XrefFlags::CODE.with(XrefFlags::DOTTED);

static ROLES: &[RoleSpec] = &[
    RoleSpec::xref("mod", CODE_DOTTED),
    RoleSpec::xref("fn", CODE_DOTTED.with(XrefFlags::PARENS)),
    RoleSpec::xref("struct", CODE_DOTTED),
    RoleSpec::xref("enum", CODE_DOTTED),
    RoleSpec::xref("union", CODE_DOTTED),
    RoleSpec::xref("trait", CODE_DOTTED),
    RoleSpec::xref("variant", CODE_DOTTED),
    RoleSpec::xref("field", CODE_DOTTED),
    RoleSpec::xref("type", CODE_DOTTED),
    RoleSpec::xref("const", CODE_DOTTED),
    RoleSpec::xref("static", CODE_DOTTED),
    RoleSpec::xref("macro", CODE_DOTTED),
    RoleSpec::xref("any", CODE_DOTTED),
];

/// Info fields hand-written docs may use: parameters with their types and returns.
static FIELDS: &[DocField] = &[
    DocField {
        name: "parameter",
        shape: FieldShape::Typed,
        label: Label::Parameters,
        has_arg: true,
        names: "param|parameter|arg|argument",
        typenames: "type",
        typerole: "any",
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
        bodyrole: "any",
        ..DocField::DEFAULT
    },
];

impl Domain for RustDomain {
    fn name(&self) -> &'static str {
        "rust"
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
        sx.state().rust.module.clone_from(&name);
        let kind = if tag == CRATE_TAG { "crate" } else { "module" };
        sx.with_cx(p, |cx| {
            module_target(cx, d, &name, kind)?;
            notes(cx, d)
        })?;
        p.parse_body(&d.content, d.sections, None)
    }

    fn content_begin(&self, cx: &mut Cx<'_, '_, '_>, o: &Object<'_>) -> Result<(), Fail> {
        notes(cx, o.d)
    }

    fn before_content(&self, cx: &mut Cx<'_, '_, '_>, o: &Object<'_>) -> u32 {
        let rust = &mut cx.st.rust;
        let mut pushed = 0;
        if nests(o.objtype) && rust.scopes.len() < MAX_NEST {
            let trait_impl = o.objtype == "impl" && is_trait_impl(o);
            let generics = o.extra.clone();
            rust.scopes.push(Scope { fullname: o.fullname.clone(), generics, trait_impl });
            pushed |= 1;
        }
        if let Some(v) = o.option("module") {
            let current = std::mem::take(&mut rust.module);
            if rust.modules.push(&current) {
                pushed |= 2;
            }
            rust.module = v.into_owned();
        }
        pushed
    }

    fn after_content(&self, st: &mut State, _o: &Object<'_>, pushed: u32) {
        let rust = &mut st.rust;
        if pushed & 1 != 0 {
            rust.scopes.pop();
        }
        if pushed & 2 != 0 {
            rust.module = rust.modules.top().to_owned();
            rust.modules.pop();
        }
    }

    fn toc_name(
        &self,
        config: &Config,
        _st: &State,
        o: &Object<'_>,
        names: TocNames<'_>,
    ) -> Option<String> {
        let callable = matches!(o.objtype, "function" | "method");
        let parens = if callable && config.add_function_parentheses { "()" } else { "" };
        let fullname = names.fullname;
        Some(match config.toc_object_entries_show_parents {
            TocParents::Domain => format!("{fullname}{parens}"),
            TocParents::Hide => format!("{}{parens}", last_part(fullname, "::")),
            TocParents::All if !names.module.is_empty() => {
                format!("{}::{fullname}{parens}", names.module)
            }
            TocParents::All => format!("{fullname}{parens}"),
        })
    }

    fn map_target(&self, st: &State, target: &str) -> Option<String> {
        resolve_path(st, target)
    }

    fn xref_context(
        &self,
        b: &mut Builder<'_>,
        st: &State,
        id: NodeId,
        _x: &XrefInfo<'_>,
    ) -> Result<(), Fail> {
        if !st.rust.module.is_empty() {
            b.add_custom_attribute(id, "rust:module", &st.rust.module)?;
        }
        if let Some(scope) = st.rust.scopes.last() {
            b.add_custom_attribute(id, "rust:type", &scope.fullname)?;
        }
        Ok(())
    }

    fn autodoc_context(&self, st: &State) -> (String, String) {
        let scope = st.rust.scopes.last().map_or("", |s| s.fullname.as_str());
        (st.rust.module.clone(), scope.to_owned())
    }
}

/// Tells whether an object type's content describes members named after it.
fn nests(objtype: &str) -> bool {
    matches!(objtype, "struct" | "enum" | "union" | "trait" | "variant" | "impl")
}

/// Returns how index entries name an object type.
fn kind_word(objtype: &str) -> &str {
    match objtype {
        "type" => "type alias",
        "associatedtype" => "associated type",
        "const" => "constant",
        "associatedconst" => "associated constant",
        other => other,
    }
}

/// Resolves the paths Rust writes relative to where they stand: `crate::`, `self::`,
/// `super::` (repeated) and `Self::`. `None` for other paths, which the project's
/// resolver looks up from the module.
pub(super) fn resolve_path(st: &State, path: &str) -> Option<String> {
    let (first, rest) = path.split_once("::")?;
    let module = st.rust.module.as_str();
    match first {
        "crate" => {
            let krate = module.split("::").next().filter(|k| !k.is_empty())?;
            Some(format!("{krate}::{rest}"))
        }
        "self" if !module.is_empty() => Some(format!("{module}::{rest}")),
        "super" => {
            let mut base = module.rsplit_once("::")?.0;
            let mut rest = rest;
            while let Some(after) = rest.strip_prefix("super::") {
                base = base.rsplit_once("::")?.0;
                rest = after;
            }
            Some(format!("{base}::{rest}"))
        }
        "Self" => {
            let scope = st.rust.scopes.last()?;
            if module.is_empty() {
                Some(format!("{}::{rest}", scope.fullname))
            } else {
                Some(format!("{module}::{}::{rest}", scope.fullname))
            }
        }
        _ => None,
    }
}

/// Emits a Rust object's signature, its id, and its index entry. Only the first
/// signature of a directive gets the id and the entry. Impls, uses and the members of
/// trait impls are never targets.
fn signature(
    cx: &mut Cx<'_, '_, '_>,
    o: &mut Object<'_>,
    sig: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let objtype = o.objtype;
    let Some(head) = scan::parse_head(sig, objtype) else {
        return cx.plain_signature(sig, span);
    };
    let module = o.option("module").map_or_else(|| cx.st.rust.module.clone(), Cow::into_owned);
    let parent = cx.st.rust.scopes.last().cloned().unwrap_or_default();
    let fullname = match objtype {
        "impl" => impl_name(o, &head, &parent.fullname),
        "use" => String::new(),
        _ if parent.fullname.is_empty() => head.name.to_owned(),
        _ => format!("{}::{}", parent.fullname, head.name),
    };
    let first = o.signatures == 0;
    let never_target = matches!(objtype, "impl" | "use") || parent.trait_impl;
    if never_target && !o.noindex {
        mark_noindex(cx, o)?;
    }
    let id = cx.b().begin_node(NodeKind::DescSignature, span)?;
    if !o.noindex && first {
        let full =
            if module.is_empty() { fullname.clone() } else { format!("{module}::{fullname}") };
        cx.add_ids(id, "rust", &full.replace("::", "."), "")?;
    }
    if !matches!(objtype, "impl" | "use") {
        cx.custom(id, "fullname", &fullname)?;
        if !module.is_empty() {
            cx.custom(id, "module", &module)?;
        }
    }
    if let Some(v) = o.option("visibility") {
        cx.custom(id, "visibility", &v)?;
    }
    cx.toc_entry_name(o, id, TocNames { fullname: &fullname, module: &module, sig })?;
    let mut c = SigCx::new(span, parent.generics);
    signature_head(cx, o, &head, &module, !parent.fullname.is_empty(), span)?;
    if objtype == "use" {
        use_path(cx, o, &c, head.rest)?;
    } else {
        render::rest(cx, &mut c, objtype, head.rest)?;
    }
    cx.b().end_node(id)?;
    if first && !never_target {
        add_entry(o, &head, &fullname, &module);
    }
    o.extra = c.generics_text();
    o.fullname = fullname;
    o.signatures += 1;
    Ok(())
}

/// Makes the object a non-target: `o.noindex`, and Custom "noindex" on its Desc (the
/// node open around the signatures).
fn mark_noindex(cx: &mut Cx<'_, '_, '_>, o: &mut Object<'_>) -> Result<(), Fail> {
    o.noindex = true;
    let desc = cx.b().current();
    if let Some(desc) = desc {
        cx.custom(desc, "noindex", "1")?;
    }
    Ok(())
}

/// Returns the name an impl's members are named after: the type it is nested in, or
/// its `:for:` type, or the type its signature names, without generic arguments.
fn impl_name(o: &Object<'_>, head: &Head<'_>, parent: &str) -> String {
    if !parent.is_empty() {
        return parent.to_owned();
    }
    let written = o.option("for");
    let self_type = match &written {
        Some(v) => trim_space(v),
        None => impl_self_type(head.rest),
    };
    let bare = self_type.trim_start_matches(['&', ' ']);
    let bare = trim_space(bare.strip_prefix("mut ").unwrap_or(bare));
    let end = bare.find('<').unwrap_or(bare.len());
    last_part(&bare[..end], "::").to_owned()
}

/// Returns the self type of an impl's text after `impl`: what follows `for` in a trait
/// impl, up to a where clause.
fn impl_self_type(rest: &str) -> &str {
    let (_, r) = scan::split_generics(rest);
    let for_at = scan::find_word(r, "for");
    let r = if for_at < r.len() { &r[for_at + 3..] } else { r };
    trim_space(&r[..scan::find_word(r, "where")])
}

/// Tells whether an impl directive describes a trait impl: it has `:trait:`, or its
/// signature has `for`.
fn is_trait_impl(o: &Object<'_>) -> bool {
    if o.option("trait").is_some() {
        return true;
    }
    let argument = o.d.argument();
    scan::parse_head(&argument, "impl").is_some_and(|head| {
        let (_, r) = scan::split_generics(head.rest);
        scan::find_word(r, "for") < r.len()
    })
}

/// Emits the attributes, the written visibility, qualifiers and keyword (the keyword
/// added for a short form like "f(x: u8)"), the module, and the name.
fn signature_head(
    cx: &mut Cx<'_, '_, '_>,
    o: &Object<'_>,
    head: &Head<'_>,
    module: &str,
    nested: bool,
    span: SourceSpan,
) -> Result<(), Fail> {
    let mut annotation = String::new();
    if let Some(v) = o.option("attributes").filter(|v| !v.is_empty())
        && !head.annotation.contains('#')
    {
        annotation.push_str(&v);
        annotation.push(' ');
    }
    annotation.push_str(head.annotation);
    let keyword = scan::keyword_of(o.objtype);
    if !head.keyword && !keyword.is_empty() {
        if !annotation.is_empty() && !annotation.ends_with(' ') {
            annotation.push(' ');
        }
        annotation.push_str(keyword);
    }
    let named = !matches!(o.objtype, "impl" | "use");
    if named && !annotation.is_empty() || o.objtype == "use" {
        annotation.push(' ');
    }
    if !trim_space(&annotation).is_empty() {
        cx.leaf_in(NodeKind::DescAnnotation, span, &annotation)?;
    }
    if !named {
        return Ok(());
    }
    if !module.is_empty() && !nested && cx.config.add_module_names {
        let addname = format!("{}::", last_part(module, "::"));
        cx.leaf_in(NodeKind::DescAddname, span, &addname)?;
    }
    cx.leaf_in(NodeKind::DescName, span, head.name)
}

/// Emits a re-export's path as a cross-reference to its `:target:` (or the path as
/// written), and its `as` rename as text. Glob and group imports stay text.
fn use_path(
    cx: &mut Cx<'_, '_, '_>,
    o: &Object<'_>,
    c: &SigCx<'_>,
    rest: &str,
) -> Result<(), Fail> {
    let as_at = scan::find_word(rest, "as");
    let path = trim_space(&rest[..as_at]);
    if path.is_empty() || path.contains(['{', '*']) {
        return cx.text_leaf(c.span, rest);
    }
    let target = match o.option("target").filter(|t| !t.is_empty()) {
        Some(t) => t.into_owned(),
        None => resolve_path(cx.st, path).unwrap_or_else(|| path.to_owned()),
    };
    let x =
        XrefParts { title: path.into(), target: target.into(), explicit: false, specific: false };
    cx.xref(c.span, render::SIGNATURE_REFTYPE, "rust", &x, false)?;
    if as_at < rest.len() {
        cx.text_leaf(c.span, " ")?;
        cx.text_leaf(c.span, &rest[as_at..])?;
    }
    Ok(())
}

/// Adds the index entry: "Point::new (method in my_crate::geometry)", with the
/// qualifiers of functions, methods and traits ("f (unsafe function in my_crate)").
fn add_entry(o: &mut Object<'_>, head: &Head<'_>, fullname: &str, module: &str) {
    let mut kind = String::new();
    if matches!(o.objtype, "function" | "method" | "trait") {
        let q = head.qualifiers;
        let flags = [
            ("const", q.is_const || o.flag("const")),
            ("async", q.is_async || o.flag("async")),
            ("unsafe", q.is_unsafe || o.flag("unsafe")),
        ];
        for (word, _) in flags.iter().filter(|f| f.1) {
            kind.push_str(word);
            kind.push(' ');
        }
    }
    kind.push_str(kind_word(o.objtype));
    if module.is_empty() {
        o.entry("single", &[fullname, " (", &kind, ")"]);
    } else {
        o.entry("single", &[fullname, " (", &kind, " in ", module, ")"]);
    }
}

/// A crate's or module's target: Index{Id "rust-module-<path>"; entries "single:
/// <path> (crate|module)"; module, domain "rust", platform, synopsis, deprecated}.
fn module_target(
    cx: &mut Cx<'_, '_, '_>,
    d: &Directive<'_>,
    name: &str,
    kind: &str,
) -> Result<(), Fail> {
    if d.has_option("no-index") || d.has_option("noindex") {
        return Ok(());
    }
    let id = cx.p.open(NodeKind::Index, d.span)?;
    cx.add_ids(id, "rust-module", &name.replace("::", "."), "")?;
    if !d.has_option("no-index-entry") && !d.has_option("noindexentry") {
        cx.custom(id, "entries", &format!("single: {name} ({kind})"))?;
    }
    cx.custom(id, "module", name)?;
    cx.custom(id, "domain", "rust")?;
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

/// Puts what rustdoc shows above an item's docs first: "Available on `cfg` only." for
/// `:cfg:`, and "Deprecated: note." for `:deprecated:`. The words the domain adds are
/// Generated.
fn notes(cx: &mut Cx<'_, '_, '_>, d: &Directive<'_>) -> Result<(), Fail> {
    let span = d.span;
    if let Some(cfg) = d.option_value("cfg").filter(|v| !v.is_empty()) {
        let para = cx.b().begin_node(NodeKind::Paragraph, span)?;
        let first = cx.node_count();
        cx.text_leaf(span, "Available on ")?;
        cx.generated(first);
        cx.b().add_text_leaf(NodeKind::Code, span, &cfg)?;
        let after = cx.node_count();
        cx.text_leaf(span, " only.")?;
        cx.generated(after);
        cx.b().end_node(para)?;
    }
    if let Some(note) = d.option_value("deprecated") {
        let para = cx.b().begin_node(NodeKind::Paragraph, span)?;
        let first = cx.node_count();
        let strong = cx.b().begin_node(NodeKind::Strong, span)?;
        cx.text_leaf(span, "Deprecated")?;
        cx.b().end_node(strong)?;
        if !note.is_empty() {
            cx.text_leaf(span, ": ")?;
        }
        cx.generated(first);
        cx.text_leaf(span, &note)?;
        if !note.is_empty() && !note.ends_with('.') {
            cx.text_leaf(span, ".")?;
        }
        cx.b().end_node(para)?;
    }
    Ok(())
}
