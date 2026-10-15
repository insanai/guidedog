//! The object description every domain shares: Desc{domain, objtype, noindex} >
//! Desc_Signature per signature line, an Index of the generated entries, and
//! Desc_Content with the nested content, read in the object's context (a Python class,
//! a C++ scope, a JavaScript object, an rst directive). The domain's hooks fill in what
//! differs.

use guidedoc::{Fail, NodeKind, SourceSpan};
use guidedoc_rst::{Directive, OptSpec, OptType, Parser, opt};

use super::{Domain, Object};
use crate::docfields::doc_fields;
use crate::extension::Sphinx;
use crate::text::trim_space;

/// The options every object description accepts.
pub(crate) const OPTS_OBJECT: &[OptSpec] = &[
    opt("no-index", OptType::Flag),
    opt("noindex", OptType::Flag),
    opt("no-index-entry", OptType::Flag),
    opt("noindexentry", OptType::Flag),
    opt("no-contents-entry", OptType::Flag),
    opt("nocontentsentry", OptType::Flag),
    opt("no-typesetting", OptType::Flag),
    opt("single-line-parameter-list", OptType::Flag),
    opt("single-line-template-parameter-list", OptType::Flag),
];

/// Joins two option tables at compile time: `T` must be `N + M`.
pub(crate) const fn join_opts<const N: usize, const M: usize, const T: usize>(
    a: [OptSpec; N],
    b: [OptSpec; M],
) -> [OptSpec; T] {
    assert!(N + M == T, "the joined table's length is the sum of the parts");
    let mut all = [opt("", OptType::Flag); T];
    let mut i = 0;
    while i < N {
        all[i] = a[i];
        i += 1;
    }
    while i < T {
        all[i] = b[i - N];
        i += 1;
    }
    all
}

/// Runs an object description of `domain` whose object type is `objtype`.
pub(crate) fn object_directive(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    d: &Directive<'_>,
    domain: &'static dyn Domain,
    objtype: &str,
) -> Result<(), Fail> {
    let mut o = Object::new(d, domain.name(), objtype);
    let id = p.open(NodeKind::Desc, d.span)?;
    p.custom(id, "domain", o.domain)?;
    p.custom(id, "objtype", o.objtype)?;
    if o.noindex {
        p.custom(id, "noindex", "1")?;
    }
    if d.has_option("no-contents-entry") || d.has_option("nocontentsentry") {
        p.custom(id, "no-contents-entry", "1")?;
    }
    if d.has_option("no-typesetting") {
        p.custom(id, "no-typesetting", "1")?;
    }
    signatures(sx, p, &mut o, domain)?;
    index_entries(p, &o, d.span)?;
    let span = p.lines_span(&d.content);
    let content = p.ctx.b.begin_node(NodeKind::DescContent, span)?;
    sx.with_cx(p, |cx| domain.content_begin(cx, &o))?;
    let pushed =
        if o.signatures > 0 { sx.with_cx(p, |cx| domain.before_content(cx, &o)) } else { 0 };
    // Titles in the content begin sections, in a title context of their own, as
    // Sphinx's parse_content_to_nodes(allow_section_headings=True) reads them.
    let parsed = if d.sections && !p.is_hosted() {
        p.parse_titled(&d.content)
    } else {
        p.parse_body(&d.content, false, None)
    };
    let fields = parsed.and_then(|()| sx.with_cx(p, |cx| doc_fields(cx, &o, content)));
    domain.after_content(&mut sx.state(), &o, pushed);
    fields?;
    p.ctx.b.end_node(content)?;
    p.close(id)
}

/// Emits Index{entries, targets} for an indexed object with entries.
pub(crate) fn index_entries(
    p: &mut Parser<'_, '_>,
    o: &Object<'_>,
    span: SourceSpan,
) -> Result<(), Fail> {
    if o.noindex || o.noentry || o.entries.is_empty() {
        return Ok(());
    }
    let b = &mut p.ctx.b;
    let ix = b.begin_node(NodeKind::Index, span)?;
    b.add_custom_attribute(ix, "entries", &o.entries)?;
    b.add_custom_attribute(ix, "targets", &o.targets)?;
    b.end_node(ix)
}

/// Joins lines ending in a backslash with the next, as Sphinx does, and reads every
/// signature.
fn signatures(
    sx: &Sphinx<'_>,
    p: &mut Parser<'_, '_>,
    o: &mut Object<'_>,
    domain: &dyn Domain,
) -> Result<(), Fail> {
    let d = o.d;
    let args = &d.arg;
    let mut k = 0;
    while k < args.len() {
        let mut e = k;
        while e + 1 < args.len() && args[e].text.ends_with('\\') {
            e += 1;
        }
        let span = p.span_of(&args[k], &args[e]);
        let mut sig = String::new();
        for (i, l) in args[k..=e].iter().enumerate() {
            let t = if i < e - k { &l.text[..l.text.len() - 1] } else { l.text };
            sig.push_str(t);
        }
        let before = o.entry_count;
        sx.with_cx(p, |cx| {
            cx.st.object_id.clear();
            domain.signature(cx, o, trim_space(&sig), span)?;
            for i in before..o.entry_count {
                if i > 0 {
                    o.targets.push('\n');
                }
                o.targets.push_str(&cx.st.object_id);
            }
            Ok::<(), Fail>(())
        })?;
        k = e + 1;
    }
    Ok(())
}
