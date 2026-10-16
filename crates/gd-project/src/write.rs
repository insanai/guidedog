//! The one place a builder kind becomes its builder.

use gd_book::{DummyBuilder, PdfBuilder, TextBuilder};
use gd_env::{BuilderKind, SourceFile};
use gd_host::Problem;
use gd_intl::{GettextBuilder, Intl, Reread};
use gd_outputs::Generation;
use gd_report::Report;
use gd_resolve::{BuildView, OutputBuilder};
use gd_site::SiteBuilder;

use crate::coverage::CoverageBuilder;

/// Writes `view.kind`'s output into `generation` with its builder: gd-site for the
/// HTML builders, gd-book for pdf, text and dummy, gd-intl for gettext (which reads a
/// stale document again with `reread`), the coverage builder for coverage (whose
/// statistics join `steps`). Fails with the builder's problem.
pub(crate) fn write_output(
    view: &BuildView<'_>,
    sources: &[SourceFile],
    intl: &Intl,
    reread: &Reread<'_>,
    generation: &mut Generation,
    reports: &mut Vec<Report>,
    steps: &mut Vec<(String, String)>,
) -> Result<(), Problem> {
    let labels = Some(&intl.labels);
    match view.kind {
        BuilderKind::Html | BuilderKind::Dirhtml | BuilderKind::Singlehtml => {
            let builder = SiteBuilder { kind: view.kind, labels, sources };
            builder.write(view, generation, reports)?;
            if view.kind != BuilderKind::Singlehtml {
                PdfBuilder { labels, ..Default::default() }
                    .write_articles(view, generation, reports)?;
            }
            Ok(())
        }
        BuilderKind::Pdf => {
            PdfBuilder { labels, ..PdfBuilder::default() }.write(view, generation, reports)
        }
        BuilderKind::Text => TextBuilder { labels }.write(view, generation, reports),
        BuilderKind::Gettext => {
            GettextBuilder { reread: Some(reread) }.write(view, generation, reports)
        }
        BuilderKind::Dummy => DummyBuilder.write(view, generation, reports),
        BuilderKind::Coverage => {
            let builder = CoverageBuilder::default();
            let written = builder.write(view, generation, reports);
            let lines = builder.statistics.into_inner().unwrap_or_default();
            for (i, line) in lines.into_iter().enumerate() {
                steps.push((if i == 0 { "coverage" } else { "" }.to_owned(), line));
            }
            written
        }
    }
}
