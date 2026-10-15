# gd-link cheat sheet

The link pass of a Guidedog build and the rendering of a linked page: the rewriter,
cross-references in the tree, navigation, the todolist, graphs and their theme, and
discussion metadata.

```rust
let mut worker = gd_link::Worker::new();                 // one per rendering thread
let options = gd_link::render_options(&view, doc, OutputFormat::Html); // + highlighter, labels
let page = gd_link::render_page(&view, &mut worker, &PageRequest {
    doc, format: OutputFormat::Html, renderer: "html", options, ..PageRequest::default() });
// page: gd_env::Rendered { body, assets, trace, reports, failed }
let place = gd_link::source_location(&view, &mut worker, &request, offset); // Typst errors
let items = gd_link::nav_for_toctree(&view, doc, 0, NavOptions::default());
```

| Item | Does |
|---|---|
| `render_page`, `source_location`, `render_options`, `PageRequest`, `Worker` | load the object, link, render |
| `Rule` (`visits`, `decide`, `replace`, `decorate`), `Action`, `Rewriter`, `rewrite_document` | the rewriter |
| `NavItem`, `NavOptions`, `nav_for_toctree`, `toc_order`, `TocChild`, `top_sections`, `untitled`, `doc_limit`, `within`, `only_shown`, `section_number`, `toctree_entries` | navigation (gd-site builds the sidebar and outline on these) |
| `evaluate_only`, `Tags` | `only` expressions |
| `book_root`, `book_path` | the paths the book reads files by |
| `todo_order` | the documents a todolist copies from |
| `adapt_graph`, `graph_adaptive`, `GRAPH_ADAPTIVE_CSS` | the adaptive graph theme |
| `discussion_field`, `discussion_prefix` | discussion metadata |

## Design notes

- **Rewriter over `copy_subtree`**: a rule is a `Rule` trait object. The rule says which nodes it `visits`; a subtree it
  takes no interest in is copied whole by `Builder::copy_subtree`, so the rewriter is a
  thin decide/replace dispatch and the link pass walks only the nodes it acts on.
- **AST-free resolution**: each Xref becomes a `gd_resolve::XrefQuery` (with every Custom
  attribute as context); `resolve_xref` answers it and notes lookups in the page's
  `Trace`. Warnings about references that still resolve come back in
  `Resolved::warning` and are placed at the reference; downloads come back as
  `Resolved::asset`. `must_resolve`, `report_numref` and the anchor warning live in
  gd-resolve with the resolution.
- **No arenas or retries**: workspaces grow on demand and `Worker` keeps them for the next page. Only the coarse
  budget stays: a page's graph files and drawings are charged to a `gd_host::Ledger`
  that settles on drop.
- **One Graphviz context per worker**: `Worker` holds a `OnceCell<Option<Context>>`,
  started on the first graph, so the build keeps no Graphviz flags; a worker whose Graphviz cannot start says so once.
- **Pass state in a `RefCell`**: `guidedoc::Pass` takes an `Fn`, so the page the pass
  fills is borrowed mutably inside it and taken back after the transform.
- **Indexes as one enum**: the per-node `sections`, `numbered` and `cpp_decls` arrays of
  `-1`/index are one `Vec<Indexed>`, which also counts toctrees, once per page rather
  than once per toctree.
- **Book inputs**: in the book (`OutputFormat::Pdf`) a page's `assets` list the files its
  Typst source reads (images, graphs) with their book paths as targets, so no
  build-wide set records them.
- **Reports** are `gd_report::Report`s located with `gd_report::set_span` on the span's own
  source (an included file's path when it came from one); a graph's warning points at
  the graph rather than at its first column.
- **Base64** for embedded graph images is written in-house (std only).
- The sidebar and the page outline (`sidebar_html`, `outline_html`) are gd-site's; the
  numfig template and the image lookup are `BuildView::number_template` and
  `BuildView::image_file` in gd-resolve, shared with lookup answers.
