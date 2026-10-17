//! Validate excerpt boundaries before an object can enter the build cache.

use gd_env::DocInfo;
use gd_report::Report;
use guidedoc::{AttributeKey, Document, NodeId, NodeKind, ROOT};

pub(super) fn collect(info: &mut DocInfo, doc: Document<'_>) {
    for i in 0..doc.node_count() {
        let id = NodeId::from_index(i);
        if doc.kind(id) != NodeKind::Directive
            || doc.attribute(id, AttributeKey::Name) != Some("more")
        {
            continue;
        }
        let parent = doc.parent_of(id);
        let top_level = parent == Some(ROOT) || parent == Some(gd_env::excerpt_root(doc));
        let reason = if info.excerpt {
            Some("A document can contain only one excerpt boundary.")
        } else if !top_level {
            Some("An excerpt boundary must be at document level, outside sections and directives.")
        } else {
            None
        };
        info.excerpt = true;
        if let Some(reason) = reason {
            info.failed = true;
            let mut report = Report {
                path: info.relative.clone(),
                ..Report::error("page.excerpt", "INVALID EXCERPT BOUNDARY", reason).hint(
                    "Put one unindented .. more:: after the introduction, before any subsection.",
                )
            };
            let span = doc.node(id).span;
            gd_report::set_span(
                &mut report,
                doc.source_str(span.source),
                span.start as usize,
                span.end as usize,
            );
            info.reports.push(report);
        }
    }
}
