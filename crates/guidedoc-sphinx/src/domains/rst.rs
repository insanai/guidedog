//! The reStructuredText domain: rst:directive, rst:directive:option, and rst:role.
//! Directives show ".. name::" and their arguments, options ":name:", roles ":name:";
//! ids are "directive-<name>", "directive-option-<directive>-<name>", and
//! "role-<name>".

use guidedoc::{Fail, NodeKind, SourceSpan, is_ascii_space};
use guidedoc_rst::{OptSpec, OptType, opt};

use super::object::{OPTS_OBJECT, join_opts};
use super::{Cx, Domain, DomainDirective, Object, RoleSpec, TocNames, XrefFlags};
use crate::config::{Config, TocParents};
use crate::state::State;
use crate::text::{last_part, trim_space};

static OPTS_RST_OPTION: [OptSpec; 8] = join_opts(
    [
        opt("no-index", OptType::Flag),
        opt("noindex", OptType::Flag),
        opt("no-index-entry", OptType::Flag),
        opt("noindexentry", OptType::Flag),
        opt("no-contents-entry", OptType::Flag),
        opt("nocontentsentry", OptType::Flag),
        opt("no-typesetting", OptType::Flag),
    ],
    [opt("type", OptType::Text)],
);

static DIRECTIVES: &[DomainDirective] = &[
    DomainDirective::object("rst:directive", OPTS_OBJECT),
    DomainDirective::object("rst:directive:option", &OPTS_RST_OPTION),
    DomainDirective::object("rst:role", OPTS_OBJECT),
];

static ROLES: &[RoleSpec] =
    &[RoleSpec::xref("dir", XrefFlags::CODE), RoleSpec::xref("role", XrefFlags::CODE)];

/// The reStructuredText domain.
#[derive(Debug)]
pub(crate) struct RstDomain;

/// The value `DOMAINS` registers.
pub(crate) static DOMAIN: RstDomain = RstDomain;

impl Domain for RstDomain {
    fn name(&self) -> &'static str {
        "rst"
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
        rst_signature(cx, o, sig, span)
    }

    fn before_content(&self, cx: &mut Cx<'_, '_, '_>, o: &Object<'_>) -> u32 {
        u32::from(o.objtype == "directive" && cx.st.rst_directives.push(&o.fullname))
    }

    fn after_content(&self, st: &mut State, _o: &Object<'_>, pushed: u32) {
        if pushed & 1 != 0 {
            st.rst_directives.pop();
        }
    }

    /// ":name:" for a directive option, and with domain or all, the enclosing directives
    /// joined with ':' for a directive (".. a:b::") or a role (":a:b:").
    fn toc_name(
        &self,
        config: &Config,
        st: &State,
        o: &Object<'_>,
        names: TocNames<'_>,
    ) -> Option<String> {
        let fullname = names.fullname;
        let (before, after) = match o.objtype {
            "directive:option" => return Some(format!(":{}:", last_part(fullname, ":"))),
            "role" => (":", ":"),
            "directive" => (".. ", "::"),
            _ => return None,
        };
        let mut name = String::from(before);
        if config.toc_object_entries_show_parents == TocParents::Hide {
            name.push_str(last_part(fullname, ":"));
        } else {
            for parent in st.rst_directives.items() {
                name.push_str(parent);
                name.push(':');
            }
            name.push_str(fullname);
        }
        name.push_str(after);
        Some(name)
    }
}

/// Splits ".. name:: arguments"; other text is a name.
fn parse_directive_signature(sig: &str) -> (&str, &str) {
    let d = trim_space(sig);
    let Some(rest) = d.strip_prefix(".. ") else { return (d, "") };
    match rest.find("::") {
        Some(k) => (trim_space(&rest[..k]), trim_space(&rest[k + 2..])),
        None => (d, ""),
    }
}

/// Splits "name: arguments" of a directive option at the first colon followed by
/// white space.
fn parse_option_signature(sig: &str) -> (&str, &str) {
    let b = sig.as_bytes();
    for i in 0..b.len() {
        if b[i] != b':' || i + 1 >= b.len() || !is_ascii_space(b[i + 1]) {
            continue;
        }
        let name = sig[..i].trim_end_matches(|c: char| c.is_ascii() && is_ascii_space(c as u8));
        return (name, trim_space(&sig[i + 1..]));
    }
    (sig, "")
}

/// Emits an rst object's signature, ids, and entry.
fn rst_signature(
    cx: &mut Cx<'_, '_, '_>,
    o: &mut Object<'_>,
    sig: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let (name, args) = match o.objtype {
        "directive" => parse_directive_signature(sig),
        "directive:option" => parse_option_signature(sig),
        _ => (sig, ""),
    };
    let parent = cx.st.rst_directives.top().to_owned();
    let id = cx.b().begin_node(NodeKind::DescSignature, span)?;
    if !o.noindex {
        let prefix = match o.objtype {
            "directive:option" if !parent.is_empty() => format!("directive:option-{parent}"),
            objtype => objtype.to_owned(),
        };
        cx.add_ids(id, &prefix, name, "")?;
    }
    cx.custom(id, "fullname", name)?;
    cx.toc_entry_name(o, id, TocNames { fullname: name, module: "", sig })?;
    if o.objtype == "directive" {
        cx.leaf_in(NodeKind::DescName, span, &format!(".. {name}::"))?;
        if !args.is_empty() {
            cx.leaf_in(NodeKind::DescAddname, span, &format!(" {args}"))?;
        }
    } else {
        cx.leaf_in(NodeKind::DescName, span, &format!(":{name}:"))?;
        if !args.is_empty() {
            cx.leaf_in(NodeKind::DescAnnotation, span, &format!(" {args}"))?;
        }
        if let Some(v) = o.option("type").filter(|v| !v.is_empty())
            && o.objtype != "role"
        {
            cx.leaf_in(NodeKind::DescAnnotation, span, &format!(" ({v})"))?;
        }
    }
    cx.b().end_node(id)?;
    o.fullname = name.to_owned();
    o.signatures += 1;
    match o.objtype {
        "directive" => o.entry("single", &[name, " (directive)"]),
        "role" => o.entry("single", &[name, " (role)"]),
        _ if !parent.is_empty() => {
            o.entry("pair", &[&parent, " (directive); :", name, ": (directive option)"]);
        }
        _ => o.entry("single", &[":", name, ": (directive option)"]),
    }
    Ok(())
}
