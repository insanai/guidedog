//! A small project for the link pass's tests: documents read by a line-based test
//! reader that builds the node shapes of docs/manual/internals/ast.rst with guidedoc's
//! Builder, saved to an object store, with an environment and tables filled by hand.
//!
//! The reader's lines:
//! - `:name: value` fields (consecutive ones form one field list);
//! - a title underlined with `===` (level 1) or `---` (level 2) opens a section;
//! - `toctree:` a toctree; `todolist` a todolist directive;
//! - `only EXPR: text` an `only` holding a paragraph;
//! - `xref ROLE TARGET` a paragraph holding a reference showing TARGET;
//! - `graph: DOT` a graphviz code block; `todo: text` a todo admonition;
//! - anything else a paragraph.

#![allow(dead_code, clippy::unwrap_used)]

use std::path::PathBuf;

use gd_conf::{Config, Digest};
use gd_env::{
    BuildOptions, BuilderKind, DocId, DocInfo, EnglishUi, Environment, ObjectStore, PluginRun,
    SourceKind,
};
use gd_host::Budget;
use gd_resolve::{BuildView, Dates, Intersphinx, Tables};
use guidedoc::{
    AdmonitionKind, AttributeKey, Conformance, Fail, Limits, LinkInfo, LinkKind, NodeId, NodeKind,
    Policy, ReadContext, Reader, Registry, Request, SourceSpan, Workspace,
};

/// The test reader.
pub const READER: Reader = Reader {
    id: "lines",
    name: "Line fixture",
    version: "1",
    conformance: Conformance::Partial,
    read: read_lines,
    read_inline: None,
};

/// A temporary folder removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("gd-link-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir.canonicalize().unwrap())
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A project: its folder, configuration, environment, tables and everything a
/// `BuildView` borrows.
pub struct Project {
    pub dir: TempDir,
    pub config: Config,
    pub options: BuildOptions,
    pub kind: BuilderKind,
    pub env: Environment,
    pub tables: Tables,
    pub intersphinx: Intersphinx,
    pub objects: ObjectStore,
    pub registry: Registry,
    pub policy: Policy,
    pub limits: Limits,
    pub dates: Dates,
    pub plugins: PluginRun,
    pub budget: Budget,
    pub roots: Vec<PathBuf>,
}

impl Project {
    /// A project in a new folder with these documents (docname, source).
    pub fn new(name: &str, docs: &[(&str, &str)]) -> Project {
        let dir = TempDir::new(name);
        let config = Config::new(dir.0.clone(), dir.0.join("_build"));
        let objects = ObjectStore::new(gd_env::doctrees_dir(&config));
        let mut project = Project {
            roots: gd_host::canonical_roots(&gd_conf::readable_roots(&config)),
            dir,
            config,
            options: BuildOptions::default(),
            kind: BuilderKind::Html,
            env: Environment::default(),
            tables: Tables::default(),
            intersphinx: Intersphinx::default(),
            objects,
            registry: Registry {
                readers: vec![READER],
                renderers: vec![
                    guidedoc_html::RENDERER,
                    guidedoc_text::RENDERER,
                    guidedoc_typst::RENDERER,
                ],
            },
            policy: Policy::default(),
            limits: Limits::default(),
            dates: Dates::default(),
            plugins: PluginRun::default(),
            budget: Budget::new(1 << 30),
        };
        for (docname, text) in docs {
            project.add(docname, text);
        }
        project.index_tables();
        project
    }

    /// Reads a document with the test reader, saves its object, and records it.
    fn add(&mut self, docname: &str, text: &str) {
        let relative = format!("{docname}.rst");
        let path = self.dir.0.join(&relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        let mut ws = Workspace::new();
        let request =
            Request { source: guidedoc::Source { name: &relative, text }, ..Request::default() };
        let doc = guidedoc::read_document(&mut ws, &READER, &request).unwrap();
        let object = self.objects.save_object(docname, doc).unwrap();
        let title = text.lines().find(|l| !l.starts_with(':') && !l.is_empty()).unwrap_or("");
        self.env.docs.push(DocInfo {
            docname: docname.to_string(),
            relative,
            kind: SourceKind::Rst,
            object: Some(object),
            title: title.to_string(),
            ..DocInfo::default()
        });
        self.env.docs.sort_by(|a, b| a.docname.cmp(&b.docname));
    }

    /// Fills the per-document tables: names, empty numbers and trees.
    pub fn index_tables(&mut self) {
        let n = self.env.docs.len();
        self.tables.by_name = self.env.iter().map(|(id, d)| (d.docname.clone(), id)).collect();
        self.tables.order = (0..n as u32).map(DocId).collect();
        self.tables.parent = vec![None; n];
        self.tables.trees = self.env.docs.iter().map(|d| vec![vec![]; d.toctrees.len()]).collect();
        self.tables.secnumbers =
            self.env.docs.iter().map(|d| vec![String::new(); d.sections.len()]).collect();
        self.tables.fignumbers =
            self.env.docs.iter().map(|d| vec![String::new(); d.numbered.len()]).collect();
        self.tables.keys = self.env.docs.iter().map(|d| d.docname.replace('/', "-")).collect();
    }

    /// The id of a document.
    pub fn id(&self, docname: &str) -> DocId {
        self.env.id_of(docname).unwrap()
    }

    /// What every builder reads.
    pub fn view(&self) -> BuildView<'_> {
        BuildView {
            config: &self.config,
            options: &self.options,
            kind: self.kind,
            env: &self.env,
            tables: &self.tables,
            intersphinx: &self.intersphinx,
            objects: &self.objects,
            registry: &self.registry,
            policy: &self.policy,
            limits: &self.limits,
            ui: &EnglishUi,
            dates: &self.dates,
            plugins: &self.plugins,
            budget: &self.budget,
            roots: &self.roots,
            settings: Digest::default(),
        }
    }
}

/// Reads the line format the module comment describes.
fn read_lines(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let source = ctx.source;
    let text = ctx.source_of(source);
    let lines = line_spans(&text);
    let span = |start: usize, end: usize| SourceSpan::new(source, start as u32, end as u32);
    let mut sections: Vec<(NodeId, i32)> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let (start, line) = lines[i];
        let end = start + line.len();
        let under = lines.get(i + 1).map_or("", |l| l.1);
        let level = match under.bytes().next() {
            Some(b'=') if under.bytes().all(|c| c == b'=') => 1,
            Some(b'-') if under.bytes().all(|c| c == b'-') => 2,
            _ => 0,
        };
        if line.is_empty() {
            i += 1;
        } else if level > 0 {
            while sections.last().is_some_and(|s| s.1 >= level) {
                ctx.b.end_node(sections.pop().unwrap().0)?;
            }
            let section = ctx.b.begin_node(NodeKind::Section, span(start, end))?;
            ctx.b.set_value(section, level);
            let heading = ctx.b.begin_node(NodeKind::Heading, span(start, end))?;
            ctx.b.set_value(heading, level);
            ctx.b.add_text_leaf(NodeKind::Text, span(start, end), line)?;
            ctx.b.end_node(heading)?;
            sections.push((section, level));
            i += 2;
        } else if line.starts_with(':') {
            let list = ctx.b.begin_node(NodeKind::FieldList, span(start, end))?;
            while let Some(&(at, field)) = lines.get(i).filter(|l| l.1.starts_with(':')) {
                read_field(ctx, source, at, field)?;
                i += 1;
            }
            ctx.b.end_node(list)?;
        } else {
            read_block(ctx, span(start, end), start, line)?;
            i += 1;
        }
    }
    while let Some((section, _)) = sections.pop() {
        ctx.b.end_node(section)?;
    }
    Ok(())
}

fn line_spans(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut at = 0;
    for line in text.split('\n') {
        out.push((at, line));
        at += line.len() + 1;
    }
    out
}

/// `:name: value` as Field > FieldName, FieldBody > Paragraph.
fn read_field(
    ctx: &mut ReadContext<'_>,
    source: guidedoc::SourceId,
    at: usize,
    line: &str,
) -> Result<(), Fail> {
    let close = line[1..].find(':').map_or(line.len(), |i| i + 1);
    let name = &line[1..close];
    let value = line[(close + 1).min(line.len())..].trim_start();
    let value_start = at + line.len() - value.len();
    let s = |a: usize, b: usize| SourceSpan::new(source, a as u32, b as u32);
    let field = ctx.b.begin_node(NodeKind::Field, s(at, at + line.len()))?;
    let n = ctx.b.begin_node(NodeKind::FieldName, s(at + 1, at + close))?;
    ctx.b.add_text_leaf(NodeKind::Text, s(at + 1, at + close), name)?;
    ctx.b.end_node(n)?;
    let body_span = s(value_start, at + line.len());
    let body = ctx.b.begin_node(NodeKind::FieldBody, body_span)?;
    let p = ctx.b.begin_node(NodeKind::Paragraph, body_span)?;
    ctx.b.add_text_leaf(NodeKind::Text, body_span, value)?;
    ctx.b.end_node(p)?;
    ctx.b.end_node(body)?;
    ctx.b.end_node(field)
}

/// One line that is not a field or a title.
fn read_block(
    ctx: &mut ReadContext<'_>,
    span: SourceSpan,
    _start: usize,
    line: &str,
) -> Result<(), Fail> {
    let b = &mut ctx.b;
    if line == "toctree:" {
        let t = b.begin_node(NodeKind::Toctree, span)?;
        return b.end_node(t);
    }
    if line == "todolist" {
        let d = b.begin_node(NodeKind::Directive, span)?;
        b.add_attribute_string(d, AttributeKey::Name, "todolist")?;
        return b.end_node(d);
    }
    if let Some(dot) = line.strip_prefix("graph: ") {
        let code = b.begin_node(NodeKind::CodeBlock, span)?;
        b.add_custom_attribute(code, "graphviz", "1")?;
        let text = b.add_text(&format!("{dot}\n"))?;
        b.set_text(code, text);
        return b.end_node(code);
    }
    if let Some(rest) = line.strip_prefix("todo: ") {
        let todo = b.begin_node(NodeKind::Admonition, span)?;
        b.set_value(todo, AdmonitionKind::Todo as i32);
        paragraph(b, span, rest)?;
        return b.end_node(todo);
    }
    if let Some((expression, rest)) = line.strip_prefix("only ").and_then(|r| r.split_once(": ")) {
        let only = b.begin_node(NodeKind::Only, span)?;
        b.add_custom_attribute(only, "expression", expression)?;
        paragraph(b, span, rest)?;
        return b.end_node(only);
    }
    if let Some((role, target)) = line.strip_prefix("xref ").and_then(|r| r.split_once(' ')) {
        let p = b.begin_node(NodeKind::Paragraph, span)?;
        let x = b.begin_node(NodeKind::Xref, span)?;
        b.add_custom_attribute(x, "reftype", role)?;
        let name = b.add_text(target)?;
        b.set_link(x, LinkInfo { name, kind: LinkKind::Unresolved, ..LinkInfo::default() })?;
        b.add_text_leaf(NodeKind::Text, span, target)?;
        b.end_node(x)?;
        return b.end_node(p);
    }
    paragraph(b, span, line)
}

fn paragraph(b: &mut guidedoc::Builder<'_>, span: SourceSpan, text: &str) -> Result<(), Fail> {
    let p = b.begin_node(NodeKind::Paragraph, span)?;
    b.add_text_leaf(NodeKind::Text, span, text)?;
    b.end_node(p)
}
