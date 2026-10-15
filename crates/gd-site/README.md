# gd-site cheat sheet

Guidedog's html, dirhtml and singlehtml builders: gd-resolve's `OutputBuilder` for the
website. Ported from `guidedog_old/lib/project` (html_builder.odin, html_site.odin,
templates.odin, additional_pages.odin, the index side of search.odin, mathjax.odin, the
singlehtml parts of builders.odin, the genindex/modindex bodies, nav.odin's sidebar and
outline, plugins.odin's HTML hooks and assets, intl.odin's template functions).

```rust
use gd_resolve::OutputBuilder;
let builder = gd_site::SiteBuilder {
    kind: BuilderKind::Dirhtml,          // must be view.kind
    labels: Some(&intl.labels),          // translated renderer labels; None: English
    sources: &sources,                   // generated pages' text for _sources
};
builder.write(&view, &mut generation, &mut reports)?; // then generation.commit(..)
```

| Item | Does |
|---|---|
| `SiteBuilder { kind, labels, sources }` | the builder (`OutputBuilder::kind`, `write`) |
| `sidebar_html(&view, current, TocOptions)`, `TocOptions::{SIDEBAR, TOCTREE}` | the global toctree (`toc`, Sphinx's `toctree()`) |
| `outline_html(&view, doc)` | the "On this page" rail |
| `tokenize(text, \|word\| ..)`, `STOPWORDS_EN` | how the search index splits words (the theme's script splits queries the same way) |
| `pygments_css(light, dark)` | `_static/pygments.css` for `pygments_style` / `pygments_dark_style` |

Output: one unit per document (`<docname>.html`, `<docname>/index.html` in dirhtml, or
the single page in singlehtml's site unit); the site unit (`SITE_UNIT`: theme files from
gd-theme, `pygments.css`, `html_static_path`, `html_extra_path`, logo and favicon,
`_images`/`_downloads`, `_sources`, `_static/plugins/<id>/`, `search.html`,
`py-modindex.html`, `genindex*.html`, `html_additional_pages`, `.nojekyll`/`CNAME`,
`objects.inv`); and the `search` unit (`_static/searchindex.js`, read by
`gd_theme::GUIDEDOG_JS`). Unit inputs are listed in
`docs/manual/internals/incremental.rst`.

Templates see Sphinx's names: `pagename`, `title`, `docstitle`, `project`, `release`,
`copyright`, `css_files`, `js_files`/`script_files`, `rellinks`, `parents`,
`metatags`, `reldelim1/2`, `embedded`, `html_context` values, and the functions
`pathto`, `hasdoc`, `toctree`, `css_tag`, `js_tag`, `_`, `gettext`, `ngettext`; plus
Guidedog's `toc`, `outline`, `prev`/`next`, `accent`, `root_url`, `search_url`, ...

Codes: `template.error` (the template's file and line), `build.output` (pages not
written), `build.template.{outside,link}`, `build.additional_page`,
`build.singlehtml.id`, `build.search` (new: a document's search words could not be
read), `plugins.config` (a plugin asset takes a project file's path), `build.builder`.

## Redesign notes

- **No `Build` god object or scratch arenas.** `SiteBuilder` borrows a `BuildView`; a
  `Site` holds the view and the layout for one `write`. Pages render on
  `gd_host::run_workers` in batches (16 per thread), each worker with its own
  `gd_link::Worker` and `HtmlHighlighter`; the main thread lays them out and writes them
  in docname order, so output does not depend on scheduling.
- **Template functions borrow the build.** `css_tag`, `js_tag` and Sphinx's fixed
  names are the environment's; `pathto`, `hasdoc`, `toctree`, `_`, `gettext` and
  `ngettext` are gd-jinja `Locals` of each page's render (`Template::render_with`),
  borrowing the `BuildView` and the page, so each page renders exactly once. The
  loader is `Loader::custom` (templates_path first, then gd-theme's `layout.html` for
  `layout.html`, `!layout.html`, `basic/...` and known theme names).
- **Plugins** go through `gd_env::PluginRun` (`process_html` per page, special pages
  included; `assets()` written in the site unit), not a build-wide mutex.
- **Additional pages and plugin assets** check the site unit's own files
  (`gd_outputs::Unit::files`, an additive accessor) instead of the previous
  generation's, so a page or asset never conflicts with itself on a rebuild.
- **Theme files**: a project's own `_static/guidedog.css`, `guidedog.js` or
  `pygments.css` (in `html_static_path`) replaces the built-in one instead of being
  written over it; page assets are copied once each, in target order.
- **Search**: a missing or damaged words file is a `build.search` warning (Odin left the
  page out silently). Words use guidedoc's Unicode L/Nd tables, as the script's
  `\p{L}\p{Nd}`.
- **singlehtml** charges the page to the budget as Odin did (a `Ledger` settled on
  drop); a document that does not fit fails the build with a budget report.
- Downloads and their `download=""` attribute come from gd-link/gd-resolve; this crate
  only copies the assets pages list.
