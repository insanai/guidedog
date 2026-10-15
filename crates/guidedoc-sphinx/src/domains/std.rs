//! The standard domain: glossary, productionlist, program, and the object descriptions
//! option (with the program), envvar and confval (a name), describe and object (the
//! text, no id), with the object types the project declares.

use guidedoc::{Builder, Fail, NodeId, NodeKind, SourceSpan, is_ascii_space};
use guidedoc_rst::{ArgRule, ContentRule, Directive, Line, OptSpec, OptType, Parser, opt};
use guidedoc_rst_inline::python_space;

use super::object::{OPTS_OBJECT, join_opts};
use super::{Cx, Domain, DomainDirective, Object, RoleSpec, TocNames, XrefFlags, XrefInfo};
use crate::config::Config;
use crate::extension::Sphinx;
use crate::glossary;
use crate::messages::OPTION_DESCRIPTION;
use crate::object_types::{declared_signature, object_type};
use crate::state::State;
use crate::text::{collapse, trim_space};

static OPTS_CONFVAL: [OptSpec; 9] = join_opts(
    [
        opt("no-index", OptType::Flag),
        opt("noindex", OptType::Flag),
        opt("no-index-entry", OptType::Flag),
        opt("noindexentry", OptType::Flag),
        opt("no-contents-entry", OptType::Flag),
        opt("nocontentsentry", OptType::Flag),
        opt("no-typesetting", OptType::Flag),
    ],
    [opt("type", OptType::Text), opt("default", OptType::Text)],
);
static OPTS_GLOSSARY: &[OptSpec] = &[opt("sorted", OptType::Flag)];

const GLOSSARY: u16 = 0;
const PRODUCTIONLIST: u16 = 1;
const PROGRAM: u16 = 2;

static DIRECTIVES: &[DomainDirective] = &[
    DomainDirective::run(
        "std:glossary",
        GLOSSARY,
        (ArgRule::None, false, ContentRule::Required),
        OPTS_GLOSSARY,
    ),
    DomainDirective::run(
        "std:productionlist",
        PRODUCTIONLIST,
        (ArgRule::Required, true, ContentRule::None),
        &[],
    ),
    DomainDirective::run(
        "std:program",
        PROGRAM,
        (ArgRule::Required, true, ContentRule::None),
        &[],
    ),
    DomainDirective::object("std:option", OPTS_OBJECT),
    DomainDirective::object("std:cmdoption", OPTS_OBJECT),
    DomainDirective::object("std:envvar", OPTS_OBJECT),
    DomainDirective::object("std:describe", OPTS_OBJECT),
    DomainDirective::object("std:object", OPTS_OBJECT),
    DomainDirective::object("std:confval", &OPTS_CONFVAL),
];

const C: XrefFlags = XrefFlags::CODE;

static ROLES: &[RoleSpec] = &[
    RoleSpec::xref("option", C),
    RoleSpec::xref("confval", C),
    RoleSpec::xref("envvar", C.with(XrefFlags::INDEX)),
    RoleSpec::xref("token", C),
    RoleSpec::xref("term", XrefFlags::NONE),
    RoleSpec::xref("ref", XrefFlags::LOWER),
    RoleSpec::xref("numref", C.with(XrefFlags::LOWER)),
    RoleSpec::xref("keyword", C),
    RoleSpec::xref("doc", XrefFlags::NONE),
];

/// The standard domain.
#[derive(Debug)]
pub(crate) struct StdDomain;

/// The value `DOMAINS` registers.
pub(crate) static DOMAIN: StdDomain = StdDomain;

impl Domain for StdDomain {
    fn name(&self) -> &'static str {
        "std"
    }

    fn directives(&self) -> &'static [DomainDirective] {
        DIRECTIVES
    }

    fn roles(&self) -> &'static [RoleSpec] {
        ROLES
    }

    fn signature(
        &self,
        cx: &mut Cx<'_, '_, '_>,
        o: &mut Object<'_>,
        sig: &str,
        span: SourceSpan,
    ) -> Result<(), Fail> {
        std_signature(cx, o, sig, span)
    }

    fn run(
        &self,
        sx: &Sphinx<'_>,
        p: &mut Parser<'_, '_>,
        d: &Directive<'_>,
        tag: u16,
    ) -> Result<(), Fail> {
        match tag {
            GLOSSARY => glossary::glossary_directive(sx, p, d),
            PRODUCTIONLIST => glossary::productionlist_directive(sx, p, d),
            _ => {
                program_directive(&mut sx.state(), d);
                Ok(())
            }
        }
    }

    /// confval puts :type: and :default: first in its content, as a field list.
    fn content_begin(&self, cx: &mut Cx<'_, '_, '_>, o: &Object<'_>) -> Result<(), Fail> {
        if o.objtype != "confval" {
            return Ok(());
        }
        let d = o.d;
        let (type_lines, default_lines) = (d.option_lines("type"), d.option_lines("default"));
        if type_lines.is_none() && default_lines.is_none() {
            return Ok(());
        }
        let list = cx.b().begin_node(NodeKind::FieldList, d.span)?;
        if let Some(lines) = type_lines {
            field(cx.p, "Type", lines, d.span)?;
        }
        if let Some(lines) = default_lines {
            field(cx.p, "Default", lines, d.span)?;
        }
        cx.b().end_node(list)
    }

    fn toc_name(
        &self,
        _config: &Config,
        _st: &State,
        o: &Object<'_>,
        names: TocNames<'_>,
    ) -> Option<String> {
        (o.objtype == "confval").then(|| names.fullname.to_owned())
    }

    fn xref_context(
        &self,
        b: &mut Builder<'_>,
        st: &State,
        id: NodeId,
        x: &XrefInfo<'_>,
    ) -> Result<(), Fail> {
        if (x.reftype == "option" || x.any) && !st.program.is_empty() {
            b.add_custom_attribute(id, "std:program", &st.program)?;
        }
        Ok(())
    }
}

/// Emits Field > Field_Name(name), Field_Body > Paragraph(lines).
fn field(
    p: &mut Parser<'_, '_>,
    name: &str,
    lines: &[Line<'_>],
    span: SourceSpan,
) -> Result<(), Fail> {
    let f = p.ctx.b.begin_node(NodeKind::Field, span)?;
    super::leaf_in(&mut p.ctx.b, NodeKind::FieldName, span, name)?;
    let body = p.ctx.b.begin_node(NodeKind::FieldBody, span)?;
    if !lines.is_empty() && !(lines.len() == 1 && lines[0].is_blank()) {
        p.inline_node(NodeKind::Paragraph, lines)?;
    }
    p.ctx.b.end_node(body)?;
    p.ctx.b.end_node(f)
}

/// program sets the program of later options, its words joined by hyphens; "None"
/// clears it.
fn program_directive(st: &mut State, d: &Directive<'_>) {
    let name = d.argument();
    let name = trim_space(&name);
    if name == "None" {
        st.program.clear();
        return;
    }
    let mut program = String::with_capacity(name.len());
    let mut space = false;
    for c in name.chars() {
        if c.is_ascii() && is_ascii_space(c as u8) {
            space = true;
            continue;
        }
        if space {
            program.push('-');
        }
        space = false;
        program.push(c);
    }
    st.program = program;
}

/// options (with the program), envvar and confval (a name), describe and object (the
/// text, no id), and declared object types.
fn std_signature(
    cx: &mut Cx<'_, '_, '_>,
    o: &mut Object<'_>,
    sig: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    if let Some(t) = object_type(cx.config, o.objtype) {
        return declared_signature(cx, o, t, sig, span);
    }
    match o.objtype {
        "option" | "cmdoption" => return option_signature(cx, o, sig, span),
        "describe" | "object" => return cx.plain_signature(sig, span),
        _ => {}
    }
    let name = collapse(sig).into_owned();
    let id = cx.b().begin_node(NodeKind::DescSignature, span)?;
    if !o.noindex {
        cx.add_ids(id, o.objtype, &name, "")?;
    }
    cx.custom(id, "fullname", &name)?;
    cx.toc_entry_name(o, id, TocNames { fullname: &name, module: "", sig })?;
    cx.leaf_in(NodeKind::DescName, span, &name)?;
    cx.b().end_node(id)?;
    o.signatures += 1;
    if o.objtype == "envvar" {
        o.entry("single", &["environment variable; ", &name]);
    } else {
        o.entry("pair", &[&name, "; configuration value"]);
    }
    o.fullname = name;
    Ok(())
}

/// Reads "-o FILE, --output=FILE": each option's name is a Desc_Name, its arguments a
/// Desc_Addname; ids are "cmdoption-<program>-<option>".
fn option_signature(
    cx: &mut Cx<'_, '_, '_>,
    o: &mut Object<'_>,
    sig: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let mut options: Vec<(&str, &str)> = Vec::new();
    let mut rest = sig;
    while !rest.is_empty() {
        let (part, tail) = rest.split_once(", ").unwrap_or((rest, ""));
        rest = tail;
        match split_option(trim_space(part)) {
            Some(option) => options.push(option),
            None => cx.report(OPTION_DESCRIPTION, span),
        }
    }
    if options.is_empty() {
        return cx.plain_signature(sig, span);
    }
    let program = cx.st.program.clone();
    let prefix =
        if program.is_empty() { "cmdoption".to_owned() } else { format!("cmdoption-{program}") };
    let id = cx.b().begin_node(NodeKind::DescSignature, span)?;
    if !o.noindex {
        for (name, _) in &options {
            let pre = if name.starts_with(['-', '/']) {
                prefix.clone()
            } else {
                format!("{prefix}-arg")
            };
            let made = cx.st.make_object_id(&pre, name);
            cx.b().add_attribute_string(id, guidedoc::AttributeKey::Id, &made)?;
            // Every entry points at the first id, as Sphinx's signode["ids"][0].
            if cx.st.object_id.is_empty() {
                cx.st.object_id = made;
            }
        }
    }
    cx.custom(id, "fullname", options[0].0)?;
    if !program.is_empty() {
        cx.custom(id, "program", &program)?;
    }
    // Every spelling of the option is a target, as Sphinx registers each one.
    let spellings: Vec<&str> = options.iter().map(|(n, _)| *n).collect();
    cx.custom(id, "options", &spellings.join("\n"))?;
    for (i, (name, arg)) in options.iter().enumerate() {
        if i > 0 {
            cx.leaf_in(NodeKind::DescAddname, span, ", ")?;
        }
        cx.leaf_in(NodeKind::DescName, span, name)?;
        if !arg.is_empty() {
            cx.leaf_in(NodeKind::DescAddname, span, arg)?;
        }
    }
    cx.b().end_node(id)?;
    o.fullname = options[0].0.to_owned();
    o.signatures += 1;
    for (name, _) in &options {
        if program.is_empty() {
            o.entry("pair", &["command line option; ", name]);
        } else {
            o.entry("pair", &[&program, " command line option; ", name]);
        }
    }
    Ok(())
}

/// Matches "((?:/|--|-|\+)?[^\s=]+)(=?\s*.*)", with "opt[=value]" read as an optional
/// value. The optional prefix is itself made of [^\s=] characters, so with backtracking
/// the name is the longest leading run of them: "-" alone is a name (standard input).
fn split_option(part: &str) -> Option<(&str, &str)> {
    let j = part
        .char_indices()
        .find(|&(_, c)| python_space(c) || c == '=')
        .map_or(part.len(), |(k, _)| k);
    if j == 0 {
        return None;
    }
    let (name, arg) = (&part[..j], &part[j..]);
    if name.ends_with('[') && arg.ends_with(']') {
        return Some((&name[..name.len() - 1], &part[j - 1..]));
    }
    Some((name, arg))
}
