//! The reader's document-wide state, shared by the parse of the document and the parses
//! of nested content (directive bodies, footnotes, substitutions).

use std::any::Any;
use std::ops::Range;
use std::sync::OnceLock;

use guidedoc::{
    Arg, Builder, Fail, Message, NameMap, ReadContext, SourceSpan, Text, normalize_label,
};

use crate::extension::Extension;
use crate::messages::SYNTAX_RULE_UNKNOWN;
use crate::myst::MystState;
use crate::settings::{MYST_RULES, Rule, Rules, Settings, SyntaxExtension, SyntaxExtensions};

/// A link reference definition: its destination and title, decoded, as byte ranges
/// of `Host::ref_text`.
#[derive(Debug, Clone, Default)]
pub(crate) struct Ref {
    pub dest: Range<usize>,
    pub title: Option<Range<usize>>,
}

/// The host's storage, kept in the workspace's scratch between reads (`Host::new`
/// takes it, `Host::finish` gives it back).
#[derive(Debug, Default)]
struct HostWork {
    refs: NameMap<Ref>,
    ref_text: String,
    label_tmp: String,
    text_tmp: String,
    rel_docs: String,
    ms: MystState,
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
    pub(crate) refs: NameMap<Ref>,
    /// The definitions' decoded destinations and titles.
    pub(crate) ref_text: String,
    /// Reused buffer for normalizing labels.
    pub(crate) label_tmp: String,
    /// Reused buffer for a decoded text.
    pub(crate) text_tmp: String,
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
    /// The storage comes from the workspace's scratch: call `finish` when the read ends.
    pub(crate) fn new(ctx: &'h mut ReadContext<'a>, settings: &'h Settings, myst: bool) -> Self {
        let mut w: HostWork = ctx.take_scratch();
        w.refs.clear();
        w.ref_text.clear();
        w.rel_docs.clear();
        w.ms.clear();
        let mut h = Host {
            ctx,
            settings,
            myst,
            disabled: Rules::default(),
            syntax: SyntaxExtensions::NONE,
            refs: w.refs,
            ref_text: w.ref_text,
            label_tmp: w.label_tmp,
            text_tmp: w.text_tmp,
            html_format: None,
            rel_images: false,
            rel_docs: w.rel_docs,
            ms: w.ms,
            ext_state: None,
        };
        if myst {
            h.configure();
        }
        h
    }

    /// Ends the read, giving the host's storage back to the workspace for the next one.
    pub(crate) fn finish(self) {
        let w = HostWork {
            refs: self.refs,
            ref_text: self.ref_text,
            label_tmp: self.label_tmp,
            text_tmp: self.text_tmp,
            rel_docs: self.rel_docs,
            ms: self.ms,
        };
        self.ctx.put_scratch(w);
    }

    /// Returns a range of the definitions' text (`Ref`).
    pub(crate) fn ref_str(&self, range: Range<usize>) -> &str {
        self.ref_text.get(range).unwrap_or("")
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

/// A host without its read context: everything else of the read's state. A handler
/// that lends the context to another parser (the Sphinx layer's embedded
/// reStructuredText parser) uses it to parse nested Markdown with that parser's context
/// (`Detached::attach`), so the two readers never hold the context at once.
pub struct Detached<'h> {
    settings: &'h Settings,
    myst: bool,
    disabled: Rules,
    syntax: SyntaxExtensions,
    refs: NameMap<Ref>,
    ref_text: String,
    label_tmp: String,
    text_tmp: String,
    html_format: Option<Text>,
    rel_images: bool,
    rel_docs: String,
    ms: MystState,
    ext_state: Option<Box<dyn Any>>,
}

impl std::fmt::Debug for Detached<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Detached").field("myst", &self.myst).finish_non_exhaustive()
    }
}

impl<'h, 'a> Host<'h, 'a> {
    /// Runs `f` with the read context and the rest of the host apart; the host is whole
    /// again afterwards (with what `f` changed in either part).
    pub fn detach<R>(
        &mut self,
        f: impl FnOnce(&mut ReadContext<'a>, &mut Detached<'h>) -> R,
    ) -> R {
        let mut d = Detached {
            settings: self.settings,
            myst: self.myst,
            disabled: self.disabled,
            syntax: self.syntax,
            refs: std::mem::take(&mut self.refs),
            ref_text: std::mem::take(&mut self.ref_text),
            label_tmp: std::mem::take(&mut self.label_tmp),
            text_tmp: std::mem::take(&mut self.text_tmp),
            html_format: self.html_format.take(),
            rel_images: self.rel_images,
            rel_docs: std::mem::take(&mut self.rel_docs),
            ms: std::mem::take(&mut self.ms),
            ext_state: self.ext_state.take(),
        };
        let r = f(self.ctx, &mut d);
        self.refs = d.refs;
        self.ref_text = d.ref_text;
        self.label_tmp = d.label_tmp;
        self.text_tmp = d.text_tmp;
        self.html_format = d.html_format;
        self.rel_images = d.rel_images;
        self.rel_docs = d.rel_docs;
        self.ms = d.ms;
        self.ext_state = d.ext_state;
        r
    }
}

impl<'h> Detached<'h> {
    /// Runs `f` with a host made of `ctx` and this state, as `Host::detach` took it
    /// apart; the state takes back what `f` changed.
    pub fn attach<'a, R>(
        &mut self,
        ctx: &mut ReadContext<'a>,
        f: impl FnOnce(&mut Host<'_, 'a>) -> R,
    ) -> R {
        let mut h = Host {
            ctx,
            settings: self.settings,
            myst: self.myst,
            disabled: self.disabled,
            syntax: self.syntax,
            refs: std::mem::take(&mut self.refs),
            ref_text: std::mem::take(&mut self.ref_text),
            label_tmp: std::mem::take(&mut self.label_tmp),
            text_tmp: std::mem::take(&mut self.text_tmp),
            html_format: self.html_format.take(),
            rel_images: self.rel_images,
            rel_docs: std::mem::take(&mut self.rel_docs),
            ms: std::mem::take(&mut self.ms),
            ext_state: self.ext_state.take(),
        };
        let r = f(&mut h);
        self.refs = h.refs;
        self.ref_text = h.ref_text;
        self.label_tmp = h.label_tmp;
        self.text_tmp = h.text_tmp;
        self.html_format = h.html_format;
        self.rel_images = h.rel_images;
        self.rel_docs = h.rel_docs;
        self.ms = h.ms;
        self.ext_state = h.ext_state;
        r
    }

    /// Stores the extension's per-read state, as `Host::set_extension_state`.
    pub fn set_extension_state<T: Any>(&mut self, state: T) {
        self.ext_state = Some(Box::new(state));
    }

    /// Returns the extension's per-read state, as `Host::extension_state`.
    pub fn extension_state<T: Any>(&mut self) -> Option<&mut T> {
        self.ext_state.as_mut().and_then(|s| s.downcast_mut::<T>())
    }
}
