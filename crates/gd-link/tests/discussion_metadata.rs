//! Discussion metadata (ported from discussion_metadata_test.odin): the leading field
//! list of a discussion record gets readable labels and its JSON lists plain text, in
//! every output; ordinary fields and the source keep their names.

#![allow(clippy::unwrap_used)]

mod common;

use common::Project;
use gd_env::OutputFormat;
use gd_link::{PageRequest, Worker, render_options, render_page};

/// Renders `index` with `renderer` and returns the body, checking it did not fail.
fn render(project: &Project, renderer: &str, format: OutputFormat) -> String {
    let view = project.view();
    let doc = project.id("index");
    let request = PageRequest {
        doc,
        format,
        renderer,
        options: render_options(&view, doc, format),
        ..PageRequest::default()
    };
    let page = render_page(&view, &mut Worker::new(), &request);
    assert!(!page.failed, "{renderer}: {:?}", page.reports);
    page.body
}

#[test]
fn discussion_metadata_presentation_preserves_source_and_ordinary_fields() {
    let source = ":gds-title: Native projects\n:gds-number: 0006\n:gds-state: committed\n\
        :gds-authors: [\"Ada\", \"René <script>\"]\n\
        :gds-labels: [\"architecture\", \"build\"]\n\n\
        Native projects\n===============\n\n\
        Ordinary fields\n---------------\n\n:gds-number: an example\n";
    let project = Project::new("discussion", &[("index", source)]);
    for (renderer, format) in [("html", OutputFormat::Html), ("text", OutputFormat::Text)] {
        let text = render(&project, renderer, format);
        assert!(text.contains("Authors"), "{text}");
        assert!(text.contains("architecture, build"));
        assert!(text.contains("Ada, René"));
        assert!(text.contains("gds-number"), "the ordinary field keeps its name");
        assert!(!text.contains("gds-authors"));
        assert!(!text.contains("gds-labels"));
        if renderer == "html" {
            assert!(text.contains("&lt;script&gt;"));
            assert!(!text.contains("René <script>"));
        }
    }
    let after = std::fs::read_to_string(project.dir.0.join("index.rst")).unwrap();
    assert_eq!(after, source);
}

#[test]
fn discussion_metadata_supports_other_namespaces_and_memo_fields() {
    for prefix in ["sds", "eds", "architecture"] {
        let header = ":PREFIX-number: XXXXX\n:PREFIX-title: Generic discussions\n\
            :PREFIX-authors: [\"Ada\", \"René <script>\"]\n\
            :PREFIX-labels: [\"process\", \"cli\"]\n:PREFIX-last-updated: 2026-10-12\n\
            :PREFIX-category: Engineering Discussion\n:PREFIX-status: Working draft\n\
            :PREFIX-summary: Reader metadata\n:gds-state: mixed namespace\n\
            :PREFIX-unknown: Preserve me\n\nGeneric discussions\n===================\n\n\
            Ordinary fields\n---------------\n\n:PREFIX-number: literal example\n";
        let source = header.replace("PREFIX", prefix);
        let project = Project::new(&format!("generic-{prefix}"), &[("index", &source)]);
        for (renderer, format) in [("html", OutputFormat::Html), ("text", OutputFormat::Text)] {
            let text = render(&project, renderer, format);
            for label in ["Authors", "Updated", "Category", "Status", "Summary"] {
                assert!(text.contains(label), "{prefix} {renderer} lacks {label}");
            }
            assert!(text.contains("Ada, René"));
            assert!(text.contains("process, cli"));
            assert!(!text.contains(&format!("{prefix}-authors")));
            assert!(text.contains(&format!("{prefix}-number")));
            assert!(text.contains(&format!("{prefix}-unknown")));
            assert!(text.contains("gds-state"));
            if renderer == "html" {
                assert!(text.contains("&lt;script&gt;"));
                assert!(!text.contains("René <script>"));
            }
        }
    }
}

// The book has the same readable metadata: its Typst source shows the labels and the
// lists, never the field names.
#[test]
fn discussion_metadata_presentation_reaches_the_book() {
    let source = ":eds-number: XXXXX\n:eds-title: Cache policy\n\
        :eds-authors: [\"Ada\", \"René\"]\n:eds-labels: [\"caching\", \"architecture\"]\n\
        :eds-last-updated: 2026-10-12\n:eds-status: Working draft\n\n\
        Cache policy\n============\n\nAbstract\n--------\n\nA proposal.\n";
    let mut project = Project::new("discussion-book", &[("index", source)]);
    project.kind = gd_env::BuilderKind::Pdf;
    let text = render(&project, "typst", OutputFormat::Pdf);
    for want in ["Authors", "Ada", "René", "caching", "architecture", "Updated", "Status"] {
        assert!(text.contains(want), "the book lacks {want}: {text}");
    }
    assert!(!text.contains("eds-authors"));
    assert!(!text.contains("eds-last-updated"));
}

// A page that starts with fields but is not a discussion keeps them as written.
#[test]
fn ordinary_leading_fields_keep_their_names() {
    let source =
        ":last-updated: 2026-10-12\n:last-reviewed: 2026-10-01\n\nNotes\n=====\n\nText.\n";
    let project = Project::new("ordinary", &[("index", source)]);
    let text = render(&project, "text", OutputFormat::Text);
    assert!(text.contains("last-updated"), "{text}");
}
