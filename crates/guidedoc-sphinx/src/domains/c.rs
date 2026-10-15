//! The C domain. C declarations are shown as written. They are tokenized far enough to
//! find the declared (possibly qualified) name and the parameters, so the name is a
//! Desc_Name, parameters are Desc_Parameters, and ids follow Sphinx: "c.<scope>.<name>".
//! Names in types and parameters link (`links`).

mod decl;
mod links;

use guidedoc::{Builder, Fail, Label, NodeId, NodeKind, SourceSpan};
use guidedoc_rst::{ArgRule, ContentRule, Directive, OptSpec, OptType, Parser, opt};
use guidedoc_rst_inline::RoleCall;

use super::object::OPTS_OBJECT;
use super::{
    Cx, DocField, Domain, DomainDirective, FieldShape, Object, RoleSpec, TocNames, XrefFlags,
    XrefInfo,
};
use crate::config::{Config, TocParents};
use crate::extension::Sphinx;
use crate::messages::{NESTING, SIGNATURE};
use crate::roles::RoleCx;
use crate::state::State;
use crate::text::{last_part, trim_space, trim_suffix};

/// The options of c:alias and cpp:alias.
pub(crate) const OPTS_ALIAS: &[OptSpec] =
    &[opt("maxdepth", OptType::Int), opt("noroot", OptType::Flag)];

/// The tags of the directives C and C++ share.
pub(crate) const ALIAS: u16 = 0;
pub(crate) const NAMESPACE: u16 = 1;

const ONE_ARG: (ArgRule, bool, ContentRule) = (ArgRule::Required, true, ContentRule::None);
const NO_ARG: (ArgRule, bool, ContentRule) = (ArgRule::None, false, ContentRule::None);

static DIRECTIVES: &[DomainDirective] = &[
    DomainDirective::object("c:member", OPTS_OBJECT),
    DomainDirective::object("c:var", OPTS_OBJECT),
    DomainDirective::object("c:function", OPTS_OBJECT),
    DomainDirective::object("c:macro", OPTS_OBJECT),
    DomainDirective::object("c:struct", OPTS_OBJECT),
    DomainDirective::object("c:union", OPTS_OBJECT),
    DomainDirective::object("c:enum", OPTS_OBJECT),
    DomainDirective::object("c:enumerator", OPTS_OBJECT),
    DomainDirective::object("c:type", OPTS_OBJECT),
    DomainDirective::run("c:alias", ALIAS, ONE_ARG, OPTS_ALIAS),
    DomainDirective::run("c:namespace", NAMESPACE, ONE_ARG, &[]),
    DomainDirective::run("c:namespace-push", NAMESPACE, ONE_ARG, &[]),
    DomainDirective::run("c:namespace-pop", NAMESPACE, NO_ARG, &[]),
];

const SCOPED: XrefFlags = XrefFlags::CODE.with(XrefFlags::SCOPED);

static ROLES: &[RoleSpec] = &[
    RoleSpec::xref("member", SCOPED),
    RoleSpec::xref("data", SCOPED),
    RoleSpec::xref("var", SCOPED),
    RoleSpec::xref("func", SCOPED.with(XrefFlags::PARENS)),
    RoleSpec::xref("macro", SCOPED),
    RoleSpec::xref("struct", SCOPED),
    RoleSpec::xref("union", SCOPED),
    RoleSpec::xref("enum", SCOPED),
    RoleSpec::xref("enumerator", SCOPED),
    RoleSpec::xref("type", SCOPED),
    RoleSpec::of("expr", crate::roles::RoleKind::Expr),
    RoleSpec::of("texpr", crate::roles::RoleKind::Texpr),
];

static FUNCTION_FIELDS: &[DocField] = &[
    DocField {
        name: "parameter",
        shape: FieldShape::Typed,
        label: Label::Parameters,
        has_arg: true,
        names: "param|parameter|arg|argument",
        typenames: "type",
        typerole: "expr",
        ..DocField::DEFAULT
    },
    DocField {
        name: "retval",
        shape: FieldShape::Grouped,
        label: Label::ReturnValues,
        has_arg: true,
        names: "retvals|retval",
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
        ..DocField::DEFAULT
    },
];

/// The C domain.
#[derive(Debug)]
pub(crate) struct CDomain;

/// The value `DOMAINS` registers.
pub(crate) static DOMAIN: CDomain = CDomain;

impl Domain for CDomain {
    fn name(&self) -> &'static str {
        "c"
    }

    fn directives(&self) -> &'static [DomainDirective] {
        DIRECTIVES
    }

    fn roles(&self) -> &'static [RoleSpec] {
        ROLES
    }

    fn fields(&self, objtype: &str) -> &'static [DocField] {
        match objtype {
            "function" | "macro" => FUNCTION_FIELDS,
            _ => &[],
        }
    }

    fn signature(
        &self,
        cx: &mut Cx<'_, '_, '_>,
        o: &mut Object<'_>,
        sig: &str,
        span: SourceSpan,
    ) -> Result<(), Fail> {
        c_signature(cx, o, sig, span)
    }

    fn run(
        &self,
        sx: &Sphinx<'_>,
        p: &mut Parser<'_, '_>,
        d: &Directive<'_>,
        tag: u16,
    ) -> Result<(), Fail> {
        match tag {
            ALIAS => alias_directive(p, d),
            _ => {
                namespace_directive(sx, p, d, false);
                Ok(())
            }
        }
    }

    /// As in Sphinx's CObject.before_content, every object is the parent of what its
    /// content declares and the scope its references are looked up from: a function's
    /// parameters are found by name there.
    fn before_content(&self, cx: &mut Cx<'_, '_, '_>, o: &Object<'_>) -> u32 {
        u32::from(cx.st.c_scope.push(&o.fullname))
    }

    fn after_content(&self, st: &mut State, _o: &Object<'_>, pushed: u32) {
        if pushed & 1 != 0 {
            st.c_scope.pop();
        }
    }

    fn toc_name(
        &self,
        config: &Config,
        _st: &State,
        o: &Object<'_>,
        names: TocNames<'_>,
    ) -> Option<String> {
        let callable = o.objtype == "function"
            || o.objtype == "method"
            || (o.objtype == "macro" && names.sig.contains('('));
        let parens = if callable && config.add_function_parentheses { "()" } else { "" };
        let name = last_part(names.fullname, ".");
        Some(match config.toc_object_entries_show_parents {
            TocParents::Hide | TocParents::Domain => format!("{name}{parens}"),
            TocParents::All => format!("{}{parens}", names.fullname.replace('.', "::")),
        })
    }

    fn xref_context(
        &self,
        b: &mut Builder<'_>,
        st: &State,
        id: NodeId,
        x: &XrefInfo<'_>,
    ) -> Result<(), Fail> {
        if !x.any || !st.c_namespace.is_empty() {
            let scope = scope_name(st, false);
            if !scope.is_empty() {
                b.add_custom_attribute(id, "c:parent", scope)?;
            }
        }
        Ok(())
    }
}

/// Tells whether content nested in an object of a type is in its scope.
pub(crate) fn c_scope_kind(objtype: &str) -> bool {
    matches!(objtype, "struct" | "union" | "enum" | "class" | "enum-struct" | "enum-class")
}

/// The namespace and the enclosing objects: the innermost scope, as C++ "a::b" or C
/// "a.b".
pub(crate) fn scope_name(st: &State, cpp: bool) -> &str {
    let (ns, stack) =
        if cpp { (&st.cpp_namespace, &st.cpp_scope) } else { (&st.c_namespace, &st.c_scope) };
    if stack.len() > 0 { stack.top() } else { ns }
}

/// Emits a C declaration.
fn c_signature(
    cx: &mut Cx<'_, '_, '_>,
    o: &mut Object<'_>,
    sig: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let Some(d) = decl::parse_c_decl(sig, o.objtype) else {
        cx.report(SIGNATURE, span);
        return cx.plain_signature(sig, span);
    };
    let scope = scope_name(cx.st, false).to_owned();
    let local = format!("{}{}", d.qualifier, d.name);
    let full = if scope.is_empty() { local } else { format!("{scope}.{local}") };
    let id = cx.b().begin_node(NodeKind::DescSignature, span)?;
    if !o.noindex {
        let made = format!("c.{full}");
        cx.b().add_attribute_string(id, guidedoc::AttributeKey::Id, &made)?;
        cx.st.note_id(&made);
        cx.st.object_id = made;
    }
    cx.custom(id, "fullname", &full)?;
    cx.toc_entry_name(o, id, TocNames { fullname: &full, module: "", sig })?;
    if o.objtype == "function"
        && let Some(params) = d.params
    {
        links::c_function_params(cx.b(), id, params)?;
    }
    let mut prefix = c_prefix(o.objtype);
    if o.objtype == "type" && decl::c_typedef(sig) {
        prefix = "typedef ";
    }
    if !prefix.is_empty() && !decl::has_word(d.before, trim_suffix(prefix, " ")) {
        cx.leaf_in(NodeKind::DescAnnotation, span, prefix)?;
    }
    signature_parts(cx, o, &d, &full, &scope, span)?;
    cx.b().end_node(id)?;
    o.signatures += 1;
    o.entry("single", &[&full, " (C ", c_display_type(o.objtype), ")"]);
    o.fullname = full;
    Ok(())
}

/// Emits the parts of a declaration after its prefix: C names in types,
/// qualifications, and parameters link, looked up from the symbol declared (a macro's
/// parameters are names, not types).
fn signature_parts(
    cx: &mut Cx<'_, '_, '_>,
    o: &Object<'_>,
    d: &decl::CDecl<'_>,
    full: &str,
    scope: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let links = if o.objtype == "macro" { "" } else { full };
    let b = cx.b();
    links::c_linked(b, span, d.before, full, "")?;
    if !d.qualifier.is_empty() {
        links::c_qualifier(b, span, d.qualifier, scope)?;
    }
    super::leaf_in(b, NodeKind::DescName, span, &anonymous_shown(d.name))?;
    if let Some(params) = d.params {
        links::c_params(b, params, span, links)?;
    }
    links::c_suffix_nodes(b, d.mid, span, links)?;
    links::c_linked(b, span, d.after, full, "")
}

/// A name as Sphinx shows it: each anonymous part ("@data") is "[anonymous]", as
/// ASTIdentifier.get_display_string gives it.
pub(crate) fn anonymous_shown(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let b = name.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'@' {
            let ch = name[i..].chars().next().unwrap_or('\0');
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        out.push_str("[anonymous]");
        i += 1;
        while i < b.len() && crate::text::is_identifier_byte(b[i]) {
            i += 1;
        }
    }
    out
}

fn c_display_type(objtype: &str) -> &str {
    if objtype == "var" { "member" } else { objtype }
}

/// The keyword Sphinx shows before C's struct-like declarations.
fn c_prefix(objtype: &str) -> &'static str {
    match objtype {
        "struct" => "struct ",
        "union" => "union ",
        "enum" => "enum ",
        "type" => "type ",
        _ => "",
    }
}

/// c:alias and cpp:alias repeat declarations documented elsewhere; the host expands
/// Directive{Name; names = one name per line, maxdepth, noroot}.
pub(crate) fn alias_directive(p: &mut Parser<'_, '_>, d: &Directive<'_>) -> Result<(), Fail> {
    let name = d.handler().map_or(d.name, |h| h.name);
    let id = p.directive_node(d, name)?;
    let names = p.pool_lines(&d.arg, "\n")?;
    let key = p.ctx.b.add_text("names")?;
    p.ctx.b.add_attribute_named(id, guidedoc::AttributeKey::Custom, names, key)?;
    crate::directives::value_attribute(p, id, d, "maxdepth")?;
    crate::directives::flag_attribute(p, id, d, "noroot")?;
    p.close(id)
}

/// c:namespace and cpp:namespace set the scope of later declarations ("NULL", "0", or
/// "nullptr" reset it); -push and -pop nest them.
pub(crate) fn namespace_directive(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
    cpp: bool,
) {
    let full = d.handler().map_or(d.name, |h| h.name);
    let local = full.split_once(':').map_or(full, |(_, l)| l);
    let arg = d.argument();
    let mut name = trim_space(&arg);
    if cpp {
        super::cpp::cpp_namespace(sx, p, local, name, d);
    }
    if matches!(name, "NULL" | "0" | "nullptr") {
        name = "";
    }
    let sep = if cpp { "::" } else { "." };
    let mut st = sx.state();
    let st = &mut *st;
    let (current, stack) = if cpp {
        (&mut st.cpp_namespace, &mut st.cpp_namespaces)
    } else {
        (&mut st.c_namespace, &mut st.c_namespaces)
    };
    match local {
        "namespace" => {
            stack.clear();
            *current = name.to_owned();
        }
        "namespace-push" => {
            if !stack.push(current) {
                p.report(
                    NESTING,
                    d.name_span,
                    guidedoc::arg_span(d.name_span),
                    guidedoc::Arg::None,
                );
                return;
            }
            if !current.is_empty() && !name.is_empty() {
                *current = format!("{current}{sep}{name}");
            } else if !name.is_empty() {
                *current = name.to_owned();
            }
        }
        _ => {
            if stack.len() > 0 {
                *current = stack.top().to_owned();
                stack.pop();
            } else {
                current.clear();
            }
        }
    }
}

/// :c:expr: and :c:texpr: as Sphinx's CExprRole describes them: Role{"c-expr" or
/// "c-texpr"} > the text with its names linked, looked up from the object the role
/// stands in.
pub(crate) fn expr_role(
    rx: &mut RoleCx<'_, '_>,
    call: &RoleCall<'_>,
    code: bool,
) -> Result<(), Fail> {
    let scope = scope_name(rx.st, false).to_owned();
    let b = rx.b();
    let id = b.begin_node(NodeKind::Role, call.span)?;
    let t = b.add_text(if code { "c-expr" } else { "c-texpr" })?;
    b.set_text(id, t);
    links::c_linked(b, call.span, trim_space(call.text), &scope, "")?;
    b.end_node(id)
}
