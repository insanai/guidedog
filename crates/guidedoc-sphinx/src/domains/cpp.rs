//! The C++ domain: declarations parsed as Sphinx parses them (`guidedoc-cpp`), their
//! signatures described with every name a type or expression uses as a link
//! (`describe`), symbol scopes for nested content, and the lookup data cross-references
//! carry.

mod describe;

use guidedoc::{
    Arg, AttributeKey, Builder, CORE_DEPTH, CORE_STACK, CORE_WORK, Fail, Label, NodeId, NodeKind,
    SourceSpan, arg_int, diagnostic,
};
use guidedoc_cpp::{CppError, ErrorKind, Limits, Stop, Symbol};
use guidedoc_rst::{ArgRule, ContentRule, Directive, Parser};
use guidedoc_rst_inline::RoleCall;

use super::c::{ALIAS, OPTS_ALIAS, alias_directive, c_scope_kind, namespace_directive};
use super::object::OPTS_OBJECT;
use super::{
    Cx, DocField, Domain, DomainDirective, FieldShape, Object, RoleSpec, TocNames, XrefFlags,
    XrefInfo,
};
use crate::config::{Config, TocParents};
use crate::extension::Sphinx;
use crate::messages::SIGNATURE;
use crate::roles::{RoleCx, RoleKind};
use crate::state::{CppScope, State};
use crate::text::last_part;

const NAMESPACE: u16 = super::c::NAMESPACE;
const ONE_ARG: (ArgRule, bool, ContentRule) = (ArgRule::Required, true, ContentRule::None);
const NO_ARG: (ArgRule, bool, ContentRule) = (ArgRule::None, false, ContentRule::None);

static DIRECTIVES: &[DomainDirective] = &[
    DomainDirective::object("cpp:class", OPTS_OBJECT),
    DomainDirective::object("cpp:struct", OPTS_OBJECT),
    DomainDirective::object("cpp:union", OPTS_OBJECT),
    DomainDirective::object("cpp:function", OPTS_OBJECT),
    DomainDirective::object("cpp:member", OPTS_OBJECT),
    DomainDirective::object("cpp:var", OPTS_OBJECT),
    DomainDirective::object("cpp:type", OPTS_OBJECT),
    DomainDirective::object("cpp:concept", OPTS_OBJECT),
    DomainDirective::object("cpp:enum", OPTS_OBJECT),
    DomainDirective::object("cpp:enum-struct", OPTS_OBJECT),
    DomainDirective::object("cpp:enum-class", OPTS_OBJECT),
    DomainDirective::object("cpp:enumerator", OPTS_OBJECT),
    DomainDirective::run("cpp:alias", ALIAS, ONE_ARG, OPTS_ALIAS),
    DomainDirective::run("cpp:namespace", NAMESPACE, ONE_ARG, &[]),
    DomainDirective::run("cpp:namespace-push", NAMESPACE, ONE_ARG, &[]),
    DomainDirective::run("cpp:namespace-pop", NAMESPACE, NO_ARG, &[]),
];

const SCOPED: XrefFlags = XrefFlags::CODE.with(XrefFlags::SCOPED);

static ROLES: &[RoleSpec] = &[
    RoleSpec::xref("any", SCOPED),
    RoleSpec::xref("class", SCOPED),
    RoleSpec::xref("struct", SCOPED),
    RoleSpec::xref("union", SCOPED),
    RoleSpec::xref("func", SCOPED.with(XrefFlags::PARENS)),
    RoleSpec::xref("member", SCOPED),
    RoleSpec::xref("var", SCOPED),
    RoleSpec::xref("type", SCOPED),
    RoleSpec::xref("concept", SCOPED),
    RoleSpec::xref("enum", SCOPED),
    RoleSpec::xref("enumerator", SCOPED),
    RoleSpec::of("expr", RoleKind::Expr),
    RoleSpec::of("texpr", RoleKind::Texpr),
];

const TPARAM: DocField = DocField {
    name: "template parameter",
    shape: FieldShape::Grouped,
    label: Label::TemplateParameters,
    has_arg: true,
    names: "tparam|template parameter",
    collapse: true,
    ..DocField::DEFAULT
};

/// Every C++ object's fields. Sphinx lists "template parameter" too, which no field
/// name can match, since the name ends at the first space.
static FIELDS: &[DocField] = &[TPARAM];

static FUNCTION_FIELDS: &[DocField] = &[
    TPARAM,
    DocField {
        name: "parameter",
        shape: FieldShape::Grouped,
        label: Label::Parameters,
        has_arg: true,
        names: "param|parameter|arg|argument",
        collapse: true,
        ..DocField::DEFAULT
    },
    DocField {
        name: "exceptions",
        shape: FieldShape::Grouped,
        label: Label::Throws,
        has_arg: true,
        names: "throws|throw|exception",
        role: "expr",
        collapse: true,
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
];

/// The C++ domain.
#[derive(Debug)]
pub(crate) struct CppDomain;

/// The value `DOMAINS` registers.
pub(crate) static DOMAIN: CppDomain = CppDomain;

impl Domain for CppDomain {
    fn name(&self) -> &'static str {
        "cpp"
    }

    fn directives(&self) -> &'static [DomainDirective] {
        DIRECTIVES
    }

    fn roles(&self) -> &'static [RoleSpec] {
        ROLES
    }

    fn fields(&self, objtype: &str) -> &'static [DocField] {
        if objtype == "function" { FUNCTION_FIELDS } else { FIELDS }
    }

    fn signature(
        &self,
        cx: &mut Cx<'_, '_, '_>,
        o: &mut Object<'_>,
        sig: &str,
        span: SourceSpan,
    ) -> Result<(), Fail> {
        cpp_signature(cx, o, sig, span)
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
                // A limit fails the builder, which the next call reports.
                namespace_directive(sx, p, d, true);
                Ok(())
            }
        }
    }

    fn before_content(&self, cx: &mut Cx<'_, '_, '_>, o: &Object<'_>) -> u32 {
        let st = &mut *cx.st;
        let mut pushed = 0;
        if c_scope_kind(o.objtype) && st.cpp_scope.push(&o.fullname) {
            pushed |= 1;
        }
        if st.cpp_objects.push(&cpp_object_name(&o.fullname)) {
            pushed |= 2;
        }
        // Content is declared in the last signature's symbol, whatever it declares.
        if scope_push(st, &o.cpp_path, &o.cpp_objtype, o.objtype) {
            pushed |= 4;
        }
        pushed
    }

    fn after_content(&self, st: &mut State, _o: &Object<'_>, pushed: u32) {
        if pushed & 1 != 0 {
            st.cpp_scope.pop();
        }
        if pushed & 2 != 0 {
            st.cpp_objects.pop();
        }
        if pushed & 4 != 0 {
            scope_pop(st);
        }
    }

    /// The names of the objects the declaration is nested in, joined with "::";
    /// template arguments are not part of a name.
    fn toc_name(
        &self,
        config: &Config,
        st: &State,
        o: &Object<'_>,
        names: TocNames<'_>,
    ) -> Option<String> {
        let callable = o.objtype == "function" || o.objtype == "method";
        let parens = if callable && config.add_function_parentheses { "()" } else { "" };
        let name = strip_template(last_part(names.fullname, "::"));
        Some(match config.toc_object_entries_show_parents {
            TocParents::Hide => format!("{name}{parens}"),
            TocParents::Domain => {
                let mut out = String::new();
                for part in st.cpp_objects.items() {
                    out.push_str(part);
                    out.push_str("::");
                }
                format!("{out}{name}{parens}")
            }
            TocParents::All => {
                let parts: Vec<&str> = names.fullname.split("::").map(strip_template).collect();
                format!("{}{parens}", parts.join("::"))
            }
        })
    }

    fn xref_context(
        &self,
        b: &mut Builder<'_>,
        st: &State,
        id: NodeId,
        x: &XrefInfo<'_>,
    ) -> Result<(), Fail> {
        if !st.cpp_scope_path.is_empty() {
            // The symbol it is looked up from.
            b.add_custom_attribute(id, "cpp:parent", &st.cpp_scope_path)?;
        }
        if st.external || x.reftype == "cpp:expr" || x.reftype == "cpp:texpr" {
            return Ok(());
        }
        let mut target = x.target;
        // CPPXRefRole.process_link drops "()" from an any role's target.
        if x.reftype == "cpp:any" {
            target = target.strip_suffix("()").unwrap_or(target);
        }
        let parens = x.any || x.reftype == "cpp:func" || x.reftype == "cpp:any";
        xref_target(b, id, target, parens, role_limits(b))
    }
}

/// The C++ parser's bounds where no parser is at hand (roles): the depth limit and half
/// the stack budget.
fn role_limits(b: &Builder<'_>) -> Limits {
    Limits { max_depth: b.limits.max_depth, stack_bytes: b.limits.stack_bytes.max(1) / 2 }
}

/// The bounds for a parse inside the reader's guarded recursion: what remains of its
/// stack budget.
fn parser_limits(p: &Parser<'_, '_>) -> Limits {
    let guard = p.stack_guard();
    let remaining = guard.limit().saturating_sub(guard.used());
    Limits { max_depth: p.ctx.limits.max_depth, stack_bytes: remaining }
}

/// Fails the read for a parse that stopped at a limit; other errors are `Ok(error)`
/// for the caller to warn about.
fn limit_failure(b: &mut Builder<'_>, e: CppError, span: SourceSpan) -> Result<CppError, Fail> {
    let ErrorKind::Limit(stop) = e.kind else { return Ok(e) };
    let m = match stop {
        Stop::Depth => CORE_DEPTH,
        Stop::Stack => CORE_STACK,
        Stop::Work => CORE_WORK,
    };
    let limit = i64::try_from(e.limit).unwrap_or(i64::MAX);
    Err(b.fail(diagnostic(m, span, arg_int(limit), Arg::None)))
}

/// The name a C++ object adds to the entries of what is nested in it (Sphinx's
/// cpp_domain_name): its last part, without template arguments.
pub(crate) fn cpp_object_name(full: &str) -> String {
    strip_template(last_part(full, "::")).to_owned()
}

/// Leaves out a C++ name's template arguments ("vector<T>" is "vector"); an operator
/// keeps its symbol ("operator<").
fn strip_template(name: &str) -> &str {
    if name.starts_with("operator") {
        return name;
    }
    match name.find('<') {
        Some(k) if k > 0 => &name[..k],
        _ => name,
    }
}

/// The object type of a directive's local name.
fn cpp_object_type(objtype: &str) -> &str {
    match objtype {
        "class" | "struct" => "class",
        "member" | "var" => "member",
        "enum" | "enum-struct" | "enum-class" => "enum",
        other => other,
    }
}

/// Reads a C++ declaration as Sphinx's CPPObject.handle_signature and
/// add_target_and_index do: Desc_Signature{Id per id version, newest first (the older
/// ones only while free in the document, and none when the newest is taken), fullname,
/// "cpp:display" when the display differs, "cpp:path", "cpp:objtype", "cpp:id",
/// "cpp:tparams" and "cpp:fparams"} > the signature (`describe`).
fn cpp_signature(
    cx: &mut Cx<'_, '_, '_>,
    o: &mut Object<'_>,
    sig: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let Some(directive) = guidedoc_cpp::Directive::from_name(o.objtype) else {
        return cx.plain_signature(sig, span);
    };
    let limits = parser_limits(cx.p);
    let parsed = guidedoc_cpp::parse_declaration(sig, directive, &limits)
        .and_then(|decl| decl.symbol(&cx.st.cpp_scope_path).map(|symbol| (decl, symbol)));
    let (decl, symbol) = match parsed {
        Ok(found) => found,
        Err(e) => {
            limit_failure(cx.b(), e, span)?;
            cx.report(SIGNATURE, span);
            return cx.plain_signature(sig, span);
        }
    };
    let object_type = cpp_object_type(o.objtype);
    let id = cx.b().begin_node(NodeKind::DescSignature, span)?;
    if !o.noindex {
        add_cpp_ids(cx, id, &symbol)?;
    }
    cx.custom(id, "fullname", &symbol.full_name)?;
    if symbol.display_name != symbol.full_name {
        cx.custom(id, "cpp:display", &symbol.display_name)?;
    }
    cx.custom(id, "cpp:path", &symbol.path)?;
    cx.custom(id, "cpp:objtype", object_type)?;
    cx.custom(id, "cpp:id", symbol.newest_id())?;
    if !symbol.template_params.is_empty() {
        cx.custom(id, "cpp:tparams", &symbol.template_params.join("\n"))?;
    }
    if !symbol.function_params.is_empty() {
        cx.custom(id, "cpp:fparams", &symbol.function_params.join("\n"))?;
    }
    cx.toc_entry_name(o, id, TocNames { fullname: &symbol.full_name, module: "", sig })?;
    describe::describe_declaration(cx.b(), &decl, &symbol.path, span)?;
    cx.b().end_node(id)?;
    o.fullname.clone_from(&symbol.full_name);
    o.cpp_path.clone_from(&symbol.path);
    o.cpp_objtype = object_type.to_owned();
    o.signatures += 1;
    if !cx.st.cpp_in_concept {
        let kind = if object_type == "class" { o.objtype } else { object_type };
        o.entry("single", &[&symbol.display_name, " (C++ ", kind, ")"]);
    }
    Ok(())
}

/// Adds the ids, newest first, as add_target_and_index does.
fn add_cpp_ids(cx: &mut Cx<'_, '_, '_>, id: NodeId, symbol: &Symbol) -> Result<(), Fail> {
    let newest = symbol.newest_id().to_owned();
    cx.st.object_id.clone_from(&newest);
    if cx.st.id_taken(&newest) {
        return Ok(());
    }
    for (v, made) in symbol.ids.iter().enumerate().rev() {
        let Some(made) = made else { continue };
        if v < 3 && cx.st.id_taken(made) {
            continue;
        }
        cx.b().add_attribute_string(id, AttributeKey::Id, made)?;
        cx.st.note_id(made);
    }
    Ok(())
}

/// Sets the symbol later C++ declarations are declared in: the named namespace below
/// the root, or below the current symbol for namespace-push; pop goes back to the
/// namespace pushed before, or the root.
pub(crate) fn cpp_namespace(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    local: &str,
    name: &str,
    d: &Directive<'_>,
) {
    let limits = parser_limits(p);
    let mut st = sx.state();
    let path = match local {
        "namespace" => {
            st.cpp_namespace_paths.clear();
            let path = namespace_path(p, "", name, &limits, d);
            if !path.is_empty() {
                st.cpp_namespace_paths.push(&path);
            }
            path
        }
        "namespace-push" => {
            if matches!(name, "NULL" | "0" | "nullptr") {
                return;
            }
            let base = st.cpp_scope_path.clone();
            let path = namespace_path(p, &base, name, &limits, d);
            st.cpp_namespace_paths.push(&path);
            path
        }
        _ => {
            st.cpp_namespace_paths.pop();
            st.cpp_namespace_paths.top().to_owned()
        }
    };
    st.cpp_scope_path = path;
    st.cpp_parent_objtype.clear();
    st.cpp_parent_directive.clear();
}

/// The path of a namespace name below `base`; `base` (with a warning) when the name
/// does not parse.
fn namespace_path(
    p: &mut Parser<'_, '_>,
    base: &str,
    name: &str,
    limits: &Limits,
    d: &Directive<'_>,
) -> String {
    match guidedoc_cpp::namespace_path(base, name, limits) {
        Ok(path) => path,
        Err(e) => {
            if limit_failure(&mut p.ctx.b, e, d.span).is_ok() {
                p.report(SIGNATURE, d.span, guidedoc::arg_span(d.span), Arg::None);
            }
            base.to_owned()
        }
    }
}

/// Saves the scope C++ declarations are declared in and enters the symbol an object
/// declared (CPPObject.before_content); `scope_pop` goes back.
fn scope_push(st: &mut State, path: &str, objtype: &str, directive: &str) -> bool {
    if st.cpp_scopes.len() >= crate::state::MAX_NEST {
        return false;
    }
    st.cpp_scopes.push(CppScope {
        path: std::mem::replace(&mut st.cpp_scope_path, path.to_owned()),
        objtype: std::mem::replace(&mut st.cpp_parent_objtype, objtype.to_owned()),
        directive: std::mem::replace(&mut st.cpp_parent_directive, directive.to_owned()),
        in_concept: st.cpp_in_concept,
    });
    st.cpp_in_concept = st.cpp_in_concept || objtype == "concept";
    true
}

fn scope_pop(st: &mut State) {
    let saved = st.cpp_scopes.pop().unwrap_or_default();
    st.cpp_scope_path = saved.path;
    st.cpp_parent_objtype = saved.objtype;
    st.cpp_parent_directive = saved.directive;
    st.cpp_in_concept = saved.in_concept;
}

/// Records on an Xref (right after its other attributes) how the host looks a C++
/// target up: "cpp:name" (the names, one per line, identifier and template arguments
/// apart by U+001F), "cpp:rooted", "cpp:tdecls", and for a whole function declaration
/// "cpp:query" and "cpp:own"; "cpp:unparsed" when the target does not parse, as Sphinx
/// then warns and leaves it unresolved.
pub(crate) fn xref_target(
    b: &mut Builder<'_>,
    id: NodeId,
    target: &str,
    parens: bool,
    limits: Limits,
) -> Result<(), Fail> {
    let x = match guidedoc_cpp::parse_xref_target(target, parens, &limits) {
        Ok(x) => x,
        Err(e) => {
            let span = b.snap.nodes.get(id.index()).map(|n| n.span).unwrap_or_default();
            limit_failure(b, e, span)?;
            return b.add_custom_attribute(id, "cpp:unparsed", "1");
        }
    };
    b.add_custom_attribute(id, "cpp:name", &x.encoded_names())?;
    if x.is_rooted {
        b.add_custom_attribute(id, "cpp:rooted", "1")?;
    }
    if !x.template_decls.is_empty() {
        b.add_custom_attribute(id, "cpp:tdecls", &x.encoded_template_decls())?;
    }
    if let Some(query) = &x.query {
        b.add_custom_attribute(id, "cpp:query", query)?;
        if let Some(own) = &x.own {
            b.add_custom_attribute(id, "cpp:own", own)?;
        }
    }
    Ok(())
}

/// :cpp:expr: and :cpp:texpr:: Role{"cpp-expr" or "cpp-texpr"} > the expression or type
/// described; false when it does not parse.
pub(crate) fn expr_role(
    rx: &mut RoleCx<'_, '_>,
    call: &RoleCall<'_>,
    code: bool,
) -> Result<bool, Fail> {
    let limits = role_limits(rx.b());
    let expr = match guidedoc_cpp::parse_expression_or_type(call.text, &limits) {
        Ok(expr) => expr,
        Err(e) => {
            limit_failure(rx.b(), e, call.span)?;
            return Ok(false);
        }
    };
    let key = rx.st.cpp_scope_path.clone();
    let b = rx.b();
    let id = b.begin_node(NodeKind::Role, call.span)?;
    let t = b.add_text(if code { "cpp-expr" } else { "cpp-texpr" })?;
    b.set_text(id, t);
    describe::describe_expression(b, &expr, &key, call.span)?;
    b.end_node(id)?;
    Ok(true)
}
