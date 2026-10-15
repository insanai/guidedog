//! The adaptive graph theme (graph_theme.odin): a graph with the `graphviz-adaptive`
//! class gets a fixed stylesheet in its SVG root, so a browser in dark mode recolors
//! it. The stylesheet is part of the drawing's cache identity; it carries no document
//! text, script, external URL, or machine path. The book keeps Graphviz's light colors.

use gd_host::Ledger;
use guidedoc::{Document, NodeId};

/// The fixed stylesheet adaptive drawings carry.
pub const GRAPH_ADAPTIVE_CSS: &str = include_str!("../assets/graphviz-adaptive.css");

/// The class a graph opts in with.
pub const ADAPTIVE_CLASS: &str = "graphviz-adaptive";

/// Why a drawing could not be adapted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaptError {
    /// The budget refused the adapted copy (else Graphviz's SVG was not as expected).
    pub out_of_memory: bool,
    /// What went wrong.
    pub message: &'static str,
    /// How to fix it.
    pub hint: &'static str,
}

/// Whether a graph opted in to the adaptive theme with its class, not by its colors.
pub fn graph_adaptive(doc: Document<'_>, id: NodeId) -> bool {
    doc.has_class(id, ADAPTIVE_CLASS)
}

/// Inserts the stylesheet right after Graphviz's SVG root tag. A separate SVG image
/// inherits the embedding element's color-scheme; no inline SVG or JavaScript is
/// needed. The whole copy is charged to `ledger` before it is made; the charge stays
/// with the ledger.
pub fn adapt_graph(svg: &[u8], ledger: &Ledger<'_>) -> Result<Vec<u8>, AdaptError> {
    const HINT: &str = "Remove graphviz-adaptive from this graph's class option and build again.";
    const PREFIX: &str = "\n<style type=\"text/css\"><![CDATA[\n";
    const SUFFIX: &str = "\n]]></style>\n";
    let Some(start) = find(svg, b"<svg ") else {
        return Err(AdaptError {
            out_of_memory: false,
            message: "Graphviz returned no SVG root.",
            hint: HINT,
        });
    };
    let Some(end) = find(&svg[start..], b">") else {
        return Err(AdaptError {
            out_of_memory: false,
            message: "Graphviz returned an unfinished SVG root.",
            hint: HINT,
        });
    };
    let at = start + end + 1;
    let extra = PREFIX.len() + GRAPH_ADAPTIVE_CSS.len() + SUFFIX.len();
    let size = svg.len().checked_add(extra);
    let mut out = Vec::new();
    let reserved = size.is_some_and(|n| ledger.charge(n));
    if !reserved || out.try_reserve_exact(size.unwrap_or(usize::MAX)).is_err() {
        if reserved {
            ledger.discharge(size.unwrap_or(0));
        }
        return Err(AdaptError {
            out_of_memory: true,
            message: "The adaptive drawing exceeds the build's memory budget.",
            hint: "Raise the build's --budget, or remove graphviz-adaptive from its class option.",
        });
    }
    out.extend_from_slice(&svg[..at]);
    out.extend_from_slice(PREFIX.as_bytes());
    out.extend_from_slice(GRAPH_ADAPTIVE_CSS.as_bytes());
    out.extend_from_slice(SUFFIX.as_bytes());
    out.extend_from_slice(&svg[at..]);
    Ok(out)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gd_host::Budget;

    const SOURCE: &str = "<?xml version=\"1.0\"?><svg width=\"72pt\" viewBox=\"0 0 72 30\">\
        <g class=\"node gd-plugin\"><text>A &amp; B</text></g></svg>";

    // Styling preserves the SVG root, geometry, escaped labels and external-image
    // isolation, and reserves the copy before making it.
    #[test]
    fn adaptive_graph_preserves_svg_and_reserves_before_copy() {
        let budget = Budget::new(1 << 20);
        let ledger = Ledger::new(&budget);
        let out = adapt_graph(SOURCE.as_bytes(), &ledger).expect("adapted");
        let text = std::str::from_utf8(&out).expect("UTF-8");
        let root_end = SOURCE.find('>').map_or(0, |i| i + 1);
        assert!(text.starts_with(&SOURCE[..root_end]));
        assert!(text.contains("viewBox=\"0 0 72 30\">\n<style"));
        assert!(text.contains("@media screen and (prefers-color-scheme: dark)"));
        assert!(text.ends_with("<g class=\"node gd-plugin\"><text>A &amp; B</text></g></svg>"));
        assert_eq!(ledger.held(), out.len());
        assert!(!GRAPH_ADAPTIVE_CSS.contains("url("));
    }

    #[test]
    fn adaptive_graph_refusals_hold_nothing() {
        let small = Budget::new(1);
        let refused_ledger = Ledger::new(&small);
        let refusal = adapt_graph(SOURCE.as_bytes(), &refused_ledger).expect_err("refused");
        assert!(refusal.out_of_memory);
        assert_eq!(refused_ledger.held(), 0);
        assert!(refusal.hint.contains("--budget"));
        let unbounded = Ledger::unbounded();
        let malformed = adapt_graph(b"not SVG", &unbounded).expect_err("malformed");
        assert!(!malformed.out_of_memory);
        assert!(malformed.message.contains("no SVG root"));
    }
}
