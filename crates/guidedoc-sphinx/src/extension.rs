//! The Sphinx layer as the reStructuredText reader's dialect: directive lookup (the
//! default domain, then the standard domain, then the project's declarations), the
//! dispatch of every directive to its handler, the role hook, and the document hooks
//! (rst_prolog, rst_epilog, default substitutions).

use std::cell::{RefCell, RefMut};

use guidedoc::{DocumentReading, Fail, NodeKind, ROOT, ReadContext};
use guidedoc_rst::{ArgRule, ContentRule, Directive, DirectiveHandler, Extension, Parser};
use guidedoc_rst_inline::RoleCall;

use crate::config::Config;
use crate::domains::{Cx, DOMAINS, DirectiveKind, DomainDirective, object};
use crate::state::State;
use crate::text::equal_fold;
use crate::{autodoc, directives, object_types, roles};

/// The tag group of the layer's own directives (toctree, code-block, ...).
const CORE: usize = 0xFE;
/// The tag group of the object types the project declares.
const DECLARED: usize = 0xFF;
/// The most object types a project may declare.
const MAX_OBJECT_TYPES: usize = 64;

/// The Sphinx layer for one document read: the configuration, the per-document state,
/// and the names of the project's declared directives. Handlers run re-entrantly, so the
/// state is in a cell; borrow it (`state`) only between calls that parse nested content.
#[derive(Debug)]
pub(crate) struct Sphinx<'c> {
    /// The configuration.
    pub config: &'c Config,
    state: RefCell<State>,
    /// "std:<directive>" for each declared object type.
    declared: Vec<String>,
}

impl<'c> Sphinx<'c> {
    /// The layer for one read with `config`.
    pub(crate) fn new(config: &'c Config) -> Sphinx<'c> {
        let declared = config
            .object_types
            .iter()
            .take(MAX_OBJECT_TYPES)
            .map(|t| format!("std:{}", t.directive))
            .collect();
        Sphinx { config, state: RefCell::new(State::new(config)), declared }
    }

    /// The layer for one read with `config`, continuing from `state` (the MyST
    /// extension keeps the state between its calls).
    pub(crate) fn with_state(config: &'c Config, state: State) -> Sphinx<'c> {
        let sx = Sphinx::new(config);
        *sx.state.borrow_mut() = state;
        sx
    }

    /// Gives the per-document state back.
    pub(crate) fn into_state(self) -> State {
        self.state.into_inner()
    }

    /// Borrows the per-document state. Panics when it is already borrowed: a handler
    /// that holds it while parsing nested content violates the layer's invariant.
    pub(crate) fn state(&self) -> RefMut<'_, State> {
        self.state.borrow_mut()
    }

    /// Runs `f` with a domain context: the parser, the state, the configuration.
    pub(crate) fn with_cx<R>(
        &self,
        p: &mut Parser<'_, '_>,
        f: impl FnOnce(&mut Cx<'_, '_, '_>) -> R,
    ) -> R {
        let mut st = self.state();
        let mut cx = Cx { p, st: &mut st, config: self.config };
        f(&mut cx)
    }

    /// Finds `name` in a domain's table: the full name, compared case-insensitively.
    fn qualified(&self, domain: &str, name: &str) -> Option<DirectiveHandler<'static>> {
        let (group, d) = DOMAINS.iter().enumerate().find(|(_, d)| equal_fold(d.name(), domain))?;
        let (k, dd) = d.directives().iter().enumerate().find(|(_, dd)| {
            let local = dd.local_name();
            dd.name.len() == domain.len() + 1 + local.len() && equal_fold(local, name)
        })?;
        Some(domain_handler(dd, group, k))
    }

    /// Finds a name with a domain ("py:function", "rst:directive:option").
    fn with_domain(&self, name: &str) -> Option<DirectiveHandler<'static>> {
        let (domain, local) = name.split_once(':')?;
        self.qualified(domain, local)
    }

    /// Finds a declared object type's directive, or an alias's target.
    fn declared(&self, name: &str) -> Option<DirectiveHandler<'_>> {
        for (k, t) in self.config.object_types.iter().take(MAX_OBJECT_TYPES).enumerate() {
            if equal_fold(&t.directive, name) {
                return Some(declared_handler(&self.declared[k], t.target, k));
            }
        }
        let domain = self.state.borrow().domain.clone();
        for a in &self.config.directive_aliases {
            if !equal_fold(&a.name, name) || equal_fold(&a.target, name) {
                continue;
            }
            // A target may name its domain: awaitablefunction = "py:function".
            if a.target.contains(':') {
                return self.with_domain(&a.target);
            }
            if let Some(h) = self.qualified("std", &a.target) {
                return Some(h);
            }
            if !domain.is_empty()
                && let Some(h) = self.qualified(&domain, &a.target)
            {
                return Some(h);
            }
        }
        None
    }
}

/// A domain directive's handler; its tag is the domain's index and the directive's.
fn domain_handler(dd: &DomainDirective, group: usize, k: usize) -> DirectiveHandler<'static> {
    DirectiveHandler {
        name: dd.name,
        arg: dd.arg,
        final_ws: dd.final_ws,
        content: dd.content,
        options: dd.options,
        tag: (group << 16) | k,
    }
}

/// A declared object type's handler: an object description, or a target only.
fn declared_handler(name: &str, target: bool, k: usize) -> DirectiveHandler<'_> {
    DirectiveHandler {
        name,
        arg: ArgRule::Required,
        final_ws: true,
        content: if target { ContentRule::None } else { ContentRule::Optional },
        options: if target { &[] } else { object::OPTS_OBJECT },
        tag: (DECLARED << 16) | k,
    }
}

impl Extension for Sphinx<'_> {
    fn directive(&self, name: &str) -> Option<DirectiveHandler<'_>> {
        if let Some((k, c)) = directives::find_core(name) {
            return Some(DirectiveHandler {
                name: c.name,
                arg: c.arg,
                final_ws: c.final_ws,
                content: c.content,
                options: c.options,
                tag: (CORE << 16) | k,
            });
        }
        if name.contains(':') {
            return self.with_domain(name);
        }
        let domain = self.state.borrow().domain.clone();
        if !domain.is_empty()
            && let Some(h) = self.qualified(&domain, name)
        {
            return Some(h);
        }
        self.qualified("std", name).or_else(|| self.declared(name))
    }

    fn run_directive(&self, p: &mut Parser<'_, '_>, d: &Directive<'_>) -> Result<(), Fail> {
        let Some(tag) = d.handler().map(|h| h.tag) else { return Ok(()) };
        let (group, k) = (tag >> 16, tag & 0xFFFF);
        match group {
            CORE => directives::run_core(self, p, d, k),
            DECLARED => object_types::run_declared(self, p, d, k),
            _ => {
                let Some(domain) = DOMAINS.get(group).copied() else { return Ok(()) };
                let Some(dd) = domain.directives().get(k) else { return Ok(()) };
                match dd.kind {
                    DirectiveKind::Object => {
                        object::object_directive(self, p, d, domain, dd.local_name())
                    }
                    DirectiveKind::Run(t) => domain.run(self, p, d, t),
                    DirectiveKind::Autodoc => {
                        autodoc::autodoc_directive(self, p, d, domain.name())
                    }
                }
            }
        }
    }

    fn role(&self, ctx: &mut ReadContext<'_>, call: &RoleCall<'_>) -> bool {
        roles::sphinx_role(self, ctx, call)
    }

    fn begin(&self, p: &mut Parser<'_, '_>) -> Result<(), Fail> {
        // Sphinx's transforms give ids of their own (guidedoc's anchors_order).
        let b = &mut p.ctx.b;
        if !b.snap.nodes.is_empty() && b.snap.nodes[ROOT].kind == NodeKind::Document {
            b.set_value(ROOT, DocumentReading::Sphinx as i32);
        }
        if !self.config.default_role.is_empty() {
            p.inline_state().default_role.clone_from(&self.config.default_role);
        }
        Ok(())
    }

    fn has_prologue(&self) -> bool {
        !self.config.rst_prolog.is_empty()
    }

    fn prologue(&self, p: &mut Parser<'_, '_>) -> Result<(), Fail> {
        parse_added(p, "<rst_prolog>", &self.config.rst_prolog)
    }

    fn end(&self, p: &mut Parser<'_, '_>) -> Result<(), Fail> {
        parse_added(p, "<rst_epilog>", &self.config.rst_epilog)
    }

    /// Sphinx's DefaultSubstitutions: |version|, |release|, and |today| come from the
    /// configuration when the document does not define them.
    fn substitution(&self, name: &str) -> Option<String> {
        match name {
            "version" => Some(self.config.version.clone()),
            "release" => Some(self.config.release.clone()),
            "today" => Some(self.config.today.clone()),
            _ => None,
        }
    }
}

/// Parses `text` as a source of its own named `name`, where sections are allowed.
fn parse_added(p: &mut Parser<'_, '_>, name: &str, text: &str) -> Result<(), Fail> {
    if text.is_empty() {
        return Ok(());
    }
    match p.ctx.add_source(name, text) {
        Some(id) => p.parse_source(id, true),
        None => Ok(()),
    }
}
