//! A standalone record carries its own PDF layout and structured front matter.

use std::fmt::Write as _;

use gd_env::DocId;
use gd_link::{Discussion, discussion_record};
use gd_report::Report;
use guidedoc::Workspace;

use crate::compose::{Book, Composer, typst_string};

impl Composer<'_, '_> {
    /// Reads the root's metadata using the same contract as the PDF link pass.
    /// Object failures fail the composition instead of silently changing its layout.
    pub(crate) fn discussion(
        &self,
        root: DocId,
        out: &mut Book,
        reports: &mut Vec<Report>,
    ) -> Option<Discussion> {
        let object = self.view.doc(root).object.as_ref()?;
        let mut workspace = Workspace::new();
        match self.view.objects.load_object(&mut workspace, object, self.view.limits) {
            Ok(doc) => discussion_record(doc),
            Err(error) => {
                out.failed += 1;
                reports.push(error.problem.report());
                None
            }
        }
    }
}

/// Writes metadata as quoted Typst strings, so names and titles never become code.
pub(crate) fn record_arguments(out: &mut String, record: &Discussion, catalog: &str) {
    let id = format!("{} {}", record.prefix.to_uppercase(), record.number);
    let _ = write!(
        out,
        ", record: (id: {}, catalog: {}, fields: (",
        typst_string(&id),
        typst_string(catalog),
    );
    for (label, value) in &record.fields {
        let _ = write!(out, "({}, {}),", typst_string(label), typst_string(value));
    }
    out.push_str("))");
}
