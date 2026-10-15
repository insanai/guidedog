//! Static analysis of Python source, through tree-sitter: modules and their docstrings,
//! `__all__`, and imports; classes with their bases, decorators, and members in source
//! order; functions with their parameters as written; and the attribute documentation
//! Sphinx's `ModuleAnalyzer` finds (`#:` comments and docstrings after assignments).
//! Names resolve across the modules of the configured source roots, so a class
//! re-exported by a package is found where it is defined. Nothing is executed.
//!
//! A [`Project`] owns every module it loads ([`Module`], shared as `Arc` because a loaded
//! module never changes); classes, functions, definitions, and syntax nodes are plain
//! indices into their module ([`ClassId`], [`FunctionId`], [`NodeId`]), and objects
//! name them with [`ClassRef`], [`FunctionRef`], and [`DefRef`]. Lookups are bounded in
//! steps, depth, and stack, and a module nested too deeply is marked `too_deep`, not
//! walked. A project is `Send`, so a host may share one behind a `Mutex`.

#![forbid(unsafe_code)]

mod analysis;
mod literal;
mod mro;
mod resolve;
mod scan;
mod syntax;
mod unparse;

#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use analysis::{Analysis, AttrKey};
pub use literal::{StringKind, prepare_docstring, repr_bytes, repr_str, split_lines};
pub use literal::{str_value, string_value};
pub use mro::{MroEntry, Slot};
pub use resolve::{Object, ObjectKind, Wrapper, is_builtin, is_builtin_exception};
pub use syntax::{Node, NodeId, Syntax};
pub use unparse::{UnparseMode, number_repr, try_unparse, unparse, write_arguments};

/// How deep a module's syntax tree may nest. Scanning, analysis and unparsing walk the
/// tree recursively, so a module nested deeper is not analysed: it is treated like a
/// compiled module, with `too_deep` set. Python itself refuses 200 nested brackets or
/// 100 indentation levels, but not long left-nested operator chains (`a + b + ...`),
/// which are refused here past about 250 operands.
pub const MAX_SYNTAX_DEPTH: usize = 256;

/// Reads a file for the analyser; `None` for a file that is missing or refused. A host
/// confines reads to what it may read. It is `Send` so the project is.
pub type FileReader = Box<dyn FnMut(&Path) -> Option<String> + Send>;

/// The index of a module in its [`Project`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ModuleId(pub u32);

/// The index of a class in its module's [`Module::classes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClassId(pub u32);

/// The index of a function in its module's [`Module::functions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FunctionId(pub u32);

/// A class of a project: its module and its index there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClassRef {
    /// The module that defines the class.
    pub module: ModuleId,
    /// The class's index in that module.
    pub class: ClassId,
}

/// Which function a [`FunctionRef`] names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunctionKind {
    /// A `def` (or an `@overload` variant) the module scanned.
    Def(FunctionId),
    /// A lambda an assignment binds, made into a function when it is looked at; `class`
    /// is the class whose body binds it.
    Lambda {
        /// The `lambda` node.
        node: NodeId,
        /// The class whose body binds it, if any.
        class: Option<ClassId>,
    },
}

/// A function of a project: its module and which function it is there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionRef {
    /// The module that defines the function.
    pub module: ModuleId,
    /// The function within that module.
    pub kind: FunctionKind,
}

/// A binding of a project: its module, the class body holding it (or the module's), and
/// its index in that scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DefRef {
    /// The module the binding is in.
    pub module: ModuleId,
    /// The class body holding it; `None` for the module body.
    pub class: Option<ClassId>,
    /// The binding's index in that scope's `defs`.
    pub index: u32,
}

/// What kind of module a dotted name found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModuleKind {
    /// A `.py` file.
    #[default]
    Source,
    /// A folder with `__init__.py`.
    Package,
    /// A folder without `__init__.py` (PEP 420).
    Namespace,
    /// A compiled extension (`.so`, `.pyd`), or a source that cannot be read or analysed.
    Native,
}

/// A module's `__all__` when it can be read without running code.
#[derive(Debug, Clone, Default)]
pub struct All {
    /// The names, in order.
    pub names: Vec<String>,
    /// The module assigns `__all__`.
    pub defined: bool,
    /// And its value was read: literal lists, `+=`, `append`, `extend`.
    pub known: bool,
}

/// How a name is bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DefKind {
    /// `class Name:`.
    Class,
    /// `def name():`.
    Function,
    /// `name = value`, `name: type = value`.
    #[default]
    Assign,
    /// `name: type`, with no value.
    Annotation,
    /// `import a.b [as c]`.
    Import,
    /// `from m import name [as alias]`.
    ImportFrom,
    /// `type Name = value` (PEP 695).
    TypeAlias,
}

/// One binding of a name in a module or class body, in source order.
#[derive(Debug, Clone, Default)]
pub struct Def {
    /// The name bound.
    pub name: Box<str>,
    /// How it is bound.
    pub kind: DefKind,
    /// The statement, or the definition.
    pub node: Option<NodeId>,
    /// `false` under `if TYPE_CHECKING:`.
    pub runtime: bool,
    /// [`DefKind::Class`]: the class.
    pub class: Option<ClassId>,
    /// [`DefKind::Function`]: the function.
    pub function: Option<FunctionId>,
    /// The right-hand side, if any.
    pub value: Option<NodeId>,
    /// The type node of an annotated assignment.
    pub annotation: Option<NodeId>,
    /// Imports: the absolute module imported from, or imported.
    pub target: Box<str>,
    /// [`DefKind::ImportFrom`]: the name imported; `"*"` for a wildcard.
    pub member: Box<str>,
    /// Bound by unpacking: its value is one part of `value`.
    pub tuple: bool,
    /// The 1-based line of the statement.
    pub line: u32,
}

/// The bindings of a module or class body, in source order.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    /// Every binding, in order.
    pub defs: Vec<Def>,
}

impl Scope {
    /// The binding of `name` in force when the body has run: the last runtime one.
    pub fn find_def(&self, name: &str) -> Option<&Def> {
        self.find_def_index(name).map(|i| &self.defs[i])
    }

    /// The index of [`find_def`](Self::find_def)'s binding.
    pub fn find_def_index(&self, name: &str) -> Option<usize> {
        self.defs.iter().rposition(|d| d.runtime && &*d.name == name)
    }
}

/// A keyword argument of a class statement, such as `metaclass=Meta`.
#[derive(Debug, Clone, Default)]
pub struct Keyword {
    /// The keyword.
    pub name: Box<str>,
    /// Its value.
    pub value: Option<NodeId>,
}

/// A class statement.
#[derive(Debug, Clone, Default)]
pub struct Class {
    /// Its name.
    pub name: Box<str>,
    /// Its qualified name (`Outer.Inner`).
    pub qualname: Box<str>,
    /// The `class_definition` node.
    pub node: Option<NodeId>,
    /// The positional arguments of the class statement.
    pub bases: Vec<NodeId>,
    /// `metaclass=...`, and others.
    pub keywords: Vec<Keyword>,
    /// The decorator expressions.
    pub decorators: Vec<NodeId>,
    /// Its docstring.
    pub doc: Option<String>,
    /// What its body binds.
    pub scope: Scope,
    /// The class it is nested in.
    pub outer: Option<ClassId>,
}

/// How a parameter takes its argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParamKind {
    /// Before `/`.
    PositionalOnly,
    /// An ordinary parameter.
    #[default]
    PositionalOrKeyword,
    /// `*args`.
    VarPositional,
    /// After `*` or `*args`.
    KeywordOnly,
    /// `**kwargs`.
    VarKeyword,
}

/// A parameter as written.
#[derive(Debug, Clone, Default)]
pub struct Param {
    /// Its name, without stars.
    pub name: Box<str>,
    /// How it takes its argument.
    pub kind: ParamKind,
    /// The type node, if any.
    pub annotation: Option<NodeId>,
    /// The default expression, if any.
    pub default: Option<NodeId>,
    /// `a,  # type: str`: a per-parameter type comment, or `""`.
    pub type_comment: String,
}

/// A function definition or lambda.
#[derive(Debug, Clone, Default)]
pub struct Function {
    /// Its name; `<lambda>` for a lambda.
    pub name: Box<str>,
    /// Its qualified name.
    pub qualname: Box<str>,
    /// The `function_definition` or `lambda` node.
    pub node: Option<NodeId>,
    /// `async def`.
    pub is_async: bool,
    /// A lambda.
    pub is_lambda: bool,
    /// The decorator expressions.
    pub decorators: Vec<NodeId>,
    /// The parameters as written.
    pub params: Vec<Param>,
    /// The return type node, if any.
    pub returns: Option<NodeId>,
    /// Its docstring.
    pub doc: Option<String>,
    /// The class whose body defines it.
    pub class: Option<ClassId>,
    /// A PEP 484 type comment after the header, `# type: (int, str) -> bool`.
    pub type_comment: bool,
    /// The comment's parameter types as written (`...` alone when elided).
    pub type_args: Vec<String>,
    /// The comment's return type.
    pub type_returns: String,
}

/// A module the project found: its source, syntax, and what scanning and analysis
/// found in it.
#[derive(Debug, Clone, Default)]
pub struct Module {
    /// Its index in the project; for a parsed snippet, the module it was parsed for.
    pub id: ModuleId,
    /// Its dotted name.
    pub name: String,
    /// What kind of module it is.
    pub kind: ModuleKind,
    /// The file read; the folder of a namespace package.
    pub path: PathBuf,
    /// The source (line breaks normalised to `\n`) and its syntax tree.
    pub syntax: Syntax,
    /// The module docstring.
    pub doc: Option<String>,
    /// What its body binds.
    pub scope: Scope,
    /// Every class statement, nested ones too.
    pub classes: Vec<Class>,
    /// Every function definition, and the `@overload` variants the analyser recorded.
    pub functions: Vec<Function>,
    /// Its `__all__`.
    pub all: All,
    /// What Sphinx's `ModuleAnalyzer` finds.
    pub info: Analysis,
    /// tree-sitter found syntax errors.
    pub broken: bool,
    /// It nests past [`MAX_SYNTAX_DEPTH`], so it is not analysed (kind `Native`).
    pub too_deep: bool,
}

impl Module {
    /// The node an id names (null for `None`).
    pub fn node(&self, id: impl Into<Option<NodeId>>) -> Node<'_> {
        self.syntax.node(id)
    }

    /// The source a node spans; `""` for `None`.
    pub fn text(&self, id: impl Into<Option<NodeId>>) -> &str {
        self.syntax.node(id).text()
    }

    /// A class of this module.
    pub fn class(&self, id: ClassId) -> &Class {
        &self.classes[id.0 as usize]
    }

    /// A function of this module: borrowed for a definition, made for a lambda.
    pub fn function(&self, kind: FunctionKind) -> Cow<'_, Function> {
        match kind {
            FunctionKind::Def(id) => Cow::Borrowed(&self.functions[id.0 as usize]),
            FunctionKind::Lambda { node, class } => {
                let qualname = match class {
                    Some(c) => format!("{}.<lambda>", self.class(c).qualname),
                    None => "<lambda>".to_string(),
                };
                let mut f = scan::function_of(self.node(node), &qualname);
                f.class = class;
                Cow::Owned(f)
            }
        }
    }

    /// The scope a binding lives in: a class body, or the module's.
    pub fn scope(&self, class: Option<ClassId>) -> &Scope {
        match class {
            Some(c) => &self.class(c).scope,
            None => &self.scope,
        }
    }
}

pub(crate) fn parent_package(name: &str, kind: ModuleKind) -> &str {
    if matches!(kind, ModuleKind::Package | ModuleKind::Namespace) {
        return name;
    }
    name.rfind('.').map_or("", |i| &name[..i])
}

/// Why a project could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectError {
    /// tree-sitter refused the Python grammar (a version mismatch of the two crates).
    Grammar(String),
}

impl ProjectError {
    /// A stable code for the error.
    pub fn code(&self) -> &'static str {
        "pyscan.grammar"
    }

    /// A short upper-case title.
    pub fn title(&self) -> &'static str {
        "PYTHON GRAMMAR NOT LOADED"
    }

    /// How to fix it.
    pub fn hint(&self) -> &'static str {
        "Build Guidedog with the tree-sitter and tree-sitter-python versions the workspace \
         pins."
    }
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ProjectError::Grammar(why) = self;
        let title = self.title();
        let rule = "-".repeat(80usize.saturating_sub(title.len() + 4));
        write!(f, "-- {title} {rule}\n\ntree-sitter cannot use the Python grammar: {why}.\n\n")?;
        write!(f, "Hint: {}", self.hint())
    }
}

impl std::error::Error for ProjectError {}

/// Finds and analyses modules below its source roots, each once, and resolves names
/// across them.
pub struct Project {
    roots: Vec<PathBuf>,
    reader: Option<FileReader>,
    parser: tree_sitter::Parser,
    names: HashMap<String, Option<ModuleId>>,
    modules: Vec<Arc<Module>>,
    files: Vec<ModuleId>,
    pub(crate) bounds: resolve::Bounds,
    pub(crate) mro_cache: HashMap<ClassRef, mro::MroState>,
}

impl fmt::Debug for Project {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Project")
            .field("roots", &self.roots)
            .field("modules", &self.modules.len())
            .finish()
    }
}

impl Project {
    /// A project over source folders (absolute, or relative to the working directory),
    /// read with `std::fs`. Fails only when tree-sitter refuses the Python grammar.
    pub fn new<P: AsRef<Path>>(roots: &[P]) -> Result<Project, ProjectError> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .map_err(|e| ProjectError::Grammar(e.to_string()))?;
        Ok(Project {
            roots: roots.iter().map(|r| r.as_ref().to_path_buf()).collect(),
            reader: None,
            parser,
            names: HashMap::with_capacity(64),
            modules: Vec::new(),
            files: Vec::new(),
            bounds: resolve::Bounds::default(),
            mro_cache: HashMap::new(),
        })
    }

    /// [`Project::new`] reading files through `reader`, which may confine them.
    pub fn with_reader<P: AsRef<Path>>(
        roots: &[P],
        reader: FileReader,
    ) -> Result<Project, ProjectError> {
        let mut p = Project::new(roots)?;
        p.reader = Some(reader);
        Ok(p)
    }

    /// The source roots, as given.
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Every source file read so far, in order, with its text (line breaks
    /// normalised): a build's dependencies.
    pub fn files(&self) -> impl Iterator<Item = (&Path, &str)> {
        self.files.iter().map(|&id| {
            let m = self.module(id);
            (m.path.as_path(), m.syntax.source())
        })
    }

    /// Finds, reads, and analyses a module by dotted name, once; `None` when no root
    /// has it.
    pub fn load_module(&mut self, name: &str) -> Option<ModuleId> {
        if let Some(&found) = self.names.get(name) {
            return found;
        }
        let id = self.read_module(name);
        self.names.insert(name.to_string(), id);
        id
    }

    /// A module the project loaded.
    pub fn module(&self, id: ModuleId) -> &Arc<Module> {
        &self.modules[id.0 as usize]
    }

    /// How many modules the project has loaded (found or not).
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// A class of a module the project loaded.
    pub fn class(&self, c: ClassRef) -> &Class {
        self.module(c.module).class(c.class)
    }

    /// A binding of a module the project loaded.
    pub fn def(&self, d: DefRef) -> &Def {
        &self.module(d.module).scope(d.class).defs[d.index as usize]
    }

    fn read_module(&mut self, name: &str) -> Option<ModuleId> {
        let (path, kind) = self.locate(name)?;
        let id = ModuleId(self.modules.len() as u32);
        let mut m = Module { id, name: name.to_string(), kind, path, ..Module::default() };
        if kind == ModuleKind::Source || kind == ModuleKind::Package {
            self.parse_module(&mut m);
        }
        self.modules.push(Arc::new(m));
        Some(id)
    }

    fn parse_module(&mut self, m: &mut Module) {
        let text = match &mut self.reader {
            Some(read) => read(&m.path),
            None => std::fs::read_to_string(&m.path).ok(),
        };
        let Some(text) = text else {
            m.kind = ModuleKind::Native; // unreadable: a module whose source is unknown.
            return;
        };
        let source = match normalise_newlines(&text) {
            Cow::Borrowed(_) => text,
            Cow::Owned(s) => s,
        };
        let Some(tree) = self.parser.parse(&source, None) else {
            m.kind = ModuleKind::Native;
            return;
        };
        self.files.push(m.id);
        let root = tree.root_node();
        if syntax::deeper_than(root, MAX_SYNTAX_DEPTH) {
            m.kind = ModuleKind::Native; // not analysed: see MAX_SYNTAX_DEPTH.
            m.too_deep = true;
            m.syntax = Syntax::from_tree(&tree, source);
            return;
        }
        m.broken = root.has_error();
        m.syntax = Syntax::from_tree(&tree, source);
        drop(tree);
        scan::scan_module(m);
        analysis::analyse(m);
    }

    /// Looks for a module in each root in turn: a package folder first, then a source
    /// file, then a compiled extension, as Python's path finder does.
    fn locate(&self, name: &str) -> Option<(PathBuf, ModuleKind)> {
        if name.is_empty() || name.split('.').any(|p| p.is_empty() || p == "..") {
            return None;
        }
        let relative: PathBuf = name.split('.').collect();
        for root in &self.roots {
            let base = root.join(&relative);
            let init = base.join("__init__.py");
            if init.is_file() {
                return Some((init, ModuleKind::Package));
            }
            let mut file = base.clone().into_os_string();
            file.push(".py");
            let file = PathBuf::from(file);
            if file.is_file() {
                return Some((file, ModuleKind::Source));
            }
            if let Some(found) = native_file(&base) {
                return Some((found, ModuleKind::Native));
            }
            if base.is_dir() {
                return Some((base, ModuleKind::Namespace));
            }
        }
        None
    }

    /// Parses `s`, such as a string annotation, as an expression whose names resolve in
    /// `m`. The module returned holds the snippet's source; `None` when it is not one
    /// expression or nests past [`MAX_SYNTAX_DEPTH`].
    pub fn parse_expression(&mut self, m: &Module, s: &str) -> Option<(Module, NodeId)> {
        let source = format!("({s}\n)");
        let tree = self.parser.parse(&source, None)?;
        let root = tree.root_node();
        if root.has_error() || syntax::deeper_than(root, MAX_SYNTAX_DEPTH) {
            return None;
        }
        let syntax = Syntax::from_tree(&tree, source);
        drop(tree);
        let statement = syntax.root().named_child(0);
        if statement.kind() != "expression_statement" {
            return None;
        }
        let mut node = statement.named_child(0);
        if node.kind() == "parenthesized_expression" {
            node = node.named_child(0);
        }
        let id = node.id()?;
        let snippet = Module {
            id: m.id,
            name: m.name.clone(),
            kind: m.kind,
            path: m.path.clone(),
            syntax,
            ..Module::default()
        };
        Some((snippet, id))
    }
}

const NATIVE_SUFFIXES: [&str; 3] = [".so", ".pyd", ".dylib"];

/// Finds a compiled extension such as `name.cpython-313-darwin.so`.
fn native_file(base: &Path) -> Option<PathBuf> {
    let dir = base.parent()?;
    let stem = base.file_name()?.to_str()?;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(rest) = name.strip_prefix(stem) else { continue };
        if !rest.starts_with('.') || !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        if NATIVE_SUFFIXES.iter().any(|s| rest.ends_with(s)) {
            return Some(entry.path());
        }
    }
    None
}

/// Turns `\r\n` and `\r` into `\n`, as Python reads source.
fn normalise_newlines(text: &str) -> Cow<'_, str> {
    if !text.contains('\r') {
        return Cow::Borrowed(text);
    }
    Cow::Owned(text.replace("\r\n", "\n").replace('\r', "\n"))
}
