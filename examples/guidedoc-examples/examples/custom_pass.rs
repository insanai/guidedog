//! A pass rewrites a document between reading and rendering. This one shifts every
//! heading down one level, as a host might when embedding a page inside another page.
//!
//!     cargo run -p guidedoc-examples --example custom_pass

use std::process::ExitCode;

use guidedoc::{
    Builder, Document, NodeId, NodeKind, Pass, PassOutcome, ROOT, Workspace, convert,
    default_request,
};

/// The deepest heading level HTML has.
const DEEPEST: i32 = 6;

/// Copies the document, one top-level subtree at a time, and demotes each heading copy.
///
/// Borrows the current document (immutable) and the builder of the next one. Returns
/// `Rebuilt`, or `Failed` when the builder refuses a copy (it has recorded why, and the
/// engine reports that failure).
fn demote(doc: Document<'_>, b: &mut Builder<'_>) -> PassOutcome {
    for child in doc.children(ROOT) {
        let Ok(copy) = b.copy_subtree(doc, child) else {
            return PassOutcome::Failed;
        };
        // A subtree is a contiguous run of nodes, and its copy is laid out the same way,
        // so node `i` of the original is node `copy + (i - child)` of the copy.
        for index in child.index()..doc.subtree_end(child).index() {
            let id = NodeId::from_index(index);
            if doc.kind(id) != NodeKind::Heading {
                continue;
            }
            let level = doc.node(id).value;
            let offset = (index - child.index()) as u32;
            b.set_value(copy.plus(offset), (level + 1).min(DEEPEST));
        }
    }
    PassOutcome::Rebuilt
}

fn main() -> ExitCode {
    let text = "# Title\n\n## Part\n\nBody.\n";
    let passes = [Pass { name: "demote", run: &demote }];
    // The "commonmark" reader (pure CommonMark) keeps headings flat, with their level as
    // the node's value; "myst" would nest them in sections, whose depth gives the level.
    let mut request = default_request("page.md", text, "commonmark", "html");
    request.render.standalone = false;
    request.passes = &passes;

    let registry = guidedoc_examples::registry();
    let mut ws = Workspace::new();
    match convert(&mut ws, &registry, &request) {
        Ok(html) => {
            println!("{html}");
            ExitCode::SUCCESS
        }
        Err(fail) => {
            eprintln!("{fail}");
            ExitCode::FAILURE
        }
    }
}
