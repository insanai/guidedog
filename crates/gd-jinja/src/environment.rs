//! The environment: options, filters, tests, globals and the template cache.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::ast::Compiled;
use crate::calls::Call;
use crate::convert::quoted;
use crate::error::{Error, ErrorKind};
use crate::lexer::LexOptions;
use crate::loader::Loader;
use crate::locals::Locals;
use crate::parser::{self, ParseOptions};
use crate::runtime::State;
use crate::stack::{
    DEFAULT_MAX_NESTING, DEFAULT_MAX_RECURSION, DEFAULT_STACK_BYTES, StackGuard, stack_position,
};
use crate::value::{CallKind, Callable, Native, Value};

/// How undefined values behave, like Jinja's Undefined classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UndefinedMode {
    /// Renders as "", fails on attribute access, calls and arithmetic.
    #[default]
    Default,
    /// Fails on every use except `is defined` and `|default`.
    Strict,
    /// Like Default, but attributes of undefined values are undefined too.
    Chainable,
    /// Like Default, but renders as `{{ name }}`.
    Debug,
}

/// Which templates HTML-escape the `{{ }}` output that is not Markup.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Autoescape {
    /// No template does.
    #[default]
    Never,
    /// Every template does.
    Always,
    /// Templates whose names end with one of these (".html") do, like Jinja's
    /// `select_autoescape()`.
    ForExtensions(Vec<String>),
}

/// Plain options; fill the ones you need with struct-update syntax:
/// `Options { autoescape: Autoescape::Always, ..Options::default() }`.
#[derive(Debug, Clone)]
pub struct Options {
    /// Which templates escape their output.
    pub autoescape: Autoescape,
    /// Drop the first newline after a block tag.
    pub trim_blocks: bool,
    /// Strip spaces and tabs before a block tag at the start of a line.
    pub lstrip_blocks: bool,
    /// Keep the source's final newline.
    pub keep_trailing_newline: bool,
    /// How undefined values behave.
    pub undefined: UndefinedMode,
    /// Where `load_template`, `extends`, `include` and `import` read from.
    pub loader: Option<Loader>,
    /// How deep macros, includes, blocks and recursive loops may nest.
    pub max_recursion: usize,
    /// Loop iterations (and `range()` sizes) per render.
    pub max_iterations: usize,
    /// Bytes of output per render, and per value a render builds (a repetition,
    /// padding, a join), checked before it is allocated; 0 means unlimited.
    pub max_output: usize,
    /// How deep template syntax nests: brackets, tags, and operator, filter, attribute
    /// and elif chains.
    pub max_nesting: usize,
    /// The machine stack parsing and rendering may use. Run on a thread whose stack is
    /// at least this plus what your caller and your filters need.
    pub stack_bytes: usize,
    /// Seeds `random` and `lipsum`; 0 picks a fixed seed. Renders are deterministic.
    pub random_seed: u64,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            autoescape: Autoescape::Never,
            trim_blocks: false,
            lstrip_blocks: false,
            keep_trailing_newline: false,
            undefined: UndefinedMode::Default,
            loader: None,
            max_recursion: DEFAULT_MAX_RECURSION,
            max_iterations: 10_000_000,
            max_output: 0,
            max_nesting: DEFAULT_MAX_NESTING,
            stack_bytes: DEFAULT_STACK_BYTES,
            random_seed: 0,
        }
    }
}

/// Options, filters, tests, globals and the template cache. An environment is not
/// thread-safe; use one per thread.
pub struct Environment {
    /// The options it was made with.
    pub options: Options,
    pub(crate) filters: HashMap<String, Native>,
    pub(crate) tests: HashMap<String, Native>,
    pub(crate) globals: HashMap<String, Value>,
    cache: RefCell<HashMap<String, Rc<Compiled>>>,
    stack: Cell<StackGuard>,
}

impl fmt::Debug for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Environment").field("options", &self.options).finish_non_exhaustive()
    }
}

impl Default for Environment {
    fn default() -> Environment {
        Environment::new(Options::default())
    }
}

/// A parsed template, ready to render. It borrows its environment.
#[derive(Debug, Clone)]
pub struct Template<'env> {
    env: &'env Environment,
    compiled: Rc<Compiled>,
}

/// Starts the environment's stack guard at the caller's frame, unless an outer call on
/// the same environment already did; dropping it stops the guard it started.
struct StackScope<'a> {
    stack: &'a Cell<StackGuard>,
    started: bool,
}

impl Drop for StackScope<'_> {
    fn drop(&mut self) {
        if self.started {
            self.stack.set(StackGuard::default());
        }
    }
}

impl Environment {
    /// Makes an environment with the built-in filters, tests and globals.
    pub fn new(options: Options) -> Environment {
        let mut env = Environment {
            options,
            filters: HashMap::with_capacity(64),
            tests: HashMap::with_capacity(48),
            globals: HashMap::with_capacity(16),
            cache: RefCell::default(),
            stack: Cell::default(),
        };
        crate::filters_text::register(&mut env);
        crate::filters_seq::register(&mut env);
        crate::filters_html::register(&mut env);
        crate::tests_builtin::register(&mut env);
        crate::globals::register(&mut env);
        env
    }

    /// Registers a filter: `call.args[0]` is the filtered value.
    pub fn add_filter(
        &mut self,
        name: &str,
        f: impl Fn(&mut Call<'_, '_>) -> Result<Value, Error> + 'static,
    ) {
        self.filters.insert(name.to_string(), Rc::new(f));
    }

    /// Registers a test: `call.args[0]` is the tested value; its truthiness is the answer.
    pub fn add_test(
        &mut self,
        name: &str,
        f: impl Fn(&mut Call<'_, '_>) -> Result<Value, Error> + 'static,
    ) {
        self.tests.insert(name.to_string(), Rc::new(f));
    }

    /// Makes a value visible to every template.
    pub fn add_global(&mut self, name: &str, value: Value) {
        self.globals.insert(name.to_string(), value);
    }

    /// Registers a native procedure as a global function.
    pub fn add_function(
        &mut self,
        name: &str,
        f: impl Fn(&mut Call<'_, '_>) -> Result<Value, Error> + 'static,
    ) {
        let function = Callable::value(Rc::from(name), CallKind::Native(Rc::new(f)), 0);
        self.add_global(name, function);
    }

    /// Forgets parsed templates, so the next load reads them again.
    pub fn clear_cache(&self) {
        self.cache.borrow_mut().clear();
    }

    /// Loads a template through the loader, cached until clear_cache.
    pub fn load_template(&self, name: &str) -> Result<Template<'_>, Error> {
        let _stack = self.begin_stack();
        Ok(Template { env: self, compiled: self.load_compiled(name)? })
    }

    /// Parses source as a template called name; it is not cached, so extends and
    /// include do not find it (see add_template).
    pub fn parse_template(&self, name: &str, source: &str) -> Result<Template<'_>, Error> {
        let _stack = self.begin_stack();
        Ok(Template { env: self, compiled: Rc::new(self.parse(name, source)?) })
    }

    /// Parses source and caches it under name, so that extends, include and import find
    /// it without a loader.
    pub fn add_template(&self, name: &str, source: &str) -> Result<(), Error> {
        let t = self.parse_template(name, source)?;
        self.cache.borrow_mut().insert(name.to_string(), t.compiled);
        Ok(())
    }

    /// Parses and renders source once, caching nothing. `ctx` is a dict value, or
    /// Undefined for none.
    pub fn render_str(&self, source: &str, ctx: &Value) -> Result<String, Error> {
        let _stack = self.begin_stack();
        let t = Rc::new(self.parse("<template>", source)?);
        self.render_compiled(&t, ctx, None)
    }

    /// Parses source and renders it once with ctx and the render's own functions and
    /// filters (`locals`), which may borrow the caller's data; caches nothing.
    pub fn render_str_with(
        &self,
        source: &str,
        ctx: &Value,
        locals: &Locals<'_>,
    ) -> Result<String, Error> {
        let _stack = self.begin_stack();
        let t = Rc::new(self.parse("<template>", source)?);
        self.render_compiled(&t, ctx, Some(locals))
    }

    /// Evaluates one expression against ctx (Jinja's `compile_expression`).
    pub fn evaluate(&self, expression: &str, ctx: &Value) -> Result<Value, Error> {
        let _stack = self.begin_stack();
        let e = parser::parse_expression(expression, self.parse_options())?;
        let mut s = State::new(self, context_dict(ctx, "<expression>")?);
        s.autoescape = self.options.autoescape == Autoescape::Always;
        let root = s.root_scope();
        Ok(crate::eval::eval(&mut s, root, &e)?)
    }

    fn begin_stack(&self) -> StackScope<'_> {
        let started = self.stack.get().base == 0;
        if started {
            self.stack.set(StackGuard { base: stack_position(), limit: self.options.stack_bytes });
        }
        StackScope { stack: &self.stack, started }
    }

    pub(crate) fn stack_guard(&self) -> StackGuard {
        self.stack.get()
    }

    fn parse_options(&self) -> ParseOptions {
        let o = &self.options;
        let lex = LexOptions {
            trim_blocks: o.trim_blocks,
            lstrip_blocks: o.lstrip_blocks,
            keep_trailing_newline: o.keep_trailing_newline,
        };
        ParseOptions { lex, max_nesting: o.max_nesting, stack: self.stack.get() }
    }

    fn parse(&self, name: &str, source: &str) -> Result<Compiled, Error> {
        let autoescape = match &self.options.autoescape {
            Autoescape::Never => false,
            Autoescape::Always => true,
            Autoescape::ForExtensions(extensions) => {
                extensions.iter().any(|e| name.ends_with(e.as_str()))
            }
        };
        parser::parse(source, name, autoescape, self.parse_options())
    }

    /// Finds a template in the cache, or loads, parses and caches it.
    pub(crate) fn load_compiled(&self, name: &str) -> Result<Rc<Compiled>, Error> {
        if let Some(t) = self.cache.borrow().get(name) {
            return Ok(t.clone());
        }
        let not_found = |message: String| Error {
            kind: ErrorKind::NotFound,
            template: name.to_string(),
            message,
            hint: "Check the name, and that the loader's directories hold it.".to_string(),
            ..Error::default()
        };
        let Some(loader) = &self.options.loader else {
            return Err(not_found(format!("template {} not found (no loader)", quoted(name))));
        };
        let source = loader
            .load(name)
            .ok_or_else(|| not_found(format!("template {} not found", quoted(name))))?;
        let t = Rc::new(self.parse(name, &source)?);
        self.cache.borrow_mut().insert(name.to_string(), t.clone());
        Ok(t)
    }

    fn render_compiled<'r>(
        &'r self,
        t: &Rc<Compiled>,
        ctx: &Value,
        locals: Option<&'r Locals<'r>>,
    ) -> Result<String, Error> {
        let mut s = State::new(self, context_dict(ctx, &t.name)?);
        s.locals = locals;
        s.current = Some(t.clone());
        let root = s.ctx;
        crate::inherit::render_in_context(&mut s, root, t)?;
        Ok(std::mem::take(&mut s.out))
    }
}

/// The dict a render context value gives: None for Undefined.
fn context_dict(ctx: &Value, name: &str) -> Result<Option<Rc<crate::value::Dict>>, Error> {
    match ctx {
        Value::Dict(d) => Ok(Some(d.clone())),
        Value::Undefined(_) => Ok(None),
        _ => Err(Error {
            kind: ErrorKind::Type,
            template: name.to_string(),
            message: "context must be a dict".to_string(),
            hint: "Build the context with Value::dict() and set its entries.".to_string(),
            ..Error::default()
        }),
    }
}

impl Template<'_> {
    /// The template's name.
    pub fn name(&self) -> &str {
        &self.compiled.name
    }

    /// Renders the template with ctx (a dict value, or Undefined for none).
    pub fn render(&self, ctx: &Value) -> Result<String, Error> {
        let _stack = self.env.begin_stack();
        self.env.render_compiled(&self.compiled, ctx, None)
    }

    /// Renders the template with ctx and the render's own functions and filters
    /// (`locals`), which may borrow the caller's data. They are visible in every
    /// template, block, macro and call block of this render and shadow the environment's
    /// globals and filters of the same name.
    pub fn render_with(&self, ctx: &Value, locals: &Locals<'_>) -> Result<String, Error> {
        let _stack = self.env.begin_stack();
        self.env.render_compiled(&self.compiled, ctx, Some(locals))
    }
}
