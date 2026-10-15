# gd-autodoc cheat sheet

Generates the reStructuredText Sphinx 9.1's `sphinx.ext.autodoc` generates, from Python
source that `gd-pyscan` analyses instead of imported modules: the same `py:` directives,
docstrings prepared as Sphinx prepares them, members chosen, filtered and ordered as the
options say. What static analysis cannot know (compiled modules, objects made at run
time, members of classes outside the source roots) is a `Message` naming the object.

## Use

```toml
[dependencies]
gd-autodoc = "0.7"
gd-pyscan = "0.7"
```

We generate the `autoclass` output for a class:

```rust
use gd_autodoc::{Config, DirectiveOption, Request, Session, document};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join("gd-autodoc-example");
    std::fs::create_dir_all(&root)?;
    let source = "class Square:\n    \"\"\"A square.\"\"\"\n\n    \
                  def area(self):\n        \"\"\"The area.\"\"\"\n";
    std::fs::write(root.join("shapes.py"), source)?;

    let mut session = Session::new(gd_pyscan::Project::new(&[&root])?);
    let request = Request {
        directive: "autoclass".into(),
        argument: "shapes.Square".into(),
        options: vec![DirectiveOption::flag("members")],
        ..Request::default()
    };
    let output = document(&mut session, &Config::default(), &request);
    for message in &output.messages {
        eprintln!("{message}");
    }
    println!("{}", output.lines.join("\n"));
    Ok(())
}
```

The API documentation is on [docs.rs](https://docs.rs/gd-autodoc). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

```rust
use gd_autodoc::{Config, DirectiveOption, MemberOrder, Request, Session, document};

let mut session = Session::new(gd_pyscan::Project::new(&["src"])?);
let config = Config { autodoc_member_order: MemberOrder::Bysource, ..Config::default() };
let request = Request {
    directive: "autoclass".into(),
    argument: "mypackage.core.Parser".into(),
    options: vec![DirectiveOption::flag("members")],
    ..Request::default()
};
let out = document(&mut session, &config, &request);
for m in &out.messages { eprintln!("{m}"); }           // Elm-style: title, text, hint
println!("{}", out.lines.join("\n"));                 // ".. py:class:: Parser(...)", ...
```

| Item | Does |
|---|---|
| `Session::new(project)`, `project()`, `into_project()` | one build's state; owns the project |
| `document(&mut session, &config, &request) -> Output` | the reST of one directive: `automodule`, `autoclass`, `autoexception`, `autofunction`, `automethod`, `autoattribute`, `autodata`, `autoproperty`, `autodecorator`, `autotype` |
| `Config` (`Default` = Sphinx's defaults) | `autoclass_content`, `autodoc_*`, `tab_width`, `typehint_fields` |
| `ClassContent`, `ClassSignature`, `MemberOrder`, `Typehints`, `DescriptionTarget`, `TypehintsFormat` | the string settings as enums; `parse("bysource")` reads Sphinx's names |
| `Request`, `DirectiveOption::{flag, new}`, `DefaultOption`, `TypeAlias` | inputs |
| `Output { lines, messages }`, `Output::has_message(code)` | results |
| `Message { severity, code, title, text, hint }` (`Display`, `Error`) | Elm-style diagnostics |
| `ALL_OPTIONS` | every option an autodoc directive may take, for a reader's table |
| `Config::napoleon: Option<Napoleon>` | sphinx.ext.napoleon: docstrings converted before they are emitted |
| `napoleon_docstring(lines, &Napoleon, &NapoleonContext) -> NapoleonOutput`, `convert_docstring(style, ..)` | the converter alone: `lines`, `origins` (output line to docstring line), `notes` |
| `summarize(&mut session, &config, &SummaryRequest) -> Summary`, `summary_table(items, classes)` | sphinx.ext.autosummary's table rows and their `list-table` |
| `stub(&mut session, &config, name, StubOptions) -> Result<Stub, Message>`, `module_docstring` | a stub page's template variables |
| `module_objects`, `package_modules` | what sphinx.ext.coverage inspects in a module and a package |

Codes: `napoleon.section`, `.field`, `.type`; `autosummary.import`, `.import_cycle`; `autodoc.directive`, `.option`, `.signature`, `.module`, `.import`, `.native`,
`.too_deep`, `.outside`, `.kind`, `.all`, `.member`, `.members`, `.docstring`, `.value`,
`.default`, `.decorator`, `.limit`.

Limits: `MAX_EVAL_STEPS` (10,000 nodes per annotation), `MAX_DESCRIBE_STEPS` (10,000
nodes per value), member nesting 24, `MAX_GENERATED` (10,000 directives, then warning
`autodoc.limit`).

## Design notes

- `Session` owns the `Project`, and `document` returns an owned `Output` (named so it
  does not clash with `std::result::Result`).
- Settings Sphinx takes as strings (`autodoc_member_order`, `autodoc_typehints`, ...) are
  enums, so unknown values are rejected by the host when it reads `conf.toml`.
- Evaluated annotations are a `TypeValue` enum with data per variant; the docstring
  "None vs empty" distinction is `Option<Vec<Vec<String>>>`; `__doc__` lookups return
  `Result<Option<String>, Unknown>`; signature sources are an enum (`Function` or
  `Fixed`); annotation sources are `Node` or `Text`.
- Values that were `value` + `has_value` pairs (`value_repr`, `property_type`,
  `type_annotation`, `__all__`, recorded hints, `:annotation:`) are `Option`s.
- `autodoc_type_aliases` and `autodoc_mock_imports` are carried in `Config` but not yet
  used.
- napoleon is a pure function from docstring lines and settings to lines, with a map
  from each output line to the docstring line it came from. Where it cannot read a
  line (`napoleon.*` notes), the lines before are converted and the rest kept as
  written, rather than garbled. Sphinx's 148 napoleon test conversions are compared
  line for line (`tests/napoleon.rs`).
- autosummary's `extract_summary` decides where the first sentence ends without a
  reStructuredText parser: headings, non-paragraph blocks and inline markup left open
  are recognised from the text.
