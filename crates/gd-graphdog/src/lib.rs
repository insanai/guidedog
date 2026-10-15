//! gd-graphdog turns Graphviz DOT source into SVG (or PNG, or laid-out DOT) by calling the
//! Graphviz C library in process: no external `dot` binary, no temporary files.
//!
//! ```no_run
//! let ctx = gd_graphdog::Context::new()?;          // plugins load once
//! let source = "digraph { a -> b }";
//! match ctx.render(source, gd_graphdog::Options::default()) {
//!     Ok(rendered) => std::fs::write("g.svg", rendered.output).expect("write"),
//!     Err(err) => eprint!("{}", gd_graphdog::format_error(&err, source, "g.dot")),
//! }
//! # Ok::<(), gd_graphdog::Error>(())
//! ```
//!
//! **Threads.** Graphviz keeps process-wide state (the parser, the error handler, the
//! attribute prototypes), so it is not thread-safe even across separate contexts.
//! graphdog holds one process-wide mutex around every call into Graphviz: render from as
//! many threads as you like, but renders run one at a time. The `files` and `reserve`
//! callbacks run inside that lock, so they must not render themselves.

// The crate is the binding to the Graphviz C library: every module calls it.
#![allow(unsafe_code)]

#[cfg(graphviz_static)]
mod builtin;
mod defaults;
mod errors;
mod ffi;
mod files;
mod format;
mod log;

use std::ffi::{CStr, CString, c_char};
use std::fmt;
use std::marker::PhantomData;
use std::ptr::{self, NonNull};
use std::sync::{Mutex, MutexGuard};

pub use errors::{Error, ErrorKind};
pub use format::format_error;

/// True when Graphviz is linked statically (built with `GUIDEDOG_GRAPHVIZ_STATIC` or the
/// `static-graphviz` feature): the program then needs no Graphviz installation, and its
/// plugins are the built-in dot and neato layouts and the core SVG and DOT renderers.
pub const STATIC: bool = cfg!(graphviz_static);

/// The attributes through which Graphviz reads files; [`Options::files`] decides them.
pub const FILE_ATTRIBUTES: [&str; 4] = ["image", "shapefile", "imagepath", "fontpath"];

/// Layout engines. Each one is a Graphviz layout plugin. The default is `Dot`, like the
/// dot command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Engine {
    /// Hierarchical, for directed graphs.
    #[default]
    Dot,
    /// Spring model (stress majorization).
    Neato,
    /// Spring model (force directed).
    Fdp,
    /// Multiscale force directed, for large graphs.
    Sfdp,
    /// Circular.
    Circo,
    /// Radial.
    Twopi,
    /// Clustered array packing.
    Osage,
    /// Squarified treemap.
    Patchwork,
}

impl Engine {
    /// Every engine, in declaration order.
    pub const ALL: [Engine; 8] = [
        Engine::Dot,
        Engine::Neato,
        Engine::Fdp,
        Engine::Sfdp,
        Engine::Circo,
        Engine::Twopi,
        Engine::Osage,
        Engine::Patchwork,
    ];

    /// The Graphviz name of the engine (`"dot"`, `"neato"`, ...).
    pub fn name(self) -> &'static str {
        match self {
            Engine::Dot => "dot",
            Engine::Neato => "neato",
            Engine::Fdp => "fdp",
            Engine::Sfdp => "sfdp",
            Engine::Circo => "circo",
            Engine::Twopi => "twopi",
            Engine::Osage => "osage",
            Engine::Patchwork => "patchwork",
        }
    }

    /// The engine Graphviz calls `name`, or `None` for an unknown name.
    pub fn from_name(name: &str) -> Option<Engine> {
        Engine::ALL.into_iter().find(|e| e.name() == name)
    }
}

/// Output formats. The default is `Svg`, which the core plugin always provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Format {
    /// Standalone SVG 1.1 document, UTF-8.
    #[default]
    Svg,
    /// Needs the cairo (pango) or gd plugin.
    Png,
    /// The input with layout positions attached, like `dot -Tdot`.
    Dot,
}

impl Format {
    /// Every format, in declaration order.
    pub const ALL: [Format; 3] = [Format::Svg, Format::Png, Format::Dot];

    /// The -T name of the format (`"svg"`, `"png"`, `"dot"`).
    pub fn name(self) -> &'static str {
        match self {
            Format::Svg => "svg",
            Format::Png => "png",
            Format::Dot => "dot",
        }
    }

    /// The format whose -T name is `name`, or `None` for an unknown name.
    pub fn from_name(name: &str) -> Option<Format> {
        Format::ALL.into_iter().find(|f| f.name() == name)
    }
}

/// What a default attribute applies to: -G, -N and -E on the dot command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AttributeKind {
    /// `-G`: the graph.
    #[default]
    Graph,
    /// `-N`: every node.
    Node,
    /// `-E`: every edge.
    Edge,
}

/// One -G/-N/-E default. Like dot, it overrides the graph's own defaults
/// (`node [shape=box]`) but not attributes written on a single node or edge.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Attribute {
    /// What the default applies to.
    pub kind: AttributeKind,
    /// The attribute's name, such as `"shape"`.
    pub name: String,
    /// Its value, such as `"box"`.
    pub value: String,
}

/// What [`Options::files`] decides for one file name a graph gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileDecision {
    /// Graphviz may open this path, and the output names it.
    Open(String),
    /// Leave the attribute out (set it to "").
    Omit,
    /// Refuse the whole graph: render fails with [`ErrorKind::FileRefused`].
    Refuse,
}

/// The decision procedure of [`Options::files`]: called with the attribute's name (or
/// `"src"`) and the file name the graph gives.
pub type FileAccess<'a> = dyn FnMut(&str, &str) -> FileDecision + 'a;

/// The procedure of [`Options::reserve`]: called with the output's size in bytes.
pub type Reserve<'a> = dyn FnMut(usize) -> bool + 'a;

/// How to render. Fill it with struct-update syntax:
/// `Options { engine: Engine::Neato, ..Default::default() }`.
#[derive(Default)]
pub struct Options<'a> {
    /// The layout engine.
    pub engine: Engine,
    /// The output format.
    pub format: Format,
    /// -G/-N/-E defaults.
    pub attributes: &'a [Attribute],
    /// When set, decides every file the graph names, called with the attribute's name (or
    /// `"src"` for an `<IMG>`) and the file name, after parsing and before layout.
    /// Otherwise Graphviz opens what the graph names, as dot does.
    pub files: Option<&'a mut FileAccess<'a>>,
    /// When set, is shown the output's size before render copies it; `false` refuses it
    /// as [`ErrorKind::OutOfMemory`].
    pub reserve: Option<&'a mut Reserve<'a>>,
}

impl fmt::Debug for Options<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Options")
            .field("engine", &self.engine)
            .field("format", &self.format)
            .field("attributes", &self.attributes)
            .field("files", &self.files.is_some())
            .field("reserve", &self.reserve.is_some())
            .finish()
    }
}

/// What [`Context::render`] made: the output and the warnings Graphviz gave on the way.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rendered {
    /// The rendered bytes (SVG or DOT text, or PNG data).
    pub output: Vec<u8>,
    /// Each Graphviz warning, such as "using box for unknown shape boxx".
    pub warnings: Vec<String>,
}

/// A Graphviz context: loaded plugins and nothing else. Make one with [`Context::new`]
/// and keep it; loading plugins is the expensive part. Dropping it frees the context;
/// rendered output stays valid.
#[derive(Debug)]
pub struct Context {
    gvc: NonNull<ffi::Gvc>,
}

// SAFETY: the context is only touched through Graphviz calls made while holding GV_LOCK
// (`new`, `version`, `render` and `drop` all take it first), so moving it to another
// thread cannot race with a call on the thread it came from.
unsafe impl Send for Context {}
// SAFETY: as above: every use through `&Context` takes GV_LOCK first, so shared
// references on several threads never call into Graphviz at the same time.
unsafe impl Sync for Context {}

impl Context {
    /// Creates a Graphviz context and loads its plugins: those listed in Graphviz's config
    /// file (written by `dot -c`), or the built-in ones in a static build ([`STATIC`]).
    /// Fails with [`ErrorKind::NotInitialized`] only when Graphviz cannot make one.
    pub fn new() -> Result<Context, Error> {
        let lock = lock();
        let gvc = new_gvc(&lock);
        NonNull::new(gvc).map(|gvc| Context { gvc }).ok_or(Error::new(ErrorKind::NotInitialized))
    }

    /// The linked Graphviz version, such as `"2.42.4"`, copied into a new string.
    pub fn version(&self) -> String {
        let _lock = lock();
        // SAFETY: the lock is held and `self.gvc` is live; the result is null or a
        // NUL-terminated string Graphviz owns until the context is freed, and it is
        // copied before the lock is released.
        unsafe {
            let text = ffi::gvcVersion(self.gvc.as_ptr());
            if text.is_null() {
                return String::new();
            }
            CStr::from_ptr(text).to_string_lossy().into_owned()
        }
    }

    /// Lays out the first graph in `source` and renders it, as `dot` does without -O.
    ///
    /// Borrows `source` for the call; returns the rendered bytes, which the caller owns
    /// (Graphviz's own buffers are already freed), and the warnings. Fails with an
    /// [`Error`] saying what went wrong and how to fix it: a syntax or HTML-label problem
    /// with its location, a missing plugin, a refused file, or a refused output size.
    /// Source text after a NUL byte is ignored, as C would.
    pub fn render(&self, source: &str, mut options: Options<'_>) -> Result<Rendered, Error> {
        let text = c_string(source);
        let lock = lock();
        let session = log::begin(&lock);
        let output = render_locked(&lock, self, source, &text, &mut options);
        let warnings = log::warnings();
        drop(session);
        Ok(Rendered { output: output?, warnings })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        let _lock = lock();
        // SAFETY: `gvc` came from gvContext and is freed only here; no graph or layout
        // outlives the `&Context` borrow that made it, so nothing uses it afterwards.
        unsafe { ffi::gvFreeContext(self.gvc.as_ptr()) };
    }
}

/// Graphviz is not thread-safe, so every call into it takes this lock.
static GV_LOCK: Mutex<()> = Mutex::new(());

/// Proof that the current thread holds [`GV_LOCK`]. Helpers that call Graphviz take a
/// `&Locked`, and the handles they return borrow it, so no Graphviz call or handle can
/// escape the lock.
pub(crate) struct Locked {
    _guard: MutexGuard<'static, ()>,
}

fn lock() -> Locked {
    // A panic inside a render leaves no Graphviz state half-changed that a later render
    // depends on (each render resets the log and restores the prototypes), so a poisoned
    // lock is recovered.
    Locked { _guard: GV_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner) }
}

/// A new Graphviz context with the plugins of the config file, or null.
#[cfg(not(graphviz_static))]
fn new_gvc(_lock: &Locked) -> *mut ffi::Gvc {
    // SAFETY: the lock is held; gvContext takes no arguments and returns an owned context
    // or null.
    unsafe { ffi::gvContext() }
}

/// A new Graphviz context with the built-in plugins, or null.
#[cfg(graphviz_static)]
fn new_gvc(_lock: &Locked) -> *mut ffi::Gvc {
    // SAFETY: the lock is held; the table is a static that ends with a null entry and
    // outlives every context, and 0 loads every plugin at once.
    unsafe { ffi::gvContextPlugins(builtin::builtins(), 0) }
}

/// A C string of `text` up to its first NUL byte, as C would read it.
pub(crate) fn c_string(text: &str) -> CString {
    let end = text.find('\0').unwrap_or(text.len());
    CString::new(&text[..end]).unwrap_or_default()
}

/// A parsed root graph, closed when dropped. It borrows the lock, so it is closed
/// before the lock is released.
pub(crate) struct Graph<'l> {
    ptr: NonNull<ffi::Agraph>,
    _lock: PhantomData<&'l Locked>,
}

impl<'l> Graph<'l> {
    /// Parses the first graph in `text`; `None` when there is none.
    fn read(_lock: &'l Locked, text: &CStr) -> Option<Graph<'l>> {
        // SAFETY: the lock is held and `text` is NUL-terminated; agmemread returns an
        // owned root graph or null.
        let ptr = NonNull::new(unsafe { ffi::agmemread(text.as_ptr()) })?;
        Some(Graph { ptr, _lock: PhantomData })
    }

    pub(crate) fn as_ptr(&self) -> *mut ffi::Agraph {
        self.ptr.as_ptr()
    }
}

impl Drop for Graph<'_> {
    fn drop(&mut self) {
        // SAFETY: the graph came from agmemread and is closed only here; the borrowed
        // lock is still held, and any layout of it (which borrows it) is already freed.
        unsafe { ffi::agclose(self.as_ptr()) };
    }
}

/// A graph's layout, freed when dropped; it borrows the context and the graph, so it is
/// freed before either.
struct Layout<'c, 'g, 'l> {
    ctx: &'c Context,
    graph: &'g Graph<'l>,
}

impl<'c, 'g, 'l> Layout<'c, 'g, 'l> {
    /// Lays `graph` out with `engine`; `None` when Graphviz reports a failure.
    fn new(ctx: &'c Context, graph: &'g Graph<'l>, engine: Engine) -> Option<Self> {
        let engine = c_string(engine.name());
        // SAFETY: the graph's borrow of the lock proves it is held; context and graph are
        // live and the engine name is NUL-terminated.
        let status = unsafe { ffi::gvLayout(ctx.gvc.as_ptr(), graph.as_ptr(), engine.as_ptr()) };
        (status == 0).then_some(Layout { ctx, graph })
    }
}

impl Drop for Layout<'_, '_, '_> {
    fn drop(&mut self) {
        // SAFETY: gvLayout succeeded on this graph and context, both still borrowed, and
        // the lock the graph borrows is held; the layout is freed only here.
        unsafe { ffi::gvFreeLayout(self.ctx.gvc.as_ptr(), self.graph.as_ptr()) };
    }
}

fn render_locked(
    lock: &Locked,
    ctx: &Context,
    source: &str,
    text: &CStr,
    options: &mut Options<'_>,
) -> Result<Vec<u8>, Error> {
    let fixed = defaults::fix(lock, options.attributes);
    let graph = Graph::read(lock, text);
    drop(fixed); // the graph has its own copy of the defaults now
    // Older Graphviz versions can return a partial graph after a syntax error.
    let syntax_error = log::first_error().is_some_and(|e| e.contains("syntax error"));
    let Some(graph) = graph.filter(|_| !syntax_error) else {
        return Err(errors::parse_failure(source));
    };
    if let Some(access) = options.files.as_deref_mut() {
        files::check(&graph, access)?;
    }
    let Some(layout) = Layout::new(ctx, &graph, options.engine) else {
        return Err(errors::layout_failure(source, options.engine));
    };
    // Some errors do not stop the layout: Graphviz drops a malformed HTML-like label,
    // reports it, and carries on. dot exits with status 1 then, and so does graphdog.
    if log::first_error().is_some() {
        return Err(errors::layout_failure(source, options.engine));
    }
    render_layout(&layout, options.format, options.reserve.as_deref_mut())
}

fn render_layout(
    layout: &Layout<'_, '_, '_>,
    format: Format,
    reserve: Option<&mut Reserve<'_>>,
) -> Result<Vec<u8>, Error> {
    let format_c = c_string(format.name());
    let mut data: *mut c_char = ptr::null_mut();
    let mut length: usize = 0; // zeroed on purpose: see gvRenderData in ffi.rs
    // SAFETY: the lock is held (the layout's graph borrows it); the graph is laid out
    // in this context; `data` and `length` are valid places for the outputs.
    let status = unsafe {
        let (gvc, graph) = (layout.ctx.gvc.as_ptr(), layout.graph.as_ptr());
        ffi::gvRenderData(gvc, graph, format_c.as_ptr(), &mut data, &mut length)
    };
    let buffer = RenderData(data);
    if status != 0 {
        return Err(errors::render_failure(format));
    }
    // SAFETY: Graphviz returned `data` holding `length` bytes, or reported no output;
    // the view is dropped before `buffer` frees them.
    let Some(rendered) = (unsafe { rendered_bytes(buffer.0, length) }) else {
        return Err(errors::render_failure(format));
    };
    if let Some(reserve) = reserve
        && !reserve(rendered.len())
    {
        return Err(Error::with(
            ErrorKind::OutOfMemory,
            "The output is larger than the memory the caller allows.",
        ));
    }
    let mut output = Vec::new();
    if output.try_reserve_exact(rendered.len()).is_err() {
        return Err(Error::new(ErrorKind::OutOfMemory));
    }
    output.extend_from_slice(rendered);
    Ok(output)
}

/// A gvRenderData buffer, freed when dropped.
struct RenderData(*mut c_char);

impl Drop for RenderData {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the buffer came from gvRenderData and is freed only here, inside
            // the render that holds the lock; gvFreeRenderData only calls free().
            unsafe { ffi::gvFreeRenderData(self.0) };
        }
    }
}

/// The output gvRenderData returned, as a view of Graphviz's buffer, or `None`. A result
/// Graphviz did not return, or whose length no slice can hold, is a failure to render,
/// never a read through a null or wild pointer; an empty result is empty output.
///
/// # Safety
/// When `data` is non-null and `length` fits in `isize`, `data` must point to `length`
/// readable bytes that outlive `'a`.
unsafe fn rendered_bytes<'a>(data: *const c_char, length: usize) -> Option<&'a [u8]> {
    if length == 0 {
        return Some(&[]);
    }
    if data.is_null() || length > isize::MAX as usize {
        return None;
    }
    // SAFETY: the caller guarantees `length` readable bytes at the non-null `data`.
    Some(unsafe { std::slice::from_raw_parts(data.cast::<u8>(), length) })
}

#[cfg(test)]
mod tests;
