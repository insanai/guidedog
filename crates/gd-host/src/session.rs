//! `Session`: a workspace that converts with the host's conveniences (Ctrl+C
//! cancellation, reports described for rendering). The Odin session sized storage,
//! retried on `Capacity`, and charged a budget; the Rust workspace grows its own
//! reusable buffers within `Limits`, so a `Capacity` result is a limit reached, which a
//! retry would only reach again.

use gd_report::Report;
use guidedoc::{
    Document, Fail, Reader, Registry, Request, Workspace, convert, read_document,
    transform_document,
};

use crate::diagnostics::describe_all;
use crate::interrupt::cancel_probe;

/// One thread's conversions. Reuse it: its workspace keeps its buffers between runs.
/// What a run returns (output, document) and `reports` describe the last run only.
#[derive(Debug, Default)]
pub struct Session {
    ws: Workspace,
}

impl Session {
    /// Makes a session with an empty workspace; it allocates nothing until used.
    pub fn new() -> Session {
        Session::default()
    }

    /// Converts `request` with `registry`: read, passes, render. A request without a
    /// cancel probe gets the Ctrl+C one (`cancel_probe`). The output borrows the session
    /// until the next run; on failure, `reports` explains it.
    pub fn run(&mut self, registry: &Registry, request: &Request<'_>) -> Result<&str, Fail> {
        let mut request = *request;
        request.cancel = request.cancel.or(Some(cancel_probe()));
        convert(&mut self.ws, registry, &request)
    }

    /// Reads `request`'s source with `reader` without rendering, then runs
    /// `request.passes` over the document (a translation, for example). The document
    /// borrows the session until the next run.
    pub fn read(&mut self, reader: &Reader, request: &Request<'_>) -> Result<Document<'_>, Fail> {
        let mut request = *request;
        request.cancel = request.cancel.or(Some(cancel_probe()));
        if request.passes.is_empty() {
            return read_document(&mut self.ws, reader, &request);
        }
        read_document(&mut self.ws, reader, &request)?;
        transform_document(&mut self.ws, request.passes, &request.limits)
    }

    /// The current document: the last successful read or run.
    pub fn document(&self) -> Option<Document<'_>> {
        self.ws.document()
    }

    /// Describes what the last run reported (`describe_all`); the reports are owned.
    pub fn reports(&self) -> Vec<Report> {
        describe_all(&self.ws)
    }

    /// The workspace, for the source table, source map, and raw diagnostics.
    pub fn workspace(&self) -> &Workspace {
        &self.ws
    }

    /// The workspace, for the engine's staged calls (`check_render`, `load_object`).
    pub fn workspace_mut(&mut self) -> &mut Workspace {
        &mut self.ws
    }
}
