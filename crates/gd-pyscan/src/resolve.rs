//! Resolution follows names as Python would at run time, without running anything: a
//! module attribute is its last binding (imports followed to where the object is
//! defined), a class attribute is found along the method resolution order, and a name
//! in a function or class body falls back to the module and then to builtins. Objects
//! from modules outside the source roots are `External`, known only by their path.

use std::sync::Arc;

use crate::literal::str_value;
use crate::{ClassRef, DefKind, DefRef, FunctionKind, FunctionRef, Module, ModuleId};
use crate::{ModuleKind, NodeId, Project};

/// What an [`Object`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObjectKind {
    /// Not found.
    #[default]
    None,
    /// A module of the roots.
    Module,
    /// A class of the roots.
    Class,
    /// A function of the roots.
    Function,
    /// Anything an assignment made: `value` is its expression.
    Value,
    /// From a module outside the source roots: only `path` is known.
    External,
    /// A builtin such as `str` or `Exception`: `path` is `builtins.<name>`.
    Builtin,
}

/// What the decorators of a function made of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Wrapper {
    /// Nothing that changes how it is documented.
    #[default]
    None,
    /// `@staticmethod`.
    Staticmethod,
    /// `@classmethod`.
    Classmethod,
    /// `@property`.
    Property,
    /// `@functools.cached_property`.
    CachedProperty,
    /// A decorator from outside the roots that is not known to keep functions.
    Unknown,
}

/// An object a name resolved to.
#[derive(Debug, Clone, Default)]
pub struct Object {
    /// What it is.
    pub kind: ObjectKind,
    /// `Module`: the module itself; else the module that defines it.
    pub module: Option<ModuleId>,
    /// `Class`: the class.
    pub class: Option<ClassRef>,
    /// `Function`: the function.
    pub function: Option<FunctionRef>,
    /// `Value`: the expression assigned.
    pub value: Option<NodeId>,
    /// `Value`: bound by unpacking, so `value` is only its source.
    pub tuple: bool,
    /// `External`, `Builtin`: the dotted name.
    pub path: String,
    /// What its decorators made of a function.
    pub wrapper: Wrapper,
    /// Decorated with `abstractmethod`.
    pub is_abstract: bool,
    /// The class whose body defines it, when found in one.
    pub owner: Option<ClassRef>,
    /// The binding it was found through.
    pub def: Option<DefRef>,
    /// `Wrapper::Unknown`: the decorator as written.
    pub decorator: String,
    /// A docstring given explicitly: `property(fget, doc="...")`.
    pub doc: Option<String>,
    /// A property under `classmethod`: a property of the class.
    pub class_property: bool,
    /// `functools.partial` and `partialmethod`: how many leading parameters (after
    /// self, for partialmethod) the call already gave.
    pub partial_args: usize,
    /// Made by `functools.partial` or `partialmethod`.
    pub partial: bool,
    /// `partialmethod`: as got from the class, a function with no docstring.
    pub partial_method: bool,
}

impl Object {
    /// The object of a module.
    pub fn of_module(m: ModuleId) -> Object {
        Object { kind: ObjectKind::Module, module: Some(m), ..Object::default() }
    }

    /// An object outside the roots, known by its dotted path.
    pub fn external(path: String) -> Object {
        Object { kind: ObjectKind::External, path, ..Object::default() }
    }

    fn builtin(name: &str) -> Object {
        Object { kind: ObjectKind::Builtin, path: format!("builtins.{name}"), ..Object::default() }
    }

    fn function(f: FunctionRef) -> Object {
        Object {
            kind: ObjectKind::Function,
            module: Some(f.module),
            function: Some(f),
            ..Object::default()
        }
    }

    /// Whether the object was found.
    pub fn is_found(&self) -> bool {
        self.kind != ObjectKind::None
    }
}

/// Bounds one resolution path: cycles of imports, aliases and wrappers end there, found
/// or not.
pub(crate) const MAX_DEPTH: usize = 48;

/// Bounds the work of one lookup the host starts. `MAX_DEPTH` cuts every path, but
/// paths multiply: modules that each `import *` from two others give exponentially many
/// paths within it. Every step of a lookup spends one; past it the lookup finds nothing.
pub(crate) const MAX_STEPS: usize = 100_000;

/// Bounds the machine stack of one lookup. A lookup can start others with a fresh
/// `MAX_DEPTH` (an attribute of a class needs its MRO, which resolves the bases), so
/// depth alone does not bound it; past this budget the lookup finds nothing. Debug
/// builds keep larger frames, so they get a larger budget for the same depth.
pub(crate) const LOOKUP_STACK_BYTES: usize =
    (256 << 10) * if cfg!(debug_assertions) { 2 } else { 1 };

/// The lookup in progress: its steps, where its stack starts, and how deep lookups and
/// MRO computations are nested.
#[derive(Debug, Default)]
pub(crate) struct Bounds {
    pub(crate) steps: usize,
    pub(crate) stack_base: usize,
    pub(crate) resolving: usize,
    pub(crate) mro_depth: usize,
}

const DECORATOR_PROPERTY_SUFFIXES: [&str; 3] = [".setter", ".getter", ".deleter"];

impl Project {
    /// Counts one step of the lookup in progress, starting a new lookup when none is,
    /// and runs `f` when the lookup may go on (within `MAX_STEPS`, `MAX_DEPTH`, and
    /// `LOOKUP_STACK_BYTES`); otherwise the lookup finds nothing.
    fn bounded(&mut self, depth: usize, f: impl FnOnce(&mut Project) -> Object) -> Object {
        let here = 0u8;
        // The frame's address measures the stack; nothing reads through it.
        let at = std::hint::black_box(&here) as *const u8 as usize;
        let b = &mut self.bounds;
        if b.resolving == 0 {
            b.steps = 0;
            b.stack_base = at;
        }
        b.resolving += 1;
        b.steps += 1;
        let used = b.stack_base.abs_diff(at);
        let going = b.steps <= MAX_STEPS && used < LOOKUP_STACK_BYTES && depth <= MAX_DEPTH;
        let out = if going { f(self) } else { Object::default() };
        self.bounds.resolving -= 1;
        out
    }

    /// `getattr(obj, name)` as far as the source tells.
    pub fn get_attr(&mut self, obj: &Object, name: &str) -> Object {
        self.get_attr_at(obj, name, 0)
    }

    pub(crate) fn get_attr_at(&mut self, obj: &Object, name: &str, depth: usize) -> Object {
        if depth > MAX_DEPTH {
            return Object::default();
        }
        match (obj.kind, obj.module, obj.class) {
            (ObjectKind::Module, Some(m), _) => self.module_attr_at(m, name, depth + 1),
            (ObjectKind::Class, _, Some(c)) => self.class_attr_at(c, name, depth + 1),
            (ObjectKind::External | ObjectKind::Builtin, _, _) if !obj.path.is_empty() => {
                Object::external(format!("{}.{name}", obj.path))
            }
            // Nothing, or an object whose attributes only exist at run time.
            _ => Object::default(),
        }
    }

    /// Looks a name up in a module's namespace, then among its submodules.
    pub fn module_attr(&mut self, m: ModuleId, name: &str) -> Object {
        self.module_attr_at(m, name, 0)
    }

    fn module_attr_at(&mut self, m: ModuleId, name: &str, depth: usize) -> Object {
        self.bounded(depth, |p| {
            let module = Arc::clone(p.module(m));
            if let Some(i) = module.scope.find_def_index(name) {
                let d = DefRef { module: m, class: None, index: i as u32 };
                return p.def_object_at(d, depth + 1);
            }
            let o = p.wildcard_import_attr(m, name, depth + 1);
            if o.is_found() {
                return o;
            }
            let is_package = matches!(module.kind, ModuleKind::Package | ModuleKind::Namespace);
            if is_package && let Some(sub) = p.load_module(&format!("{}.{name}", module.name)) {
                return Object::of_module(sub);
            }
            Object::default()
        })
    }

    /// Finds a name that a `from x import *` brought in.
    fn wildcard_import_attr(&mut self, m: ModuleId, name: &str, depth: usize) -> Object {
        let module = Arc::clone(self.module(m));
        let wildcards = module
            .scope
            .defs
            .iter()
            .rev()
            .filter(|d| d.runtime && d.kind == DefKind::ImportFrom && &*d.member == "*");
        for d in wildcards {
            let Some(source) = self.load_module(&d.target) else { continue };
            if !self.exported(source, name) {
                continue;
            }
            let o = self.module_attr_at(source, name, depth + 1);
            if o.is_found() {
                return o;
            }
        }
        Object::default()
    }

    /// Whether `from m import *` binds `name`: `__all__` when `m` has one, else the
    /// public names.
    pub fn exported(&self, m: ModuleId, name: &str) -> bool {
        let m = self.module(m);
        if m.all.defined && m.all.known {
            return m.all.names.iter().any(|n| n == name);
        }
        !name.starts_with('_') && m.scope.find_def(name).is_some()
    }

    /// Turns a binding into the object it holds.
    pub fn def_object(&mut self, d: DefRef) -> Object {
        self.def_object_at(d, 0)
    }

    pub(crate) fn def_object_at(&mut self, dref: DefRef, depth: usize) -> Object {
        self.bounded(depth, |p| {
            let module = Arc::clone(p.module(dref.module));
            let d = &module.scope(dref.class).defs[dref.index as usize];
            let owner = dref.class.map(|class| ClassRef { module: dref.module, class });
            let mut o = match d.kind {
                DefKind::Class => Object {
                    kind: ObjectKind::Class,
                    module: Some(dref.module),
                    class: d.class.map(|class| ClassRef { module: dref.module, class }),
                    ..Object::default()
                },
                DefKind::Function => match d.function {
                    Some(id) => {
                        let f = FunctionRef { module: dref.module, kind: FunctionKind::Def(id) };
                        let mut o = Object::function(f);
                        p.wrap(&mut o, f, depth);
                        o
                    }
                    None => Object::default(),
                },
                DefKind::Import => p.import_object(&d.target),
                DefKind::ImportFrom => p.import_from_object(&d.target, &d.member, depth),
                DefKind::Assign | DefKind::TypeAlias => p.value_object(&module, owner, d, depth),
                DefKind::Annotation => return Object::default(),
            };
            if o.owner.is_none() {
                o.owner = owner;
            }
            o.def = Some(dref);
            o
        })
    }

    /// The object an assignment binds: what its value names, a wrapped function, a
    /// lambda, or else the value itself.
    fn value_object(
        &mut self,
        m: &Arc<Module>,
        owner: Option<ClassRef>,
        d: &crate::Def,
        depth: usize,
    ) -> Object {
        let value = Object {
            kind: ObjectKind::Value,
            module: Some(m.id),
            value: d.value,
            tuple: d.tuple,
            ..Object::default()
        };
        let Some(node) = d.value.filter(|_| !d.tuple) else { return value };
        let reference = self.value_reference(m, owner, node, depth);
        if reference.is_found() {
            return reference;
        }
        if let Some(wrapped) = self.wrapper_call(m, owner, node, depth) {
            return wrapped;
        }
        if m.node(node).kind() == "lambda" {
            return Object::function(lambda_ref(m.id, owner, node));
        }
        value
    }

    fn import_object(&mut self, target: &str) -> Object {
        match self.load_module(target) {
            Some(m) => Object::of_module(m),
            None => Object::external(target.to_string()),
        }
    }

    fn import_from_object(&mut self, from: &str, member: &str, depth: usize) -> Object {
        let Some(m) = self.load_module(from) else {
            return Object::external(format!("{from}.{member}"));
        };
        // `from . import sub` names a submodule; looking the name up in the package
        // would find this very import.
        let is_package =
            matches!(self.module(m).kind, ModuleKind::Package | ModuleKind::Namespace);
        if is_package && let Some(sub) = self.load_module(&format!("{from}.{member}")) {
            return Object::of_module(sub);
        }
        self.module_attr_at(m, member, depth + 1)
    }

    /// Evaluates an expression that names an object: a name, or attributes of one, or a
    /// subscripted generic (its origin), where `code` runs (in the body of `scope`, if
    /// given). Other expressions give nothing. `code` may be a parsed snippet.
    pub fn reference(&mut self, code: &Module, scope: Option<ClassRef>, n: NodeId) -> Object {
        self.reference_at(code, scope, n, 0)
    }

    pub(crate) fn reference_at(
        &mut self,
        code: &Module,
        scope: Option<ClassRef>,
        n: NodeId,
        depth: usize,
    ) -> Object {
        self.bounded(depth, |p| {
            let node = code.node(n);
            let first = node.named_child(0).id();
            match node.kind() {
                "identifier" => p.lookup_name_at(code.id, scope, node.text(), depth + 1),
                "attribute" => {
                    let Some(object) = node.field("object").id() else { return Object::default() };
                    let base = p.reference_at(code, scope, object, depth + 1);
                    if matches!(base.kind, ObjectKind::None | ObjectKind::Value) {
                        return Object::default();
                    }
                    p.get_attr_at(&base, node.field_text("attribute"), depth + 1)
                }
                "type" | "parenthesized_expression" | "generic_type" => match first {
                    Some(inner) => p.reference_at(code, scope, inner, depth + 1),
                    None => Object::default(),
                },
                "subscript" => match node.field("value").id() {
                    Some(value) => p.reference_at(code, scope, value, depth + 1),
                    None => Object::default(),
                },
                _ => Object::default(),
            }
        })
    }

    /// [`reference`](Self::reference) for the value of an assignment: only names and
    /// attributes are the object they name; a subscripted generic is an alias of its
    /// own, not its origin.
    fn value_reference(
        &mut self,
        m: &Module,
        scope: Option<ClassRef>,
        n: NodeId,
        depth: usize,
    ) -> Object {
        let mut node = m.node(n);
        while node.kind() == "parenthesized_expression" {
            node = node.named_child(0);
        }
        match (node.kind(), node.id()) {
            ("identifier" | "attribute", Some(id)) => self.reference_at(m, scope, id, depth),
            _ => Object::default(),
        }
    }

    /// Resolves a name where a class body (`scope`) or the module `code` runs: the class
    /// body first, then the module, then builtins.
    pub fn lookup_name(&mut self, code: ModuleId, scope: Option<ClassRef>, name: &str) -> Object {
        self.lookup_name_at(code, scope, name, 0)
    }

    fn lookup_name_at(
        &mut self,
        code: ModuleId,
        scope: Option<ClassRef>,
        name: &str,
        depth: usize,
    ) -> Object {
        if let Some(c) = scope
            && let Some(i) = self.class(c).scope.find_def_index(name)
        {
            let d = DefRef { module: c.module, class: Some(c.class), index: i as u32 };
            return self.def_object_at(d, depth + 1);
        }
        if let Some(i) = self.module(code).scope.find_def_index(name) {
            let d = DefRef { module: code, class: None, index: i as u32 };
            return self.def_object_at(d, depth + 1);
        }
        let o = self.wildcard_import_attr(code, name, depth + 1);
        if o.is_found() {
            return o;
        }
        if is_builtin(name) { Object::builtin(name) } else { Object::default() }
    }

    /// Applies what a function's decorators make of it.
    fn wrap(&mut self, o: &mut Object, f: FunctionRef, depth: usize) {
        let module = Arc::clone(self.module(f.module));
        let function = module.function(f.kind);
        let owner = function.class.map(|class| ClassRef { module: f.module, class });
        for &dec in &function.decorators {
            let mut target = module.node(dec);
            if target.kind() == "call" {
                target = target.field("function");
            }
            let written = target.text();
            if DECORATOR_PROPERTY_SUFFIXES.iter().any(|s| written.ends_with(s)) {
                // The property keeps its getter: its docstring and return annotation.
                o.wrapper = Wrapper::Property;
                let name = written.rsplit_once('.').map_or(written, |(name, _)| name);
                if let Some(getter) = property_getter(&module, f, name) {
                    o.function = Some(getter);
                }
                continue;
            }
            let Some(target_id) = target.id() else { continue };
            let reference = self.reference_at(&module, owner, target_id, depth + 1);
            self.apply_decorator(o, &reference, written);
        }
    }

    fn apply_decorator(&mut self, o: &mut Object, reference: &Object, written: &str) {
        let path = match reference.kind {
            ObjectKind::Builtin | ObjectKind::External => reference.path.as_str(),
            _ => "",
        };
        match path {
            "builtins.staticmethod" => o.wrapper = Wrapper::Staticmethod,
            "builtins.classmethod" if o.wrapper == Wrapper::Property => o.class_property = true,
            "builtins.classmethod" => o.wrapper = Wrapper::Classmethod,
            "builtins.property" | "types.DynamicClassAttribute" | "enum.property" => {
                o.class_property |= o.wrapper == Wrapper::Classmethod;
                o.wrapper = Wrapper::Property;
            }
            "functools.cached_property" => o.wrapper = Wrapper::CachedProperty,
            "abc.abstractmethod" => o.is_abstract = true,
            "abc.abstractproperty" => (o.is_abstract, o.wrapper) = (true, Wrapper::Property),
            "abc.abstractclassmethod" => (o.is_abstract, o.wrapper) = (true, Wrapper::Classmethod),
            "abc.abstractstaticmethod" => {
                (o.is_abstract, o.wrapper) = (true, Wrapper::Staticmethod)
            }
            _ => self.apply_other_decorator(o, reference, written),
        }
    }

    fn apply_other_decorator(&mut self, o: &mut Object, reference: &Object, written: &str) {
        let external = reference.kind == ObjectKind::External;
        if let (ObjectKind::Class, Some(c)) = (reference.kind, reference.class) {
            if self.is_property_class(c) {
                o.wrapper = Wrapper::CachedProperty;
            }
        } else if external && reference.path.ends_with("cached_property") {
            o.wrapper = Wrapper::CachedProperty;
        } else if external && !keeps_function(&reference.path) && o.wrapper == Wrapper::None {
            o.wrapper = Wrapper::Unknown;
            o.decorator = written.to_string();
        }
    }

    /// Whether a class in the roots is a property (a decorator made by subclassing
    /// `property` or `functools.cached_property`).
    fn is_property_class(&mut self, c: ClassRef) -> bool {
        let (order, _) = self.mro(c);
        order
            .iter()
            .any(|e| e.path == "builtins.property" || e.path == "functools.cached_property")
    }

    /// Reads `staticmethod(f)`, `classmethod(f)`, and `property(f, ...)` assignments as
    /// the function they wrap.
    fn wrapper_call(
        &mut self,
        m: &Arc<Module>,
        owner: Option<ClassRef>,
        n: NodeId,
        depth: usize,
    ) -> Option<Object> {
        let call = m.node(n);
        if call.kind() != "call" {
            return None;
        }
        let callee = self.reference_at(m, owner, call.field("function").id()?, depth + 1);
        if callee.kind == ObjectKind::External {
            return self.partial_call(m, owner, n, &callee.path, depth);
        }
        let wrapper = match (callee.kind, callee.path.as_str()) {
            (ObjectKind::Builtin, "builtins.staticmethod") => Wrapper::Staticmethod,
            (ObjectKind::Builtin, "builtins.classmethod") => Wrapper::Classmethod,
            (ObjectKind::Builtin, "builtins.property") => Wrapper::Property,
            _ => return None,
        };
        let args = call.field("arguments");
        if args.kind() != "argument_list" {
            return None;
        }
        let first = args.named_child(0);
        let mut inner = if first.kind() == "lambda" {
            Object::function(lambda_ref(m.id, owner, first.id()?))
        } else {
            self.reference_at(m, owner, first.id()?, depth + 1)
        };
        if inner.kind != ObjectKind::Function {
            return None;
        }
        inner.wrapper = wrapper;
        if wrapper == Wrapper::Property {
            for a in args.named_children().filter(|a| a.kind() == "keyword_argument") {
                if a.field_text("name") == "doc"
                    && let Some(s) = str_value(a.field("value"))
                {
                    inner.doc = Some(s.into_owned());
                }
            }
        }
        Some(inner)
    }

    /// Reads `functools.partial(f, a, ...)` and `partialmethod(f, a, ...)` as `f` with
    /// its leading parameters given; keyword arguments are left as they are.
    fn partial_call(
        &mut self,
        m: &Arc<Module>,
        owner: Option<ClassRef>,
        n: NodeId,
        path: &str,
        depth: usize,
    ) -> Option<Object> {
        if path != "functools.partial" && path != "functools.partialmethod" {
            return None;
        }
        let call = m.node(n);
        if call.kind() != "call" || depth > MAX_DEPTH {
            return None;
        }
        let args = call.field("arguments");
        if args.kind() != "argument_list" {
            return None;
        }
        let first = args.named_child(0);
        let mut inner = self.reference_at(m, owner, first.id()?, depth + 1);
        if inner.kind != ObjectKind::Function {
            if first.kind() != "call" {
                return None;
            }
            let callee = self.reference_at(m, owner, first.field("function").id()?, depth + 1);
            if callee.kind != ObjectKind::External {
                return None;
            }
            inner = self.partial_call(m, owner, first.id()?, &callee.path, depth + 1)?;
        }
        let given = args.named_children().filter(|a| {
            *a != first
                && !matches!(
                    a.kind(),
                    "keyword_argument" | "comment" | "list_splat" | "dictionary_splat"
                )
        });
        inner.partial_args += given.count();
        inner.partial = true;
        inner.partial_method |= path == "functools.partialmethod";
        Some(inner)
    }
}

/// The function a lambda assigned in `owner`'s body (or the module's) makes.
fn lambda_ref(m: ModuleId, owner: Option<ClassRef>, node: NodeId) -> FunctionRef {
    let class = owner.filter(|c| c.module == m).map(|c| c.class);
    FunctionRef { module: m, kind: FunctionKind::Lambda { node, class } }
}

/// Finds the getter of a property that `@name.setter` extends: the earlier definition of
/// `name` in the same class body that is not itself a setter or deleter.
fn property_getter(m: &Module, f: FunctionRef, name: &str) -> Option<FunctionRef> {
    let FunctionKind::Def(id) = f.kind else { return None };
    let class = m.function(f.kind).class?;
    let mut found = None;
    for d in &m.class(class).scope.defs {
        if d.function == Some(id) {
            break;
        }
        let Some(candidate) =
            d.function.filter(|_| &*d.name == name && d.kind == DefKind::Function)
        else {
            continue;
        };
        let extends = m.functions[candidate.0 as usize].decorators.iter().any(|&dec| {
            let written = m.text(dec);
            written.ends_with(".setter") || written.ends_with(".deleter")
        });
        if !extends {
            found = Some(FunctionRef { module: f.module, kind: FunctionKind::Def(candidate) });
        }
    }
    found
}

/// Decorators from outside the roots that return a function (or the function itself)
/// with the decorated one's signature and docstring.
fn keeps_function(path: &str) -> bool {
    matches!(
        path,
        "functools.wraps"
            | "functools.lru_cache"
            | "functools.cache"
            | "functools.singledispatch"
            | "functools.total_ordering"
            | "contextlib.contextmanager"
            | "contextlib.asynccontextmanager"
            | "typing.final"
            | "typing.overload"
            | "typing.no_type_check"
            | "typing_extensions.final"
            | "typing_extensions.overload"
            | "typing.override"
            | "typing_extensions.override"
            | "typing_extensions.deprecated"
            | "warnings.deprecated"
            | "abc.abstractmethod"
    )
}

/// The names of Python's builtins module that matter to documentation, besides the
/// exception classes.
const BUILTINS: &[&str] = &[
    "object",
    "type",
    "str",
    "bytes",
    "bytearray",
    "int",
    "float",
    "complex",
    "bool",
    "list",
    "tuple",
    "dict",
    "set",
    "frozenset",
    "memoryview",
    "range",
    "slice",
    "property",
    "staticmethod",
    "classmethod",
    "super",
    "len",
    "print",
    "repr",
    "isinstance",
    "issubclass",
    "callable",
    "getattr",
    "setattr",
    "hasattr",
    "iter",
    "next",
    "open",
    "id",
    "hash",
    "vars",
    "dir",
    "sorted",
    "reversed",
    "enumerate",
    "zip",
    "map",
    "filter",
    "min",
    "max",
    "sum",
    "any",
    "all",
    "abs",
    "round",
    "divmod",
    "pow",
    "format",
    "chr",
    "ord",
    "hex",
    "oct",
    "bin",
    "input",
    "exec",
    "eval",
    "compile",
    "globals",
    "locals",
    "NotImplemented",
    "Ellipsis",
    "__import__",
];

/// The builtin exception classes.
const BUILTIN_EXCEPTIONS: &[&str] = &[
    "BaseException",
    "Exception",
    "ArithmeticError",
    "AssertionError",
    "AttributeError",
    "BufferError",
    "EOFError",
    "FloatingPointError",
    "GeneratorExit",
    "ImportError",
    "ModuleNotFoundError",
    "IndexError",
    "KeyError",
    "KeyboardInterrupt",
    "LookupError",
    "MemoryError",
    "NameError",
    "NotImplementedError",
    "OSError",
    "OverflowError",
    "RecursionError",
    "ReferenceError",
    "RuntimeError",
    "StopIteration",
    "StopAsyncIteration",
    "SyntaxError",
    "IndentationError",
    "TabError",
    "SystemError",
    "SystemExit",
    "TypeError",
    "UnboundLocalError",
    "UnicodeError",
    "UnicodeEncodeError",
    "UnicodeDecodeError",
    "UnicodeTranslateError",
    "ValueError",
    "ZeroDivisionError",
    "EnvironmentError",
    "IOError",
    "BlockingIOError",
    "ChildProcessError",
    "ConnectionError",
    "BrokenPipeError",
    "ConnectionAbortedError",
    "ConnectionRefusedError",
    "ConnectionResetError",
    "FileExistsError",
    "FileNotFoundError",
    "InterruptedError",
    "IsADirectoryError",
    "NotADirectoryError",
    "PermissionError",
    "ProcessLookupError",
    "TimeoutError",
    "Warning",
    "UserWarning",
    "DeprecationWarning",
    "PendingDeprecationWarning",
    "SyntaxWarning",
    "RuntimeWarning",
    "FutureWarning",
    "ImportWarning",
    "UnicodeWarning",
    "BytesWarning",
    "ResourceWarning",
    "EncodingWarning",
    "BaseExceptionGroup",
    "ExceptionGroup",
    "PythonFinalizationError",
];

/// Whether a name is one of Python's builtins that matter to documentation.
pub fn is_builtin(name: &str) -> bool {
    BUILTINS.contains(&name) || is_builtin_exception(name)
}

/// Whether a name is a builtin exception class.
pub fn is_builtin_exception(name: &str) -> bool {
    BUILTIN_EXCEPTIONS.contains(&name)
}
