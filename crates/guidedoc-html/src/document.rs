//! The page around a standalone document: head, title, meta tags, and style.

use guidedoc::{AttributeKey, NodeId, NodeKind, ROOT};

use crate::state::Html;

/// The built-in style of standalone documents: a readable measure, light and dark
/// schemes, and scrolling for wide tables and code.
const STYLESHEET: &str = r#":root { color-scheme: light dark; --fg: #1f2328; --bg: #ffffff;
  --muted: #59636e; --line: #d1d9e0; --code: #f6f8fa; --link: #0969da;
  --note: #ddf4ff; --warn: #fff8c5; }
@media (prefers-color-scheme: dark) { :root { --fg: #e6edf3; --bg: #0d1117;
  --muted: #9198a1; --line: #3d444d; --code: #151b23; --link: #4493f8;
  --note: #0c2d6b; --warn: #3b2e00; } }
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--fg);
  font: 17px/1.6 system-ui, -apple-system, "Segoe UI", sans-serif; }
main { max-width: 46rem; margin: 0 auto; padding: 2rem 1rem 4rem; }
h1, h2, h3, h4, h5, h6 { line-height: 1.25; margin: 2rem 0 1rem; }
a { color: var(--link); }
img { max-width: 100%; height: auto; }
pre, code { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 0.9em; background: var(--code); border-radius: 4px; }
code { padding: 0.1em 0.3em; }
pre { padding: 0.8rem 1rem; overflow-x: auto; }
pre code { padding: 0; background: none; }
blockquote { margin: 1rem 0; padding: 0 1rem; color: var(--muted);
  border-left: 4px solid var(--line); }
.table-wrapper { overflow-x: auto; margin: 1rem 0; }
table { border-collapse: collapse; }
th, td { border: 1px solid var(--line); padding: 0.4rem 0.7rem; vertical-align: top; }
hr { border: 0; border-top: 1px solid var(--line); margin: 2rem 0; }
aside { margin: 1rem 0; padding: 0.6rem 1rem; border-left: 4px solid var(--line); }
.admonition { background: var(--note); border-color: var(--link); }
.admonition.warning, .admonition.caution, .admonition.danger, .admonition.attention,
.admonition.error { background: var(--warn); border-color: #bf8700; }
.admonition-title, .topic-title, .sidebar-title, .contents-title { font-weight: 600;
  margin: 0 0 0.4rem; }
.footnote, .citation { font-size: 0.9em; border-color: var(--line); }
.footnote .label, .citation .label { margin: 0; }
.attribution, .rubric, figcaption { color: var(--muted); }
.line-block .line { min-height: 1.6em; }
.math { font-family: ui-monospace, monospace; }
nav.contents ul { padding-left: 1.2rem; }
.headerlink { visibility: hidden; margin-left: 0.3em; text-decoration: none; }
:hover > .headerlink, .eqno:hover .headerlink { visibility: visible; }
.hll { background: var(--warn); }
.linenos { color: var(--muted); padding-right: 1em; user-select: none; }
.code-block-caption { font-weight: 600; margin-top: 1rem; }
dt.sig { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
.sig-name { font-weight: 600; }
.versionmodified { font-style: italic; }
.eqno { float: right; }
.hlist td { border: 0; padding-right: 2rem; }
kbd { font-family: ui-monospace, monospace; font-size: 0.9em; padding: 0.1em 0.4em;
  border: 1px solid var(--line); border-radius: 4px; }
"#;

impl Html<'_, '_> {
    pub fn document_start(&mut self) {
        let o = self.options;
        self.w.put("<!DOCTYPE html>\n<html lang=\"");
        self.w.esc(if o.language.is_empty() { "en" } else { o.language });
        self.w.put("\">\n<head>\n<meta charset=\"utf-8\" />\n");
        self.w.put("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\n");
        self.w.put("<title>");
        self.write_document_title();
        self.w.put("</title>\n");
        self.write_meta();
        self.w.put("<style>\n");
        self.w.put(if o.stylesheet.is_empty() { STYLESHEET } else { o.stylesheet });
        self.w.cr();
        self.w.put("</style>\n</head>\n<body>\n<main>\n");
    }

    pub fn document_end(&mut self) {
        self.w.cr();
        self.w.put("</main>\n</body>\n</html>\n");
    }

    /// Writes the page title from, in order: the title option, the document's Title
    /// node, a title directive, the first heading, and a generic name.
    fn write_document_title(&mut self) {
        let doc = self.doc;
        if !self.options.title.is_empty() {
            self.w.esc(self.options.title);
            return;
        }
        if let Some(first) = doc.first_child(ROOT).filter(|&f| doc.kind(f) == NodeKind::Title) {
            self.write_plain(first);
            return;
        }
        let ids = (0..doc.node_count()).map(NodeId::from_index);
        let directive_title = ids
            .clone()
            .filter(|&id| {
                doc.kind(id) == NodeKind::Directive && self.directive_name(id) == "title"
            })
            .find_map(|id| doc.attribute(id, AttributeKey::Title));
        if let Some(title) = directive_title {
            self.w.esc(title);
            return;
        }
        match ids.clone().find(|&id| doc.kind(id) == NodeKind::Heading) {
            Some(heading) => self.write_plain(heading),
            None => self.w.put("Document"),
        }
    }

    /// Writes a meta tag for every Custom attribute of the meta directives.
    fn write_meta(&mut self) {
        let doc = self.doc;
        for i in 0..doc.node_count() {
            let id = NodeId::from_index(i);
            if doc.kind(id) != NodeKind::Directive || self.directive_name(id) != "meta" {
                continue;
            }
            for a in doc.attributes(id).iter().filter(|a| a.key == AttributeKey::Custom) {
                self.w.put("<meta name=\"");
                self.w.esc(doc.text(a.name));
                self.w.put("\" content=\"");
                self.w.esc(doc.text(a.value));
                self.w.put("\" />\n");
            }
        }
    }
}
