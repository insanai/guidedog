# gd-theme

Guidedog's built-in theme: `layout.html` (plus `article.html` and `landing.html`), `theme.typ`, `book.typ`, `discussion.typ`, `article.typ`, `guidedog.css`, `guidedog.js`, and `typography.js`,
embedded at compile time. `guidedog quickstart` and `guidedog ssg init` copy the eight `FILES` into a new project, and the
HTML and PDF builders use them when a project has no copy of its own.

Standalone discussion records use the embedded RFC-style `discussion.typ` layout. `guidedog dsc init` copies `layout.html`, `discussion.typ`, and `theme.typ` into `_templates/` and `guidedog.css`, `guidedog.js`, and `typography.js` into `_static/` for customization, independently of `book.typ`. Existing catalogs without a copy use the embedded files.

```rust
let css = gd_theme::GUIDEDOG_CSS;
for file in gd_theme::FILES {
    println!("{} ({} bytes)", file.path, file.text.len());
}
assert_eq!(gd_theme::builtin("_templates/layout.html"), Some(gd_theme::LAYOUT_HTML));
```

The shared PDF theme defines Guidedog colors, fonts and a language-aware content wrapper. The book, article and discussion layouts import it with ordinary Typst imports and declare their horizontal page margins in a leading `// guidedog: margin-x = ...` header comment for PDF music width calculation (`guidedog.css` also styles responsive `lilypond` figures). Built-in fallback layouts inline the shared module, so they need no project files and never run project code in an untrusted build.

`pack` defines bounded theme archives, canonical tree hashes and offline installation verification.
Archives contain base64 file bytes beneath `_templates/` and `_static/`, without extraction hooks or platform-dependent archive metadata.
The `gd-ssg` command owns downloads and atomic lock selection; the configuration loader verifies the selected tree before adding its template and static directories.
