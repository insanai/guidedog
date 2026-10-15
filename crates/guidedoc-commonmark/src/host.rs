//! The reader's document-wide state, shared by the parse of the document and the parses
//! of nested content (directive bodies, footnotes, substitutions).

use std::any::Any;
use std::collections::HashMap;
use std::sync::OnceLock;

use guidedoc::{Arg, Builder, Fail, Message, ReadContext, SourceSpan, Text, normalize_label};

use crate::extension::Extension;
use crate::messages::SYNTAX_RULE_UNKNOWN;
use crate::myst::MystState;
use crate::settings::{MYST_RULES, Rule, Rules, Settings, SyntaxExtension, SyntaxExtensions};

/// A link reference definition, its destination and title decoded.
#[derive(Debug, Clone, Default)]
pub(crate) struct Ref {
    pub dest: String,
    pub title: Option<String>,
}

/// The reader's state for one read. Directive and role handlers receive it and use its
/// public methods (see `extension.rs`) to build nodes and parse nested content.
pub struct Host<'h, 'a> {
    pub(crate) ctx: &'h mut ReadContext<'a>,
    pub(crate) settings: &'h Settings,
    /// The MyST syntax is on: the "myst" reader, not "commonmark".
    pub(crate) myst: bool,
    /// myst_disable_syntax.
    pub(crate) disabled: Rules,
    /// myst_enable_extensions in force.
    pub(crate) syntax: SyntaxExtensions,
    /// Link reference definitions by normalized label, over the whole document.
    pub(crate) refs: HashMap<String, Ref>,
    /// Reused buffer for normalizing labels.
    pub(crate) label_tmp: String,
    html_format: Option<Text>,
    /// MyST include options in force: rewrite image paths, and link paths starting with
    /// `rel_docs`, of included sources relative to the main source.
    pub(crate) rel_images: bool,
    pub(crate) rel_docs: String,
    /// The MyST layer's state.
    pub(crate) ms: MystState,
    pub(crate) ext_state: Option<Box<dyn Any>>,
}

impl std::fmt::Debug for Host<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("myst", &self.myst)
            .field("refs", &self.refs.len())
            .finish_non_exhaustive()
    }
}

/// The settings a read without settings uses.
pub(crate) fn default_settings() -> &'static Settings {
    static DEFAULTS: OnceLock<Settings> = OnceLock::new();
    DEFAULTS.get_or_init(Settings::default)
}

impl<'h, 'a> Host<'h, 'a> {
    /// Prepares the state of one read with `settings`: pure CommonMark when `myst` is
    /// false (the settings are then the defaults and unused), MyST otherwise.
    pub(crate) fn new(ctx: &'h mut ReadContext<'a>, settings: &'h Settings, myst: bool) -> Self {
        let mut h = Host {
            ctx,
            settings,
            myst,
            disabled: Rules::default(),
            syntax: SyntaxExtensions::NONE,
            refs: HashMap::new(),
            label_tmp: String::new(),
            html_format: None,
            rel_images: false,
            rel_docs: String::new(),
            ms: MystState::default(),
            ext_state: None,
        };
        if myst {
            h.configure();
        }
        h
    }

    /// Applies the syntax settings: commonmark_only, enable_extensions, disable_syntax.
    fn configure(&mut self) {
        let settings = self.settings;
        if settings.commonmark_only {
            self.disabled = MYST_RULES;
            return;
        }
        self.syntax = settings.enable_extensions;
        for name in &settings.disable_syntax {
            match Rule::named(name) {
                Some(r) => self.disabled.insert(r),
                None => {
                    let span = SourceSpan::new(self.ctx.source, 0, 0);
                    self.report(SYNTAX_RULE_UNKNOWN, span, guidedoc::arg_text(name), Arg::None);
                }
            }
        }
    }

    /// The builder of the read.
    pub(crate) fn b(&mut self) -> &mut Builder<'a> {
        &mut self.ctx.b
    }

    /// The extension, borrowed from the settings (not from the host).
    pub(crate) fn ext(&self) -> Option<&'h dyn Extension> {
        self.settings.extension.as_deref()
    }

    pub(crate) fn report(&mut self, m: Message, span: SourceSpan, a: Arg, b: Arg) {
        self.ctx.report(m, span, a, b);
    }

    /// Reports `m` at `span`, quoting the span.
    pub(crate) fn report_span(&mut self, m: Message, span: SourceSpan) {
        self.ctx.report(m, span, guidedoc::arg_span(span), Arg::None);
    }

    /// Charges `steps` of work.
    pub(crate) fn spend(&mut self, steps: usize) -> Result<(), Fail> {
        self.ctx.b.spend(steps as u64)
    }

    /// Tells whether a syntax rule is in force.
    pub(crate) fn on(&self, r: Rule) -> bool {
        !self.disabled.has(r)
    }

    /// Tells whether an optional syntax is enabled.
    pub(crate) fn has(&self, e: SyntaxExtension) -> bool {
        self.myst && self.syntax.has(e)
    }

    /// The shared pool text "html" of raw content attributes.
    pub(crate) fn html_format(&mut self) -> Result<Text, Fail> {
        if let Some(t) = self.html_format {
            return Ok(t);
        }
        let t = self.ctx.b.add_text("html")?;
        self.html_format = Some(t);
        Ok(t)
    }

    /// Writes a reference name into the pool, case-folded with white space collapsed.
    pub(crate) fn normalized_name(&mut self, s: &str) -> Result<Text, Fail> {
        self.ctx.b.build_text(|b| {
            normalize_label(b, s);
            Ok(())
        })
    }
}
