# gd-theme

Guidedog's built-in theme: `layout.html`, `book.typ`, `guidedog.css` and `guidedog.js`,
embedded at compile time. `guidedog quickstart` copies them into a new project, and the
HTML and PDF builders use them when a project has no copy of its own.

Standalone discussion records use the embedded RFC-style `discussion.typ` layout. `guidedog dsc init` copies it into `_templates/discussion.typ` for customization, independently of `book.typ`. Existing catalogs without a copy use the embedded layout.

```rust
let css = gd_theme::GUIDEDOG_CSS;
for file in gd_theme::FILES {
    println!("{} ({} bytes)", file.path, file.text.len());
}
assert_eq!(gd_theme::builtin("_templates/layout.html"), Some(gd_theme::LAYOUT_HTML));
```
