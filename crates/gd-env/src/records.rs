//! The environment as JSON, written and read field by field with gd-json, and the
//! report form the manifest shares (`report_to_json`, `report_from_json`).
//!
//! Every field is written, and reading requires every field: a record from another
//! format is not half-read but refused, and the build reads everything again.

use std::path::{Path, PathBuf};

use gd_conf::Digest;
use gd_host::{Budget, Ledger, Problem};
use gd_json::{Object, Value};
use gd_report::{Excerpt, Report, Severity};

use crate::discover::SourceKind;
use crate::environment::{
    CachedFile, CppDeclInfo, Dependency, DocInfo, ENVIRONMENT_VERSION, Environment, IndexInfo,
    LabelInfo, LabelKind, ModuleInfo, NumberedInfo, ObjectInfo, RefInfo, SectionInfo,
    SourceLocation, TermInfo, TocEntry, TocObjectInfo, ToctreeInfo, TodoInfo,
};

/// The text of a record file at `path` (the environment, a manifest, a journal), read
/// within `budget`; `None` when it is missing, unreadable, or not UTF-8. Fails only when
/// the budget cannot hold it.
pub fn read_record(path: &Path, budget: &Budget) -> Result<Option<String>, Problem> {
    let ledger = Ledger::new(budget);
    match gd_host::read_file(path, Some(&ledger), usize::MAX) {
        Ok(bytes) => Ok(String::from_utf8(bytes).ok()),
        Err(p) if p.code == "host.budget" => Err(p),
        Err(_) => Ok(None),
    }
}

/// Builds a JSON object field by field, for records saved with gd-json (the
/// environment here, the manifest and the commit journal in gd-outputs).
#[derive(Debug, Default)]
pub struct ObjectBuilder(Object);

impl ObjectBuilder {
    /// Adds a string.
    pub fn text(mut self, key: &str, value: &str) -> ObjectBuilder {
        self.0.insert(key.into(), Value::from(value));
        self
    }
    /// Adds a boolean.
    pub fn flag(mut self, key: &str, value: bool) -> ObjectBuilder {
        self.0.insert(key.into(), Value::from(value));
        self
    }
    /// Adds a number.
    pub fn number(mut self, key: &str, value: u64) -> ObjectBuilder {
        self.0.insert(key.into(), Value::from(value));
        self
    }
    /// Adds a number, or null for `None`.
    pub fn option(self, key: &str, value: Option<u32>) -> ObjectBuilder {
        let mut f = self;
        f.0.insert(key.into(), value.map_or(Value::Null, Value::from));
        f
    }
    /// Adds a digest as its hexadecimal text.
    pub fn digest(self, key: &str, value: &Digest) -> ObjectBuilder {
        self.text(key, &value.to_string())
    }
    /// Adds a list of strings.
    pub fn texts(mut self, key: &str, values: &[String]) -> ObjectBuilder {
        let items = values.iter().map(|v| Value::from(v.as_str())).collect();
        self.0.insert(key.into(), Value::Array(items));
        self
    }
    /// Adds a list, each item written by `f`.
    pub fn list<T>(mut self, key: &str, items: &[T], f: fn(&T) -> Value) -> ObjectBuilder {
        self.0.insert(key.into(), Value::Array(items.iter().map(f).collect()));
        self
    }
    /// Adds a value as it is.
    pub fn value(mut self, key: &str, value: Value) -> ObjectBuilder {
        self.0.insert(key.into(), value);
        self
    }
    /// The object.
    pub fn done(self) -> Value {
        Value::Object(self.0)
    }
}

/// Reads a JSON object's fields; every getter is `None` when the field is missing or
/// of another type, so a reader is a chain of `?`.
#[derive(Debug, Clone, Copy)]
pub struct ObjectReader<'a>(pub &'a Object);

impl<'a> ObjectReader<'a> {
    /// Parses `text` as a JSON object whose `version` field is `version`, and reads it
    /// with `read`; `None` for malformed JSON, another version, or what `read` refuses.
    pub fn read_versioned<T>(
        text: &str,
        version: u64,
        read: impl FnOnce(ObjectReader<'_>) -> Option<T>,
    ) -> Option<T> {
        let value = gd_json::parse(text).ok()?;
        let r = ObjectReader::of(&value)?;
        if r.number("version")? != version {
            return None;
        }
        read(r)
    }

    /// Reads `value` when it is an object.
    pub fn of(value: &'a Value) -> Option<ObjectReader<'a>> {
        value.as_object().map(ObjectReader)
    }
    /// A string field.
    pub fn text(&self, key: &str) -> Option<String> {
        self.0.get(key)?.as_str().map(str::to_string)
    }
    /// A boolean field.
    pub fn flag(&self, key: &str) -> Option<bool> {
        self.0.get(key)?.as_bool()
    }
    /// A non-negative whole number field.
    pub fn number(&self, key: &str) -> Option<u64> {
        self.0.get(key)?.as_u64()
    }
    /// A number field that fits in a `u32`.
    pub fn small(&self, key: &str) -> Option<u32> {
        u32::try_from(self.number(key)?).ok()
    }
    /// A `u32` field or null.
    pub fn option(&self, key: &str) -> Option<Option<u32>> {
        match self.0.get(key)? {
            Value::Null => Some(None),
            v => Some(Some(u32::try_from(v.as_u64()?).ok()?)),
        }
    }
    /// A digest field.
    pub fn digest(&self, key: &str) -> Option<Digest> {
        Digest::from_hex(self.0.get(key)?.as_str()?)
    }
    /// A list of strings.
    pub fn texts(&self, key: &str) -> Option<Vec<String>> {
        let items = self.0.get(key)?.as_array()?;
        items.iter().map(|v| v.as_str().map(str::to_string)).collect()
    }
    /// A list, each item read by `f`; `None` when one is not.
    pub fn list<T>(&self, key: &str, f: fn(&Value) -> Option<T>) -> Option<Vec<T>> {
        self.0.get(key)?.as_array()?.iter().map(f).collect()
    }
    /// A field as it is.
    pub fn value(&self, key: &str) -> Option<&'a Value> {
        self.0.get(key)
    }
}

/// The environment as JSON.
pub(crate) fn environment_to_json(env: &Environment) -> String {
    let value = ObjectBuilder::default()
        .number("version", u64::from(ENVIRONMENT_VERSION))
        .digest("settings", &env.settings)
        .list("docs", &env.docs, doc_to_json)
        .list("ineligible_docs", &env.ineligible_docs, doc_to_json)
        .done();
    gd_json::to_string(&value)
}

/// Reads the environment's JSON; `None` for another version or a malformed record.
pub(crate) fn environment_from_json(text: &str) -> Option<Environment> {
    ObjectReader::read_versioned(text, u64::from(ENVIRONMENT_VERSION), |r| {
        Some(Environment {
            settings: r.digest("settings")?,
            docs: r.list("docs", doc_from_json)?,
            ineligible_docs: r.list("ineligible_docs", doc_from_json)?,
        })
    })
}

fn cached_to_json(file: &Option<CachedFile>) -> Value {
    match file {
        None => Value::Null,
        Some(f) => {
            ObjectBuilder::default().text("path", &f.path).digest("digest", &f.digest).done()
        }
    }
}

fn cached_from_json(value: &Value) -> Option<Option<CachedFile>> {
    if value.is_null() {
        return Some(None);
    }
    let r = ObjectReader::of(value)?;
    Some(Some(CachedFile { path: r.text("path")?, digest: r.digest("digest")? }))
}

fn doc_to_json(d: &DocInfo) -> Value {
    ObjectBuilder::default()
        .flag("excerpt", d.excerpt)
        .value("page", d.page.to_json())
        .text("docname", &d.docname)
        .text("relative", &d.relative)
        .text("kind", d.kind.name())
        .digest("digest", &d.digest)
        .list("deps", &d.deps, dependency_to_json)
        .value("object", cached_to_json(&d.object))
        .text("title", &d.title)
        .list("sections", &d.sections, section_to_json)
        .list("toctrees", &d.toctrees, toctree_to_json)
        .list("toc_objects", &d.toc_objects, toc_object_to_json)
        .list("labels", &d.labels, label_to_json)
        .list("terms", &d.terms, term_to_json)
        .list("objects", &d.objects, object_to_json)
        .list("index", &d.index, index_to_json)
        .list("numbered", &d.numbered, numbered_to_json)
        .texts("images", &d.images)
        .texts("downloads", &d.downloads)
        .list("refs", &d.refs, ref_to_json)
        .list("todos", &d.todos, todo_to_json)
        .flag("todolist", d.todolist)
        .list("meta", &d.meta, pair_to_json)
        .list("modules", &d.modules, module_to_json)
        .list("cpp", &d.cpp, cpp_to_json)
        .list("locations", &d.locations, location_to_json)
        .value("search", cached_to_json(&d.search))
        .flag("orphan", d.orphan)
        .number("tocdepth", u64::from(d.tocdepth))
        .flag("failed", d.failed)
        .flag("generated", d.generated)
        .list("reports", &d.reports, report_to_json)
        .done()
}

fn doc_from_json(value: &Value) -> Option<DocInfo> {
    let r = ObjectReader::of(value)?;
    Some(DocInfo {
        excerpt: r.flag("excerpt")?,
        page: crate::PageMeta::from_json(r.value("page")?)?,
        docname: r.text("docname")?,
        relative: r.text("relative")?,
        kind: SourceKind::from_name(&r.text("kind")?)?,
        digest: r.digest("digest")?,
        deps: r.list("deps", dependency_from_json)?,
        object: cached_from_json(r.value("object")?)?,
        title: r.text("title")?,
        sections: r.list("sections", section_from_json)?,
        toctrees: r.list("toctrees", toctree_from_json)?,
        toc_objects: r.list("toc_objects", toc_object_from_json)?,
        labels: r.list("labels", label_from_json)?,
        terms: r.list("terms", term_from_json)?,
        objects: r.list("objects", object_from_json)?,
        index: r.list("index", index_from_json)?,
        numbered: r.list("numbered", numbered_from_json)?,
        images: r.texts("images")?,
        downloads: r.texts("downloads")?,
        refs: r.list("refs", ref_from_json)?,
        todos: r.list("todos", todo_from_json)?,
        todolist: r.flag("todolist")?,
        meta: r.list("meta", pair_from_json)?,
        modules: r.list("modules", module_from_json)?,
        cpp: r.list("cpp", cpp_from_json)?,
        locations: r.list("locations", location_from_json)?,
        search: cached_from_json(r.value("search")?)?,
        orphan: r.flag("orphan")?,
        tocdepth: r.small("tocdepth")?,
        failed: r.flag("failed")?,
        generated: r.flag("generated")?,
        reports: r.list("reports", report_from_json)?,
        written: false,
    })
}

fn dependency_to_json(d: &Dependency) -> Value {
    ObjectBuilder::default()
        .text("path", &d.path.to_string_lossy())
        .digest("digest", &d.digest)
        .flag("included", d.included)
        .done()
}

fn dependency_from_json(v: &Value) -> Option<Dependency> {
    let r = ObjectReader::of(v)?;
    Some(Dependency {
        path: PathBuf::from(r.text("path")?),
        digest: r.digest("digest")?,
        included: r.flag("included")?,
    })
}

fn section_to_json(s: &SectionInfo) -> Value {
    ObjectBuilder::default()
        .number("level", u64::from(s.level))
        .text("title", &s.title)
        .text("anchor", &s.anchor)
        .option("parent", s.parent)
        .text("slug", &s.slug)
        .texts("ids", &s.ids)
        .flag("in_object", s.in_object)
        .text("raw_title", &s.raw_title)
        .text("only", &s.only)
        .done()
}

fn section_from_json(v: &Value) -> Option<SectionInfo> {
    let r = ObjectReader::of(v)?;
    Some(SectionInfo {
        level: r.small("level")?,
        title: r.text("title")?,
        anchor: r.text("anchor")?,
        parent: r.option("parent")?,
        slug: r.text("slug")?,
        ids: r.texts("ids")?,
        in_object: r.flag("in_object")?,
        raw_title: r.text("raw_title")?,
        only: r.text("only")?,
    })
}

fn toctree_to_json(t: &ToctreeInfo) -> Value {
    ObjectBuilder::default()
        .option("section", t.section)
        .number("position", u64::from(t.position))
        .number("line", u64::from(t.line))
        .number("maxdepth", u64::from(t.maxdepth))
        .text("caption", &t.caption)
        .flag("hidden", t.hidden)
        .flag("includehidden", t.includehidden)
        .flag("titlesonly", t.titlesonly)
        .flag("glob", t.glob)
        .flag("reversed", t.reversed)
        .number("numbered", u64::from(t.numbered))
        .list("entries", &t.entries, |e| {
            ObjectBuilder::default().text("target", &e.target).text("title", &e.title).done()
        })
        .text("only", &t.only)
        .done()
}

fn toc_entry_from_json(v: &Value) -> Option<TocEntry> {
    let r = ObjectReader::of(v)?;
    Some(TocEntry { target: r.text("target")?, title: r.text("title")? })
}

fn toctree_from_json(v: &Value) -> Option<ToctreeInfo> {
    let r = ObjectReader::of(v)?;
    Some(ToctreeInfo {
        section: r.option("section")?,
        position: r.small("position")?,
        line: r.small("line")?,
        maxdepth: r.small("maxdepth")?,
        caption: r.text("caption")?,
        hidden: r.flag("hidden")?,
        includehidden: r.flag("includehidden")?,
        titlesonly: r.flag("titlesonly")?,
        glob: r.flag("glob")?,
        reversed: r.flag("reversed")?,
        numbered: r.small("numbered")?,
        entries: r.list("entries", toc_entry_from_json)?,
        only: r.text("only")?,
    })
}

fn toc_object_to_json(t: &TocObjectInfo) -> Value {
    ObjectBuilder::default()
        .text("title", &t.title)
        .text("anchor", &t.anchor)
        .option("section", t.section)
        .option("parent", t.parent)
        .number("position", u64::from(t.position))
        .number("toctrees", u64::from(t.toctrees))
        .text("only", &t.only)
        .done()
}

fn toc_object_from_json(v: &Value) -> Option<TocObjectInfo> {
    let r = ObjectReader::of(v)?;
    Some(TocObjectInfo {
        title: r.text("title")?,
        anchor: r.text("anchor")?,
        section: r.option("section")?,
        parent: r.option("parent")?,
        position: r.small("position")?,
        toctrees: r.small("toctrees")?,
        only: r.text("only")?,
    })
}

fn label_to_json(l: &LabelInfo) -> Value {
    ObjectBuilder::default()
        .text("name", &l.name)
        .text("anchor", &l.anchor)
        .text("title", &l.title)
        .text("kind", l.kind.name())
        .done()
}

fn label_from_json(v: &Value) -> Option<LabelInfo> {
    let r = ObjectReader::of(v)?;
    Some(LabelInfo {
        name: r.text("name")?,
        anchor: r.text("anchor")?,
        title: r.text("title")?,
        kind: LabelKind::from_name(&r.text("kind")?)?,
    })
}

fn term_to_json(t: &TermInfo) -> Value {
    ObjectBuilder::default().text("name", &t.name).text("anchor", &t.anchor).done()
}

fn term_from_json(v: &Value) -> Option<TermInfo> {
    let r = ObjectReader::of(v)?;
    Some(TermInfo { name: r.text("name")?, anchor: r.text("anchor")? })
}

fn object_to_json(o: &ObjectInfo) -> Value {
    ObjectBuilder::default()
        .text("domain", &o.domain)
        .text("objtype", &o.objtype)
        .text("fullname", &o.fullname)
        .text("anchor", &o.anchor)
        .flag("aliased", o.aliased)
        .number("line", u64::from(o.line))
        .done()
}

fn object_from_json(v: &Value) -> Option<ObjectInfo> {
    let r = ObjectReader::of(v)?;
    Some(ObjectInfo {
        domain: r.text("domain")?,
        objtype: r.text("objtype")?,
        fullname: r.text("fullname")?,
        anchor: r.text("anchor")?,
        aliased: r.flag("aliased")?,
        line: r.small("line")?,
    })
}

fn index_to_json(i: &IndexInfo) -> Value {
    ObjectBuilder::default()
        .text("entries", &i.entries)
        .text("anchor", &i.anchor)
        .text("targets", &i.targets)
        .text("key", &i.key)
        .done()
}

fn index_from_json(v: &Value) -> Option<IndexInfo> {
    let r = ObjectReader::of(v)?;
    Some(IndexInfo {
        entries: r.text("entries")?,
        anchor: r.text("anchor")?,
        targets: r.text("targets")?,
        key: r.text("key")?,
    })
}

fn numbered_to_json(n: &NumberedInfo) -> Value {
    ObjectBuilder::default()
        .text("kind", n.kind.name())
        .text("anchor", &n.anchor)
        .texts("ids", &n.ids)
        .option("section", n.section)
        .number("toctrees", u64::from(n.toctrees))
        .done()
}

fn numbered_from_json(v: &Value) -> Option<NumberedInfo> {
    let r = ObjectReader::of(v)?;
    Some(NumberedInfo {
        kind: LabelKind::from_name(&r.text("kind")?)?,
        anchor: r.text("anchor")?,
        ids: r.texts("ids")?,
        section: r.option("section")?,
        toctrees: r.small("toctrees")?,
    })
}

fn ref_to_json(x: &RefInfo) -> Value {
    ObjectBuilder::default().text("reftype", &x.reftype).text("target", &x.target).done()
}

fn ref_from_json(v: &Value) -> Option<RefInfo> {
    let r = ObjectReader::of(v)?;
    Some(RefInfo { reftype: r.text("reftype")?, target: r.text("target")? })
}

fn todo_to_json(t: &TodoInfo) -> Value {
    ObjectBuilder::default()
        .number("node", u64::from(t.node))
        .text("anchor", &t.anchor)
        .number("line", u64::from(t.line))
        .done()
}

fn todo_from_json(v: &Value) -> Option<TodoInfo> {
    let r = ObjectReader::of(v)?;
    Some(TodoInfo { node: r.small("node")?, anchor: r.text("anchor")?, line: r.small("line")? })
}

fn pair_to_json(p: &(String, String)) -> Value {
    Value::Array(vec![Value::from(p.0.as_str()), Value::from(p.1.as_str())])
}

fn pair_from_json(v: &Value) -> Option<(String, String)> {
    match v.as_array()?.as_slice() {
        [a, b] => Some((a.as_str()?.to_string(), b.as_str()?.to_string())),
        _ => None,
    }
}

fn module_to_json(m: &ModuleInfo) -> Value {
    ObjectBuilder::default()
        .text("name", &m.name)
        .text("domain", &m.domain)
        .text("anchor", &m.anchor)
        .text("synopsis", &m.synopsis)
        .text("platform", &m.platform)
        .flag("deprecated", m.deprecated)
        .done()
}

fn module_from_json(v: &Value) -> Option<ModuleInfo> {
    let r = ObjectReader::of(v)?;
    Some(ModuleInfo {
        name: r.text("name")?,
        domain: r.text("domain")?,
        anchor: r.text("anchor")?,
        synopsis: r.text("synopsis")?,
        platform: r.text("platform")?,
        deprecated: r.flag("deprecated")?,
    })
}

fn location_to_json(l: &SourceLocation) -> Value {
    ObjectBuilder::default()
        .text("domain", &l.domain)
        .text("module", &l.module)
        .text("defined_in", &l.defined_in)
        .text("fullname", &l.fullname)
        .text("anchor", &l.anchor)
        .text("path", &l.path.to_string_lossy())
        .number("line", u64::from(l.line))
        .number("end_line", u64::from(l.end_line))
        .done()
}

fn location_from_json(v: &Value) -> Option<SourceLocation> {
    let r = ObjectReader::of(v)?;
    Some(SourceLocation {
        domain: r.text("domain")?,
        module: r.text("module")?,
        defined_in: r.text("defined_in")?,
        fullname: r.text("fullname")?,
        anchor: r.text("anchor")?,
        path: PathBuf::from(r.text("path")?),
        line: r.small("line")?,
        end_line: r.small("end_line")?,
    })
}

fn cpp_to_json(c: &CppDeclInfo) -> Value {
    ObjectBuilder::default()
        .text("path", &c.path)
        .text("objtype", &c.objtype)
        .text("directive", &c.directive)
        .text("id", &c.id)
        .texts("tparams", &c.tparams)
        .texts("fparams", &c.fparams)
        .number("group", u64::from(c.group))
        .number("line", u64::from(c.line))
        .done()
}

fn cpp_from_json(v: &Value) -> Option<CppDeclInfo> {
    let r = ObjectReader::of(v)?;
    Some(CppDeclInfo {
        path: r.text("path")?,
        objtype: r.text("objtype")?,
        directive: r.text("directive")?,
        id: r.text("id")?,
        tparams: r.texts("tparams")?,
        fparams: r.texts("fparams")?,
        group: r.small("group")?,
        line: r.small("line")?,
    })
}

/// A report as JSON, whole: its excerpt too, so it can be shown again exactly when the
/// document or output unit it came from is reused.
pub fn report_to_json(r: &Report) -> Value {
    let excerpt = match &r.excerpt {
        None => Value::Null,
        Some(e) => ObjectBuilder::default()
            .text("text", &e.text)
            .number("start", e.start as u64)
            .number("end", e.end as u64)
            .text("before", &e.before)
            .done(),
    };
    ObjectBuilder::default()
        .text("code", &r.code)
        .text("severity", r.severity.name())
        .text("title", &r.title)
        .text("message", &r.message)
        .texts("hints", &r.hints)
        .text("path", &r.path)
        .number("line", r.line as u64)
        .number("column", r.column as u64)
        .number("end_line", r.end_line as u64)
        .number("end_column", r.end_column as u64)
        .value("excerpt", excerpt)
        .text("category", &r.category)
        .done()
}

/// Reads a report [`report_to_json`] wrote; `None` for anything else.
pub fn report_from_json(v: &Value) -> Option<Report> {
    let r = ObjectReader::of(v)?;
    let severity = match r.text("severity")?.as_str() {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        "note" => Severity::Note,
        _ => return None,
    };
    let size = |key: &str| r.number(key).and_then(|n| usize::try_from(n).ok());
    let excerpt = match r.value("excerpt")? {
        Value::Null => None,
        e => {
            let x = ObjectReader::of(e)?;
            let start = usize::try_from(x.number("start")?).ok()?;
            let end = usize::try_from(x.number("end")?).ok()?;
            Some(Excerpt { text: x.text("text")?, start, end, before: x.text("before")? })
        }
    };
    Some(Report {
        code: r.text("code")?,
        severity,
        title: r.text("title")?,
        message: r.text("message")?,
        hints: r.texts("hints")?,
        path: r.text("path")?,
        line: size("line")?,
        column: size("column")?,
        end_line: size("end_line")?,
        end_column: size("end_column")?,
        excerpt,
        category: r.text("category")?,
    })
}
