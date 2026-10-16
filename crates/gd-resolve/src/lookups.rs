//! Table lookups noted in a page's trace, and their answers.
//!
//! A page's references look things up in the project's tables and folders: labels,
//! terms, objects, documents, files and images. Each lookup is noted in the page's
//! `Trace`; a later build answers each again with [`lookup_answer`]: when every answer's
//! digest is the one stored, the page would come out the same.

use std::path::Path;

use gd_env::{Digest, Digester, DocId, LabelKind, Lookup, LookupKind, Trace};

use crate::tables::TargetRef;
use crate::view::{BuildView, doc_target, file_target, item_number};
use crate::xref::{Resolved, Unresolved, XrefQuery};

/// One reference being resolved: the view it is resolved against, the query, the
/// trace its lookups are noted in, and a warning about a reference that resolved.
pub(crate) struct Look<'a, 'b> {
    pub(crate) view: &'a BuildView<'b>,
    pub(crate) query: &'a XrefQuery,
    pub(crate) trace: &'a mut Trace,
    pub(crate) warning: Option<Box<Unresolved>>,
}

impl<'a, 'b> Look<'a, 'b> {
    pub(crate) fn label(&mut self, name: &str) -> Option<&'a TargetRef> {
        self.trace.note_key(LookupKind::Label, name);
        self.view.tables.labels.get(name)
    }

    pub(crate) fn term(&mut self, name: &str) -> Option<&'a TargetRef> {
        self.trace.note_key(LookupKind::Term, name);
        self.view.tables.terms.get(name)
    }

    pub(crate) fn object(&mut self, key: &str) -> Option<&'a TargetRef> {
        self.trace.note_key(LookupKind::Object, key);
        self.view.tables.objects.get(key)
    }

    pub(crate) fn doc(&mut self, docname: &str) -> Option<DocId> {
        self.trace.note_key(LookupKind::Doc, docname);
        self.view.doc_named(docname)
    }

    /// A document named in a document in folder `base`. Readers may already supply a
    /// docname, so it is tried before dropping a file suffix: a document named
    /// `notes.md` may have come from `notes.md.rst`.
    pub(crate) fn doc_target(&mut self, base: &str, target: &str) -> Option<DocId> {
        self.doc(&file_target(base, target)).or_else(|| self.doc(&doc_target(base, target)))
    }

    pub(crate) fn file(&mut self, path: &Path) -> bool {
        self.trace.note_key(LookupKind::File, &path.to_string_lossy());
        path.is_file()
    }

    /// The object of a domain (`prefix`, such as `py:`) whose name ends with `suffix`,
    /// of a type `role` may name; see [`suffix_object`].
    pub(crate) fn suffix(
        &mut self,
        prefix: &str,
        suffix: &str,
        role: &str,
    ) -> Option<&'a TargetRef> {
        self.trace.note(Lookup::suffix(prefix, suffix, role));
        suffix_object(self.view, prefix, suffix, role)
    }

    /// An object resolved at its anchor, its title left to the reference's content.
    pub(crate) fn at(target: &TargetRef) -> Resolved {
        Resolved { doc: Some(target.doc), anchor: target.anchor.clone(), ..Resolved::default() }
    }
}

/// Finds the object of a domain whose key starts with `prefix` and ends with `suffix`,
/// of a type the role (such as `py:data`) may name, as Sphinx's searchmode 1 filters by
/// `objtypes_for_role`; `""` takes any type. When several match, an object is preferred
/// to an alias, then the first by key, so every build picks the same one.
pub(crate) fn suffix_object<'t>(
    view: &BuildView<'t>,
    prefix: &str,
    suffix: &str,
    role: &str,
) -> Option<&'t TargetRef> {
    let mut best: Option<(&str, &TargetRef)> = None;
    for (key, o) in &view.tables.objects {
        if !key.starts_with(prefix) || !key.ends_with(suffix) || !role_takes(role, &o.objtype) {
            continue;
        }
        let better = match best {
            None => true,
            Some((k, b)) => {
                (b.aliased && !o.aliased) || (b.aliased == o.aliased && key.as_str() < k)
            }
        };
        if better {
            best = Some((key, o));
        }
    }
    best.map(|(_, o)| o)
}

/// The roles a domain's object type may be named by (Sphinx's `object_types`), for
/// suffix searches and `any`: `role_takes("py:meth", "method")` is true. Roles of other
/// domains, and an empty role, take any type.
pub fn role_takes(role: &str, objtype: &str) -> bool {
    let (domain, name) = role.split_once(':').unwrap_or((role, ""));
    match domain {
        "py" => py_role_takes(name, objtype),
        "odin" => odin_role_takes(name, objtype),
        "rust" => crate::rust_xref::rust_role_takes(name, objtype),
        "cpp" => crate::cpp_lookup::cpp_role_takes(name, objtype),
        _ => true,
    }
}

/// The Python domain's object types and their roles (`PythonDomain.object_types`).
fn py_role_takes(role: &str, objtype: &str) -> bool {
    if role == "obj" || role == "any" || role.is_empty() {
        return true;
    }
    match objtype {
        "function" => role == "func",
        "data" => role == "data",
        "class" | "exception" => role == "class" || role == "exc",
        "method" | "classmethod" | "staticmethod" => role == "meth",
        "attribute" => role == "attr",
        "property" => role == "attr" || role == "_prop",
        "type" => role == "type" || role == "class",
        "module" => role == "mod",
        _ => true,
    }
}

/// Whether an odin role may name an object of a type.
fn odin_role_takes(role: &str, objtype: &str) -> bool {
    match role {
        "obj" | "any" | "" => true,
        "pkg" => objtype == "package",
        "proc" => objtype == "procedure" || objtype == "procgroup",
        "type" => matches!(objtype, "struct" | "union" | "enum" | "bitset" | "bitfield" | "type"),
        _ => role == objtype,
    }
}

/// What every document's output shares (`document_settings`): the
/// output settings, the translations and reading settings (`Environment::settings`), the
/// inventories, every document's name and title (a page may test whether any page
/// exists), and in toctree order, per document: its name, anchor key, parent, number of
/// toctrees, and per section its title, anchor and number. A change of navigation
/// therefore rewrites every page. The HTML and book builders share it.
pub fn document_settings(view: &BuildView<'_>) -> Digest {
    let mut d = Digester::new();
    d.digest(&view.settings).digest(&view.env.settings).digest(&view.intersphinx.digest);
    for doc in &view.env.docs {
        d.text(&doc.docname).text(&doc.title);
    }
    for &id in &view.tables.order {
        let doc = view.doc(id);
        let parent = view.tables.parent.get(id.index()).copied().flatten();
        d.text(&doc.docname).text(view.doc_key(id));
        d.number(parent.map_or(-1, |p| i64::from(p.0))).number(doc.toctrees.len() as i64);
        let numbers = view.tables.secnumbers.get(id.index());
        for (i, s) in doc.sections.iter().enumerate() {
            let number = numbers.and_then(|n| n.get(i)).map_or("", String::as_str);
            d.text(&s.title).text(&s.anchor).text(number);
        }
    }
    d.finish()
}

/// A document page's own inputs besides `common`: its source and object, its figure
/// numbers, what it was made from, and for a page with a todolist every document with
/// todos. What its references found is in its lookups (see [`lookup_answer`]), not here.
pub fn page_inputs(view: &BuildView<'_>, id: DocId, common: &Digest) -> Digest {
    let doc = view.doc(id);
    let mut d = Digester::new();
    d.digest(common).digest(&doc.digest);
    d.digest(&doc.object.as_ref().map_or(Digest::default(), |o| o.digest));
    if let Some(numbers) = view.tables.fignumbers.get(id.index()) {
        d.texts(numbers);
    }
    if let Some(numbers) = view.tables.secnumbers.get(id.index()) {
        d.texts(numbers);
    }
    for dep in &doc.deps {
        d.digest(&dep.digest);
    }
    if doc.todolist {
        for other in view.env.docs.iter().filter(|o| !o.todos.is_empty()) {
            d.digest(&other.digest);
        }
    }
    d.finish()
}

/// Answers a lookup the page of `page` (a docname) made, against this build's tables
/// and folders, as the digest of the answer: the target's document, anchor, title,
/// kind, type, number and key for a name; the title for a document; whether a file
/// exists; the file an image resolves to; whether there is a module index; the C++
/// tree's digest. A missing target has its own answer. `Generation::reuse` compares it
/// with the digest stored when the page was written.
pub fn lookup_answer(view: &BuildView<'_>, page: &str, lookup: &Lookup) -> Digest {
    let t = view.tables;
    let mut d = Digester::new();
    d.text(&lookup.kind.letter().to_string());
    match lookup.kind {
        LookupKind::Label => target_answer(view, t.labels.get(&lookup.key), &mut d),
        LookupKind::Term => target_answer(view, t.terms.get(&lookup.key), &mut d),
        LookupKind::Object => target_answer(view, t.objects.get(&lookup.key), &mut d),
        LookupKind::Suffix => {
            let (prefix, suffix, role) = lookup.suffix_parts().unwrap_or(("", "", ""));
            target_answer(view, suffix_object(view, prefix, suffix, role), &mut d);
        }
        LookupKind::Doc => {
            match view.doc_named(&lookup.key) {
                None => d.flag(false),
                Some(id) => d.flag(true).text(&view.doc(id).title),
            };
        }
        LookupKind::File => {
            d.flag(Path::new(&lookup.key).is_file());
        }
        LookupKind::Image => {
            match view.image_file(page, &lookup.key) {
                None => d.flag(false),
                Some(file) => d
                    .flag(true)
                    .text(&file.path.to_string_lossy())
                    .text(&file.target)
                    .flag(file.path.is_file()),
            };
        }
        LookupKind::Modules => {
            d.flag(crate::modindex::has_module_index(view));
        }
        LookupKind::Cpp => {
            d.digest(&t.cpp.digest);
        }
        LookupKind::Template => {
            // HTML resolves template names through its own confined loader.
            d.text(&lookup.key);
        }
        LookupKind::Mount => {
            d.text(&lookup.key);
        }
        LookupKind::Navigation => {
            d.text(&lookup.key);
        }
        LookupKind::Postlist => {
            if let Some(spec) = gd_env::PostlistSpec::decode(&lookup.key) {
                for id in spec.select(view.env) {
                    let doc = view.doc(id);
                    d.text(&doc.docname).text(&doc.title).digest(&doc.page.digest());
                }
            }
        }
    }
    d.finish()
}

fn target_answer(view: &BuildView<'_>, target: Option<&TargetRef>, d: &mut Digester) {
    let Some(r) = target else {
        d.flag(false);
        return;
    };
    d.flag(true)
        .text(&view.doc(r.doc).docname)
        .text(&r.anchor)
        .text(&r.title)
        .text(r.kind.name())
        .text(&r.objtype)
        .flag(r.aliased)
        .text(item_number(view.env, view.tables, r.doc, &r.anchor))
        .text(view.doc_key(r.doc));
    // A section's number shows in a :numref: to it.
    if r.kind == LabelKind::Section {
        d.text(view.section_number(r.doc, &r.anchor));
    }
}
