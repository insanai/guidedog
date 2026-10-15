//! The JavaScript domain: js:module and the object descriptions (function, method,
//! class, data, attribute), their roles and info fields.

use guidedoc::{Builder, Fail, Label, NodeId, NodeKind, SourceSpan};
use guidedoc_rst::{ArgRule, ContentRule, Directive, OptSpec, OptType, Parser, opt};

use super::object::OPTS_OBJECT;
use super::python::{dotted_toc_name, module_target, pseudo_arglist};
use super::{
    Cx, DocField, Domain, DomainDirective, FieldShape, Object, RoleSpec, TocNames, XrefFlags,
    XrefInfo,
};
use crate::config::Config;
use crate::extension::Sphinx;
use crate::state::State;
use crate::text::trim_space;

static OPTS_JS_MODULE: &[OptSpec] = &[
    opt("no-index", OptType::Flag),
    opt("noindex", OptType::Flag),
    opt("no-contents-entry", OptType::Flag),
    opt("nocontentsentry", OptType::Flag),
    opt("no-typesetting", OptType::Flag),
];

static DIRECTIVES: &[DomainDirective] = &[
    DomainDirective::run(
        "js:module",
        0,
        (ArgRule::Required, false, ContentRule::Optional),
        OPTS_JS_MODULE,
    ),
    DomainDirective::object("js:function", OPTS_OBJECT),
    DomainDirective::object("js:method", OPTS_OBJECT),
    DomainDirective::object("js:class", OPTS_OBJECT),
    DomainDirective::object("js:data", OPTS_OBJECT),
    DomainDirective::object("js:attribute", OPTS_OBJECT),
];

const DOTTED: XrefFlags = XrefFlags::CODE.with(XrefFlags::DOTTED);

static ROLES: &[RoleSpec] = &[
    RoleSpec::xref("func", DOTTED.with(XrefFlags::PARENS)),
    RoleSpec::xref("meth", DOTTED.with(XrefFlags::PARENS)),
    RoleSpec::xref("class", DOTTED),
    RoleSpec::xref("data", DOTTED),
    RoleSpec::xref("attr", DOTTED),
    RoleSpec::xref("mod", DOTTED),
];

static CALLABLE_FIELDS: &[DocField] = &[
    DocField {
        name: "arguments",
        shape: FieldShape::Typed,
        label: Label::Arguments,
        has_arg: true,
        names: "argument|arg|parameter|param",
        typenames: "paramtype|type",
        typerole: "class",
        ..DocField::DEFAULT
    },
    DocField {
        name: "errors",
        shape: FieldShape::Grouped,
        label: Label::Throws,
        has_arg: true,
        names: "throws",
        role: "class",
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

/// The JavaScript domain.
#[derive(Debug)]
pub(crate) struct JsDomain;

/// The value `DOMAINS` registers.
pub(crate) static DOMAIN: JsDomain = JsDomain;

impl Domain for JsDomain {
    fn name(&self) -> &'static str {
        "js"
    }

    fn directives(&self) -> &'static [DomainDirective] {
        DIRECTIVES
    }

    fn roles(&self) -> &'static [RoleSpec] {
        ROLES
    }

    fn fields(&self, objtype: &str) -> &'static [DocField] {
        match objtype {
            "function" | "method" | "class" => CALLABLE_FIELDS,
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
        js_signature(cx, o, sig, span)
    }

    /// js:module: its target, then the content; it sets the current module.
    fn run(
        &self,
        sx: &Sphinx<'_>,
        p: &mut Parser<'_, '_>,
        d: &Directive<'_>,
        _tag: u16,
    ) -> Result<(), Fail> {
        let name = trim_space(&d.argument()).to_owned();
        sx.state().js_module.clone_from(&name);
        sx.with_cx(p, |cx| module_target(cx, d, "js", &name, ("single: ", " (module)")))?;
        p.parse_body(&d.content, d.sections, None)
    }

    /// As Sphinx's JSConstructor, a class nests: what it contains is in it. Any other
    /// object makes its own prefix the object its content is in (JSObject), so what a
    /// function contains is its sibling, and references there look up from its parent.
    fn before_content(&self, cx: &mut Cx<'_, '_, '_>, o: &Object<'_>) -> u32 {
        let top = if o.objtype == "class" { &o.fullname } else { &o.prefix };
        u32::from(!top.is_empty() && cx.st.js_objects.push(top))
    }

    fn after_content(&self, st: &mut State, _o: &Object<'_>, pushed: u32) {
        if pushed & 1 != 0 {
            st.js_objects.pop();
        }
    }

    fn toc_name(
        &self,
        config: &Config,
        _st: &State,
        o: &Object<'_>,
        names: TocNames<'_>,
    ) -> Option<String> {
        Some(dotted_toc_name(config, o, names))
    }

    fn xref_context(
        &self,
        b: &mut Builder<'_>,
        st: &State,
        id: NodeId,
        _x: &XrefInfo<'_>,
    ) -> Result<(), Fail> {
        if !st.js_module.is_empty() {
            b.add_custom_attribute(id, "js:module", &st.js_module)?;
        }
        let top = st.js_objects.top();
        if !top.is_empty() {
            b.add_custom_attribute(id, "js:object", top)?;
        }
        Ok(())
    }
}

/// "[prefix.]name[(arguments)]"; classes show "class", callables an argument list; ids
/// are the full dotted name, with the module.
fn js_signature(
    cx: &mut Cx<'_, '_, '_>,
    o: &mut Object<'_>,
    sig: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let (mut member, mut arglist) = (sig, None);
    if let Some(paren) = sig.find('(')
        && sig.ends_with(')')
    {
        member = trim_space(&sig[..paren]);
        arglist = Some(trim_space(&sig[paren + 1..sig.len() - 1]));
    }
    let (member_prefix, name) = member.rsplit_once('.').unwrap_or(("", member));
    let mut prefix = cx.st.js_objects.top().to_owned();
    if !member_prefix.is_empty() {
        prefix = if prefix.is_empty() {
            member_prefix.to_owned()
        } else {
            format!("{prefix}.{member_prefix}")
        };
    }
    let fullname = if prefix.is_empty() { name.to_owned() } else { format!("{prefix}.{name}") };
    let module = cx.st.js_module.clone();
    let full = if module.is_empty() { fullname.clone() } else { format!("{module}.{fullname}") };
    let id = cx.b().begin_node(NodeKind::DescSignature, span)?;
    if !o.noindex {
        cx.add_ids(id, "", &full, &full)?;
    }
    cx.custom(id, "fullname", &fullname)?;
    if !module.is_empty() {
        cx.custom(id, "module", &module)?;
    }
    cx.toc_entry_name(o, id, TocNames { fullname: &fullname, module: &module, sig })?;
    if !prefix.is_empty() {
        cx.custom(id, "object", &prefix)?;
    }
    if o.objtype == "class" {
        cx.leaf_in(NodeKind::DescAnnotation, span, "class ")?;
    }
    let shown = if prefix.is_empty() { &module } else { &prefix };
    if !shown.is_empty() {
        cx.leaf_in(NodeKind::DescAddname, span, &format!("{shown}."))?;
    }
    cx.leaf_in(NodeKind::DescName, span, name)?;
    if matches!(o.objtype, "function" | "method" | "class") {
        match arglist {
            Some(args) if !args.is_empty() => pseudo_arglist(cx.b(), args, span)?,
            _ => cx.leaf_in(NodeKind::DescParameterlist, span, "")?,
        }
    }
    cx.b().end_node(id)?;
    o.signatures += 1;
    js_entry(o, name, &prefix);
    o.fullname = fullname;
    o.prefix = prefix;
    Ok(())
}

/// Adds Sphinx's index entry for a JavaScript object.
fn js_entry(o: &mut Object<'_>, name: &str, obj: &str) {
    match o.objtype {
        "function" if obj.is_empty() => o.entry("single", &[name, "() (built-in function)"]),
        "function" | "method" => o.entry("single", &[name, "() (", obj, " method)"]),
        "class" => o.entry("single", &[name, "() (class)"]),
        "data" => o.entry("single", &[name, " (global variable or constant)"]),
        "attribute" => o.entry("single", &[name, " (", obj, " attribute)"]),
        _ => {}
    }
}
