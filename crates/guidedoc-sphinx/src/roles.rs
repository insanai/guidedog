//! Roles. Lookup follows Sphinx: a name with a domain ("py:func") goes to that domain;
//! a name without one is looked up in the default domain, then the standard domain, then
//! among Sphinx's generic roles and extlinks; anything else is left to the base reader.

mod text_roles;

use guidedoc::{
    Arg, AttributeKey, Builder, Fail, INVALID_NODE, LinkInfo, LinkKind, NodeId, NodeKind,
    ReadContext, SourceSpan, arg_span,
};
use guidedoc_rst_inline::{RoleCall, unescape};

use crate::config::{Config, Extlink};
use crate::domains::{DOMAINS, XrefInfo, c, cpp, domain_known, find_domain};
use crate::extension::Sphinx;
use crate::messages::{EXTERNAL_DOMAIN, EXTERNAL_ROLE, EXTERNAL_SUFFIX};
use crate::state::State;
use crate::text::{collapse, equal_fold, split_title, trim_prefix, trim_suffix};

/// What a role does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RoleKind {
    /// A cross-reference: Xref.
    Xref,
    /// A Role{name} with the text (command, dfn, program, ...).
    Generic,
    /// Keys, possibly a compound of keys.
    Kbd,
    /// guilabel and menuselection: accelerators.
    Gui,
    /// samp and file: {variable} parts as Emphasis.
    Samp,
    /// An abbreviation with its explanation.
    Abbr,
    /// A manual page, linked through manpages_url.
    Manpage,
    /// A PEP.
    Pep,
    /// An RFC.
    Rfc,
    /// Sphinx's :cve:.
    Cve,
    /// Sphinx's :cwe:.
    Cwe,
    /// An index entry and its target.
    Index,
    /// c:expr, cpp:expr: code.
    Expr,
    /// c:texpr, cpp:texpr: text.
    Texpr,
}

/// How a cross-reference role treats its text: a set of flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct XrefFlags(u8);

impl XrefFlags {
    /// No flags.
    pub(crate) const NONE: XrefFlags = XrefFlags(0);
    /// The display is wrapped in Code (Sphinx's literal inner node).
    pub(crate) const CODE: XrefFlags = XrefFlags(1);
    /// "()" handling for callables (add_function_parentheses).
    pub(crate) const PARENS: XrefFlags = XrefFlags(2);
    /// The target is lowercased (ref, numref).
    pub(crate) const LOWER: XrefFlags = XrefFlags(4);
    /// Python and JavaScript: "~" shortens at ".", "." makes it specific.
    pub(crate) const DOTTED: XrefFlags = XrefFlags(8);
    /// :py:deco: shows "@" before the title.
    pub(crate) const AT: XrefFlags = XrefFlags(16);
    /// C and C++: "~" shortens at "." or "::".
    pub(crate) const SCOPED: XrefFlags = XrefFlags(32);
    /// envvar: an index entry and target come first.
    pub(crate) const INDEX: XrefFlags = XrefFlags(64);

    /// The union of two sets.
    pub(crate) const fn with(self, other: XrefFlags) -> XrefFlags {
        XrefFlags(self.0 | other.0)
    }

    /// Tells whether every flag of `other` is set.
    pub(crate) const fn has(self, other: XrefFlags) -> bool {
        self.0 & other.0 == other.0
    }
}

/// One role of a domain's table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RoleSpec {
    /// The role's name without its domain ("func").
    pub name: &'static str,
    /// What it does.
    pub kind: RoleKind,
    /// For cross-references, how the text is treated.
    pub flags: XrefFlags,
}

impl RoleSpec {
    /// A cross-reference role.
    pub(crate) const fn xref(name: &'static str, flags: XrefFlags) -> RoleSpec {
        RoleSpec { name, kind: RoleKind::Xref, flags }
    }

    /// A role of another kind.
    pub(crate) const fn of(name: &'static str, kind: RoleKind) -> RoleSpec {
        RoleSpec { name, kind, flags: XrefFlags::NONE }
    }
}

/// A cross-reference after Sphinx's processing of its text.
#[derive(Debug, Clone, Default)]
pub(crate) struct XrefParts<'x> {
    /// What it shows.
    pub title: std::borrow::Cow<'x, str>,
    /// What it refers to.
    pub target: std::borrow::Cow<'x, str>,
    /// The title was given ("title <target>").
    pub explicit: bool,
    /// A leading "." asked for the most specific match.
    pub specific: bool,
}

const C: XrefFlags = XrefFlags::CODE;

/// Sphinx's roles without a domain.
static CORE_ROLES: &[RoleSpec] = &[
    RoleSpec::xref("any", C),
    RoleSpec::xref("download", C),
    RoleSpec::xref("eq", C),
    RoleSpec::of("command", RoleKind::Generic),
    RoleSpec::of("dfn", RoleKind::Generic),
    RoleSpec::of("mailheader", RoleKind::Generic),
    RoleSpec::of("makevar", RoleKind::Generic),
    RoleSpec::of("mimetype", RoleKind::Generic),
    RoleSpec::of("newsgroup", RoleKind::Generic),
    RoleSpec::of("program", RoleKind::Generic),
    RoleSpec::of("regexp", RoleKind::Generic),
    RoleSpec::of("kbd", RoleKind::Kbd),
    RoleSpec::of("guilabel", RoleKind::Gui),
    RoleSpec::of("menuselection", RoleKind::Gui),
    RoleSpec::of("samp", RoleKind::Samp),
    RoleSpec::of("file", RoleKind::Samp),
    RoleSpec::of("abbr", RoleKind::Abbr),
    RoleSpec::of("manpage", RoleKind::Manpage),
    RoleSpec::of("pep", RoleKind::Pep),
    RoleSpec::of("rfc", RoleKind::Rfc),
    RoleSpec::of("cve", RoleKind::Cve),
    RoleSpec::of("cwe", RoleKind::Cwe),
    RoleSpec::of("index", RoleKind::Index),
];

/// A role found for a name: its domain ("" for Sphinx's generic roles) and its spec.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Found<'r> {
    pub domain: &'r str,
    pub name: &'r str,
    pub kind: RoleKind,
    pub flags: XrefFlags,
}

impl Found<'_> {
    fn of(domain: &str, spec: &RoleSpec) -> Found<'static> {
        let domain = find_domain(domain).map_or("", |(_, d)| d.name());
        Found { domain, name: spec.name, kind: spec.kind, flags: spec.flags }
    }
}

/// What role code works with: the read context (builder, reports), the per-document
/// state, and the configuration.
pub(crate) struct RoleCx<'r, 'a> {
    pub ctx: &'r mut ReadContext<'a>,
    pub st: &'r mut State,
    pub config: &'r Config,
}

impl<'a> RoleCx<'_, 'a> {
    /// The builder.
    pub(crate) fn b(&mut self) -> &mut Builder<'a> {
        &mut self.ctx.b
    }

    /// Reports `m` at `span`, quoting it.
    pub(crate) fn report(&mut self, m: guidedoc::Message, span: SourceSpan) {
        self.ctx.report(m, span, arg_span(span), Arg::None);
    }
}

/// Finds the role `name` of `domain` ("" for Sphinx's generic ones).
pub(crate) fn role_in(domain: &str, name: &str) -> Option<Found<'static>> {
    if domain.is_empty() {
        return CORE_ROLES.iter().find(|r| equal_fold(r.name, name)).map(|r| Found::of("", r));
    }
    let (_, d) = find_domain(domain)?;
    d.roles().iter().find(|r| equal_fold(r.name, name)).map(|r| Found::of(domain, r))
}

/// Finds a role a project's object type declares.
fn declared_role<'c>(config: &'c Config, domain: &str, name: &str) -> Option<Found<'c>> {
    if !domain.is_empty() && domain != "std" {
        return None;
    }
    let t = config
        .object_types
        .iter()
        .take(64)
        .find(|t| !t.role.is_empty() && equal_fold(&t.role, name))?;
    Some(Found { domain: "std", name: &t.role, kind: RoleKind::Xref, flags: XrefFlags::CODE })
}

/// Returns the role a name denotes in the current state.
pub(crate) fn find_role<'c>(config: &'c Config, st: &State, name: &str) -> Option<Found<'c>> {
    // sphinx.ext.autosummary's autolink, with Python's provider: a py:obj reference.
    if equal_fold(name, "autolink") && config.autodoc_for("py").is_some() {
        return role_in("py", "obj");
    }
    if let Some((domain, local)) = name.split_once(':') {
        return role_in(domain, local).or_else(|| declared_role(config, domain, local));
    }
    if !st.domain.is_empty()
        && let Some(r) = role_in(&st.domain, name)
    {
        return Some(r);
    }
    role_in("std", name).or_else(|| role_in("", name)).or_else(|| declared_role(config, "", name))
}

/// Tells whether a role name is known in the current state (extlinks included).
pub(crate) fn role_known(config: &Config, st: &State, name: &str) -> bool {
    find_role(config, st, name).is_some()
        || config.extlinks.iter().any(|e| equal_fold(&e.name, name))
}

/// The reader's role hook: extlinks, :external: roles, then Sphinx's roles.
pub(crate) fn sphinx_role(
    sx: &Sphinx<'_>,
    ctx: &mut ReadContext<'_>,
    call: &RoleCall<'_>,
) -> bool {
    let config = sx.config;
    if let Some(e) = config.extlinks.iter().find(|e| equal_fold(&e.name, call.name)) {
        let _ = extlink_role(ctx, call, e);
        return true;
    }
    let mut st = sx.state();
    let mut rx = RoleCx { ctx, st: &mut st, config };
    if config.intersphinx && is_external(call.name) {
        let _ = external_role(&mut rx, call);
        return true;
    }
    let Some(r) = find_role(config, rx.st, call.name) else { return false };
    let _ = dispatch_role(&mut rx, call, &r);
    true
}

/// Reads a call of a known role.
pub(crate) fn dispatch_role(
    rx: &mut RoleCx<'_, '_>,
    call: &RoleCall<'_>,
    r: &Found<'_>,
) -> Result<(), Fail> {
    match r.kind {
        RoleKind::Xref => xref_role(rx, call, r),
        RoleKind::Generic => text_roles::generic_role(rx.b(), call, call.text),
        RoleKind::Kbd => text_roles::kbd_role(rx.b(), call),
        RoleKind::Gui => text_roles::gui_role(rx.b(), call),
        RoleKind::Samp => text_roles::samp_role(rx.b(), call),
        RoleKind::Abbr => text_roles::abbr_role(rx.b(), call),
        RoleKind::Manpage => text_roles::manpage_role(rx, call),
        RoleKind::Pep | RoleKind::Rfc => {
            text_roles::pep_rfc_role(rx, call, r.kind == RoleKind::Pep)
        }
        RoleKind::Cve | RoleKind::Cwe => {
            text_roles::cve_cwe_role(rx, call, r.kind == RoleKind::Cve)
        }
        RoleKind::Index => text_roles::index_role(rx, call),
        RoleKind::Expr | RoleKind::Texpr => expr_role(rx, call, r),
    }
}

/// c:expr, cpp:expr (code) and c:texpr, cpp:texpr (text).
fn expr_role(rx: &mut RoleCx<'_, '_>, call: &RoleCall<'_>, r: &Found<'_>) -> Result<(), Fail> {
    let code = r.kind == RoleKind::Expr;
    if r.domain == "cpp" && cpp::expr_role(rx, call, code)? {
        return Ok(());
    }
    if r.domain == "c" {
        return c::expr_role(rx, call, code);
    }
    if code {
        let id = rx.b().add_text_leaf(NodeKind::Code, call.span, call.text)?;
        let class = format!("{}-expr", r.domain);
        rx.b().add_attribute_string(id, AttributeKey::Class, &class)
    } else {
        text_roles::role_node(rx.b(), call.span, "texpr", call.text)
    }
}

/// Unescapes marked text.
pub(crate) fn cook(marked: &str) -> String {
    let mut out = String::with_capacity(marked.len());
    unescape(&mut out, marked);
    out
}

/// The reftype of a role: its name, with its domain unless it is the standard one.
fn reftype_of(r: &Found<'_>) -> String {
    if r.domain.is_empty() || r.domain == "std" {
        r.name.to_owned()
    } else {
        format!("{}:{}", r.domain, r.name)
    }
}

/// A cross-reference role: Xref with the processed title and target.
fn xref_role(rx: &mut RoleCx<'_, '_>, call: &RoleCall<'_>, r: &Found<'_>) -> Result<(), Fail> {
    let reftype = reftype_of(r);
    if let Some(rest) = call.marked.strip_prefix('!') {
        let mut title = cook(rest);
        if r.flags.has(XrefFlags::PARENS) {
            title = fix_parens(rx.config, &title, "", false).0;
        }
        return display(rx.b(), call.span, &title, r.flags.has(XrefFlags::CODE));
    }
    let (title_m, target_m, explicit) = split_title(call.marked);
    let mut title = cook(title_m);
    let mut target = collapse(&cook(target_m)).into_owned();
    if r.flags.has(XrefFlags::LOWER) {
        target = target.to_lowercase();
    }
    if r.flags.has(XrefFlags::PARENS) {
        (title, target) = fix_parens(rx.config, &title, &target, explicit);
    }
    let mut x =
        XrefParts { title: title.into(), target: target.into(), explicit, specific: false };
    if r.flags.has(XrefFlags::DOTTED) {
        process_dotted(&mut x, r.domain == "rust");
    }
    if let Some((_, d)) = find_domain(r.domain)
        && let Some(mapped) = d.map_target(rx.st, &x.target)
    {
        x.target = mapped.into();
    }
    if r.flags.has(XrefFlags::AT) {
        x.title = format!("@{}", x.title).into();
    }
    if r.flags.has(XrefFlags::SCOPED) {
        process_scoped(&mut x, r.domain == "cpp");
    }
    if reftype == "token" && !explicit {
        process_token(&mut x);
    }
    if r.flags.has(XrefFlags::INDEX) {
        envvar_index(rx, call.span, &x.target.clone())?;
    }
    let code = r.flags.has(XrefFlags::CODE);
    let (b, st) = rx.b_and_state();
    emit_xref(b, st, call.span, &reftype, r.domain, &x, code)
}

impl<'a> RoleCx<'_, 'a> {
    fn b_and_state(&mut self) -> (&mut Builder<'a>, &mut State) {
        (&mut self.ctx.b, self.st)
    }
}

/// A production token's reference drops a leading "~" from the target and shows the
/// part after its group.
fn process_token(x: &mut XrefParts<'_>) {
    x.target = trim_prefix(&x.target, "~").to_owned().into();
    if let Some(rest) = x.title.strip_prefix('~') {
        let shown = rest.find(':').map_or(rest, |k| &rest[k + 1..]).to_owned();
        x.title = shown.into();
    }
}

/// Emits Xref{link name = target, unresolved; reftype, refexplicit, refspecific, and
/// the context of the domain} > the title (in Code when `code`).
pub(crate) fn emit_xref(
    b: &mut Builder<'_>,
    st: &State,
    span: SourceSpan,
    reftype: &str,
    domain: &str,
    x: &XrefParts<'_>,
    code: bool,
) -> Result<(), Fail> {
    let id = xref_begin(b, st, span, reftype, domain, x)?;
    if code {
        b.add_text_leaf(NodeKind::Code, span, &x.title)?;
    } else if !x.title.is_empty() {
        b.add_text_leaf(NodeKind::Text, span, &x.title)?;
    }
    b.end_node(id)
}

/// Opens an Xref with its link and attributes; the caller adds the display and closes
/// it.
pub(crate) fn xref_begin(
    b: &mut Builder<'_>,
    st: &State,
    span: SourceSpan,
    reftype: &str,
    domain: &str,
    x: &XrefParts<'_>,
) -> Result<NodeId, Fail> {
    let id = b.begin_node(NodeKind::Xref, span)?;
    let name = b.add_text(&x.target)?;
    b.set_link(
        id,
        LinkInfo {
            name,
            kind: LinkKind::Unresolved,
            resolved: INVALID_NODE,
            ..LinkInfo::UNRESOLVED
        },
    )?;
    b.add_custom_attribute(id, "reftype", reftype)?;
    if x.explicit {
        b.add_custom_attribute(id, "refexplicit", "1")?;
    }
    if st.external {
        b.add_custom_attribute(id, "intersphinx", "1")?;
        if !st.inventory.is_empty() {
            b.add_custom_attribute(id, "inventory", &st.inventory)?;
        }
    }
    if x.specific {
        b.add_custom_attribute(id, "refspecific", "1")?;
    }
    let any = reftype == "any";
    let info = XrefInfo { reftype, target: &x.target, any };
    for d in DOMAINS {
        if any || d.name() == domain {
            d.xref_context(b, st, id, &info)?;
        }
    }
    Ok(id)
}

/// Shows text as Code or Text.
pub(crate) fn display(
    b: &mut Builder<'_>,
    span: SourceSpan,
    text: &str,
    code: bool,
) -> Result<(), Fail> {
    let kind = if code { NodeKind::Code } else { NodeKind::Text };
    b.add_text_leaf(kind, span, text)?;
    Ok(())
}

/// Reads text with a domain's role, as an info field of that domain links its argument
/// or type (Sphinx's Field.make_xref with an inliner).
pub(crate) fn field_role(
    rx: &mut RoleCx<'_, '_>,
    domain: &str,
    name: &str,
    text: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    let Some(r) = role_in(domain, name) else { return display(rx.b(), span, text, false) };
    let call = RoleCall { name, marked: text, text, span, content: span };
    dispatch_role(rx, &call, &r)
}

/// Without an explicit title, a trailing "()" leaves the title and comes back when
/// add_function_parentheses is set; the target never keeps it.
pub(crate) fn fix_parens(
    config: &Config,
    title: &str,
    target: &str,
    explicit: bool,
) -> (String, String) {
    let mut t = title.to_owned();
    if !explicit {
        t = trim_suffix(&t, "()").to_owned();
        if config.add_function_parentheses {
            t.push_str("()");
        }
    }
    (t, trim_suffix(target, "()").to_owned())
}

/// Python's and JavaScript's link processing (and the Odin and Rust domains').
/// With `colons` (the rust domain), "~" also shortens at the last "::".
fn process_dotted(x: &mut XrefParts<'_>, colons: bool) {
    if !x.explicit {
        x.title = x.title.trim_start_matches('.').to_owned().into();
        x.target = x.target.trim_start_matches('~').to_owned().into();
        if let Some(rest) = x.title.strip_prefix('~') {
            let dot = rest.rfind('.').map(|k| k + 1);
            let path = if colons { rest.rfind("::").map(|k| k + 2) } else { None };
            let cut = dot.max(path).unwrap_or(0);
            let shown = rest[cut..].to_owned();
            x.title = shown.into();
        }
    }
    if let Some(rest) = x.target.strip_prefix('.') {
        x.target = rest.to_owned().into();
        x.specific = true;
    }
}

/// C's and C++'s: "~" shows only the last component.
fn process_scoped(x: &mut XrefParts<'_>, cpp: bool) {
    if x.explicit {
        return;
    }
    x.target = x.target.trim_start_matches('~').to_owned().into();
    let Some(rest) = x.title.strip_prefix('~') else { return };
    let b = rest.as_bytes();
    let cut = (0..b.len()).rev().find(|&i| {
        let dot = b[i] == b'.' && !cpp;
        let colons = cpp && b[i] == b':' && i > 0 && b[i - 1] == b':';
        dot || colons
    });
    let shown = cut.map_or(rest, |i| &rest[i + 1..]).to_owned();
    x.title = shown.into();
}

/// Emits the index entries and target Sphinx adds before :envvar:.
fn envvar_index(rx: &mut RoleCx<'_, '_>, span: SourceSpan, name: &str) -> Result<(), Fail> {
    let index = rx.st.index_id();
    let b = rx.b();
    let id = b.begin_node(NodeKind::Index, span)?;
    b.add_attribute_string(id, AttributeKey::Id, &index)?;
    let entries = format!("single: {name}\nsingle: environment variable; {name}");
    b.add_custom_attribute(id, "entries", &entries)?;
    b.end_node(id)
}

/// Link{External, url with %s replaced; class extlink-<name>} > the explicit title, the
/// caption with %s replaced, or the URL.
fn extlink_role(ctx: &mut ReadContext<'_>, call: &RoleCall<'_>, e: &Extlink) -> Result<(), Fail> {
    let (title_m, target_m, explicit) = split_title(call.marked);
    let (title, part) = (cook(title_m), cook(target_m));
    let dest = percent_s(&e.url, &part);
    let b = &mut ctx.b;
    let id = b.begin_node(NodeKind::Link, call.span)?;
    let destination = b.add_text(&dest)?;
    b.set_link(
        id,
        LinkInfo {
            destination,
            kind: LinkKind::External,
            resolved: INVALID_NODE,
            ..LinkInfo::UNRESOLVED
        },
    )?;
    b.add_attribute_string(id, AttributeKey::Class, &format!("extlink-{}", e.name))?;
    let label = if explicit {
        title
    } else if !e.caption.is_empty() {
        percent_s(&e.caption, &part)
    } else {
        dest
    };
    b.add_text_leaf(NodeKind::Text, call.span, &label)?;
    b.end_node(id)
}

/// Returns `pattern` with "%s" replaced by `part` and "%%" by "%".
pub(crate) fn percent_s(pattern: &str, part: &str) -> String {
    let mut out = String::with_capacity(pattern.len() + part.len());
    let mut it = pattern.chars().peekable();
    while let Some(c) = it.next() {
        if c == '%' {
            match it.peek() {
                Some('s') => {
                    out.push_str(part);
                    it.next();
                    continue;
                }
                Some('%') => {
                    out.push('%');
                    it.next();
                    continue;
                }
                _ => {}
            }
        }
        out.push(c);
    }
    out
}

/// Tells whether a role name is sphinx.ext.intersphinx's :external:name: or
/// :external+inventory:name: form.
fn is_external(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() > 9 && equal_fold(&name[..8], "external") && (b[8] == b':' || b[8] == b'+')
}

/// Reads :external:domain:role:, :external:role:, and their +inventory forms, as
/// Sphinx's IntersphinxRole: the role is looked up in the domain given, or in the
/// default domain and then the standard one; the reference it makes resolves only
/// through the inventories (one named, or any), which the host checks.
fn external_role(rx: &mut RoleCx<'_, '_>, call: &RoleCall<'_>) -> Result<(), Fail> {
    let mut inventory = "";
    let mut suffix = &call.name[9..];
    if call.name.as_bytes()[8] == b'+' {
        let (head, _, tail) = crate::text::partition(suffix, ':');
        (inventory, suffix) = (head, tail);
    }
    let (domain, role) = match suffix.split_once(':') {
        Some((d, r)) if r.contains(':') || d.is_empty() => {
            rx.report(EXTERNAL_SUFFIX, call.span);
            return Ok(());
        }
        Some((d, r)) => (d, r),
        None => ("", suffix),
    };
    let r = if !domain.is_empty() {
        if !domain_known(domain) {
            rx.report(EXTERNAL_DOMAIN, call.span);
            return Ok(());
        }
        role_in(domain, role).or_else(|| if domain == "std" { role_in("", role) } else { None })
    } else {
        let current = rx.st.domain.clone();
        let own =
            if !current.is_empty() && current != "std" { role_in(&current, role) } else { None };
        own.or_else(|| role_in("std", role)).or_else(|| role_in("", role))
    };
    let Some(r) = r else {
        rx.report(EXTERNAL_ROLE, call.span);
        return Ok(());
    };
    let own = !inventory.is_empty() && inventory == rx.config.intersphinx_resolve_self;
    rx.st.external = !own;
    rx.st.inventory = if own { String::new() } else { inventory.to_owned() };
    let result = dispatch_role(rx, call, &r);
    rx.st.external = false;
    rx.st.inventory.clear();
    result
}
