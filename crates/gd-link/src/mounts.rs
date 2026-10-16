//! mount://NAME/file links address public files in a declarative site's mounts.

use gd_env::{LookupKind, OutputFormat};
use gd_resolve::relative_url;
use guidedoc::{Document, Fail, LinkInfo, LinkKind, NodeId};

use crate::page::{Page, Warning};
use crate::rewrite::Rewriter;

pub(crate) fn is_mount(doc: Document<'_>, id: NodeId) -> bool {
    doc.link_info(id).is_some_and(|link| doc.text(link.destination).starts_with("mount://"))
}

pub(crate) fn replace(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
) -> Result<(), Fail> {
    let doc = rw.doc;
    let info = doc.link_info(id).unwrap_or(LinkInfo::UNRESOLVED);
    let uri = doc.text(info.destination);
    let target = gd_conf::mount_target(page.config(), uri);
    let Some(target) = target else {
        page.warn(
            doc,
            doc.node(id).span,
            Warning {
                code: "site.mount_link",
                title: "INVALID MOUNT LINK",
                category: "site",
                message: format!(
                    "The mount link {uri} does not name a configured mount and safe file path."
                ),
                hint: "Use mount://NAME/path.html with a name from site.mounts.",
            },
        );
        return rw.copy_node(page, id);
    };
    if target.split('#').next().is_some_and(|p| p.ends_with(".pdf"))
        && page.config().html.context.get("ssg_no_pdf").and_then(|v| v.as_bool()) == Some(true)
    {
        return rw.rewrite_children(page, id);
    }
    page.trace.note_key(LookupKind::Mount, &target);
    let context = &page.config().html.context;
    let text = |key| context.get(key).and_then(|v| v.as_str()).unwrap_or("");
    let here = crate::mounts::join(text("ssg_prefix"), &page.view.page_name(&page.info.docname));
    let url = if page.format == OutputFormat::Pdf && !text("ssg_base_url").is_empty() {
        format!("{}/{target}", text("ssg_base_url").trim_end_matches('/'))
    } else {
        relative_url(&here, &target)
    };
    let link = rw.open_link(&url, doc.text(info.title), LinkKind::External, doc.node(id).span)?;
    rw.rewrite_children(page, id)?;
    rw.close(link)
}

fn join(prefix: &str, path: &str) -> String {
    if prefix.is_empty() { path.into() } else { format!("{prefix}/{path}") }
}
