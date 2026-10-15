//! The adapter interface: reader and renderer descriptors, the registry that holds them,
//! and the contexts the engine hands an adapter for one stage.

use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use crate::builder::Builder;
use crate::document::Document;
use crate::emit::Emitter;
use crate::options::{Limits, Policy, ReadOptions, RenderOptions, ResourceProvider};
use crate::report::{
    Arg, CORE_SETTINGS_NONE, CORE_SETTINGS_TYPE, Diagnostic, Fail, Message, REGISTRY_INVALID,
    Reports, arg_static, arg_text, diagnostic,
};
use crate::scratch::Scratch;
use crate::snapshot::{SourceEntry, entry_text};
use crate::types::{SourceId, SourceSpan, Status};

/// How completely an adapter implements its format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Conformance {
    /// Not implemented yet.
    #[default]
    Planned,
    /// Implemented with known gaps.
    Partial,
    /// Passes its conformance suite.
    Conformant,
}

/// Reads `ctx.source` into `ctx.b`.
pub type ReadFn = fn(&mut ReadContext<'_>) -> Result<(), Fail>;
/// Parses a span of `ctx.source` as inline content of the builder's open node, as a
/// translated message is parsed. It neither resolves references nor reports unresolved
/// ones. `span.source` must be `ctx.source`.
pub type ReadInlineFn = fn(&mut ReadContext<'_>, SourceSpan) -> Result<(), Fail>;
/// Writes `ctx.doc` with `ctx.out`.
pub type RenderFn = fn(&mut RenderContext<'_>) -> Result<(), Fail>;

/// A reader: a constant descriptor with functions and no state.
#[derive(Debug, Clone, Copy)]
pub struct Reader {
    /// Registry id ("rst").
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Its version.
    pub version: &'static str,
    /// How complete it is.
    pub conformance: Conformance,
    /// Reads a document.
    pub read: ReadFn,
    /// Reads inline content, when the reader offers it.
    pub read_inline: Option<ReadInlineFn>,
}

/// A renderer: a constant descriptor with a function and no state.
#[derive(Debug, Clone, Copy)]
pub struct Renderer {
    /// Registry id ("html").
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Its version.
    pub version: &'static str,
    /// The media type it writes ("text/html").
    pub media_type: &'static str,
    /// How complete it is.
    pub conformance: Conformance,
    /// Renders a document.
    pub render: RenderFn,
}

/// The adapters a conversion may use. A plain value: build it once and share it.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    /// Readers, by id.
    pub readers: Vec<Reader>,
    /// Renderers, by id.
    pub renderers: Vec<Renderer>,
}

impl Registry {
    /// Returns the reader with `id`.
    pub fn find_reader(&self, id: &str) -> Option<&Reader> {
        self.readers.iter().find(|r| r.id == id)
    }

    /// Returns the renderer with `id`.
    pub fn find_renderer(&self, id: &str) -> Option<&Renderer> {
        self.renderers.iter().find(|r| r.id == id)
    }

    /// Checks that every adapter has an id and a version and that no id is registered
    /// twice. `convert` checks its registry the same way, so a bad registry fails with
    /// `InvalidInput` instead of the first match silently winning.
    pub fn validate(&self) -> Result<(), Fail> {
        match self.problem() {
            Some(d) => Err(Fail::from_diagnostic(d)),
            None => Ok(()),
        }
    }

    pub(crate) fn problem(&self) -> Option<Diagnostic> {
        let readers = self.readers.iter().map(|r| (r.id, r.version));
        let renderers = self.renderers.iter().map(|r| (r.id, r.version));
        adapters_problem(readers).or_else(|| adapters_problem(renderers))
    }
}

/// Finds the first adapter without metadata or registered twice.
fn adapters_problem(
    adapters: impl Iterator<Item = (&'static str, &'static str)> + Clone,
) -> Option<Diagnostic> {
    for (i, (id, version)) in adapters.clone().enumerate() {
        if id.is_empty() || version.is_empty() {
            return Some(adapter_diagnostic(id, "missing metadata"));
        }
        if adapters.clone().take(i).any(|(other, _)| other == id) {
            return Some(adapter_diagnostic(id, "registered twice"));
        }
    }
    None
}

fn adapter_diagnostic(id: &'static str, why: &'static str) -> Diagnostic {
    diagnostic(REGISTRY_INVALID, SourceSpan::default(), arg_static(id), arg_static(why))
}

/// Validates a registry: fails when an adapter lacks metadata or is registered twice.
pub fn validate_registry(r: &Registry) -> Result<(), Fail> {
    r.validate()
}

/// The text of a source, as a reader holds it while it keeps building: borrowed for the
/// main source, shared for included ones. It dereferences to `&str`.
#[derive(Debug, Clone)]
pub enum SourceText<'a> {
    /// The main source, borrowed from the workspace.
    Borrowed(&'a str),
    /// An included or added source.
    Shared(Arc<str>),
}

impl Deref for SourceText<'_> {
    type Target = str;

    fn deref(&self) -> &str {
        match self {
            SourceText::Borrowed(s) => s,
            SourceText::Shared(s) => s,
        }
    }
}

/// What a reader works with for one read: the builder, the source being read, the
/// options, and the source table it may add included sources to.
pub struct ReadContext<'a> {
    /// The builder of the document being read.
    pub b: Builder<'a>,
    /// The source to read.
    pub source: SourceId,
    /// The read options.
    pub options: &'a ReadOptions<'a>,
    /// The trust decisions.
    pub policy: Policy,
    /// The bounds.
    pub limits: Limits,
    pub(crate) resources: Option<&'a dyn ResourceProvider>,
    pub(crate) main: &'a str,
    pub(crate) sources: &'a mut Vec<SourceEntry>,
    /// Bytes of included sources, charged against `Limits::max_included_bytes`.
    pub(crate) included: usize,
    pub(crate) scratch: &'a mut Scratch,
}

impl fmt::Debug for ReadContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadContext")
            .field("b", &self.b)
            .field("source", &self.source)
            .field("sources", &self.sources.len())
            .finish_non_exhaustive()
    }
}

impl<'a> ReadContext<'a> {
    /// Returns the reports of the read.
    pub fn reports(&mut self) -> &mut Reports {
        self.b.reports()
    }

    /// Takes the reader's working state of type `T` out of the workspace (see
    /// `Scratch::take`): what an earlier read left, or `T::default()`. Clear what you
    /// use and give it back with `put_scratch`, so the next read reuses its storage.
    pub fn take_scratch<T: Default + Send + 'static>(&mut self) -> T {
        self.scratch.take()
    }

    /// Gives working state taken with `take_scratch` back to the workspace.
    pub fn put_scratch<T: Send + 'static>(&mut self, value: T) {
        self.scratch.put(value);
    }

    /// Reports message `m` at `span`, filled with the arguments `a` and `b`.
    pub fn report(&mut self, m: Message, span: SourceSpan, a: Arg, b: Arg) {
        self.b.report(m, span, a, b);
    }

    /// Returns the text of source `id`, which the reader may keep while it builds;
    /// "" for an unknown id.
    pub fn source_of(&self, id: SourceId) -> SourceText<'a> {
        match self.sources.get(id.index()).and_then(|e| e.shared.clone()) {
            Some(shared) => SourceText::Shared(shared),
            None if id.index() < self.sources.len() => SourceText::Borrowed(self.main),
            None => SourceText::Borrowed(""),
        }
    }

    /// Returns the source-table entry of `id` (its name, parent, and include span).
    pub fn source_entry(&self, id: SourceId) -> Option<&SourceEntry> {
        self.sources.get(id.index())
    }

    /// Returns the number of sources in the table.
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// Returns the source text a span covers, or "" for an invalid span.
    pub fn source_text(&self, span: SourceSpan) -> &str {
        let text = entry_text(self.main, self.sources, span.source);
        text.get(span.start as usize..span.end as usize).unwrap_or("")
    }

    /// Views the document built so far with its sources.
    pub fn document(&self) -> Document<'_> {
        Document::new(self.b.snap, self.main, self.sources)
    }

    /// Returns the reader's settings of type `T`: `None` for the defaults when the read
    /// options carry none. Settings of another type are reported
    /// (`core.read.settings_type`) and fail with `InvalidInput`.
    pub fn reader_settings<T: 'static>(&mut self) -> Result<Option<&'a T>, Fail> {
        let Some(extension) = self.options.extension else {
            return Ok(None);
        };
        if let Some(settings) = extension.downcast::<T>() {
            return Ok(Some(settings));
        }
        let wanted = std::any::type_name::<T>();
        let d = diagnostic(
            CORE_SETTINGS_TYPE,
            SourceSpan::default(),
            arg_text(extension.type_name),
            arg_static(wanted),
        );
        self.reports().report(d.clone());
        Err(Fail { status: Status::InvalidInput, ..Fail::from_diagnostic(d) })
    }

    /// The check of a reader that takes no settings: any settings are reported
    /// (`core.read.settings_none`) and fail with `InvalidInput`.
    pub fn no_reader_settings(&mut self) -> Result<(), Fail> {
        let Some(extension) = self.options.extension else {
            return Ok(());
        };
        let d = diagnostic(
            CORE_SETTINGS_NONE,
            SourceSpan::default(),
            arg_text(extension.type_name),
            Arg::None,
        );
        self.reports().report(d.clone());
        Err(Fail::from_diagnostic(d))
    }
}

/// What a renderer works with for one render.
pub struct RenderContext<'a> {
    /// The document to render.
    pub doc: Document<'a>,
    /// Where the output goes.
    pub out: Emitter<'a>,
    /// The render options.
    pub options: &'a RenderOptions<'a>,
    /// The trust decisions.
    pub policy: Policy,
    /// The bounds.
    pub limits: Limits,
    pub(crate) reports: &'a mut Reports,
    /// The workspace's working state; `None` for a context made with `new`.
    pub(crate) scratch: Option<&'a mut Scratch>,
}

impl fmt::Debug for RenderContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderContext").field("out", &self.out).finish_non_exhaustive()
    }
}

impl<'a> RenderContext<'a> {
    /// Makes a context for rendering `doc` into `out`; the engine makes one per render,
    /// and renderer tests may make their own.
    pub fn new(
        doc: Document<'a>,
        out: Emitter<'a>,
        options: &'a RenderOptions<'a>,
        reports: &'a mut Reports,
    ) -> Self {
        RenderContext {
            doc,
            out,
            options,
            policy: Policy::default(),
            limits: Limits::default(),
            reports,
            scratch: None,
        }
    }

    /// Lets the context keep working state in `scratch` between renders (the engine
    /// passes the workspace's); a context made with `new` alone keeps none.
    pub fn with_scratch(self, scratch: &'a mut Scratch) -> Self {
        RenderContext { scratch: Some(scratch), ..self }
    }

    /// Takes the renderer's working state of type `T` (see `Scratch::take`): what an
    /// earlier render left, or `T::default()`. Clear what you use and give it back with
    /// `put_scratch`, so the next render reuses its storage.
    pub fn take_scratch<T: Default + Send + 'static>(&mut self) -> T {
        match &mut self.scratch {
            Some(scratch) => scratch.take(),
            None => T::default(),
        }
    }

    /// Gives working state taken with `take_scratch` back; a context without a store
    /// drops it.
    pub fn put_scratch<T: Send + 'static>(&mut self, value: T) {
        if let Some(scratch) = &mut self.scratch {
            scratch.put(value);
        }
    }

    /// Returns the reports of the render.
    pub fn reports(&mut self) -> &mut Reports {
        self.reports
    }

    /// Reports message `m` at `span`.
    pub fn report(&mut self, m: Message, span: SourceSpan, a: Arg, b: Arg) {
        self.reports.message(m, span, a, b);
    }
}
