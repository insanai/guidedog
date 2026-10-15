//! The extension interface: a dialect, such as the Sphinx layer, adds directives and
//! roles to the reader. The reader consults a dialect's handlers before the standard
//! ones, so a dialect may also redefine a standard directive or role (Sphinx's
//! code-block and math take more options than Docutils').
//!
//! Handlers use the parser's public methods: the parsed `Directive`, `parse_body` and
//! `inline` for nested content (in the markup the directive was written in; see
//! `Host`), `open`/`close` and `ctx.b` for nodes, and `report` for messages.

use guidedoc::{Arg, Fail, ReadContext, arg_span};
use guidedoc_rst_inline::{RoleCall, RoleHook};

use crate::directives::Directive;
use crate::messages::DIRECTIVE_UNKNOWN;
use crate::parser::Parser;
use crate::specs::{ArgRule, ContentRule, OptSpec, equal_fold};

/// A dialect's directive: its rules, checked by the reader before it runs, and a tag
/// the dialect chooses to find its own code again in `Extension::run_directive`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectiveHandler<'h> {
    /// The directive's canonical name ("py:function"), compared case-insensitively.
    pub name: &'h str,
    /// Whether it takes an argument.
    pub arg: ArgRule,
    /// The last argument may contain white space.
    pub final_ws: bool,
    /// Whether it takes content.
    pub content: ContentRule,
    /// The options it accepts.
    pub options: &'h [OptSpec],
    /// The dialect's own identification of the handler, such as a table index.
    pub tag: usize,
}

/// Returns the handler of `table` named `name`, compared case-insensitively.
pub fn find_handler<'h>(
    table: &'h [DirectiveHandler<'h>],
    name: &str,
) -> Option<&'h DirectiveHandler<'h>> {
    table.iter().find(|h| equal_fold(h.name, name))
}

/// A dialect's directives, roles, and hooks. Methods take `&self`: a dialect keeps its
/// per-document state in cells, because handlers run re-entrantly (a directive's
/// content holds more directives).
pub trait Extension {
    /// Returns the handler of directive `name`, or `None` for one the dialect does not
    /// define (the standard directives are consulted next).
    fn directive(&self, name: &str) -> Option<DirectiveHandler<'_>>;

    /// Runs a checked directive whose handler `directive` returned (`d.handler()`).
    /// Problems are reported; only a builder failure is an `Err`.
    fn run_directive(&self, p: &mut Parser<'_, '_>, d: &Directive<'_>) -> Result<(), Fail> {
        p.report(DIRECTIVE_UNKNOWN, d.name_span, arg_span(d.name_span), Arg::None);
        Ok(())
    }

    /// Handles interpreted text before the document's and the standard roles: emits
    /// nodes through `ctx.b` and returns true, or returns false for a role it does not
    /// know.
    fn role(&self, _ctx: &mut ReadContext<'_>, _call: &RoleCall<'_>) -> bool {
        false
    }

    /// Runs before the main source's blocks.
    fn begin(&self, _p: &mut Parser<'_, '_>) -> Result<(), Fail> {
        Ok(())
    }

    /// Tells whether the dialect has a prologue (`prologue`). With one, the main
    /// source's leading field lines are read first, on their own, as Sphinx inserts
    /// rst_prolog after a document's bibliographic fields.
    fn has_prologue(&self) -> bool {
        false
    }

    /// Runs after the main source's leading field lines (see `has_prologue`).
    fn prologue(&self, _p: &mut Parser<'_, '_>) -> Result<(), Fail> {
        Ok(())
    }

    /// Runs after the main source's blocks, before sections close (an epilogue).
    fn end(&self, _p: &mut Parser<'_, '_>) -> Result<(), Fail> {
        Ok(())
    }

    /// Supplies a default substitution, such as Sphinx's `|version|`: resolution asks it
    /// for every reference whose name no definition in the document has.
    fn substitution(&self, _name: &str) -> Option<String> {
        None
    }
}

/// The inline parser's role hook for an extension.
pub(crate) struct ExtensionHook<'e>(pub Option<&'e dyn Extension>);

impl RoleHook for ExtensionHook<'_> {
    fn role(&mut self, ctx: &mut ReadContext<'_>, call: &RoleCall<'_>) -> bool {
        self.0.is_some_and(|e| e.role(ctx, call))
    }
}
