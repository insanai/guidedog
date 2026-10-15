//! The Python domain: py:module and py:currentmodule, the object descriptions
//! (function, class, method, ...), their roles and info fields.

mod params;

use guidedoc::{Builder, Fail, Label, NodeId, NodeKind, SourceSpan};
use guidedoc_rst::{ArgRule, ContentRule, Directive, OptSpec, OptType, Parser, opt};

pub(crate) use params::pseudo_arglist;

use super::object::join_opts;
use super::{
    Cx, DocField, Domain, DomainDirective, FieldShape, Object, RoleSpec, TocNames, XrefFlags,
    XrefInfo, XrefParts,
};
use crate::config::{Config, TocParents};
use crate::extension::Sphinx;
use crate::state::State;
use crate::text::{has_at, is_identifier_byte, last_part, trim_space, trim_suffix};

/// The options every Python object accepts first.
const COMMON: [OptSpec; 7] = [
    opt("no-index", OptType::Flag),
    opt("noindex", OptType::Flag),
    opt("no-index-entry", OptType::Flag),
    opt("noindexentry", OptType::Flag),
    opt("no-contents-entry", OptType::Flag),
    opt("nocontentsentry", OptType::Flag),
    opt("no-typesetting", OptType::Flag),
];

const SINGLE_LINE: OptSpec = opt("single-line-parameter-list", OptType::Flag);
const SINGLE_LINE_TYPE: OptSpec = opt("single-line-type-parameter-list", OptType::Flag);
const MODULE: OptSpec = opt("module", OptType::Text);
const CANONICAL: OptSpec = opt("canonical", OptType::Text);
const ANNOTATION: OptSpec = opt("annotation", OptType::Text);

static OPTS_PY_OBJECT: [OptSpec; 12] =
    join_opts(COMMON, [SINGLE_LINE, SINGLE_LINE_TYPE, MODULE, CANONICAL, ANNOTATION]);
static OPTS_PY_FUNCTION: [OptSpec; 13] = join_opts(
    COMMON,
    [SINGLE_LINE, SINGLE_LINE_TYPE, MODULE, CANONICAL, ANNOTATION, opt("async", OptType::Flag)],
);
static OPTS_PY_DATA: [OptSpec; 12] = join_opts(
    COMMON,
    [MODULE, CANONICAL, ANNOTATION, opt("type", OptType::Text), opt("value", OptType::Text)],
);
static OPTS_PY_CLASS: [OptSpec; 13] = join_opts(
    COMMON,
    [SINGLE_LINE, SINGLE_LINE_TYPE, MODULE, CANONICAL, ANNOTATION, opt("final", OptType::Flag)],
);
static OPTS_PY_METHOD: [OptSpec; 18] = join_opts(
    COMMON,
    [
        SINGLE_LINE,
        SINGLE_LINE_TYPE,
        MODULE,
        CANONICAL,
        ANNOTATION,
        opt("abstractmethod", OptType::Flag),
        opt("abstract", OptType::Flag),
        opt("async", OptType::Flag),
        opt("classmethod", OptType::Flag),
        opt("final", OptType::Flag),
        opt("staticmethod", OptType::Flag),
    ],
);
static OPTS_PY_PROPERTY: [OptSpec; 14] = join_opts(
    COMMON,
    [
        MODULE,
        CANONICAL,
        ANNOTATION,
        opt("abstractmethod", OptType::Flag),
        opt("abstract", OptType::Flag),
        opt("classmethod", OptType::Flag),
        opt("type", OptType::Text),
    ],
);
static OPTS_PY_TYPE: [OptSpec; 11] =
    join_opts(COMMON, [MODULE, CANONICAL, ANNOTATION, opt("value", OptType::Text)]);
static OPTS_PY_MODULE: &[OptSpec] = &[
    opt("platform", OptType::Text),
    opt("synopsis", OptType::Text),
    opt("no-index", OptType::Flag),
    opt("noindex", OptType::Flag),
    opt("no-index-entry", OptType::Flag),
    opt("noindexentry", OptType::Flag),
    opt("no-contents-entry", OptType::Flag),
    opt("nocontentsentry", OptType::Flag),
    opt("no-typesetting", OptType::Flag),
    opt("deprecated", OptType::Flag),
];

const MODULE_TAG: u16 = 0;
const CURRENTMODULE_TAG: u16 = 1;

static DIRECTIVES: &[DomainDirective] = &[
    DomainDirective::run(
        "py:module",
        MODULE_TAG,
        (ArgRule::Required, false, ContentRule::Optional),
        OPTS_PY_MODULE,
    ),
    DomainDirective::run(
        "py:currentmodule",
        CURRENTMODULE_TAG,
        (ArgRule::Required, false, ContentRule::None),
        &[],
    ),
    DomainDirective::object("py:function", &OPTS_PY_FUNCTION),
    DomainDirective::object("py:data", &OPTS_PY_DATA),
    DomainDirective::object("py:exception", &OPTS_PY_CLASS),
    DomainDirective::object("py:class", &OPTS_PY_CLASS),
    DomainDirective::object("py:attribute", &OPTS_PY_DATA),
    DomainDirective::object("py:property", &OPTS_PY_PROPERTY),
    DomainDirective::object("py:method", &OPTS_PY_METHOD),
    DomainDirective::object("py:staticmethod", &OPTS_PY_METHOD),
    DomainDirective::object("py:classmethod", &OPTS_PY_METHOD),
    DomainDirective::object("py:decorator", &OPTS_PY_OBJECT),
    DomainDirective::object("py:decoratormethod", &OPTS_PY_OBJECT),
    DomainDirective::object("py:type", &OPTS_PY_TYPE),
];

const DOTTED: XrefFlags = XrefFlags::CODE.with(XrefFlags::DOTTED);

static ROLES: &[RoleSpec] = &[
    RoleSpec::xref("data", DOTTED),
    RoleSpec::xref("exc", DOTTED),
    RoleSpec::xref("func", DOTTED.with(XrefFlags::PARENS)),
    RoleSpec::xref("class", DOTTED),
    RoleSpec::xref("const", DOTTED),
    RoleSpec::xref("attr", DOTTED),
    RoleSpec::xref("type", DOTTED),
    RoleSpec::xref("meth", DOTTED.with(XrefFlags::PARENS)),
    RoleSpec::xref("mod", DOTTED),
    RoleSpec::xref("obj", DOTTED),
    RoleSpec::xref("deco", DOTTED.with(XrefFlags::AT)),
];

static FIELDS: &[DocField] = &[
    DocField {
        name: "parameter",
        shape: FieldShape::Typed,
        label: Label::Parameters,
        has_arg: true,
        names: "param|parameter|arg|argument|keyword|kwarg|kwparam",
        typenames: "paramtype|type",
        typerole: "class",
        collapse: true,
        python: true,
        ..DocField::DEFAULT
    },
    DocField {
        name: "variable",
        shape: FieldShape::Typed,
        label: Label::Variables,
        has_arg: true,
        names: "var|ivar|cvar",
        typenames: "vartype",
        typerole: "class",
        collapse: true,
        python: true,
        ..DocField::DEFAULT
    },
    DocField {
        name: "exceptions",
        shape: FieldShape::Grouped,
        label: Label::Raises,
        has_arg: true,
        names: "raises|raise|exception|except",
        role: "exc",
        collapse: true,
        python: true,
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
        bodyrole: "class",
        python: true,
        ..DocField::DEFAULT
    },
];

/// The Python domain.
#[derive(Debug)]
pub(crate) struct PythonDomain;

/// The value `DOMAINS` registers.
pub(crate) static DOMAIN: PythonDomain = PythonDomain;

impl Domain for PythonDomain {
    fn name(&self) -> &'static str {
        "py"
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
        py_signature(cx, o, sig, span)
    }

    fn run(
        &self,
        sx: &Sphinx<'_>,
        p: &mut Parser<'_, '_>,
        d: &Directive<'_>,
        tag: u16,
    ) -> Result<(), Fail> {
        let name = trim_space(&d.argument()).to_owned();
        if tag == CURRENTMODULE_TAG {
            sx.state().py_module = if name == "None" { String::new() } else { name };
            return Ok(());
        }
        sx.state().py_module.clone_from(&name);
        sx.with_cx(p, |cx| module_target(cx, d, "py", &name, ("pair: module; ", "")))?;
        p.parse_body(&d.content, d.sections, None)
    }

    fn before_content(&self, cx: &mut Cx<'_, '_, '_>, o: &Object<'_>) -> u32 {
        let st = &mut *cx.st;
        let nesting = o.objtype == "class" || o.objtype == "exception";
        let prefix = if nesting { o.fullname.as_str() } else { trim_suffix(&o.prefix, ".") };
        let mut pushed = 0;
        if !prefix.is_empty() {
            st.py_class = prefix.to_owned();
            if nesting && st.py_classes.push(prefix) {
                pushed |= 1;
            }
        }
        if let Some(v) = o.option("module") {
            let current = std::mem::take(&mut st.py_module);
            if st.py_modules.push(&current) {
                pushed |= 2;
            }
            st.py_module = v.into_owned();
        }
        pushed
    }

    fn after_content(&self, st: &mut State, o: &Object<'_>, pushed: u32) {
        let nesting = o.objtype == "class" || o.objtype == "exception";
        if nesting && pushed & 1 != 0 {
            st.py_classes.pop();
        }
        st.py_class = st.py_classes.top().to_owned();
        if pushed & 2 != 0 {
            st.py_module = st.py_modules.top().to_owned();
            st.py_modules.pop();
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
        if !st.py_module.is_empty() {
            b.add_custom_attribute(id, "py:module", &st.py_module)?;
        }
        if !st.py_class.is_empty() {
            b.add_custom_attribute(id, "py:class", &st.py_class)?;
        }
        Ok(())
    }

    fn autodoc_context(&self, st: &State) -> (String, String) {
        (st.py_module.clone(), st.py_class.clone())
    }
}

/// The Python and JavaScript table of contents entry: the full name within the module
/// (domain), the last part (hide), or the module and every part (all); functions and
/// methods get "()" with add_function_parentheses.
pub(crate) fn dotted_toc_name(config: &Config, o: &Object<'_>, names: TocNames<'_>) -> String {
    let callable = o.objtype == "function" || o.objtype == "method";
    let parens = if callable && config.add_function_parentheses { "()" } else { "" };
    let fullname = names.fullname;
    match config.toc_object_entries_show_parents {
        TocParents::Domain => format!("{fullname}{parens}"),
        TocParents::Hide => format!("{}{parens}", last_part(fullname, ".")),
        TocParents::All if !names.module.is_empty() => {
            format!("{}.{fullname}{parens}", names.module)
        }
        TocParents::All => format!("{fullname}{parens}"),
    }
}

/// A module's target: Index{Id "module-<name>"; entries before + name + after; module,
/// domain, platform, synopsis, deprecated}. Nothing with `:no-index:`.
pub(crate) fn module_target(
    cx: &mut Cx<'_, '_, '_>,
    d: &Directive<'_>,
    domain: &str,
    name: &str,
    entry: (&str, &str),
) -> Result<(), Fail> {
    if d.has_option("no-index") || d.has_option("noindex") {
        return Ok(());
    }
    let id = cx.p.open(NodeKind::Index, d.span)?;
    cx.add_ids(id, "module", name, "")?;
    if !d.has_option("no-index-entry") && !d.has_option("noindexentry") {
        let entries = format!("{}{name}{}", entry.0, entry.1);
        cx.custom(id, "entries", &entries)?;
    }
    cx.custom(id, "module", name)?;
    // The module index lists Python modules only.
    cx.custom(id, "domain", domain)?;
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

/// A Python signature split as Sphinx's pattern splits it:
/// "[prefix.]name [\[type params\]] [(arglist) [-> retann]]".
#[derive(Debug, Default)]
struct PySig<'s> {
    prefix: &'s str,
    name: &'s str,
    tparams: Option<&'s str>,
    arglist: Option<&'s str>,
    retann: Option<&'s str>,
}

/// Splits a signature; `None` for text the pattern does not match.
fn parse_py_sig(sig: &str) -> Option<PySig<'_>> {
    let b = sig.as_bytes();
    let i = b.iter().position(|&c| !(is_identifier_byte(c) || c == b'.')).unwrap_or(b.len());
    let head = &sig[..i];
    let dot = head.rfind('.').map_or(0, |k| k + 1);
    let mut ps = PySig { prefix: &head[..dot], name: &head[dot..], ..PySig::default() };
    if ps.name.is_empty() {
        return None;
    }
    let mut rest = sig[i..].trim_start_matches(|c: char| c.is_ascii_whitespace());
    if rest.starts_with('[') {
        let close = matching(rest, b'[', b']')?;
        ps.tparams = Some(trim_space(&rest[1..close]));
        rest = &rest[close + 1..];
    }
    if rest.is_empty() {
        return Some(ps);
    }
    if !rest.starts_with('(') {
        return None;
    }
    // The argument list runs to the last ")" after which only "-> annotation" or
    // nothing follows (the pattern's greedy group).
    let r = rest.as_bytes();
    for k in (1..r.len()).rev() {
        if r[k] != b')' {
            continue;
        }
        let after = trim_space(&rest[k + 1..]);
        if after.is_empty() {
            ps.arglist = Some(&rest[1..k]);
            return Some(ps);
        }
        if let Some(ret) = after.strip_prefix("->") {
            ps.arglist = Some(&rest[1..k]);
            ps.retann = Some(trim_space(ret));
            return Some(ps);
        }
    }
    None
}

/// Returns the index of the bracket closing the one `s` starts with.
fn matching(s: &str, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0;
    for (k, &c) in s.as_bytes().iter().enumerate() {
        if c == open {
            depth += 1;
        }
        if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(k);
            }
        }
    }
    None
}

/// Where a Python signature's names come from.
struct PyNames {
    modname: String,
    classname: String,
    prefix: String,
    fullname: String,
    add_module: bool,
}

/// Works out the names of a signature in the current module and class.
fn py_names(st: &State, o: &Object<'_>, ps: &PySig<'_>) -> PyNames {
    let modname = o.option("module").map_or_else(|| st.py_module.clone(), |v| v.into_owned());
    let mut classname = st.py_class.clone();
    let mut prefix = ps.prefix;
    let fullname;
    let mut add_module = false;
    if !classname.is_empty() {
        let starts = prefix.len() > classname.len()
            && has_at(prefix, 0, &classname)
            && prefix.as_bytes()[classname.len()] == b'.';
        if !prefix.is_empty() && (prefix[..prefix.len() - 1] == classname || starts) {
            fullname = format!("{prefix}{}", ps.name);
            prefix = prefix[classname.len().min(prefix.len())..].trim_start_matches('.');
        } else if !prefix.is_empty() {
            fullname = format!("{classname}.{prefix}{}", ps.name);
        } else {
            fullname = format!("{classname}.{}", ps.name);
        }
    } else {
        add_module = true;
        classname = trim_suffix(prefix, ".").to_owned();
        fullname = format!("{prefix}{}", ps.name);
    }
    PyNames { modname, classname, prefix: prefix.to_owned(), fullname, add_module }
}

/// Emits a Python object's signature: annotation prefixes, the module or class prefix
/// (Desc_Addname), the name, parameters, the return annotation, and the :type:,
/// :value:, and :annotation: options. Ids are the full dotted name.
fn py_signature(
    cx: &mut Cx<'_, '_, '_>,
    o: &mut Object<'_>,
    sig: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let Some(ps) = parse_py_sig(sig) else { return cx.plain_signature(sig, span) };
    let n = py_names(cx.st, o, &ps);
    let full = if n.modname.is_empty() {
        n.fullname.clone()
    } else {
        format!("{}.{}", n.modname, n.fullname)
    };
    // As ObjectDescription.run, only the first signature of a name gets a target and an
    // index entry; the others of the same object (overloads) have neither.
    let first = first_py_name(cx.st, o, &n.fullname, &n.prefix);
    let id = cx.b().begin_node(NodeKind::DescSignature, span)?;
    if !o.noindex && first {
        cx.add_ids(id, "", &full, &full)?;
    }
    cx.custom(id, "fullname", &n.fullname)?;
    if !n.modname.is_empty() {
        cx.custom(id, "module", &n.modname)?;
    }
    let names = TocNames { fullname: &n.fullname, module: &n.modname, sig };
    cx.toc_entry_name(o, id, names)?;
    if !n.classname.is_empty() {
        cx.custom(id, "class", &n.classname)?;
    }
    // The canonical name is an alias of the object (PythonDomain.note_object with
    // aliased); py:type gives the option another meaning, the aliased type.
    if let Some(v) = o.option("canonical")
        && !v.is_empty()
        && !o.noindex
        && o.objtype != "type"
    {
        cx.custom(id, "canonical", &v)?;
    }
    signature_body(cx, o, &ps, &n, span)?;
    cx.b().end_node(id)?;
    o.fullname.clone_from(&n.fullname);
    o.prefix.clone_from(&n.prefix);
    o.signatures += 1;
    if first {
        py_entry(cx.config, o, &n.fullname, &n.modname);
    }
    Ok(())
}

/// Records a signature's name (fullname and prefix) for its object, and tells whether
/// no earlier signature of the object had it.
fn first_py_name(st: &mut State, o: &Object<'_>, fullname: &str, prefix: &str) -> bool {
    if o.signatures == 0 {
        st.py_names.clear();
    }
    if st.py_names.iter().any(|(f, p)| f == fullname && p == prefix) {
        return false;
    }
    if st.py_names.len() < 16 {
        st.py_names.push((fullname.to_owned(), prefix.to_owned()));
    }
    true
}

/// Emits what follows a signature's ids: prefixes, names, parameters, annotations.
fn signature_body(
    cx: &mut Cx<'_, '_, '_>,
    o: &Object<'_>,
    ps: &PySig<'_>,
    n: &PyNames,
    span: SourceSpan,
) -> Result<(), Fail> {
    if o.objtype == "decorator" || o.objtype == "decoratormethod" {
        cx.leaf_in(NodeKind::DescAddname, span, "@")?;
    }
    let annotation = py_prefix(o);
    if !annotation.is_empty() {
        cx.leaf_in(NodeKind::DescAnnotation, span, &annotation)?;
    }
    if !n.prefix.is_empty() {
        cx.leaf_in(NodeKind::DescAddname, span, &n.prefix)?;
    } else if !n.modname.is_empty() && n.add_module && cx.config.add_module_names {
        cx.leaf_in(NodeKind::DescAddname, span, &format!("{}.", n.modname))?;
    }
    cx.leaf_in(NodeKind::DescName, span, ps.name)?;
    if let Some(t) = ps.tparams {
        cx.text_leaf(span, &format!("[{t}]"))?;
    }
    let callable = matches!(o.objtype, "function" | "method" | "classmethod" | "staticmethod");
    match ps.arglist {
        Some(args) => params::py_arglist(cx, args, span)?,
        None if callable => cx.leaf_in(NodeKind::DescParameterlist, span, "")?,
        None => {}
    }
    if let Some(ret) = ps.retann {
        let r = cx.b().begin_node(NodeKind::DescReturns, span)?;
        annotation_xrefs(cx, ret, span)?;
        cx.b().end_node(r)?;
    }
    option_annotations(cx, o, span)
}

/// The :type:, :value:, :annotation:, and (for py:type) :canonical: options.
fn option_annotations(
    cx: &mut Cx<'_, '_, '_>,
    o: &Object<'_>,
    span: SourceSpan,
) -> Result<(), Fail> {
    if let Some(v) = o.option("type").filter(|v| !v.is_empty()) {
        let a = cx.b().begin_node(NodeKind::DescAnnotation, span)?;
        cx.text_leaf(span, ": ")?;
        annotation_xrefs(cx, &v, span)?;
        cx.b().end_node(a)?;
    }
    if let Some(v) = o.option("value").filter(|v| !v.is_empty()) {
        cx.leaf_in(NodeKind::DescAnnotation, span, &format!(" = {v}"))?;
    }
    if let Some(v) = o.option("annotation").filter(|v| !v.is_empty()) {
        cx.leaf_in(NodeKind::DescAnnotation, span, &format!(" {v}"))?;
    }
    if let Some(v) = o.option("canonical").filter(|v| !v.is_empty())
        && o.objtype == "type"
    {
        let a = cx.b().begin_node(NodeKind::DescAnnotation, span)?;
        cx.text_leaf(span, " = ")?;
        annotation_xrefs(cx, &v, span)?;
        cx.b().end_node(a)?;
    }
    Ok(())
}

/// The signature prefix of an object type and its options.
fn py_prefix(o: &Object<'_>) -> String {
    let abstract_ = o.flag("abstractmethod") || o.flag("abstract");
    let mut s = String::new();
    match o.objtype {
        "function" | "decorator" if o.flag("async") => s.push_str("async "),
        "class" | "exception" => {
            if o.flag("final") {
                s.push_str("final ");
            }
            s.push_str(o.objtype);
            s.push(' ');
        }
        "method" | "classmethod" | "staticmethod" | "decoratormethod" => {
            let parts = [
                (o.flag("final"), "final "),
                (abstract_, "abstract "),
                (o.flag("async"), "async "),
                (o.flag("classmethod") || o.objtype == "classmethod", "classmethod "),
                (o.flag("staticmethod") || o.objtype == "staticmethod", "static "),
            ];
            parts.iter().filter(|(on, _)| *on).for_each(|(_, w)| s.push_str(w));
        }
        "property" => {
            if abstract_ {
                s.push_str("abstract ");
            }
            if o.flag("classmethod") {
                s.push_str("class ");
            }
            s.push_str("property ");
        }
        "type" => s.push_str("type "),
        _ => {}
    }
    s
}

/// Adds Sphinx's index entry for a Python object.
fn py_entry(config: &Config, o: &mut Object<'_>, fullname: &str, modname: &str) {
    let (mut cls, name) =
        fullname.rsplit_once('.').map_or((String::new(), fullname), |(c, n)| (c.to_owned(), n));
    if !cls.is_empty() && !modname.is_empty() && config.add_module_names {
        cls = format!("{modname}.{cls}");
    }
    let in_module = !modname.is_empty();
    match o.objtype {
        "function" | "decorator" if in_module => {
            o.entry("single", &[fullname, "() (in module ", modname, ")"]);
        }
        "function" | "decorator" => o.entry("pair", &["built-in function; ", fullname, "()"]),
        "data" if in_module => o.entry("single", &[fullname, " (in module ", modname, ")"]),
        "data" => o.entry("single", &[fullname, " (built-in variable)"]),
        "class" if in_module => o.entry("single", &[fullname, " (class in ", modname, ")"]),
        "class" => o.entry("single", &[fullname, " (built-in class)"]),
        "exception" => o.entry("single", &[fullname]),
        "type" if in_module => o.entry("single", &[fullname, " (type alias in ", modname, ")"]),
        "type" => o.entry("single", &[fullname, " (type alias)"]),
        "method" | "classmethod" | "staticmethod" | "decoratormethod" => {
            method_entry(o, &cls, name, fullname, modname);
        }
        "attribute" | "property" => {
            let kind = if o.objtype == "attribute" { " attribute)" } else { " property)" };
            if !cls.is_empty() {
                o.entry("single", &[name, " (", &cls, kind]);
            } else if in_module {
                o.entry("single", &[fullname, " (in module ", modname, ")"]);
            } else {
                o.entry("single", &[fullname]);
            }
        }
        _ => {}
    }
}

/// The entry of a method: by its class's name, or its module's without a class.
fn method_entry(o: &mut Object<'_>, cls: &str, name: &str, fullname: &str, modname: &str) {
    if cls.is_empty() {
        if modname.is_empty() {
            o.entry("single", &[fullname, "()"]);
        } else {
            o.entry("single", &[fullname, "() (in module ", modname, ")"]);
        }
    } else if o.objtype == "classmethod" || o.flag("classmethod") {
        o.entry("single", &[name, "() (", cls, " class method)"]);
    } else if o.objtype == "staticmethod" || o.flag("staticmethod") {
        o.entry("single", &[name, "() (", cls, " static method)"]);
    } else {
        o.entry("single", &[name, "() (", cls, " method)"]);
    }
}

/// Emits a type annotation: dotted names become Xref{py:class} (py:obj for None and
/// typing names, whose "typing." is not shown); everything else is text.
pub(crate) fn annotation_xrefs(
    cx: &mut Cx<'_, '_, '_>,
    text: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let t = text.as_bytes();
    let (mut pos, mut i) = (0, 0);
    while i < t.len() {
        let c = t[i];
        if c == b'\'' || c == b'"' {
            let mut k = i + 1;
            while k < t.len() && t[k] != c {
                if t[k] == b'\\' {
                    k += 1;
                }
                k += 1;
            }
            i = (k + 1).min(t.len());
            continue;
        }
        if !is_identifier_byte(c) || c.is_ascii_digit() {
            i += 1;
            continue;
        }
        let mut k = i;
        while k < t.len() && (is_identifier_byte(t[k]) || t[k] == b'.') {
            k += 1;
        }
        if i > pos {
            cx.text_leaf(span, &text[pos..i])?;
        }
        type_xref(cx, &text[i..k], span)?;
        (i, pos) = (k, k);
    }
    if pos < t.len() {
        cx.text_leaf(span, &text[pos..])?;
    }
    Ok(())
}

/// One name of an annotation as a cross-reference.
fn type_xref(cx: &mut Cx<'_, '_, '_>, target: &str, span: SourceSpan) -> Result<(), Fail> {
    let typing = has_at(target, 0, "typing.");
    let shown = if typing { &target[7..] } else { target };
    let x = XrefParts { title: shown.into(), target: target.into(), ..XrefParts::default() };
    let reftype = if target == "None" || typing { "py:obj" } else { "py:class" };
    cx.xref(span, reftype, "py", &x, false)
}
