//! The analysis and the reST it writes, over packages held in memory.

use gd_odindoc::{
    Config, Decl, DirectiveOption, Import, Kind, MemoryReader, Output, Package, Private, Project,
    Request, Root, document,
};

const POINT: &str = "package geo

import \"core:fmt\"
import m \"../mathx\"

// Point is a place.
Point :: struct($T: typeid) #packed {
\t// Horizontal.
\tx: T, // metres
\tusing base: Base,
\ty, z: T `json:\"y\"`,
}

Base :: struct {}

// Colour of a shape.
Colour :: enum u8 {
\t// The first.
\tRed = 1,
\tGreen, // grass
}

Flags :: bit_set[Colour; u8]

Id :: distinct int
Alias :: m.Vec

Reg :: bit_field u32 {
\tlow: u8 | 4, // low bits
}

// Area of a shape.
@(require_results)
area :: #force_inline proc \"contextless\" (
\tw, h: f32, // sides
\t$N: int, rest: ..int,
) -> (a: f32, ok: bool) where N > 0 {
\treturn w * h, true
}

@(private)
hidden :: proc() {}

lookup :: proc(k: string) -> (int, bool) #optional_ok {
\treturn 0, false
}

measure :: proc{area, hidden}

MAX :: 64
LIMIT : int : 10
counter: int
name := \"geo\"

foreign import libc \"system:c\"

foreign libc {
\t// Puts a string.
\tputs :: proc \"c\" (s: cstring) -> i32 ---
}
";

fn fixture() -> Vec<(&'static str, &'static str)> {
    vec![
        ("/src/geo/doc.odin", "// Package geo measures shapes.\n// It is small.\npackage geo\n"),
        ("/src/geo/geo.odin", POINT),
        ("/src/mathx/mathx.odin", "package mathx\n\nVec :: [2]f32\n"),
    ]
}

fn project_of(files: Vec<(&str, &str)>) -> Project {
    let roots = vec![Root { dir: "/src".into(), ..Root::default() }];
    Project::new(roots, Box::new(MemoryReader::new(files)))
}

fn find<'p>(pkg: &'p Package, name: &str, variant: usize) -> Option<&'p Decl> {
    pkg.decls.iter().find(|d| d.name == name && d.variant == variant)
}

fn decl<'p>(pkg: &'p Package, name: &str) -> &'p Decl {
    find(pkg, name, 0).unwrap_or_else(|| panic!("no declaration {name}"))
}

fn request(directive: &str, argument: &str, options: &[(&str, &str)]) -> Request {
    Request {
        directive: directive.into(),
        argument: argument.into(),
        options: options.iter().map(|(n, v)| DirectiveOption::new(n, v)).collect(),
        ..Request::default()
    }
}

fn text(output: &Output) -> String {
    output.lines.join("\n")
}

#[test]
fn declarations_have_their_kinds_signatures_and_docs() {
    let mut p = project_of(fixture());
    let pkg = p.load_package("geo").expect("geo loads");
    assert_eq!(pkg.name, "geo");
    assert_eq!(pkg.synopsis, "Package geo measures shapes.");
    assert_eq!(pkg.platform, "");
    let fmt = Import { alias: "fmt".into(), id: "core:fmt".into() };
    let m = Import { alias: "m".into(), id: "mathx".into() };
    assert_eq!(pkg.imports, [fmt, m]);
    let point = decl(pkg, "Point");
    assert_eq!(point.kind, Kind::Struct);
    assert_eq!(point.signature, "Point :: struct($T: typeid) #packed");
    assert_eq!(point.docs, "Point is a place.");
    assert_eq!(point.members.len(), 4);
    assert_eq!(point.members[0].signature, "x: T");
    assert_eq!(point.members[0].docs, "Horizontal.\n\nmetres");
    assert_eq!(point.members[1].signature, "using base: Base");
    assert_eq!(point.members[3].signature, "z: T `json:\"y\"`");
    let colour = decl(pkg, "Colour");
    assert_eq!(colour.signature, "Colour :: enum u8");
    assert_eq!(colour.members[0].signature, "Red = 1");
    assert_eq!(colour.members[0].docs, "The first.");
    assert_eq!(colour.members[1].docs, "grass");
    assert_eq!(decl(pkg, "Flags").kind, Kind::BitSet);
    assert_eq!(decl(pkg, "Id").signature, "Id :: distinct int");
    assert_eq!(decl(pkg, "Alias").kind, Kind::Type);
    assert_eq!(decl(pkg, "Reg").members[0].signature, "low: u8 | 4");
    let area = decl(pkg, "area");
    assert_eq!(area.kind, Kind::Proc);
    assert_eq!(
        area.signature,
        "area :: #force_inline proc \"contextless\" (w, h: f32, $N: int, rest: ..int) -> \
         (a: f32, ok: bool) where N > 0"
    );
    assert_eq!(area.attributes, ["@(require_results)"]);
    assert_eq!(decl(pkg, "hidden").private, Private::Package);
    assert_eq!(
        decl(pkg, "lookup").signature,
        "lookup :: proc(k: string) -> (int, bool) #optional_ok"
    );
    assert_eq!(decl(pkg, "measure").signature, "measure :: proc{area, hidden}");
    assert_eq!(decl(pkg, "MAX").kind, Kind::Const);
    assert_eq!(decl(pkg, "LIMIT").signature, "LIMIT : int : 10");
    assert_eq!(decl(pkg, "counter").signature, "counter: int");
    assert_eq!(decl(pkg, "name").signature, "name := \"geo\"");
    assert_eq!(decl(pkg, "libc").signature, "libc \"system:c\"");
    let puts = decl(pkg, "puts");
    assert_eq!(puts.foreign_lib, "libc");
    assert_eq!(puts.signature, "puts :: proc \"c\" (s: cstring) -> i32");
}

#[test]
fn platforms_merge_and_say_where_a_declaration_exists() {
    let mut p = project_of(vec![
        ("/src/os2/os2.odin", "package os2\n\nopen :: proc(path: string) {}\n"),
        (
            "/src/os2/file_windows.odin",
            "package os2\n\nHandle :: distinct rawptr\nflush :: proc() {}\n",
        ),
        (
            "/src/os2/file_linux.odin",
            "package os2\n\nHandle :: distinct i32\nflush :: proc() {}\n",
        ),
        ("/src/os2/file_darwin_arm64.odin", "package os2\n\nflush :: proc() {}\n"),
        ("/src/os2/file_haiku.odin", "package os2\n\nhaiku_only :: proc() {}\n"),
        ("/src/os2/tagged.odin", "#+build !windows\npackage os2\n\nposix :: proc() {}\n"),
        ("/src/os2/legacy.odin", "//+build linux\npackage os2\n\nold :: proc() {}\n"),
        (
            "/src/os2/when.odin",
            "package os2\n\nwhen ODIN_OS == .Windows {\n\twin :: 1\n} else {\n\tother :: 2\n}\n\
             when ODIN_ARCH == .amd64 && !(ODIN_OS == .Linux) {\n\tx86 :: 3\n}\n\
             when SOME_FLAG {\n\tunknown :: 4\n}\n",
        ),
    ]);
    let pkg = p.load_package("os2").expect("os2 loads");
    let availability =
        |name: &str, variant: usize| find(pkg, name, variant).map(|d| d.availability.as_str());
    assert_eq!(availability("open", 0), Some(""));
    assert_eq!(availability("haiku_only", 0), None);
    assert_eq!(availability("Handle", 0), Some("Linux"));
    assert_eq!(availability("Handle", 1), Some("Windows"));
    assert_eq!(availability("flush", 0), Some("Windows, Darwin (arm64), Linux"));
    assert_eq!(
        availability("posix", 0),
        Some("Darwin, Linux, FreeBSD, OpenBSD, NetBSD, WASI, JS, Orca, Freestanding")
    );
    assert_eq!(availability("old", 0), Some("Linux"));
    assert_eq!(availability("win", 0), Some("Windows"));
    assert_eq!(availability("unknown", 0), Some(""));
    assert_eq!(
        availability("x86", 0),
        Some(
            "Windows (amd64), Darwin (amd64), FreeBSD (amd64), OpenBSD (amd64), \
             NetBSD (amd64), Freestanding (amd64)"
        )
    );
    assert_eq!(decl(pkg, "other").variant, 0);
}

#[test]
fn autopackage_writes_the_domain_directives() {
    let mut p = project_of(fixture());
    let c = Config::default();
    let r = document(&mut p, &c, &request("odin:autopackage", "geo", &[("members", "")]));
    assert!(r.messages.is_empty(), "{:?}", r.messages);
    let t = text(&r);
    let want = ".. odin:package:: geo\n   :synopsis: Package geo measures shapes.\n\
        \x20  :imports: fmt=core:fmt m=mathx\n   :private: hidden\n\n\
        Package geo measures shapes.\nIt is small.\n\n\
        .. odin:struct:: Point :: struct($T: typeid) #packed\n\n   Point is a place.\n\n\
        \x20  .. odin:field:: x: T\n\n      Horizontal.\n\n      metres\n\n";
    assert!(t.starts_with(want), "{t}");
    assert!(!t.contains("hidden ::"));
    assert!(!t.contains("odin:bitset"));
    assert!(t.contains(".. odin:procedure:: @(require_results) area ::"));
    assert!(t.contains("   :foreign: libc\n"));
    let options = [
        ("members", ""),
        ("undoc-members", ""),
        ("private-members", ""),
        ("member-order", "groupwise"),
        ("exclude-members", "MAX"),
    ];
    let t = text(&document(&mut p, &c, &request("odin:autopackage", "geo", &options)));
    assert!(t.contains(".. odin:procedure:: @(private) hidden :: proc()"));
    assert!(t.contains(".. rubric:: Types"));
    assert!(!t.contains("MAX ::"));
    let mut proc_request = request("odin:autoproc", "area", &[]);
    proc_request.current = "geo".into();
    let r = document(&mut p, &c, &proc_request);
    assert_eq!(
        r.lines[0],
        ".. odin:procedure:: @(require_results) area :: #force_inline proc \"contextless\" \
         (w, h: f32, $N: int, rest: ..int) -> (a: f32, ok: bool) where N > 0"
    );
    let r = document(&mut p, &c, &request("odin:autotype", "geo.Colour", &[("members", "")]));
    assert_eq!(r.lines[1], "   :package: geo");
    assert!(text(&r).contains("   .. odin:enumerator:: Green\n\n      grass"));
    let code = |r: Output| r.messages.first().map(|m| m.code.clone()).unwrap_or_default();
    let r = document(&mut p, &c, &request("odin:autoproc", "geo.nothing", &[]));
    assert_eq!(code(r), "odin.autodoc.object");
    let r = document(&mut p, &c, &request("odin:autopackage", "nowhere", &[]));
    assert_eq!(code(r), "odin.autodoc.package");
    let r = document(&mut p, &c, &request("odin:autopackage", "geo", &[("bogus", "")]));
    assert_eq!(code(r), "odin.autodoc.option");
    let r = document(&mut p, &c, &request("odin:autofoo", "geo", &[]));
    assert_eq!(code(r), "odin.autodoc.directive");
    let r = document(&mut p, &c, &request("odin:autopackage", "geo", &[("member-order", "x")]));
    assert_eq!(code(r), "odin.autodoc.order");
    let r = document(&mut p, &c, &request("odin:autotype", "geo.area", &[]));
    assert_eq!(code(r), "odin.autodoc.kind");
}

/// A public declaration may name a private type, which has no description: the
/// package's directive lists it, so the domain shows it as text, and not when private
/// members are documented and it has one.
#[test]
fn autopackage_lists_private_types() {
    let src = "package box

// Box holds its state.
Box :: struct {
\tstate: State,
}

@(private)
State :: struct {
\tn: int,
}

@(private)
Mode :: enum u8 {On, Off}

@(private)
helper :: proc() {}

@(private)
show_box :: proc(b: Box) {}

show :: proc{show_box}
";
    let mut p = project_of(vec![("/src/box/box.odin", src)]);
    let c = Config::default();
    let t = text(&document(&mut p, &c, &request("odin:autopackage", "box", &[("members", "")])));
    assert!(t.starts_with(".. odin:package:: box\n   :private: Mode State show_box\n"), "{t}");
    let options = [("members", ""), ("private-members", "")];
    let t = text(&document(&mut p, &c, &request("odin:autopackage", "box", &options)));
    assert!(!t.contains(":private:"));
}

/// A constant declaration the parser kept without its value (a syntax error, or more
/// names than values) is documented by name.
#[test]
fn constants_without_values_are_documented_by_name() {
    let mut p =
        project_of(vec![("/src/geo/geo.odin", "package geo\n\nA, B :: 1\nD :: proc() {}\n")]);
    let options = [("members", ""), ("undoc-members", "")];
    let r = document(&mut p, &Config::default(), &request("odin:autopackage", "geo", &options));
    let t = text(&r);
    assert!(t.contains("odin:procedure:: D :: proc()"), "{t}");
    assert!(t.contains("odin:const:: B"), "{t}");
    assert!(r.messages.iter().any(|m| m.code == "odin.autodoc.syntax"));
}

#[test]
fn long_when_conditions_are_not_followed_to_the_end() {
    let cond = format!("ODIN_OS == .Linux{}", " || ODIN_OS == .Linux".repeat(100_000));
    let p_text = format!(
        "package p\n\nwhen {cond} {{\n// Only on Linux.\nf :: proc() {{}}\n}}\n\n\
         // Everywhere.\ng :: proc() {{}}\n"
    );
    let deep = format!("package p\n\nX :: {}1{}\n", "(".repeat(100_000), ")".repeat(100_000));
    let mut p = project_of(vec![("/src/p/p.odin", &p_text), ("/src/p/deep.odin", &deep)]);
    let r = document(
        &mut p,
        &Config::default(),
        &request("odin:autopackage", "p", &[("members", "")]),
    );
    let t = text(&r);
    assert!(t.contains("odin:procedure:: g"));
    assert!(t.contains("odin:procedure:: f"));
    assert!(r.messages.iter().any(|m| m.code == "odin.autodoc.nesting"));
}

/// A file over the size limit is not parsed, and the package says so.
#[test]
fn files_over_the_limit_are_left_out() {
    let mut p = project_of(vec![("/src/p/a.odin", "package p\n// A is a.\nA :: 1\n")]);
    p.limits.max_file_bytes = 8;
    let pkg = p.load_package("p").expect("p loads");
    assert_eq!(pkg.too_large.len(), 1);
    assert!(pkg.decls.is_empty());
    let r = document(&mut p, &Config::default(), &request("odin:autopackage", "p", &[]));
    assert!(r.messages.iter().any(|m| m.code == "odin.autodoc.memory"));
}

#[test]
fn packages_are_listed_by_id() {
    let mut p = project_of(fixture());
    let ids: Vec<String> = p.list_packages().into_iter().map(|r| r.id).collect();
    assert_eq!(ids, ["geo", "mathx"]);
    let mut core = Project::new(
        vec![Root { dir: "/odin/core".into(), collection: "core".into() }],
        Box::new(MemoryReader::new([("/odin/core/fmt/fmt.odin", "package fmt\n")])),
    );
    let refs = core.list_packages();
    assert_eq!(refs[0].id, "core:fmt");
    assert!(core.load_package("core:fmt").is_some());
    assert!(core.load_package("fmt").is_none());
}
