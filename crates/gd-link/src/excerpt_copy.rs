//! Relocate linked AST nodes without inspecting or rewriting rendered HTML.

use gd_env::OutputFormat;
use gd_resolve::{BuildView, relative_url};
use guidedoc::{
    Anchors, AttributeKey, Builder, Document, Fail, LinkKind, NodeId, Pass, PassOutcome, ROOT,
    RenderOptions, Workspace, build_anchors,
};

use crate::{Excerpt, PageRequest};

pub(crate) fn relocate(
    view: &BuildView<'_>,
    ws: &mut Workspace,
    request: &PageRequest<'_>,
    excerpt: &Excerpt<'_>,
) -> bool {
    let source = view.page_name(&view.doc(request.doc).docname);
    let run = |doc: Document<'_>, b: &mut Builder<'_>| {
        b.set_value(ROOT, doc.node(ROOT).value);
        let options = RenderOptions { anchor_prefix: excerpt.prefix, ..Default::default() };
        let anchors = build_anchors(doc, &options, true);
        let copy = Copy { anchors, source: &source, request, excerpt };
        for child in doc.children(ROOT) {
            if copy.node(doc, b, child).is_err() {
                return PassOutcome::Failed;
            }
        }
        PassOutcome::Rebuilt
    };
    guidedoc::transform_document(ws, &[Pass { name: "excerpt-urls", run: &run }], view.limits)
        .is_ok()
}

struct Copy<'a> {
    anchors: Anchors,
    source: &'a str,
    request: &'a PageRequest<'a>,
    excerpt: &'a Excerpt<'a>,
}

impl Copy<'_> {
    fn node(&self, doc: Document<'_>, b: &mut Builder<'_>, id: NodeId) -> Result<(), Fail> {
        let n = doc.node(id);
        let copy = b.begin_node(n.kind, n.span)?;
        b.set_value(copy, n.value);
        b.add_flags(copy, n.flags);
        let text = b.add_text(doc.node_text(id))?;
        b.set_text(copy, text);
        for a in doc.attributes(id) {
            if a.key == AttributeKey::Id
                || a.key == AttributeKey::Custom && doc.text(a.name) == "name"
            {
                continue;
            }
            let value = b.add_text(doc.text(a.value))?;
            let name = b.add_text(doc.text(a.name))?;
            b.add_attribute_named(copy, a.key, value, name)?;
        }
        for anchor in self.anchors.anchor_ids(id) {
            b.add_attribute_string(copy, AttributeKey::Id, anchor)?;
        }
        b.add_custom_attribute(copy, "pinned-ids", "1")?;
        if let Some(mut link) = doc.link_info(id) {
            let url = self.url(doc.text(link.destination), link.kind);
            link.destination = b.add_text(&url)?;
            link.title = b.add_text(doc.text(link.title))?;
            link.name = b.add_text(doc.text(link.name))?;
            b.set_link(copy, link)?;
        } else {
            b.copy_payload(doc, id, copy)?;
        }
        for child in doc.children(id) {
            self.node(doc, b, child)?;
        }
        b.end_node(copy)
    }

    fn url(&self, url: &str, kind: LinkKind) -> String {
        if self.request.format != OutputFormat::Html
            || matches!(kind, LinkKind::Internal | LinkKind::Email | LinkKind::Unresolved)
            || url.is_empty()
            || url.starts_with('/')
            || url.split(['/', '#', '?']).next().is_some_and(|s| s.contains(':'))
        {
            return url.to_owned();
        }
        let split = url.find(['#', '?']).unwrap_or(url.len());
        let path = if split == 0 {
            self.source.to_owned()
        } else {
            source_target(self.source, &url[..split])
        };
        let mut relative = relative_url(self.excerpt.destination, &path);
        if url[..split].ends_with('/') {
            relative.push('/');
        }
        format!("{relative}{}", &url[split..])
    }
}

/// Cross-mount URLs can climb above the local output root. Unlike docname lookup,
/// relocation must retain those leading parent components.
fn source_target(source: &str, url: &str) -> String {
    let dir = source.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut parts: Vec<_> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for part in url.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|p| *p != "..") => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::source_target;

    #[test]
    fn relocation_preserves_cross_mount_parent_components() {
        assert_eq!(
            source_target("posts/one.html", "../../manual/index.html"),
            "../manual/index.html"
        );
        assert_eq!(source_target("one.html", "../manual/index.html"), "../manual/index.html");
        assert_eq!(source_target("posts/one.html", "../_images/lead.svg"), "_images/lead.svg");
    }
}
