//! gd-site: Guidedog's html, dirhtml and singlehtml builders. [`SiteBuilder`]
//! implements gd-resolve's `OutputBuilder`: documents are linked and rendered by
//! gd-link on worker threads (each with its own workspace and highlighter), laid out
//! with the project's `layout.html` or the built-in theme's (gd-jinja, gd-theme), passed
//! through the build plugins' HTML hooks, and written in docname order as units of a
//! gd-outputs `Generation`, unchanged ones reused. The site unit holds the theme's
//! files, static and extra files, images, downloads, sources, plugin assets, the search
//! page, the general and module indexes, additional pages and `objects.inv`; the
//! search index (`_static/searchindex.js`) is a unit of its own.
//!
//! ```no_run
//! # fn build(view: &gd_resolve::BuildView<'_>, generation: &mut gd_outputs::Generation)
//! #     -> Result<(), gd_host::Problem> {
//! use gd_resolve::OutputBuilder;
//! let builder = gd_site::SiteBuilder { kind: view.kind, ..Default::default() };
//! let mut reports = Vec::new();
//! builder.write(view, generation, &mut reports)?;
//! # Ok(())
//! # }
//! ```
//!
//! Nothing here prints: problems come back as reports.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod context;
mod digests;
mod escape;
mod index_pages;
mod mathjax;
mod nav;
mod pages;
mod render;
mod search;
mod site;
mod site_files;
mod templates;

#[cfg(test)]
extern crate self as gd_site; // the shared test harness names the crate.
#[cfg(test)]
mod tests;

pub use nav::{TocOptions, outline_html, sidebar_html};
pub use search::{STOPWORDS_EN, tokenize};
pub use site_files::pygments_css;

use gd_env::{BuilderKind, SourceFile};
use gd_host::Problem;
use gd_outputs::Generation;
use gd_report::Report;
use gd_resolve::{BuildView, OutputBuilder};
use guidedoc::Labels;

use crate::site::Site;
use crate::templates::Layout;

/// The html, dirhtml and singlehtml builders. Fill it with struct-update syntax:
/// `SiteBuilder { kind: BuilderKind::Dirhtml, labels: Some(&intl.labels), ..Default::default() }`.
#[derive(Clone, Copy, Default)]
pub struct SiteBuilder<'a> {
    /// The builder: `Html`, `Dirhtml` or `Singlehtml`; it must be the view's.
    pub kind: BuilderKind,
    /// The renderer's translated labels (gd-intl's `Intl::labels`); `None`: English.
    pub labels: Option<&'a Labels>,
    /// The project's documents as discovered: a generated page's `_sources` copy is its
    /// text. Empty: generated pages get no source copy.
    pub sources: &'a [SourceFile],
}

impl std::fmt::Debug for SiteBuilder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SiteBuilder")
            .field("kind", &self.kind)
            .field("labels", &self.labels.is_some())
            .field("sources", &self.sources.len())
            .finish()
    }
}

impl OutputBuilder for SiteBuilder<'_> {
    fn kind(&self) -> BuilderKind {
        self.kind
    }

    /// Writes the site into `generation`. Fails with `build.builder` when the builder
    /// or the view is not an HTML one, `template.error` when a template fails,
    /// `build.output` when a page could not be made (each reported), and with the
    /// problem of a file that cannot be written or a plugin that fails.
    fn write(
        &self,
        view: &BuildView<'_>,
        generation: &mut Generation,
        reports: &mut Vec<Report>,
    ) -> Result<(), Problem> {
        if view.kind != self.kind || !context::is_site_kind(self.kind) {
            return Err(not_a_site(self.kind, view.kind));
        }
        let layout = Layout::load(view, reports)?;
        let site = Site { view, layout, labels: self.labels, sources: self.sources };
        match view.kind {
            BuilderKind::Singlehtml => pages::write_singlehtml(&site, generation, reports),
            _ => pages::write_html(&site, generation, reports),
        }
    }
}

fn not_a_site(builder: BuilderKind, view: BuilderKind) -> Problem {
    Problem::new(
        "build.builder",
        "NOT AN HTML BUILDER",
        builder.name(),
        format!("The site builder was asked to write {} output for a {} build.", builder, view),
        "Report this as a Guidedog bug, with the command you ran and the output of \
         guidedog --version.",
        5,
    )
}
