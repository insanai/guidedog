//! The workspace owns everything a conversion builds: two snapshots (the document and
//! the one a pass rebuilds into), the source table, reports, and the output. Each
//! conversion clears them and keeps their storage, so steady-state conversion of
//! similar inputs does not allocate.

use std::fmt;

use crate::adapter::ReadContext;
use crate::builder::Builder;
use crate::document::Document;
use crate::options::{Limits, MapEntry, Policy, Request, default_limits};
use crate::report::{Arg, CORE_SOURCE_SIZE, CORE_STRICT, Diagnostic, Fail, Reports, arg_int};
use crate::snapshot::{Snapshot, SourceEntry, entry_text};
use crate::types::{CapacityNeed, NodeId, Region, SourceId, SourceSpan, Status};
use crate::validate::validate_with;

/// One conversion's storage. Use one workspace per thread; documents borrow it.
#[derive(Default)]
pub struct Workspace {
    pub(crate) banks: [Snapshot; 2],
    pub(crate) active: usize,
    pub(crate) has_document: bool,
    pub(crate) main: String,
    pub(crate) sources: Vec<SourceEntry>,
    pub(crate) reports: Reports,
    pub(crate) open: Vec<NodeId>,
    pub(crate) output: Vec<u8>,
    pub(crate) source_map: Vec<MapEntry>,
    pub(crate) keep_errors: bool,
    limits: Limits,
    validation_stack: Vec<u32>,
    /// The main source's name buffer, kept between conversions.
    spare_name: String,
    source_lengths: Vec<usize>,
}

impl fmt::Debug for Workspace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Workspace")
            .field("active", &self.active)
            .field("has_document", &self.has_document)
            .field("sources", &self.sources.len())
            .field("output", &self.output.len())
            .finish_non_exhaustive()
    }
}

impl Workspace {
    /// Makes an empty workspace with `default_limits()`. It allocates nothing until used.
    pub fn new() -> Workspace {
        Workspace::with_limits(default_limits())
    }

    /// Makes an empty workspace whose `convert_text` uses `limits`.
    pub fn with_limits(limits: Limits) -> Workspace {
        Workspace { limits, ..Workspace::default() }
    }

    /// Returns the limits `convert_text` uses.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Returns the current document: the last successful read, pass, or load, until
    /// the next conversion starts or the workspace is reset.
    pub fn document(&self) -> Option<Document<'_>> {
        if !self.has_document {
            return None;
        }
        Some(Document::new(&self.banks[self.active], &self.main, &self.sources))
    }

    /// Returns the reports of the current conversion, valid until the next one.
    pub fn reports(&self) -> &Reports {
        &self.reports
    }

    /// Returns the source map the last render recorded (`Request::source_map`).
    pub fn source_map(&self) -> &[MapEntry] {
        &self.source_map
    }

    /// Returns the source table of the current conversion.
    pub fn sources(&self) -> &[SourceEntry] {
        &self.sources
    }

    /// Returns the whole text of source `id`; "" for an unknown id.
    pub fn source_str(&self, id: SourceId) -> &str {
        entry_text(&self.main, &self.sources, id)
    }

    /// Returns the source text a span covers, or "" for an invalid span.
    pub fn source_text(&self, span: SourceSpan) -> &str {
        self.source_str(span.source).get(span.start as usize..span.end as usize).unwrap_or("")
    }

    /// Returns a diagnostic's message with its arguments, span arguments quoted from
    /// this workspace's sources.
    pub fn describe(&self, d: &Diagnostic) -> String {
        let mut out = String::new();
        d.format_into(d.message, &|span| self.source_text(span).to_owned(), &mut out);
        out
    }

    /// Forgets the document, sources, reports, and output, keeping their storage.
    pub fn reset(&mut self) {
        let limits = self.limits;
        self.begin_generation(&limits);
    }

    /// Starts a new conversion: every table is cleared and no document is current.
    pub(crate) fn begin_generation(&mut self, limits: &Limits) {
        self.banks[0].clear();
        self.banks[1].clear();
        self.active = 0;
        self.has_document = false;
        self.main.clear();
        // Keep the main source's name buffer for the next conversion.
        if let Some(main) = self.sources.first_mut() {
            self.spare_name = std::mem::take(&mut main.name);
        }
        self.sources.clear();
        self.reports.clear(limits.max_reports);
        self.output.clear();
        self.source_map.clear();
        self.keep_errors = false;
    }

    /// Returns the failure of a stage: `status`, the capacity it lacked, and the primary
    /// error, or `own` (an adapter's failure) when nothing was reported as an error.
    pub(crate) fn failure(
        &mut self,
        status: Status,
        need: CapacityNeed,
        own: Option<Diagnostic>,
    ) -> Fail {
        if self.reports.primary().is_none()
            && let Some(d) = own.filter(|d| !d.code.is_empty())
        {
            self.reports.report(d);
        }
        let diagnostic =
            self.reports.primary().or(self.reports.first_problem()).cloned().unwrap_or_default();
        Fail { status, need, diagnostic: Box::new(diagnostic) }
    }

    /// Starts a read of `request`'s source: a new conversion, the main source copied into
    /// the workspace, and a context whose builder has the root open. `read_document`
    /// uses it; reader tests use it to drive a reader's pieces.
    pub fn begin_read<'w>(
        &'w mut self,
        request: &'w Request<'w>,
    ) -> Result<ReadContext<'w>, Fail> {
        self.begin_generation(&request.limits);
        self.keep_errors = request.policy.keep_documents_with_errors;
        let text = request.source.text;
        let Ok(length) = u32::try_from(text.len()) else {
            let size = i64::try_from(text.len()).unwrap_or(i64::MAX);
            self.reports.message(
                CORE_SOURCE_SIZE,
                SourceSpan::default(),
                arg_int(size),
                Arg::None,
            );
            return Err(self.failure(Status::Limit, CapacityNeed::default(), None));
        };
        if self.main.try_reserve(text.len()).is_err() || self.sources.try_reserve(1).is_err() {
            let fail = Fail::capacity(Region::Sources, text.len() as u64, false);
            return Err(self.failure(Status::Capacity, fail.need, Some(*fail.diagnostic)));
        }
        self.main.push_str(text);
        let mut name = std::mem::take(&mut self.spare_name);
        name.clear();
        name.push_str(request.source.name);
        self.sources.push(SourceEntry { name, ..SourceEntry::default() });
        let Workspace { banks, main, sources, reports, open, .. } = self;
        let root = SourceSpan::new(SourceId::MAIN, 0, length);
        let mut b = Builder::new(&mut banks[0], reports, open, request.limits, root);
        b.set_cancel(request.cancel);
        Ok(ReadContext {
            b,
            source: SourceId::MAIN,
            options: &request.read,
            policy: request.policy,
            limits: request.limits,
            resources: request.resources,
            main,
            sources,
            included: 0,
        })
    }

    /// Validates the finished snapshot `bank` and applies the error policy; on success
    /// it becomes the current document.
    pub(crate) fn settle(
        &mut self,
        bank: usize,
        limits: &Limits,
        policy: &Policy,
    ) -> Result<(), Fail> {
        self.source_lengths.clear();
        for i in 0..self.sources.len() {
            let id = SourceId(u16::try_from(i).unwrap_or(u16::MAX));
            self.source_lengths.push(entry_text(&self.main, &self.sources, id).len());
        }
        let checked = validate_with(
            &self.banks[bank],
            &self.source_lengths,
            limits,
            &mut self.validation_stack,
        );
        if let Err(fail) = checked {
            self.reports.report(*fail.diagnostic);
            return Err(self.failure(fail.status, fail.need, None));
        }
        let errors = self.reports.error_count();
        if errors > 0 && !self.keep_errors {
            let status = self.reports.primary().map_or(Status::InvalidInput, |d| d.status);
            return Err(self.failure(status, CapacityNeed::default(), None));
        }
        if errors > 0 && self.reports.fatal() != Status::Ok {
            return Err(self.failure(self.reports.fatal(), CapacityNeed::default(), None));
        }
        let problems = self.reports.warning_count() + errors;
        if policy.strict && problems > 0 {
            let count = i64::try_from(problems).unwrap_or(i64::MAX);
            self.reports.message(CORE_STRICT, SourceSpan::default(), arg_int(count), Arg::None);
            return Err(self.failure(Status::Policy, CapacityNeed::default(), None));
        }
        self.active = bank;
        self.has_document = true;
        Ok(())
    }
}
