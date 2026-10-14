//! gd-graphdog turns Graphviz DOT source into SVG (or PNG, or laid-out DOT) by calling the
//! Graphviz C library in process: no external `dot` binary, no temporary files.
//!
//! ```no_run
//! let ctx = gd_graphdog::init()?;                  // plugins load once
//! let source = "digraph { a -> b }";
//! match gd_graphdog::render(&ctx, source, gd_graphdog::Options::default()) {
//!     Ok(svg) => std::fs::write("g.svg", svg).expect("write"),
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

mod defaults;
mod errors;
mod ffi;
mod files;
mod format;
mod log;

use std::ffi::{CStr, CString, c_char};
use std::fmt;
use std::ptr;
use std::sync::{Mutex, MutexGuard};

pub use errors::{Error, ErrorKind, error_title};
pub use format::{format_error, format_message};

/// True when Graphviz is linked statically. This port always links the shared libraries.
pub const STATIC: bool = false;

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

/// Every engine, in declaration order.
pub const ENGINES: [Engine; 8] = [
    Engine::Dot,
    Engine::Neato,
    Engine::Fdp,
    Engine::Sfdp,
    Engine::Circo,
    Engine::Twopi,
    Engine::Osage,
    Engine::Patchwork,
];

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

const FORMATS: [Format; 3] = [Format::Svg, Format::Png, Format::Dot];

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
    /// When set, receives each Graphviz warning ("using box for unknown shape boxx").
    pub warnings: Option<&'a mut Vec<String>>,
    /// When set, decides every file the graph names, called with the attribute's name (or
    /// `"src"` for an `<IMG>`) and the file name, after parsing and before layout.
    /// Otherwise Graphviz opens what the graph names, as dot does.
    pub files: Option<&'a mut FileAccess<'a>>,
    /// When set, is shown the output's size before render copies it; `false` refuses it
    /// as [`ErrorKind::OutOfMemory`].
    pub reserve: Option<&'a mut Reserve<'a>>,
}

/// The decision procedure of [`Options::files`]: called with the attribute's name (or
/// `"src"`) and the file name the graph gives.
pub type FileAccess<'a> = dyn FnMut(&str, &str) -> FileDecision + 'a;

/// The procedure of [`Options::reserve`]: called with the output's size in bytes.
pub type Reserve<'a> = dyn FnMut(usize) -> bool + 'a;

impl fmt::Debug for Options<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Options")
            .field("engine", &self.engine)
            .field("format", &self.format)
            .field("attributes", &self.attributes)
            .field("warnings", &self.warnings.is_some())
            .field("files", &self.files.is_some())
            .field("reserve", &self.reserve.is_some())
            .finish()
    }
}

/// A Graphviz context: loaded plugins and nothing else. Make one with [`init`] and keep
/// it; loading plugins is the expensive part. Dropping it frees the context; rendered
/// output stays valid.
#[derive(Debug)]
pub struct Context {
    gvc: *mut ffi::Gvc,
}

// SAFETY: the context is only touched through Graphviz calls made while holding GV_LOCK,
// so moving it to, or sharing it with, another thread cannot race.
unsafe impl Send for Context {}
// SAFETY: as above: every use through `&Context` takes GV_LOCK first.
unsafe impl Sync for Context {}

impl Drop for Context {
    fn drop(&mut self) {
        let _guard = lock();
        // SAFETY: `gvc` came from gvContext, is non-null, and is freed only here.
        unsafe { ffi::gvFreeContext(self.gvc) };
    }
}

/// Graphviz is not thread-safe, so every call into it takes this lock.
static GV_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    GV_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Creates a Graphviz context and loads the plugins listed in Graphviz's config file
/// (written by `dot -c`). Fails with [`ErrorKind::NotInitialized`] only when Graphviz
/// cannot make one.
pub fn init() -> Result<Context, Error> {
    let _guard = lock();
    // SAFETY: the lock is held; gvContext takes no arguments.
    let gvc = unsafe { ffi::gvContext() };
    if gvc.is_null() {
        return Err(Error::new(ErrorKind::NotInitialized));
    }
    Ok(Context { gvc })
}

/// Frees the context; the same as dropping it.
pub fn close(ctx: Context) {
    drop(ctx);
}

/// The linked Graphviz version, such as `"2.42.4"`, copied into a new string.
pub fn version(ctx: &Context) -> String {
    let _guard = lock();
    // SAFETY: the lock is held and `ctx.gvc` is live; the result is a NUL-terminated
    // string Graphviz owns until the context is freed, copied here at once.
    unsafe {
        let text = ffi::gvcVersion(ctx.gvc);
        if text.is_null() {
            return String::new();
        }
        CStr::from_ptr(text).to_string_lossy().into_owned()
    }
}

/// The Graphviz name of an engine (`"dot"`, `"neato"`, ...).
pub fn engine_name(engine: Engine) -> &'static str {
    match engine {
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

/// The -T name of a format (`"svg"`, `"png"`, `"dot"`).
pub fn format_name(format: Format) -> &'static str {
    match format {
        Format::Svg => "svg",
        Format::Png => "png",
        Format::Dot => "dot",
    }
}

/// Maps a Graphviz engine name to an [`Engine`], or `None` for an unknown name.
pub fn parse_engine(name: &str) -> Option<Engine> {
    ENGINES.into_iter().find(|&e| engine_name(e) == name)
}

/// Maps a -T format name to a [`Format`], or `None` for an unknown name.
pub fn parse_format(name: &str) -> Option<Format> {
    FORMATS.into_iter().find(|&f| format_name(f) == name)
}

/// A C string of `text` up to its first NUL byte, as C would read it.
pub(crate) fn c_string(text: &str) -> CString {
    let end = text.find('\0').unwrap_or(text.len());
    CString::new(&text[..end]).unwrap_or_default()
}

/// Lays out the first graph in `source` and renders it, as `dot` does without -O.
///
/// Borrows `source` for the call; returns the rendered bytes (Graphviz's own buffers are
/// already freed). Fails with an [`Error`] saying what went wrong and how to fix it: a
/// syntax or HTML-label problem with its location, a missing plugin, a refused file, or a
/// refused output size. Source text after a NUL byte is ignored, as C would.
pub fn render(ctx: &Context, source: &str, mut options: Options<'_>) -> Result<Vec<u8>, Error> {
    let text = c_string(source);
    let warnings = options.warnings.take();
    let _guard = lock();
    let session = log::begin();
    let result = render_locked(ctx, source, &text, &mut options);
    if let Some(out) = warnings {
        log::append_warnings(out);
    }
    drop(session);
    result
}

/// A parsed graph, closed when dropped.
struct Graph(*mut ffi::Agraph);

impl Drop for Graph {
    fn drop(&mut self) {
        // SAFETY: the graph came from agmemread and is closed only here, inside the lock.
        unsafe { ffi::agclose(self.0) };
    }
}

/// A graph's layout, freed when dropped.
struct Layout<'g>(*mut ffi::Gvc, &'g Graph);

impl Drop for Layout<'_> {
    fn drop(&mut self) {
        // SAFETY: gvLayout succeeded on this graph and context; freed once, in the lock.
        unsafe { ffi::gvFreeLayout(self.0, self.1.0) };
    }
}

fn render_locked(
    ctx: &Context,
    source: &str,
    text: &CStr,
    options: &mut Options<'_>,
) -> Result<Vec<u8>, Error> {
    let engine = options.engine;
    let saved = defaults::fix(options.attributes);
    // SAFETY: the lock is held and `text` is NUL-terminated.
    let g = unsafe { ffi::agmemread(text.as_ptr()) };
    defaults::restore(saved); // the graph has its own copy now
    // Older Graphviz versions can return a partial graph after a syntax error.
    if g.is_null() || log::first_error().contains("syntax error") {
        if !g.is_null() {
            drop(Graph(g));
        }
        return Err(errors::parse_failure(source));
    }
    let graph = Graph(g);
    if let Some(access) = options.files.as_deref_mut() {
        files::check(graph.0, access)?;
    }
    let engine_c = c_string(engine_name(engine));
    // SAFETY: the lock is held; context and graph are live.
    if unsafe { ffi::gvLayout(ctx.gvc, graph.0, engine_c.as_ptr()) } != 0 {
        return Err(errors::layout_failure(source, engine));
    }
    let _layout = Layout(ctx.gvc, &graph);
    // Some errors do not stop the layout: Graphviz drops a malformed HTML-like label,
    // reports it, and carries on. dot exits with status 1 then, and so does graphdog.
    if !log::first_error().is_empty() {
        return Err(errors::layout_failure(source, engine));
    }
    render_layout(ctx, &graph, options.format, options.reserve.as_deref_mut())
}

fn render_layout(
    ctx: &Context,
    graph: &Graph,
    format: Format,
    reserve: Option<&mut Reserve<'_>>,
) -> Result<Vec<u8>, Error> {
    let format_c = c_string(format_name(format));
    let mut data: *mut c_char = ptr::null_mut();
    let mut length: usize = 0; // zeroed on purpose: see gvRenderData in ffi.rs
    // SAFETY: the lock is held; the graph is laid out; data and length are valid outputs.
    let status = unsafe {
        ffi::gvRenderData(ctx.gvc, graph.0, format_c.as_ptr(), &mut data, &mut length)
    };
    let buffer = RenderData(data);
    if status != 0 {
        return Err(errors::render_failure(format));
    }
    // SAFETY: Graphviz returned `data` holding `length` bytes, or reported no output.
    let Some(rendered) = (unsafe { rendered_bytes(buffer.0, length) }) else {
        return Err(errors::render_failure(format));
    };
    if let Some(reserve) = reserve
        && !reserve(rendered.len())
    {
        return Err(Error::with(
            ErrorKind::OutOfMemory,
            "The output is larger than the memory the caller allows.".to_string(),
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
            // SAFETY: the buffer came from gvRenderData and is freed only here.
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
