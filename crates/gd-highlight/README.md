# gd-highlight cheat sheet

Syntax highlighting with Pygments' token classes (`k`, `nf`, `s2`, `c1`) and stylesheets,
for 35 languages, with no Python runtime and no dependencies. Guidedog uses it for code
blocks in HTML sites and PDF books (the `guidedoc` adapter lives in `guidedoc-defaults`).

## Use

```toml
[dependencies]
gd-highlight = "0.7"
```

We highlight code as HTML spans with Pygments' classes:

```rust
fn main() {
    let html = gd_highlight::highlight_html("def f(x):\n    return x\n", "python");
    assert!(html.starts_with("<span class=\"k\">def</span>"));
    println!("{html}");
}
```

The API documentation is on [docs.rs](https://docs.rs/gd-highlight). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

```rust
use gd_highlight::*;

// One call: language by alias, file name or MIME type; unknown languages are plain text.
let html = highlight_html("def f(x):\n    return x + 1\n", "python");
// <span class="k">def</span><span class="w"> </span><span class="nf">f</span>...

// Staged, reusing buffers.
let lexer = find_lexer("py").unwrap_or(Lexer::Text);   // None: see suggest_lexer/check_code
let lexer = resolve_lexer(lexer, code);                // Sphinx's choices (default, guess, >>>)
let mut tokens = Vec::new();
tokenize_into(lexer, code, &mut tokens);               // clears, then refills
let mut out = String::new();
write_html(&tokens, code, &mut out);                   // appends
for token in scan_tokens(lexer, code) { /* streaming, no buffer */ }
```

## Lexing

| Function | Does |
|---|---|
| `find_lexer(name) -> Option<Lexer>` | alias (any case), else file name (`"a/setup.py"`), else MIME type |
| `find_lexer_for_filename(path)` | the base name against the lexers' glob patterns |
| `resolve_lexer(lexer, code) -> Lexer` | `Default`: pycon if `>>>`, else Python, else Text when that has errors; `Python` with `>>>` is pycon; `Guess` guesses |
| `guess_lexer(code)` | clean JSON, else a scored rule table; `Text` when nothing matches |
| `scan_tokens(lexer, code) -> Scanner` | an `Iterator<Item = Token>`; resolves `Default`/`Guess` itself |
| `tokenize(lexer, code)`, `tokenize_into(.., &mut Vec)`, `count_tokens` | collected tokens |
| `has_errors(lexer, code)` | whether any token is `Error` (Sphinx's lexing warning) |
| `lexer_name`, `lexer_alias`, `lexer_info` | `"Python"`, `"python"`, `&LexerInfo{name, aliases, filenames, mimetypes}` |
| `suggest_lexer(name) -> Option<&str>` | the closest alias, for "Did you mean" |
| `Lexer::ALL` | every lexer |

`Token { start, end: u32, kind: TokenKind }` is a byte run of the input. Tokens are
contiguous, cover every byte exactly once, never split a UTF-8 character, and never put two
neighbours of one kind side by side. Unknown input never fails: it becomes `Text` or
`Error` tokens, as in Pygments. `token_text(t, code)` is its text. Inputs over 4 GiB are cut.

Lexers: Text, Python, Pycon, Pytb, C, Cpp, Rust, Go, Java, Kotlin, Csharp, Swift, Zig,
Odin, Javascript, Typescript, Json, Yaml, Toml, Ini, Xml, Html, Css, Bash, Console,
Powershell, Sql, Diff, Makefile, Dockerfile, Rst, Markdown, Latex, Typst, Dot, plus the
pseudo lexers Default and Guess. Aliases and patterns are Pygments' (see `lexers.rs`).
Code in reST `code-block`s, Markdown fences, Typst raw blocks, session commands, Makefile
recipes, Dockerfiles and HTML `<script>`/`<style>` is lexed as its own language.

## Writing

All writers append to a `&mut String` the caller owns and can `clear()` and reuse.

| Function | Writes |
|---|---|
| `highlight_html(code, language) -> String` | one call: find, resolve, lex, write |
| `write_html(&tokens, code, out)` | Pygments' HtmlFormatter spans (inside `<div class="highlight"><pre>`) |
| `write_code_html(lexer, code, out)` | the same, lexing as it writes |
| `write_html_lines(&tokens, code, HtmlLines{first, emphasized}, out)`, `write_code_html_lines` | Sphinx's `:linenos:` and `:emphasize-lines:` |
| `highlight_typst(code, language)`, `write_typst(&tokens, code, out)`, `write_code_typst(lexer, code, out)` | a Typst array of lines of `(class, text)` pairs |
| `write_css(style, ".highlight", out)`, `style_css(style, selector)` | Pygments' `get_style_defs`, line for line |
| `write_typst_theme(style, out)` | a Typst dictionary from class to text settings |
| `push_html(out, text)`, `push_typst_string(out, text)` | the escapers |

HTML follows Pygments: `Text` has no span, neighbours of one class share a span, no span
crosses a newline, `& < > " '` are escaped. Numbered lines start with
`<span class="linenos"> 9</span>`; an emphasized line is wrapped in
`<span class="hll">...\n</span>`. Unlike Pygments, output ends where the code ends.

Typst output is data, never markup: `((("k", "def"), ("w", " "), ), (("", "x"), ), )`;
control characters become `\u{..}` escapes, and a final newline starts no line. The theme
is `("background": rgb("#f8f8f8"), "text": (fill: none), "k": (fill: rgb("#008000"),
weight: "bold"), ...)`.

## Tokens and styles

`kind_class(k)` (`"s2"`, `""` for Text, `"p p-Indicator"` for subtypes), `kind_name(k)`
(`"Literal.String.Double"`), `kind_parent(k) -> Option`, `kind_is(k, TokenKind::String)`,
`TokenKind::ALL`. Kinds after `GenericTraceback` are the YAML lexer's subtypes and the
types only some Pygments styles name (github-dark, gruvbox, lilypond).

`Style` has every Pygments 2.19.2 style plus Sphinx's `Sphinx` and `None`; `Style::ALL`
lists them. `find_style("github-dark") -> Option<Style>` (case and spaces ignored),
`style_name(style)`, `style_info(style) -> &StyleInfo{name, dark, background, rules, ..}`,
`resolve_style(style) -> StyleLooks` (`.look(kind) -> Look{color, background, border:
Option<Color>, bold, italic, underline}`), `kind_look(style, kind)`.

## Problems

`check_code(language, code) -> Option<Problem>`: an unknown language (with a suggestion)
or code that does not lex (line, column, excerpt and the text); `"default"` never warns.
`check_style(name) -> Option<Problem>`. `write_problem(&p, "index.rst:12", out)` writes the
Elm-style report (title, explanation, excerpt with carets, hint); `Problem` also implements
`Display` (the report without a location) and `std::error::Error`. It borrows the name and
code it describes.

## Design notes

- Writers append to `&mut String` sinks; tokens go into a `Vec<Token>` (`tokenize_into`
  reuses one) or come from a `Scanner` iterator.
- What may be absent is an `Option` (`find_lexer`, `find_style`, `suggest_lexer`,
  `check_code`, `kind_parent`, internal searches); a style rule's colour is
  `Option<Color>`.
- Emphasized lines are a slice of line numbers, not a callback.
- Input is `&str`, so invalid UTF-8 cannot reach the lexers; token ends are moved forward to
  character boundaries, so no token splits a character.
- Lexer state is a fixed-size `State` with a 24-frame stack (copied in and out of
  each step); the last significant token of each running lexer lives in the scanner.
- The token, lexer and style tables are static arrays indexed by the enums, with tests
  holding their order to the enums'.
- Scanning does not allocate; writers append to a `String`. The linear-work probe counts
  work in test builds only.

## Why not syntect?

Evaluated on 2026-10-14 against `syntect` 5.3 (the main Rust highlighter):

- **Compatibility.** Guidedog follows Sphinx: themes and `pygments_style` expect Pygments
  token classes (`k`, `nf`, `s2`, ...) and the 51 Pygments styles. syntect emits TextMate
  scopes; mapping scopes back to Pygments tokens is lossy and grammar-dependent.
- **Self-containment.** syntect needs Oniguruma (C) or `fancy-regex`, plus `bincode`,
  `plist`, `yaml-rust`, `flate2`, `serde_json` and megabytes of serialized grammars.
  `gd-highlight` is std-only.
- **Bounded work.** Our scanner keeps an explicit 24-frame state stack and linear scanning;
  backtracking regex grammars give weaker guarantees on hostile input.

Decision: keep `gd-highlight`. Add lexers here when a language is needed.
