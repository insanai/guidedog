//! Renderer tests. Documents are built directly with the core builder, through a test
//! reader that runs the current test's build function; the typst CLI, when installed,
//! compiles the output to prove it valid.

mod escape;
mod math;
mod policy;
mod render;
mod sphinx;
mod stack;

use std::cell::Cell;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use guidedoc::{
    AttributeKey, Builder, Conformance, Diagnostic, Fail, Limits, LinkInfo, LinkKind, MapEntry,
    NodeId, NodeKind, Policy, ReadContext, Reader, Registry, RenderOptions, Request, Severity,
    Source, SourceSpan, Status, Text, Workspace, convert,
};

use crate::RENDERER;

/// A test's document: built with the builder shorthands of `Tb`.
pub(crate) type BuildFn = fn(&mut Tb<'_, '_>) -> Result<(), Fail>;

thread_local! {
    static BUILD: Cell<Option<BuildFn>> = const { Cell::new(None) };
}

const TEST_SOURCE: &str = "test source text";

fn test_read(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let build = BUILD.get().expect("a test sets its build function");
    build(&mut Tb { b: &mut ctx.b })
}

const TEST_READER: Reader = Reader {
    id: "test",
    name: "Test",
    version: "1",
    conformance: Conformance::Partial,
    read: test_read,
    read_inline: None,
};

/// The result of converting a built document.
#[derive(Debug)]
pub(crate) struct Rendered {
    pub status: Status,
    pub output: String,
    pub reports: Vec<Diagnostic>,
    pub source_map: Vec<MapEntry>,
}

impl Rendered {
    /// Tells whether a diagnostic with `code` and `severity` was reported.
    pub fn has_code(&self, code: &str, severity: Severity) -> bool {
        self.reports.iter().any(|d| d.code == code && d.severity == severity)
    }

    /// Asserts the output contains `part`.
    #[track_caller]
    pub fn expect(&self, part: &str) {
        assert!(self.output.contains(part), "output lacks {part:?}:\n{}", self.output);
    }
}

/// The policy most tests use: raw content refused, everything else default.
pub(crate) fn refusing() -> Policy {
    Policy::default()
}

/// Converts the document `build` makes with the "typst" renderer.
pub(crate) fn render_with(build: BuildFn, policy: Policy, options: RenderOptions<'_>) -> Rendered {
    convert_built(build, policy, options, Limits::default())
}

/// Converts standalone with the given policy and limits.
pub(crate) fn render_limited(build: BuildFn, policy: Policy, limits: Limits) -> Rendered {
    convert_built(build, policy, RenderOptions::default(), limits)
}

fn convert_built(
    build: BuildFn,
    policy: Policy,
    options: RenderOptions<'_>,
    limits: Limits,
) -> Rendered {
    BUILD.set(Some(build));
    let registry = Registry { readers: vec![TEST_READER], renderers: vec![RENDERER] };
    let request = Request {
        source: Source { name: "test.txt", text: TEST_SOURCE },
        reader: "test",
        renderer: "typst",
        policy,
        render: options,
        source_map: true,
        limits,
        ..Request::default()
    };
    let mut ws = Workspace::new();
    let (status, output) = match convert(&mut ws, &registry, &request) {
        Ok(text) => (Status::Ok, text.to_owned()),
        Err(fail) => (fail.status, String::new()),
    };
    Rendered {
        status,
        output,
        reports: ws.reports().items().to_vec(),
        source_map: ws.source_map().to_vec(),
    }
}

/// Converts standalone with the given policy.
pub(crate) fn render_policy(build: BuildFn, policy: Policy) -> Rendered {
    render_with(build, policy, RenderOptions::default())
}

/// Builder shorthands; every call propagates the builder's failure.
pub(crate) struct Tb<'t, 'b> {
    pub b: &'t mut Builder<'b>,
}

impl Tb<'_, '_> {
    pub fn open(&mut self, kind: NodeKind, value: i32) -> Result<NodeId, Fail> {
        let id = self.b.begin_node(kind, SourceSpan::default())?;
        self.b.set_value(id, value);
        Ok(id)
    }

    pub fn close(&mut self, id: NodeId) -> Result<(), Fail> {
        self.b.end_node(id)
    }

    pub fn text(&mut self, s: &str) -> Result<(), Fail> {
        self.b.add_text_leaf(NodeKind::Text, SourceSpan::default(), s).map(|_| ())
    }

    pub fn leaf(&mut self, kind: NodeKind, s: &str, value: i32) -> Result<NodeId, Fail> {
        let id = self.b.begin_node(kind, SourceSpan::default())?;
        let t = self.b.add_text(s)?;
        self.b.set_text(id, t);
        self.b.set_value(id, value);
        self.b.end_node(id)?;
        Ok(id)
    }

    pub fn set_text(&mut self, id: NodeId, s: &str) -> Result<(), Fail> {
        let t = self.b.add_text(s)?;
        self.b.set_text(id, t);
        Ok(())
    }

    pub fn para(&mut self, s: &str) -> Result<(), Fail> {
        let p = self.open(NodeKind::Paragraph, 0)?;
        self.text(s)?;
        self.close(p)
    }

    /// Opens `kind` holding the text `s` and closes it.
    pub fn wrap(&mut self, kind: NodeKind, s: &str) -> Result<NodeId, Fail> {
        let id = self.open(kind, 0)?;
        self.text(s)?;
        self.close(id)?;
        Ok(id)
    }

    pub fn attr(&mut self, id: NodeId, key: AttributeKey, value: &str) -> Result<(), Fail> {
        self.b.add_attribute_string(id, key, value)
    }

    pub fn custom(&mut self, id: NodeId, name: &str, value: &str) -> Result<(), Fail> {
        self.b.add_custom_attribute(id, name, value)
    }

    pub fn link(
        &mut self,
        id: NodeId,
        kind: LinkKind,
        destination: &str,
        name: &str,
        resolved: NodeId,
    ) -> Result<u32, Fail> {
        let destination = self.b.add_text(destination)?;
        let name = self.b.add_text(name)?;
        let info = LinkInfo { destination, name, kind, resolved, title: Text::default() };
        self.b.set_link(id, info)
    }

    /// Opens a section with its heading; close it with `close`.
    pub fn section(&mut self, level: i32, title: &str) -> Result<NodeId, Fail> {
        let s = self.open(NodeKind::Section, level)?;
        let h = self.open(NodeKind::Heading, level)?;
        self.text(title)?;
        self.close(h)?;
        Ok(s)
    }
}

/// A one-pixel PNG for image tests.
pub(crate) const PNG_PIXEL: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
    0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
    0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
    0xcf, 0xc0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0xc9, 0xfe, 0x92, 0xef, 0x00, 0x00, 0x00,
    0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// Tells whether the typst CLI is installed; tests that compile skip without it.
pub(crate) fn typst_available() -> bool {
    Command::new("typst").arg("--version").output().is_ok_and(|o| o.status.success())
}

/// A temporary directory for one compile test, holding the image tests need. Dropping it
/// removes the directory, so even a failing (panicking) test cleans up.
pub(crate) struct TempDir {
    pub path: PathBuf,
}

impl TempDir {
    /// Makes a directory unique to this process, test `name`, and call.
    pub fn new(name: &str) -> TempDir {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let unique = format!("guidedoc-typst-tests-{}-{name}-{n}", std::process::id());
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&path).expect("temporary directory");
        let dir = TempDir { path };
        std::fs::write(dir.path.join("img.png"), PNG_PIXEL).expect("image file");
        dir
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Compiles `source` as `name.typ` to PDF, or to HTML with `html`. Returns the HTML
/// text (or "") or the compiler's error output.
pub(crate) fn typst_compile(name: &str, source: &str, html: bool) -> Result<String, String> {
    let temp = TempDir::new(name);
    let dir = &temp.path;
    let typ = dir.join(format!("{name}.typ"));
    let out = dir.join(format!("{name}.{}", if html { "html" } else { "pdf" }));
    std::fs::write(&typ, source).map_err(|e| e.to_string())?;
    let mut command = Command::new("typst");
    command.arg("compile");
    if html {
        command.args(["--features", "html", "--format", "html"]);
    }
    let result = command.arg(&typ).arg(&out).output().map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).into_owned());
    }
    if html {
        return std::fs::read_to_string(&out).map_err(|e| e.to_string());
    }
    Ok(String::new())
}

/// Asserts that `source` compiles to PDF (and to HTML with `html`), when typst exists.
#[track_caller]
pub(crate) fn expect_compiles(name: &str, source: &str, html: bool) -> Option<String> {
    if !typst_available() {
        eprintln!("typst is not on PATH; skipping the compile of {name}");
        return None;
    }
    if let Err(e) = typst_compile(name, source, false) {
        panic!("typst compile of {name} failed:\n{e}");
    }
    if !html {
        return None;
    }
    match typst_compile(name, source, true) {
        Ok(text) => Some(text),
        Err(e) => panic!("typst html export of {name} failed:\n{e}"),
    }
}
