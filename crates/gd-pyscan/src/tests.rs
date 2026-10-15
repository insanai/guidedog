//! Unit tests: a small package written into a temporary root, and hostile sources that
//! must be refused or cut off, never overflow the stack or run for ever.

use std::fmt::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::resolve::MAX_STEPS;
use crate::*;

/// A temporary folder removed when dropped.
struct TempRoot(PathBuf);

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn project_of(files: &[(&str, &str)]) -> (Project, TempRoot) {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("pyscan-{}-{n}", std::process::id()));
    for (name, text) in files {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().expect("a file has a folder")).expect("mkdir");
        std::fs::write(&path, text).expect("write");
    }
    let p = Project::new(&[&dir]).expect("the grammar loads");
    (p, TempRoot(dir))
}

const PKG_INIT: &str = r#""""The package.

It re-exports.
"""
from .core import Thing as Thing
from . import helpers
from .helpers import *

__all__ = ["Thing", "helper"]
__all__ += ("VERSION",)

#: The version.
VERSION = "1.0"
"#;

const PKG_CORE: &str = r#"from __future__ import annotations

import typing as t

if t.TYPE_CHECKING:
    from .helpers import Helper

LIMIT = 10  #: the limit
"""Replaced by this docstring."""


class Base:
    """Base docstring."""

    #: Shared by all.
    shared: int = 1

    def inherited(self, x):
        """From the base."""


class Thing(Base, metaclass=Meta):
    """A thing.

    More text.
    """

    count: int
    "The count."

    def __init__(self, name: str, size=LIMIT, *args, flag: bool = False, **kw) -> None:
        self.name = name  #: The name.
        #: The size,
        #: in bytes.
        self.size = size
        self.hidden = 1

    @property
    def label(self) -> str:
        """The label."""
        return self.name

    @staticmethod
    def make(a, /, b):
        pass

    @classmethod
    async def create(cls, value="it's"):
        pass

    @t.overload
    def get(self, key: int) -> int: ...
    @t.overload
    def get(self, key: str) -> str: ...
    def get(self, key):
        pass
"#;

const PKG_HELPERS: &str = r#"def helper():
    '''Help \x41 and \\n.'''

class Helper:
    pass

_private = 1
"#;

fn package() -> (Project, TempRoot) {
    project_of(&[
        ("pkg/__init__.py", PKG_INIT),
        ("pkg/core.py", PKG_CORE),
        ("pkg/helpers.py", PKG_HELPERS),
    ])
}

// A host may keep one project behind a Mutex for many directives.
#[test]
fn projects_are_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Project>();
}

#[test]
fn modules_docstrings_and_all() {
    let (mut p, _root) = package();
    let id = p.load_module("pkg").expect("pkg is found");
    let m = p.module(id);
    assert_eq!(m.kind, ModuleKind::Package);
    assert_eq!(m.doc.as_deref(), Some("The package.\n\nIt re-exports.\n"));
    assert!(m.all.defined && m.all.known);
    assert_eq!(m.all.names.join(","), "Thing,helper,VERSION");
    assert_eq!(
        m.info.attr_doc("", "VERSION").map(|d| d.join("|")).as_deref(),
        Some("The version.|")
    );
    assert!(p.load_module("pkg.missing").is_none());
}

#[test]
fn names_resolve_across_modules() {
    let (mut p, _root) = package();
    let pkg = Object::of_module(p.load_module("pkg").expect("pkg"));
    let thing = p.get_attr(&pkg, "Thing");
    assert_eq!(thing.kind, ObjectKind::Class);
    let class = thing.class.expect("a class");
    assert_eq!(p.module(class.module).name, "pkg.core");
    assert_eq!(&*p.class(class).qualname, "Thing");
    let helper = p.get_attr(&pkg, "helper"); // through the wildcard import.
    assert_eq!(helper.kind, ObjectKind::Function);
    let f = helper.function.expect("a function");
    assert_eq!(p.module(f.module).function(f.kind).doc.as_deref(), Some("Help A and \\n."));
    assert_eq!(p.get_attr(&pkg, "helpers").kind, ObjectKind::Module);
    // Helper is imported only for type checkers: not an attribute at run time.
    let core = Object::of_module(class.module);
    assert_eq!(p.get_attr(&core, "Helper").kind, ObjectKind::None);
    let inherited = p.get_attr(&thing, "inherited").function.expect("inherited");
    assert_eq!(&*p.module(inherited.module).function(inherited.kind).qualname, "Base.inherited");
    let (order, complete) = p.mro(class);
    assert_eq!(order.len(), 3);
    assert_eq!(&*p.class(order[1].class.expect("Base")).name, "Base");
    assert_eq!(order[2].path, "builtins.object");
    assert!(complete);
}

fn kind_name(k: ParamKind) -> &'static str {
    match k {
        ParamKind::PositionalOnly => "po",
        ParamKind::PositionalOrKeyword => "pk",
        ParamKind::VarPositional => "vp",
        ParamKind::KeywordOnly => "kw",
        ParamKind::VarKeyword => "vk",
    }
}

#[test]
fn functions_and_decorators() {
    let (mut p, _root) = package();
    let core_id = p.load_module("pkg.core").expect("core");
    let core = std::sync::Arc::clone(p.module(core_id));
    let thing = p.module_attr(core_id, "Thing");
    let init = p.get_attr(&thing, "__init__").function.expect("__init__");
    let init = core.function(init.kind);
    let kinds: Vec<String> = init
        .params
        .iter()
        .map(|q| format!("{}:{}={}", q.name, kind_name(q.kind), core.text(q.default)))
        .collect();
    assert_eq!(kinds.join(" "), "self:pk= name:pk= size:pk=LIMIT args:vp= flag:kw=False kw:vk=");
    assert_eq!(core.text(init.returns), "None");
    assert_eq!(p.get_attr(&thing, "label").wrapper, Wrapper::Property);
    let make = p.get_attr(&thing, "make");
    assert_eq!(make.wrapper, Wrapper::Staticmethod);
    let make_fn = core.function(make.function.expect("make").kind);
    assert_eq!(make_fn.params[0].kind, ParamKind::PositionalOnly);
    let create = p.get_attr(&thing, "create");
    assert_eq!(create.wrapper, Wrapper::Classmethod);
    let create_fn = core.function(create.function.expect("create").kind);
    assert!(create_fn.is_async);
    assert_eq!(core.info.overloads["Thing.get"].len(), 2);
    let default = core.node(create_fn.params[1].default);
    assert_eq!(unparse(default, UnparseMode::Sphinx), "\"it's\"");
    let mut b = String::new();
    write_arguments(&mut b, &core.syntax, &init, UnparseMode::Sphinx);
    assert_eq!(b, "self, name: str, size=LIMIT, *args, flag: bool = False, **kw");
}

#[test]
fn attribute_documentation() {
    let (mut p, _root) = package();
    let id = p.load_module("pkg.core").expect("core");
    let core = p.module(id);
    let doc = |scope: &str, name: &str| core.info.attr_doc(scope, name).map(|d| d.join("|"));
    assert_eq!(doc("", "LIMIT").as_deref(), Some("Replaced by this docstring.|"));
    assert_eq!(doc("Base", "shared").as_deref(), Some("Shared by all.|"));
    assert_eq!(doc("Thing", "count").as_deref(), Some("The count.|"));
    assert_eq!(doc("Thing", "name").as_deref(), Some("The name.|"));
    assert_eq!(doc("Thing", "size").as_deref(), Some("The size,|in bytes.|"));
    assert!(doc("Thing", "hidden").is_none());
    let order = &core.info.tagorder;
    assert!(order.contains_key("Thing.hidden"));
    assert!(order["Thing.label"] > order["Thing.__init__"]);
    assert_eq!(core.info.annotations[&AttrKey::new("Thing", "count")], "int");
}

#[test]
fn literals() {
    assert_eq!(repr_str("it's"), "\"it's\"");
    assert_eq!(repr_str("a\nb\\"), "'a\\nb\\\\'");
    assert_eq!(number_repr("0x10"), "16");
    assert_eq!(number_repr("1_000"), "1000");
    assert_eq!(number_repr("1e3"), "1000.0");
    assert_eq!(number_repr("0.00001"), "1e-05");
    assert_eq!(number_repr("1.5e16"), "1.5e+16");
    assert_eq!(number_repr("2.50"), "2.5");
    assert_eq!(number_repr("1j"), "1j");
    assert_eq!(repr_bytes("a\u{ff}'"), "b\"a\\xff'\"");
    let lines = prepare_docstring("First.\n\n    Indented.\n      More.\n    ", 8);
    assert_eq!(lines.join("|"), "First.||Indented.|  More.|");
}

fn nested(open: &str, inner: &str, close: &str, n: usize) -> String {
    format!("{}{inner}{}", open.repeat(n), close.repeat(n))
}

fn indented_ifs(n: usize) -> String {
    let mut b = String::new();
    for i in 0..n {
        let _ = writeln!(b, "{}if x:", " ".repeat(i));
    }
    let _ = writeln!(b, "{}y = 1", " ".repeat(n));
    b
}

#[test]
fn deeply_nested_source_is_not_analysed() {
    const N: usize = 100_000;
    let files = [
        ("parens.py", format!("X = {}\n", nested("(", "1", ")", N))),
        ("lists.py", format!("X = {}\n", nested("[", "1", "]", N))),
        ("plus.py", format!("X: 1{} = 2\n", " + 1".repeat(N))),
        ("chain.py", format!("{}1\n", "a = ".repeat(N))),
        ("targets.py", format!("{} = 1\n", nested("(", "a", ",)", N))),
        ("decorated.py", format!("@d{}\ndef f(): pass\n", " + d".repeat(N))),
        ("ifs.py", indented_ifs(2000)),
        ("shallow.py", format!("X = {}\n", nested("(", "1", ")", 50))),
    ];
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(n, t)| (*n, t.as_str())).collect();
    let (mut p, _root) = project_of(&borrowed);
    for (file, _) in &files[..files.len() - 1] {
        let name = file.trim_end_matches(".py");
        let m = p.load_module(name).map(|id| p.module(id));
        assert!(m.is_some_and(|m| m.too_deep && m.kind == ModuleKind::Native), "{name} analysed");
    }
    let shallow = p.load_module("shallow").expect("shallow");
    let m = std::sync::Arc::clone(p.module(shallow));
    assert!(!m.too_deep && m.kind == ModuleKind::Source);
    assert_eq!(p.module_attr(shallow, "X").kind, ObjectKind::Value);
    // String annotations are parsed on demand, and refused the same way.
    assert!(p.parse_expression(&m, &nested("(", "int", ")", N)).is_none());
    assert!(p.parse_expression(&m, &nested("list[", "int", "]", 10)).is_some());
}

#[test]
fn lookups_through_multiplying_paths_stay_bounded() {
    // Layers of modules that each `import *` from both modules of the next layer, and
    // export a name nobody defines: 2^30 paths to explore without a step budget.
    const LAYERS: usize = 30;
    let mut files: Vec<(String, String)> = Vec::new();
    for i in 0..LAYERS {
        for side in ["a", "b"] {
            let mut text = String::new();
            if i + 1 < LAYERS {
                let _ = write!(text, "from .m{0}a import *\nfrom .m{0}b import *\n", i + 1);
            }
            text.push_str("__all__ = [\"missing\"]\n");
            files.push((format!("pkg/m{i}{side}.py"), text));
        }
    }
    files.push(("pkg/__init__.py".into(), String::new()));
    // Functions each decorated four times with the next: 4^40 paths.
    let mut deco = String::new();
    for i in 0..40 {
        for _ in 0..4 {
            let _ = writeln!(deco, "@f{}", i + 1);
        }
        let _ = writeln!(deco, "def f{i}(): pass");
    }
    files.push(("deco.py".into(), deco));
    let borrowed: Vec<(&str, &str)> =
        files.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let (mut p, _root) = project_of(&borrowed);
    let top = p.load_module("pkg.m0a").expect("m0a");
    assert_eq!(p.module_attr(top, "missing").kind, ObjectKind::None);
    assert!(p.bounds.steps < 2 * MAX_STEPS && p.bounds.resolving == 0);
    let deco = p.load_module("deco").expect("deco");
    assert_eq!(p.module_attr(deco, "f0").kind, ObjectKind::Function);
    assert!(p.bounds.steps < 2 * MAX_STEPS && p.bounds.resolving == 0);
}

#[test]
fn long_inheritance_chains_end_with_an_incomplete_mro() {
    const N: usize = 5000;
    let mut b = String::from("class C0: pass\n");
    for i in 1..N {
        let _ = writeln!(b, "class C{i}(C{}): pass", i - 1);
    }
    let (mut p, _root) = project_of(&[("chain.py", &b)]);
    let m = p.load_module("chain").expect("chain");
    let last = p.module_attr(m, &format!("C{}", N - 1));
    assert_eq!(last.kind, ObjectKind::Class);
    let (order, complete) = p.mro(last.class.expect("a class"));
    assert!(!complete);
    assert!(order.len() > mro::MAX_MRO_DEPTH && order.len() < N);
    // A short chain is whole.
    let first = p.module_attr(m, "C3");
    let (order, complete) = p.mro(first.class.expect("a class"));
    assert!(complete && order.len() == 5);
}
