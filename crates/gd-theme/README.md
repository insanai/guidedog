# gd-theme

Guidedog's built-in theme: `layout.html`, `theme.typ`, `book.typ`, `discussion.typ`, `guidedog.css` and `guidedog.js`,
embedded at compile time. `guidedog quickstart` copies them into a new project, and the
HTML and PDF builders use them when a project has no copy of its own.

Standalone discussion records use the embedded RFC-style `discussion.typ` layout. `guidedog dsc init` copies it and the shared `theme.typ` into `_templates/` for customization, independently of `book.typ`. Existing catalogs without a copy use the embedded layout.

```rust
let css = gd_theme::GUIDEDOG_CSS;
for file in gd_theme::FILES {
    println!("{} ({} bytes)", file.path, file.text.len());
}
assert_eq!(gd_theme::builtin("_templates/layout.html"), Some(gd_theme::LAYOUT_HTML));
```

The shared PDF theme defines Guidedog colors, fonts and a language-aware content wrapper. The book and discussion layouts import it with ordinary Typst imports. Built-in fallback layouts inline the shared module, so they need no project files and never run project code in an untrusted build.

`pack` defines bounded theme archives, canonical tree hashes and offline installation verification.
Archives contain base64 file bytes beneath `_templates/` and `_static/`, without extraction hooks or platform-dependent archive metadata.
The `gd-ssg` command owns downloads and atomic lock selection; the configuration loader verifies the selected tree before adding its template and static directories.
