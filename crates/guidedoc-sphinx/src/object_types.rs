//! Object types a project declares in conf.toml, as a Sphinx extension declares them
//! with app.add_object_type (a directive that describes an object, and a role that
//! refers to it) and app.add_crossref_type (a directive that only makes a target). Both
//! live in the standard domain. Directive aliases give an existing directive another
//! name, as an extension registers one of Sphinx's own classes under a name of its own.

use guidedoc::{Fail, NodeKind, SourceSpan};
use guidedoc_rst::{Directive, Parser};

use crate::config::{Config, ObjectType};
use crate::domains::object::{index_entries, object_directive};
use crate::domains::{Cx, Object, TocNames, std};
use crate::extension::Sphinx;
use crate::text::{collapse, trim_space};

/// Runs declared object type `k`: an object description, or a target only.
pub(crate) fn run_declared(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
    k: usize,
) -> Result<(), Fail> {
    let Some(t) = sx.config.object_types.get(k) else { return Ok(()) };
    if t.target {
        crossref_directive(sx, p, d, t)
    } else {
        object_directive(sx, p, d, &std::DOMAIN, &t.directive)
    }
}

/// Returns the declaration of a standard-domain object type, if one was declared.
pub(crate) fn object_type<'c>(config: &'c Config, objtype: &str) -> Option<&'c ObjectType> {
    config.object_types.iter().find(|t| t.directive == objtype)
}

/// Reads the signature of a declared object type as Sphinx's GenericObject does: the
/// whole signature, or its first word, is the name; it is shown through the display
/// template; the name may become the current program.
pub(crate) fn declared_signature(
    cx: &mut Cx<'_, '_, '_>,
    o: &mut Object<'_>,
    t: &ObjectType,
    sig: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let text = collapse(sig).into_owned();
    let name = if t.first_word { text.split(' ').next().unwrap_or("") } else { &text };
    let shown = if t.display.is_empty() { text.clone() } else { fill_template(&t.display, &text) };
    let id = cx.b().begin_node(NodeKind::DescSignature, span)?;
    if !o.noindex {
        cx.add_ids(id, o.objtype, name, "")?;
    }
    cx.custom(id, "fullname", name)?;
    cx.toc_entry_name(o, id, TocNames { fullname: name, module: "", sig })?;
    cx.leaf_in(NodeKind::DescName, span, &shown)?;
    cx.b().end_node(id)?;
    o.fullname = name.to_owned();
    o.signatures += 1;
    if t.program {
        cx.st.program.clone_from(&o.fullname);
    }
    index_template(o, &t.index, name);
    Ok(())
}

/// Adds the entry an indextemplate makes: its type before the first colon ("single"
/// without one), and the rest with %s the name.
fn index_template(o: &mut Object<'_>, template: &str, name: &str) {
    if template.is_empty() {
        return;
    }
    let (kind, rest) = match template.split_once(':') {
        Some((k, r)) => (trim_space(k), trim_space(r)),
        None => ("single", template),
    };
    match rest.split_once("%s") {
        Some((before, after)) => o.entry(kind, &[before, name, after]),
        None => o.entry(kind, &[rest]),
    }
}

/// Puts `text` in place of the first %s of a template.
fn fill_template(template: &str, text: &str) -> String {
    match template.split_once("%s") {
        Some((before, after)) => format!("{before}{text}{after}"),
        None => template.to_owned(),
    }
}

/// Sphinx's Target directive for a type add_crossref_type declared: an index entry and
/// a target with the id "<type>-<name>", which notes the object.
fn crossref_directive(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
    t: &ObjectType,
) -> Result<(), Fail> {
    if d.arg.is_empty() {
        return Ok(());
    }
    let mut o = Object::new(d, "std", &t.directive);
    o.noindex = false;
    o.noentry = false;
    let arg = d.argument();
    let name = collapse(trim_space(&arg)).into_owned();
    let made = sx.state().make_object_id(&t.directive, &name);
    let b = &mut p.ctx.b;
    let target = b.begin_node(NodeKind::Target, d.span)?;
    b.add_attribute_string(target, guidedoc::AttributeKey::Id, &made)?;
    b.add_custom_attribute(target, "domain", "std")?;
    b.add_custom_attribute(target, "objtype", &t.directive)?;
    b.add_custom_attribute(target, "fullname", &name)?;
    b.end_node(target)?;
    index_template(&mut o, &t.index, &name);
    o.targets = vec![made.as_str(); o.entry_count].join("\n");
    index_entries(p, &o, d.span)
}
