# gd-report cheat sheet

Elm-style diagnostic reports shared by every crate. Standard library only; it renders
and never prints. One crate owns the report shape, so the host, the readers, graphdog and
the template engine all print errors the same way.

## Use

```toml
[dependencies]
gd-report = "0.7"
```

We build a report and render it for a terminal:

```rust
use gd_report::{Report, Style, render_text};

fn main() {
    let source = "[site]\ntitle = 1\n";
    let start = source.find('1').unwrap_or(0);
    let report = Report::error("conf.type", "WRONG TYPE", "`title` must be a string.")
        .hint("Quote the value: `title = \"1\"`.")
        .at("conf.toml", source, start, start + 1);
    let text = render_text(&report, &Style { color: false, width: 80 });
    assert!(text.contains("Hint: Quote the value"));
    print!("{text}");
}
```

The API documentation is on [docs.rs](https://docs.rs/gd-report). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

```rust
let report = gd_report::Report::error("toml.duplicate", "DUPLICATE KEY", "`a` is set twice.")
    .hint("Remove one of them.")
    .at("conf.toml", source, start, end);           // line, column, excerpt, carets
eprint!("{}", gd_report::render_text(&report, &gd_report::Style { color: true, width: 80 }));
println!("{}", gd_report::render_json(&report));     // one JSON object (SCHEMA 1)
```

| Item | Does |
|---|---|
| `Report { code, severity, title, message, hints, path, line, column, end_line, end_column, excerpt, category }` | plain data, `Default` |
| `Report::error`/`warning`/`note(code, title, message)`, `.hint(h)`, `.at(path, source, start, end)` | chaining builders |
| `set_span(&mut r, source, start, end)`, `set_line_column(&mut r, source, line, col)` | position from bytes or line/column |
| `render_text(&r, &Style)` / `write_text(&mut String, ..)` | `-- TITLE [code] ---- path:line:col`, message, `N| line`, carets, `Hint:` |
| `render_json(&r)` / `write_json` / `escape_json` | JSON form for tools |
| `line_column`, `line_bounds`, `line_offset`, `floor_boundary` | positions; columns count Unicode scalars |
| `char_width`, `display_width`, `is_wide`, `is_zero_width`, `is_control` | terminal cells (Unicode 15.1 East Asian Width) |
| `escape_terminal(s) -> Cow<str>` | escapes control characters except `\n`; borrows when clean |
| `wrap` / `write_wrapped`, `suggest(name, candidates)`, `edit_distance` | prose wrapping and "Did you mean" |

Design: `Excerpt` is an `Option` holding the source line and byte span rather than
pre-rendered excerpt and marker strings, so the terminal window is computed at render
time; a report may carry several hints; template filling (`{0}`) stays in the engine.
