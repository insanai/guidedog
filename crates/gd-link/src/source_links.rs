//! `[source]` links after object signatures, in HTML: sphinx.ext.viewcode's, to the
//! highlighted source page of the module that defines the object, and
//! sphinx.ext.linkcode's, to the URL `linkcode_url` makes for it. Reading found where
//! each Python object is defined (`DocInfo::locations`); an object of another domain has
//! only its names, so only a template that needs nothing more links it.

use gd_conf::{LinkcodeObject, LinkcodeSite};
use gd_env::{BuilderKind, OutputFormat, SourceLocation};
use gd_report::Report;
use gd_resolve::relative_url;
use guidedoc::{AttributeKey, Document, Fail, LinkKind, NodeId};

use crate::page::{Page, locate};
use crate::rewrite::Rewriter;

/// The folder of the source pages, as Sphinx names it.
pub const MODULES_FOLDER: &str = "_modules";

/// The source page of a Python module, from the output root: `_modules/a/b.html`.
pub fn module_page(module: &str, suffix: &str) -> String {
    format!("{MODULES_FOLDER}/{}{suffix}", module.replace('.', "/"))
}

/// Whether the build writes viewcode's source pages and links: the extension is listed
/// and the builder is html or dirhtml, as Sphinx's `is_supported_builder` decides.
pub fn viewcode_pages(config: &gd_conf::Config, kind: BuilderKind) -> bool {
    config.has_extension("sphinx.ext.viewcode")
        && matches!(kind, BuilderKind::Html | BuilderKind::Dirhtml)
}

/// Finds the `[source]` links of signature `id`, keeps them on the page for
/// [`replace_signature`], and tells whether there are any.
pub(crate) fn find_links(page: &mut Page<'_>, doc: Document<'_>, id: NodeId) -> bool {
    page.source_links.clear();
    if page.format != OutputFormat::Html {
        return false;
    }
    let anchor = doc.attribute(id, AttributeKey::Id).unwrap_or("");
    let location = page.info.locations.iter().find(|l| !anchor.is_empty() && l.anchor == anchor);
    if let Some(l) = location
        && viewcode_pages(page.config(), page.view.kind)
    {
        let here = page.view.page_name(&page.info.docname);
        let target = module_page(&l.defined_in, &page.config().html.file_suffix);
        let url = format!("{}#{}", relative_url(&here, &target), l.fullname);
        page.source_links.push(url);
    }
    if let Some(url) = linkcode_url(page, doc, id, location) {
        page.source_links.push(url);
    }
    !page.source_links.is_empty()
}

/// The URL `linkcode_url` gives the object signature `id` describes, if any.
fn linkcode_url(
    page: &mut Page<'_>,
    doc: Document<'_>,
    id: NodeId,
    location: Option<&SourceLocation>,
) -> Option<String> {
    if page.linkcode.is_none() {
        page.linkcode = Some(LinkcodeSite::of(page.config()));
    }
    let site = page.linkcode.as_ref()?;
    if site.template.is_empty() {
        return None;
    }
    let desc = doc.parent_of(id)?;
    let object = LinkcodeObject {
        domain: doc.custom_attribute(desc, "domain").unwrap_or(""),
        module: doc.custom_attribute(id, "module").unwrap_or(""),
        fullname: doc.custom_attribute(id, "fullname").unwrap_or(""),
        source: location.map(|l| (l.path.as_path(), l.line, l.end_line)),
    };
    if object.fullname.is_empty() {
        return None;
    }
    match site.url(&object) {
        Ok(url) => url,
        Err(file) => {
            let message = format!(
                "{} is defined in {}, outside the folder linkcode_url's {{path}} is relative \
                 to, so it has no [source] link.",
                object.fullname,
                file.display()
            );
            let mut report = Report {
                path: page.info.relative.clone(),
                category: "linkcode".to_owned(),
                ..Report::note("linkcode.outside", "SOURCE OUTSIDE THE LINKCODE ROOT", &message)
                    .hint("Set linkcode_root in conf.toml to a folder that holds the file.")
            };
            locate(doc, doc.node(id).span, &mut report);
            page.reports.push(report);
            None
        }
    }
}

/// Copies signature `id` and adds the links [`find_links`] found after its content,
/// where Sphinx puts them: before the permalink.
pub(crate) fn replace_signature(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
) -> Result<(), Fail> {
    let links = std::mem::take(&mut page.source_links);
    let span = rw.doc.node(id).span;
    let label = page.view.ui.gettext("[source]");
    rw.copy_node_and(page, id, |rw| {
        for url in &links {
            let link = rw.open_link(url, "", LinkKind::External, span)?;
            rw.add_class(link, "viewcode-link")?;
            rw.add_plain(&label, span)?;
            rw.close(link)?;
        }
        Ok(())
    })
}
