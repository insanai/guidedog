//! Domains: groups of object descriptions and cross-reference roles (py, c, cpp, js,
//! rst, std, math, odin, rust). Each domain is a value implementing [`Domain`]; the
//! Sphinx layer finds its directives and roles through [`DOMAINS`] and runs the generic
//! object description (`object::object_directive`), calling the domain's hooks for the
//! parts that differ: the signature, the context nested content reads in, the info
//! fields, the table of contents entry, and the context cross-references carry.
//!
//! The surface here is stable for domain authors: changes are additive only. See the
//! README section "Domain API for domain authors".

pub(crate) mod c;
pub(crate) mod cpp;
pub(crate) mod js;
pub(crate) mod math;
pub(crate) mod object;
pub(crate) mod odin;
pub(crate) mod python;
pub(crate) mod rst;
pub(crate) mod rust;
pub(crate) mod std;

use ::std::borrow::Cow;

use guidedoc::{Arg, Builder, Fail, Message, NodeFlags, NodeId, NodeKind, SourceSpan, arg_span};
use guidedoc_rst::{ArgRule, ContentRule, Directive, OptSpec, Parser};

use crate::config::Config;
use crate::extension::Sphinx;
use crate::state::State;

pub(crate) use crate::docfields::{DocField, FieldShape};
pub(crate) use crate::roles::{RoleSpec, XrefFlags, XrefParts, emit_xref};

/// How the Sphinx layer runs a domain directive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DirectiveKind {
    /// An object description: Desc > Desc_Signature per signature line, an Index of its
    /// entries, and Desc_Content, with the domain's hooks (`Domain::signature`, ...).
    Object,
    /// A directive of the domain's own, run by `Domain::run` with this tag.
    Run(u16),
    /// A generating directive (automodule, odin:autoproc, ...): the host's autodoc
    /// provider for the domain writes reStructuredText that is parsed in its place.
    Autodoc,
}

/// One directive of a domain, with the rules the reader checks before it runs.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DomainDirective {
    /// The full name, domain included ("odin:procedure"); compared case-insensitively.
    pub name: &'static str,
    /// How it runs.
    pub kind: DirectiveKind,
    /// Whether it takes an argument.
    pub arg: ArgRule,
    /// The last argument may contain white space.
    pub final_ws: bool,
    /// Whether it takes content.
    pub content: ContentRule,
    /// The options it accepts.
    pub options: &'static [OptSpec],
}

impl DomainDirective {
    /// An object description: a required argument (the signatures, one per line),
    /// optional content.
    pub(crate) const fn object(name: &'static str, options: &'static [OptSpec]) -> Self {
        DomainDirective {
            name,
            kind: DirectiveKind::Object,
            arg: ArgRule::Required,
            final_ws: true,
            content: ContentRule::Optional,
            options,
        }
    }

    /// A generating directive: like an object description, its options checked by the
    /// host.
    pub(crate) const fn autodoc(name: &'static str, options: &'static [OptSpec]) -> Self {
        DomainDirective { kind: DirectiveKind::Autodoc, ..DomainDirective::object(name, options) }
    }

    /// A directive of the domain's own, run by `Domain::run(.., tag)`.
    pub(crate) const fn run(
        name: &'static str,
        tag: u16,
        rules: (ArgRule, bool, ContentRule),
        options: &'static [OptSpec],
    ) -> Self {
        DomainDirective {
            name,
            kind: DirectiveKind::Run(tag),
            arg: rules.0,
            final_ws: rules.1,
            content: rules.2,
            options,
        }
    }

    /// The name without its domain ("procedure").
    pub(crate) fn local_name(&self) -> &'static str {
        self.name.split_once(':').map_or(self.name, |(_, local)| local)
    }
}

/// A domain. Methods with defaults are hooks a domain overrides when it needs them.
/// Hooks that receive a `Cx` hold the per-document state borrowed: they must not parse
/// nested content (`p.parse_body`, `p.inline`, ...). `run` receives the Sphinx layer
/// instead and borrows the state itself (`sx.state()`) only between such calls.
pub(crate) trait Domain: Sync {
    /// The domain's name ("odin").
    fn name(&self) -> &'static str;

    /// Its directives.
    fn directives(&self) -> &'static [DomainDirective];

    /// Its roles.
    fn roles(&self) -> &'static [RoleSpec] {
        &[]
    }

    /// The info field types of an object type (Sphinx's doc_field_types); fields of no
    /// listed type are kept as written.
    fn fields(&self, objtype: &str) -> &'static [DocField] {
        let _ = objtype;
        &[]
    }

    /// Emits one signature of an object description as Desc_Signature (with ids,
    /// "fullname", and a toc entry), updates `o` (fullname, prefix, `signatures`), and
    /// adds index entries with `o.entry`. Signatures it cannot read become
    /// `cx.plain_signature`.
    fn signature(
        &self,
        cx: &mut Cx<'_, '_, '_>,
        o: &mut Object<'_>,
        sig: &str,
        span: SourceSpan,
    ) -> Result<(), Fail> {
        let _ = o;
        cx.plain_signature(sig, span)
    }

    /// Runs a `DirectiveKind::Run` directive.
    fn run(
        &self,
        sx: &Sphinx<'_>,
        p: &mut Parser<'_, '_>,
        d: &Directive<'_>,
        tag: u16,
    ) -> Result<(), Fail> {
        let _ = (sx, tag);
        p.report(
            guidedoc_rst::messages::DIRECTIVE_UNKNOWN,
            d.name_span,
            arg_span(d.name_span),
            Arg::None,
        );
        Ok(())
    }

    /// Emits what an object's content starts with, inside the open Desc_Content and
    /// before the content itself (the odin domain's `:availability:` paragraph).
    fn content_begin(&self, cx: &mut Cx<'_, '_, '_>, o: &Object<'_>) -> Result<(), Fail> {
        let _ = (cx, o);
        Ok(())
    }

    /// Pushes the context an object's content reads in (a class, a type, a package);
    /// returns what to pop, for `after_content`. Called only when a signature gave a
    /// name (`o.signatures > 0`).
    fn before_content(&self, cx: &mut Cx<'_, '_, '_>, o: &Object<'_>) -> u32 {
        let _ = (cx, o);
        0
    }

    /// Pops what `before_content` pushed (0 when it was not called); called after every
    /// object's content.
    fn after_content(&self, st: &mut State, o: &Object<'_>, pushed: u32) {
        let _ = (st, o, pushed);
    }

    /// The text of an object's table of contents entry (`toc_entry_name`); `None` for
    /// no entry. `module` is the module or package the signature named.
    fn toc_name(
        &self,
        config: &Config,
        st: &State,
        o: &Object<'_>,
        names: TocNames<'_>,
    ) -> Option<String> {
        let _ = (config, st, o, names);
        None
    }

    /// Rewrites the target of one of the domain's cross-references (the odin domain
    /// follows an import alias); `None` keeps it.
    fn map_target(&self, st: &State, target: &str) -> Option<String> {
        let _ = (st, target);
        None
    }

    /// Adds the context a cross-reference carries (Custom attributes such as
    /// "py:module") to the Xref `id` being built; called for the domain's own roles and
    /// for `any` (`x.any`), in the order of `DOMAINS`.
    fn xref_context(
        &self,
        b: &mut Builder<'_>,
        st: &State,
        id: NodeId,
        x: &XrefInfo<'_>,
    ) -> Result<(), Fail> {
        let _ = (b, st, id, x);
        Ok(())
    }

    /// The context an autodoc request carries: (module, class), for Python the current
    /// py:module and py:class.
    fn autodoc_context(&self, st: &State) -> (String, String) {
        let _ = st;
        (String::new(), String::new())
    }
}

/// What `Domain::xref_context` knows of the cross-reference being built.
#[derive(Debug, Clone, Copy)]
pub(crate) struct XrefInfo<'x> {
    /// Its reftype ("odin:type", "ref", "any").
    pub reftype: &'x str,
    /// Its target, after the role's processing.
    pub target: &'x str,
    /// It is an `any` reference: every domain adds its context.
    pub any: bool,
}

/// The names `Domain::toc_name` builds an entry from.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TocNames<'n> {
    /// The object's full name within its module.
    pub fullname: &'n str,
    /// The module or package, when the signature named one.
    pub module: &'n str,
    /// The signature as written.
    pub sig: &'n str,
}

/// Every domain, in lookup order.
/// The order is the order cross-references list their context attributes in.
pub(crate) static DOMAINS: [&dyn Domain; 9] = [
    &python::DOMAIN,
    &js::DOMAIN,
    &odin::DOMAIN,
    &rust::DOMAIN,
    &std::DOMAIN,
    &c::DOMAIN,
    &cpp::DOMAIN,
    &rst::DOMAIN,
    &math::DOMAIN,
];

/// Returns the domain called `name` (exactly, lowercase) and its index.
pub(crate) fn find_domain(name: &str) -> Option<(usize, &'static dyn Domain)> {
    DOMAINS.iter().copied().enumerate().find(|(_, d)| d.name() == name)
}

/// Tells whether `name` is a domain's name.
pub(crate) fn domain_known(name: &str) -> bool {
    find_domain(name).is_some()
}

/// One object description while it is read: the directive, its domain and object type,
/// its index entries, and the names of its last signature, which nested content uses
/// as context.
#[derive(Debug)]
pub(crate) struct Object<'o> {
    /// The directive.
    pub d: &'o Directive<'o>,
    /// The domain's name.
    pub domain: &'static str,
    /// The object type: the directive's name without its domain ("function").
    pub objtype: &'o str,
    /// `:no-index:` (or `:noindex:`): no ids, no entries.
    pub noindex: bool,
    /// `:no-index-entry:`: ids, but no entries.
    pub noentry: bool,
    /// The index entries, one per line ("single: name (type)").
    pub entries: String,
    /// Each entry's target: its signature's id, one per line.
    pub targets: String,
    /// The number of entries.
    pub entry_count: usize,
    /// The last signature's full name.
    pub fullname: String,
    /// The last signature's prefix.
    pub prefix: String,
    /// C++: the last signature's symbol path.
    pub cpp_path: String,
    /// C++: its object type.
    pub cpp_objtype: String,
    /// The signatures that gave a name.
    pub signatures: usize,
    /// What a domain's signature leaves for its `before_content` (for the odin
    /// domain, the polymorphic names members see).
    pub extra: String,
}

impl<'o> Object<'o> {
    /// An object description of `domain` read from `d`, whose object type is `objtype`.
    pub(crate) fn new(d: &'o Directive<'o>, domain: &'static str, objtype: &'o str) -> Self {
        Object {
            d,
            domain,
            objtype,
            noindex: d.has_option("no-index") || d.has_option("noindex"),
            noentry: d.has_option("no-index-entry") || d.has_option("noindexentry"),
            entries: String::new(),
            targets: String::new(),
            entry_count: 0,
            fullname: String::new(),
            prefix: String::new(),
            cpp_path: String::new(),
            cpp_objtype: String::new(),
            signatures: 0,
            extra: String::new(),
        }
    }

    /// Starts an index entry of a type ("single", "pair"); `entry_add` writes its text.
    pub(crate) fn entry_begin(&mut self, kind: &str) {
        self.entry_count += 1;
        if !self.entries.is_empty() {
            self.entries.push('\n');
        }
        self.entries.push_str(kind);
        self.entries.push_str(": ");
    }

    /// Appends text to the entry begun last.
    pub(crate) fn entry_add(&mut self, part: &str) {
        self.entries.push_str(part);
    }

    /// Adds an entry of a type whose text is `parts` joined.
    pub(crate) fn entry(&mut self, kind: &str, parts: &[&str]) {
        self.entry_begin(kind);
        for part in parts {
            self.entry_add(part);
        }
    }

    /// Tells whether the directive has flag option `name`.
    pub(crate) fn flag(&self, name: &str) -> bool {
        self.d.has_option(name)
    }

    /// Returns option `name`'s value, joined and trimmed.
    pub(crate) fn option(&self, name: &str) -> Option<Cow<'o, str>> {
        self.d.option_value(name)
    }
}

/// What a domain hook works with: the parser (builder, reports, spans), the
/// per-document state, and the configuration.
pub(crate) struct Cx<'c, 'p, 'a> {
    /// The parser.
    pub p: &'c mut Parser<'p, 'a>,
    /// The per-document state.
    pub st: &'c mut State,
    /// The configuration.
    pub config: &'c Config,
}

impl<'a> Cx<'_, '_, 'a> {
    /// Returns the builder.
    pub(crate) fn b(&mut self) -> &mut Builder<'a> {
        &mut self.p.ctx.b
    }

    /// Reports `m` at `span`, quoting the span.
    pub(crate) fn report(&mut self, m: Message, span: SourceSpan) {
        self.p.report(m, span, arg_span(span), Arg::None);
    }

    /// Emits `kind > Text(text)`; no Text for empty text.
    pub(crate) fn leaf_in(
        &mut self,
        kind: NodeKind,
        span: SourceSpan,
        text: &str,
    ) -> Result<(), Fail> {
        leaf_in(self.b(), kind, span, text)
    }

    /// Emits a Text leaf, unless `text` is empty.
    pub(crate) fn text_leaf(&mut self, span: SourceSpan, text: &str) -> Result<(), Fail> {
        text_leaf(self.b(), span, text)
    }

    /// Adds Custom attribute `name` = `value` to the node begun last.
    pub(crate) fn custom(&mut self, id: NodeId, name: &str, value: &str) -> Result<(), Fail> {
        self.b().add_custom_attribute(id, name, value)
    }

    /// Adds the object's id (Sphinx's make_id of `prefix` and `name`) and, when `alias`
    /// differs from the made id and is free, `alias` as a second id. The made id
    /// becomes the target of the object's entries (`st.object_id`) and is returned.
    pub(crate) fn add_ids(
        &mut self,
        id: NodeId,
        prefix: &str,
        name: &str,
        alias: &str,
    ) -> Result<String, Fail> {
        let made = self.st.make_object_id(prefix, name);
        self.b().add_attribute_string(id, guidedoc::AttributeKey::Id, &made)?;
        self.st.object_id.clone_from(&made);
        if !alias.is_empty() && alias != made && !self.st.id_taken(alias) {
            self.b().add_attribute_string(id, guidedoc::AttributeKey::Id, alias)?;
            self.st.note_id(alias);
        }
        Ok(made)
    }

    /// Sphinx's fallback for a signature it cannot read: Desc_Signature > Desc_Name with
    /// the text, no id, no entry.
    pub(crate) fn plain_signature(&mut self, sig: &str, span: SourceSpan) -> Result<(), Fail> {
        let id = self.b().begin_node(NodeKind::DescSignature, span)?;
        self.leaf_in(NodeKind::DescName, span, sig)?;
        self.b().end_node(id)
    }

    /// Adds Custom "toc" to Desc_Signature `id`: the text of the object's table of
    /// contents entry (`Domain::toc_name`). Nothing is added when toc_object_entries is
    /// off, the object is not indexed, or the domain gives no entry. Call it before the
    /// signature's first child.
    pub(crate) fn toc_entry_name(
        &mut self,
        o: &Object<'_>,
        id: NodeId,
        names: TocNames<'_>,
    ) -> Result<(), Fail> {
        if !self.config.toc_object_entries || o.noindex || names.fullname.is_empty() {
            return Ok(());
        }
        let Some((_, domain)) = find_domain(o.domain) else { return Ok(()) };
        match domain.toc_name(self.config, self.st, o, names) {
            Some(name) if !name.is_empty() => self.custom(id, "toc", &name),
            _ => Ok(()),
        }
    }

    /// Emits a cross-reference: Xref{link name = target, unresolved; reftype, the
    /// context of `domain`} > the title (in Code when `code`).
    pub(crate) fn xref(
        &mut self,
        span: SourceSpan,
        reftype: &str,
        domain: &str,
        x: &XrefParts<'_>,
        code: bool,
    ) -> Result<(), Fail> {
        emit_xref(&mut self.p.ctx.b, self.st, span, reftype, domain, x, code)
    }

    /// Returns the number of nodes written so far, for `generated`.
    pub(crate) fn node_count(&mut self) -> usize {
        self.b().snap.nodes.len()
    }

    /// Marks the nodes written since `first` (a `node_count`) as Generated: words the
    /// layer adds, which translation leaves out of the content's message.
    pub(crate) fn generated(&mut self, first: usize) {
        generated(self.b(), first);
    }
}

/// Emits `kind > Text(text)`; no Text for empty text.
pub(crate) fn leaf_in(
    b: &mut Builder<'_>,
    kind: NodeKind,
    span: SourceSpan,
    text: &str,
) -> Result<(), Fail> {
    let id = b.begin_node(kind, span)?;
    text_leaf(b, span, text)?;
    b.end_node(id)
}

/// Emits a Text leaf, unless `text` is empty.
pub(crate) fn text_leaf(b: &mut Builder<'_>, span: SourceSpan, text: &str) -> Result<(), Fail> {
    if !text.is_empty() {
        b.add_text_leaf(NodeKind::Text, span, text)?;
    }
    Ok(())
}

/// Marks the nodes written since `first` as Generated.
pub(crate) fn generated(b: &mut Builder<'_>, first: usize) {
    if !b.is_ok() {
        return;
    }
    let used = b.snap.nodes.len();
    for i in first..used {
        b.snap.nodes[NodeId::from_index(i)].flags.insert(NodeFlags::GENERATED);
    }
}
