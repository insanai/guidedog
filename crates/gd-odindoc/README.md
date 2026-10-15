# gd-odindoc cheat sheet

`gd-odindoc` reads Odin packages by static analysis, across every build target at once,
and writes the reStructuredText of Guidedog's `odin` domain for them, as
`sphinx.ext.autodoc` does for Python. It never runs the compiler. Guidedog's `odin:auto*`
directives run through it; the spec is `docs/gds/records/0005-odin-domain.rst`.

```rust
use gd_odindoc::{Config, DirectiveOption, MemberOrder, Project, Request, Root, document};

let mut project = Project::from_dirs(vec![Root { dir: "lib".into(), ..Root::default() }]);
for package in project.list_packages() {
    println!("{} {}", package.id, package.dir);
}
let config = Config { member_order: MemberOrder::Groupwise, ..Config::default() };
let request = Request {
    directive: "odin:autopackage".into(),
    argument: "gettext".into(),
    options: vec![DirectiveOption::flag("members")],
    ..Request::default()
};
let output = document(&mut project, &config, &request);
for message in &output.messages {
    eprintln!("{message}");
}
for line in &output.lines {
    println!("{line}");
}
```

| Item | Does |
|---|---|
| `Project::new(roots, Box<dyn FileReader>)` | a project over `Root { dir, collection }`s, reading through the reader |
| `Project::from_dirs(roots)` | the same over the file system (`FsReader`) |
| `project.list_packages() -> Vec<PackageRef>` | every package below the roots, by id |
| `project.load_package(id) -> Option<&Package>` | analyse a package once: its `Decl`s, imports, targets, doc |
| `document(&mut project, &config, &request) -> Output` | the reST of one directive: `lines`, and `messages` |
| `project.limits.max_file_bytes` | files above it are left out (`Package::too_large`) |
| `project.files` | every file read (`FileRead { path, text }`), for a host's dependencies |

Types: `Project` (`Send`), `Root`, `FileReader` (trait, `Send`), `FsReader`, `MemoryReader`, `FileRead`,
`Limits`, `PackageRef`, `Package`, `Import`, `Decl`, `Member`, `Kind`, `MemberKind`,
`Private`, `Targets` (`Os`, `Arch`), `Config`, `MemberOrder`, `DirectiveOption`,
`Request`, `Output`, `Message` (Elm style: code, title, text, hint), `Severity`.

Directives: `odin:autopackage`, `odin:autoproc`, `odin:autoprocgroup`, `odin:autotype`,
`odin:autoconst`, `odin:autovar`; options `:members:`, `:undoc-members:`,
`:private-members:`, `:exclude-members:`, `:member-order:`, `:no-index:`. Message codes:
`odin.autodoc.directive`, `.option`, `.order`, `.package`, `.object`, `.kind`, `.syntax`,
`.nesting`, `.memory`.

## Design (redesigned from the Odin version)

- **Own parser.** The Odin version used Odin's `core:odin/parser`. This crate has a port of
  Odin's tokenizer and a declaration-level parser that reads every file-scope statement,
  expression and type as Odin's parser does (same node spans, same comment grouping, so
  signatures and docs match byte for byte) but skips procedure bodies. Expressions are
  flat nodes in a `Vec` indexed by `NodeId`, not a pointer tree.
- **Nesting limit instead of a stack estimate.** The parser counts its recursion and gives
  a file up past `MAX_NESTING` (160 levels), reporting `odin.autodoc.nesting`. Odin's
  `PARSE_STACK_BYTES` estimate is gone; long flat chains (`a + b + ...`, `x || y || ...`)
  and anything inside procedure bodies never count, so files the old estimate refused
  for deep `else if` chains in bodies are now read. `when` conditions are still followed
  only `MAX_WHEN_DEPTH` (64) deep.
- **`"""` strings.** The tokenizer reads multi-line `"""` strings, which Odin's
  `core:odin` tokenizer predates; files using them (core's `cpp-gen.odin`) are documented
  instead of refused.
- **No arenas or allocation budgets.** Owned `String`s and `Vec`s replace the arena and
  `File_Reader.reserve`; the parse-memory bound is replaced by `Limits::max_file_bytes`
  (message `odin.autodoc.memory`).
- **Traits and enums for callbacks and strings.** `FileReader` replaces the procedure
  struct; `MemberOrder` replaces the `member_order` string (directive options are still
  parsed from strings); `Targets` implements `!` for "every other supported target".
- **Syntax errors.** Errors inside skipped procedure bodies are not counted, so
  `odin.autodoc.syntax` covers only declarations.

Checked against the Odin version: `cargo run --release -p gd-odindoc --example
odindoc_dump -- DIR [COLLECTION] OUT` over Guidedog's old `lib`, and Odin's `core`, `base`
and `vendor` (about 560,000 lines of reST) gives the same output, except the three
`cpp-gen.odin` files above.
