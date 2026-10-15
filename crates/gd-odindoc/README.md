# gd-odindoc cheat sheet

`gd-odindoc` reads Odin packages by static analysis, across every build target at once,
and writes the reStructuredText of Guidedog's `odin` domain for them, as
`sphinx.ext.autodoc` does for Python. It never runs the compiler. Guidedog's `odin:auto*`
directives run through it; the spec is `docs/gds/records/0005-odin-domain.rst`.

## Use

```toml
[dependencies]
gd-odindoc = "0.7"
```

We document an Odin package found under `lib/`:

```rust
use gd_odindoc::{Config, DirectiveOption, Project, Request, Root, document};

fn main() {
    let mut project = Project::from_dirs(vec![Root { dir: "lib".into(), ..Root::default() }]);
    let request = Request {
        directive: "odin:autopackage".into(),
        argument: "gettext".into(),
        options: vec![DirectiveOption::flag("members")],
        ..Request::default()
    };
    let output = document(&mut project, &Config::default(), &request);
    for message in &output.messages {
        eprintln!("{message}");
    }
    for line in &output.lines {
        println!("{line}");
    }
}
```

The API documentation is on [docs.rs](https://docs.rs/gd-odindoc). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

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

## Design

- **Own parser.** The crate needs no Odin compiler. It has a port of the tokenizer of
  Odin's `core:odin` library and a declaration-level parser that reads every file-scope
  statement, expression and type as `core:odin/parser` does (same node spans, same
  comment grouping, so signatures and docs match the compiler's view byte for byte) but
  skips procedure bodies. Expressions are flat nodes in a `Vec` indexed by `NodeId`, not
  a pointer tree.
- **Nesting limit, not a stack estimate.** The parser counts its recursion and gives a
  file up past `MAX_NESTING` (160 levels), reporting `odin.autodoc.nesting`. Long flat
  chains (`a + b + ...`, `x || y || ...`) and anything inside procedure bodies never
  count, so deep `else if` chains in bodies are read. `when` conditions are followed only
  `MAX_WHEN_DEPTH` (64) deep.
- **`"""` strings.** The tokenizer reads multi-line `"""` strings, which the `core:odin`
  tokenizer predates; files using them (core's `cpp-gen.odin`) are documented instead of
  refused.
- **Owned memory.** Owned `String`s and `Vec`s hold every file and node; the parse-memory
  bound is `Limits::max_file_bytes` (message `odin.autodoc.memory`).
- **Traits and enums for callbacks and strings.** `FileReader` reads the source files;
  `MemberOrder` is the `member_order` setting (directive options are still parsed from
  strings); `Targets` implements `!` for "every other supported target".
- **Syntax errors.** Errors inside skipped procedure bodies are not counted, so
  `odin.autodoc.syntax` covers only declarations.

To compare runs over a large tree: `cargo run --release -p gd-odindoc --example
odindoc_dump -- DIR [COLLECTION] OUT`. Over Odin's `core`, `base` and `vendor` (about
560,000 lines of reST) it documents every package.
