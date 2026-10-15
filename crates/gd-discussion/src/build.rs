//! Publishing a catalog: the HTML site and a PDF book per record, through a [`Builder`].

use crate::catalog::{Catalog, Format, catalog_format, check_sources, resolve};
use crate::index::check_index;
use crate::metadata::{Record, is_rst};
use crate::problem::shown;
use crate::{Prefix, Problem, Result};
use std::path::PathBuf;

/// What one publication makes: the catalog's website, or its PDF books.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The complete website, in `_build/html`.
    Html,
    /// A PDF book per record, in `_build/pdf`.
    Pdf,
}

impl Target {
    /// The builder name Guidedog uses: `"html"` or `"pdf"`.
    pub fn name(self) -> &'static str {
        match self {
            Target::Html => "html",
            Target::Pdf => "pdf",
        }
    }
}

/// One PDF a build makes: the document it starts from (`records/0007-cache`), the file
/// it writes (`gds-0007-cache.pdf`), and the title and author of its cover.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Book {
    /// The root document, relative to the catalog, without extension.
    pub root: String,
    /// The PDF's file name.
    pub file: String,
    /// The cover's title.
    pub title: String,
    /// The cover's author line: the record's authors, comma-separated.
    pub author: String,
}

/// One publication of the catalog in `dir`, prepared by [`build`] after it has checked
/// the catalog. A selected PDF build (`selection`, the record's artifact name, such as
/// `gds-0007-cache`) writes its book under `_build/selected/SELECTION/pdf` and leaves
/// the catalog's complete PDF output alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRequest {
    /// The catalog's folder, a Guidedog project.
    pub dir: PathBuf,
    /// The catalog's prefix.
    pub prefix: Prefix,
    /// What to make.
    pub target: Target,
    /// The PDF books, one per selected record (also given for HTML, which ignores them).
    pub books: Vec<Book>,
    /// The artifact name of the one record a PDF preview builds, if any.
    pub selection: Option<String>,
}

/// Publishes catalogs. Warnings are errors: a builder fails rather than publish a
/// catalog with problems, and its problem says what to fix.
///
/// [`crate::GuidedogProgram`] runs an installed `guidedog`; a program with Guidedog's
/// engine linked in (`dscc`) implements it with the in-process project builder.
pub trait Builder {
    /// A short name for messages, such as `"external"`.
    fn name(&self) -> &str;

    /// Makes one publication and returns the folder it wrote.
    fn build(&self, request: &BuildRequest) -> Result<PathBuf>;

    /// Builds an older Typst catalog's `selected` records for `targets`, returning what
    /// it wrote. The default refuses: only a builder with a Typst compiler can.
    fn build_typst_catalog(
        &self,
        catalog: &Catalog,
        selected: &[&Record],
        targets: &[Target],
    ) -> Result<Vec<PathBuf>> {
        let _ = (selected, targets);
        Err(Problem::new(
            catalog.prefix.code("legacy-build"),
            "This program cannot build a Typst catalog",
            shown(&catalog.dir),
            format!("The {} builder builds reStructuredText catalogs only.", self.name()),
            "Build the Typst catalog with guidedog gds build, or port its records to RST.",
        )
        .exit(2))
    }
}

/// What [`build`] publishes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildOptions {
    /// The one target to make, or `None` for both.
    pub target: Option<Target>,
    /// The record whose PDF to preview (number, slug, title, or file name); empty for
    /// the whole catalog.
    pub record: String,
}

/// Checks `catalog` (sections, and the index unless one record of a Typst catalog is
/// selected) and publishes it with `builder`: the complete website, and a PDF book per
/// record or for the record `options.record` names. Returns what it wrote, in order.
pub fn build(
    catalog: &Catalog,
    options: &BuildOptions,
    builder: &dyn Builder,
) -> Result<Vec<PathBuf>> {
    let prefix = &catalog.prefix;
    let format = catalog_format(&catalog.dir);
    check_sources(catalog)?;
    if options.record.is_empty() || format == Format::Rst {
        check_index(catalog)?;
    }
    let mut selected: Vec<&Record> = catalog.records.iter().collect();
    if !options.record.is_empty() {
        let r = resolve(catalog, &options.record)?;
        if r.historical {
            return Err(Problem::new(
                prefix.code("historical"),
                "Historical record is an archive",
                shown(&r.path),
                "This record is kept as provenance, not as an active discussion book.",
                "Read its archive page in the complete HTML build instead.",
            )
            .exit(2));
        }
        selected = vec![r];
    }
    let targets: Vec<Target> = match options.target {
        Some(t) => vec![t],
        None => vec![Target::Html, Target::Pdf],
    };
    if format == Format::Typst {
        return builder.build_typst_catalog(catalog, &selected, &targets);
    }
    if let Some(r) = catalog.records.iter().find(|r| !r.historical && !is_rst(&r.path)) {
        return Err(Problem::new(
            prefix.code("format"),
            "Mixed publication formats",
            shown(&r.path),
            "This RST catalog contains a Typst record.",
            "Port the record to RST, or build it in an older Typst catalog.",
        ));
    }
    let books: Vec<Book> = selected
        .iter()
        .filter(|r| !r.historical)
        .map(|r| Book {
            root: format!("records/{}", r.stem()),
            file: format!("{}.pdf", artifact_name(prefix, r)),
            title: r.title.clone(),
            author: r.authors.join(", "),
        })
        .collect();
    let mut outputs = Vec::with_capacity(targets.len());
    for target in targets {
        let selection = (target == Target::Pdf && !options.record.is_empty())
            .then(|| artifact_name(prefix, selected[0]));
        let request = BuildRequest {
            dir: catalog.dir.clone(),
            prefix: prefix.clone(),
            target,
            books: books.clone(),
            selection,
        };
        outputs.push(builder.build(&request)?);
    }
    Ok(outputs)
}

/// The name of a record's book and pages: `gds-0007-cache`.
pub fn artifact_name(prefix: &Prefix, record: &Record) -> String {
    format!("{prefix}-{}", record.stem())
}
