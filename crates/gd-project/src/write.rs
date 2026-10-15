//! The one place a builder kind becomes its builder.

use gd_book::{DummyBuilder, PdfBuilder, TextBuilder};
use gd_env::{BuilderKind, SourceFile};
use gd_host::Problem;
use gd_intl::{GettextBuilder, Intl, Reread};
use gd_outputs::Generation;
use gd_report::Report;
use gd_resolve::{BuildView, OutputBuilder};
use gd_site::SiteBuilder;

/// Writes `view.kind`'s output into `generation` with its builder: gd-site for the
/// HTML builders, gd-book for pdf, text and dummy, gd-intl for gettext (which reads a
/// stale document again with `reread`). Fails with the builder's problem.
pub(crate) fn write_output(
    view: &BuildView<'_>,
    sources: &[SourceFile],
    intl: &Intl,
    reread: &Reread<'_>,
    generation: &mut Generation,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let labels = Some(&intl.labels);
    match view.kind {
        BuilderKind::Html | BuilderKind::Dirhtml | BuilderKind::Singlehtml => {
            let builder = SiteBuilder { kind: view.kind, labels, sources };
            builder.write(view, generation, reports)
        }
        BuilderKind::Pdf => {
            PdfBuilder { labels, ..PdfBuilder::default() }.write(view, generation, reports)
        }
        BuilderKind::Text => TextBuilder { labels }.write(view, generation, reports),
        BuilderKind::Gettext => {
            GettextBuilder { reread: Some(reread) }.write(view, generation, reports)
        }
        BuilderKind::Dummy => DummyBuilder.write(view, generation, reports),
    }
}
