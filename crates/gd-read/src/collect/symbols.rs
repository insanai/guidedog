//! What other documents can link to: labels, terms, objects (with the C, C++, option,
//! grammar and module rules of Sphinx's domains), index entries; and what the document
//! links to: references, images, downloads.

use gd_env::{
    CppDeclInfo, IndexInfo, LabelInfo, LabelKind, ModuleInfo, ObjectInfo, RefInfo, TermInfo,
    TocObjectInfo,
};
use guidedoc::{AttributeKey, LinkKind, NodeId, NodeKind};

use super::{Collector, count};

impl Collector<'_, '_> {
    /// An explicit label: its anchor is the id its name gave the referent; its title is
    /// the referent's section title or caption. A declared crossref type's target
    /// (Sphinx's Target directive) notes an object instead.
    pub(super) fn add_label_from_target(&mut self, id: NodeId) {
        let doc = self.doc;
        if let Some(objtype) = doc.custom_attribute(id, "objtype") {
            let object = ObjectInfo {
                domain: "std".into(),
                objtype: objtype.to_owned(),
                fullname: self.custom(id, "fullname"),
                anchor: self.anchor(id),
                aliased: false,
                line: self.source_line(id),
            };
            self.d.objects.push(object);
            return;
        }
        let name = doc.node_text(id);
        match doc.link_info(id) {
            Some(l) if l.kind == LinkKind::Internal && !name.is_empty() => {
                self.add_label(name, l.resolved);
            }
            _ => {}
        }
    }

    /// The labels a node's names give it: a custom `name`, and the Name attribute of any
    /// node but a directive or a term.
    pub(super) fn add_named(&mut self, id: NodeId) {
        let doc = self.doc;
        if let Some(name) = doc.custom_attribute(id, "name") {
            self.add_label(name, id);
        }
        let kind = doc.kind(id);
        if kind != NodeKind::Directive
            && kind != NodeKind::Term
            && let Some(name) = doc.attribute(id, AttributeKey::Name)
        {
            self.add_label(name, id);
        }
    }

    /// A label named `raw_name` on `target`: its anchor is the target's id the name made
    /// (or its first), its title and kind what Sphinx gives the referent.
    pub(super) fn add_label(&mut self, raw_name: &str, target: NodeId) {
        let name = normalize_name(raw_name);
        let mut wanted = String::new();
        guidedoc::make_id(&mut wanted, &name);
        let mut ids = self.anchors.anchor_ids(target);
        let first = ids.next().unwrap_or("");
        let anchor =
            if first == wanted { first } else { ids.find(|a| *a == wanted).unwrap_or(first) };
        let (kind, title) = self.label_kind_and_title(target);
        self.d.labels.push(LabelInfo { name, anchor: anchor.to_owned(), title, kind });
    }

    fn label_kind_and_title(&self, target: NodeId) -> (LabelKind, String) {
        let doc = self.doc;
        match doc.kind(target) {
            NodeKind::Section => {
                let heading = target + 1;
                let title = if heading.index() < doc.node_count() {
                    self.plain(heading)
                } else {
                    String::new()
                };
                (LabelKind::Section, title)
            }
            // A label before a document's only top-level section names the promoted title.
            NodeKind::Title => (LabelKind::Section, self.plain(target)),
            NodeKind::Figure => (LabelKind::Figure, self.caption_of(target, NodeKind::Caption)),
            NodeKind::Table => (LabelKind::Table, self.caption_of(target, NodeKind::Title)),
            NodeKind::CodeBlock => {
                // A graph is a figure; the caption's text without its markup, as Sphinx's
                // clean_astext gives it.
                let kind = if self.flag(target, "graphviz") {
                    LabelKind::Figure
                } else {
                    LabelKind::Code
                };
                let title = match doc.code_caption(target) {
                    Some(_) => self.caption_of(target, NodeKind::Caption),
                    None => self.custom(target, "caption"),
                };
                (kind, title)
            }
            NodeKind::MathBlock => (LabelKind::Equation, String::new()),
            NodeKind::Rubric => (LabelKind::Other, self.plain(target)),
            _ => (LabelKind::Other, self.other_label_title(target)),
        }
    }

    /// The title Sphinx gives a label on any other node: a definition list's or field
    /// list's first term or field name, or the caption of a toctree the node holds;
    /// empty leaves the label without one (Sphinx's anonymous labels).
    fn other_label_title(&self, target: NodeId) -> String {
        let doc = self.doc;
        let mut node = target;
        let first_child_if = |node: NodeId, kinds: [NodeKind; 2]| match doc.first_child(node) {
            Some(child) if kinds.contains(&doc.kind(node)) => child,
            _ => node,
        };
        node = first_child_if(node, [NodeKind::DefinitionList, NodeKind::FieldList]);
        node = first_child_if(node, [NodeKind::DefinitionItem, NodeKind::Field]);
        if matches!(doc.kind(node), NodeKind::Term | NodeKind::FieldName) {
            return self.plain(node);
        }
        let end = doc.subtree_end(target);
        let mut id = target;
        while id < end {
            if doc.kind(id) == NodeKind::Toctree {
                return self.custom(id, "caption");
            }
            id = id + 1;
        }
        String::new()
    }

    fn caption_of(&self, id: NodeId, kind: NodeKind) -> String {
        let doc = self.doc;
        doc.children(id).find(|&c| doc.kind(c) == kind).map_or_else(String::new, |c| self.plain(c))
    }

    pub(super) fn add_term(&mut self, id: NodeId) {
        if let Some(name) = self.doc.attribute(id, AttributeKey::Name) {
            let term = TermInfo { name: normalize_name(name), anchor: self.anchor(id) };
            self.d.terms.push(term);
        }
    }

    /// A signature's object, as Sphinx's domains note it: C++ declarations go to the
    /// symbol tree, options and C objects by their rules, the rest by full name (module
    /// and name within it joined by `.`, or `::` in the rust domain).
    pub(super) fn add_object(&mut self, id: NodeId) {
        let doc = self.doc;
        let Some(parent) = doc.parent_of(id) else { return };
        let domain = self.custom(parent, "domain");
        let objtype = self.custom(parent, "objtype");
        let mut fullname = self.custom(id, "fullname");
        if domain == "cpp" {
            self.add_cpp_decl(id, &objtype, parent.0); // declared even when not indexed.
            return;
        }
        if self.flag(parent, "noindex") || fullname.is_empty() {
            return;
        }
        if domain == "std" && (objtype == "option" || objtype == "cmdoption") {
            self.add_options(id);
            return;
        }
        // As in Sphinx's desc_signature, the module is kept apart from the name within it.
        if let Some(module) = doc.custom_attribute(id, "module")
            && !module.is_empty()
        {
            let separator = if domain == "rust" { "::" } else { "." };
            fullname = format!("{module}{separator}{fullname}");
        }
        if domain == "c" {
            self.add_c_object(id, &objtype, &fullname);
            return;
        }
        // A later signature of a Python object's name (an overload), or of an Odin or
        // Rust object's, has no target: only the first notes the object.
        let targeted = doc.attribute(id, AttributeKey::Id).is_some();
        if matches!(domain.as_str(), "py" | "odin" | "rust") && !targeted {
            return;
        }
        let (anchor, line) = (self.anchor(id), self.source_line(id));
        // A Python object's canonical name is noted as an alias of it (note_object with
        // aliased), which references resolve to and objects.inv lists.
        let canonical = self.custom(id, "canonical");
        let object = ObjectInfo { domain, objtype, fullname, anchor, aliased: false, line };
        if !canonical.is_empty() {
            let alias = ObjectInfo { fullname: canonical, aliased: true, ..object.clone() };
            self.d.objects.push(object);
            self.d.objects.push(alias);
        } else {
            self.d.objects.push(object);
        }
    }

    /// A C++ declaration for the project's symbol tree: its symbol path, object type,
    /// directive, newest id, and parameter names; the signatures of one directive share
    /// a group, as siblings.
    fn add_cpp_decl(&mut self, id: NodeId, directive: &str, group: u32) {
        let Some(path) = self.doc.custom_attribute(id, "cpp:path") else { return };
        let lines = |name: &str| self.custom(id, name).lines().map(str::to_owned).collect();
        let decl = CppDeclInfo {
            path: path.to_owned(),
            objtype: self.custom(id, "cpp:objtype"),
            directive: directive.to_owned(),
            id: self.custom(id, "cpp:id"),
            tparams: lines("cpp:tparams"),
            fparams: lines("cpp:fparams"),
            group,
            line: self.source_line(id),
        };
        self.d.cpp.push(decl);
    }

    /// A C declaration, as Sphinx's C domain declares its symbols: a var directive
    /// declares a member; a function's named parameters are functionParam objects at the
    /// function's id; an enumerator of an enum is also declared in the enum's scope when
    /// that name is free there.
    fn add_c_object(&mut self, id: NodeId, objtype: &str, fullname: &str) {
        let at = self.anchor(id);
        let kind = if objtype == "var" { "member" } else { objtype };
        let line = self.source_line(id);
        let object = |objtype: &str, fullname: String, line: u32| ObjectInfo {
            domain: "c".into(),
            objtype: objtype.to_owned(),
            fullname,
            anchor: at.clone(),
            aliased: false,
            line,
        };
        self.d.objects.push(object(kind, fullname.to_owned(), line));
        for name in self.custom(id, "params").lines() {
            self.d.objects.push(object("functionParam", format!("{fullname}.{name}"), 0));
        }
        if objtype != "enumerator" || self.descs.len() < 2 {
            return;
        }
        let parent = self.descs[self.descs.len() - 2].node;
        let in_enum =
            self.custom(parent, "domain") == "c" && self.custom(parent, "objtype") == "enum";
        let Some(dot) = fullname.rfind('.') else { return };
        if !in_enum {
            return;
        }
        let scope = &fullname[..dot];
        let short = &fullname[dot + 1..];
        let name = match scope.rfind('.') {
            Some(outer) => format!("{}.{short}", &scope[..outer]),
            None => short.to_owned(),
        };
        if !self.d.objects.iter().any(|o| o.domain == "c" && o.fullname == name) {
            self.d.objects.push(object("enumerator", name, 0));
        }
    }

    /// Every spelling of a command-line option under its program, as Sphinx's
    /// progoptions does: "rm -r" and "rm --recursive", or "-r" with no program. Each
    /// spelling reaches its own id. Sphinx calls the type cmdoption, whichever directive
    /// wrote it.
    fn add_options(&mut self, id: NodeId) {
        let doc = self.doc;
        let program = self.custom(id, "program");
        let names = doc
            .custom_attribute(id, "options")
            .or_else(|| doc.custom_attribute(id, "fullname"))
            .unwrap_or("");
        let ids = self.anchor_list(id);
        let anchor = self.anchor(id);
        for (i, name) in names.lines().enumerate() {
            let target = ids.get(i).cloned().unwrap_or_else(|| anchor.clone());
            self.d.objects.push(ObjectInfo {
                domain: "std".into(),
                objtype: "cmdoption".into(),
                fullname: option_key(&program, name),
                anchor: target,
                aliased: false,
                line: 0,
            });
        }
    }

    /// A signature's table of contents entry, as Sphinx's build_toc: only a signature
    /// with a toc name (the reader's `toc`) and an id, in an object without
    /// `:no-contents-entry:`. It goes under the entry of the nearest enclosing object that
    /// has one, or else in the section; every signature of one object is a sibling, and
    /// the last holds what the object's content describes. The document's first entry
    /// has no anchor.
    pub(super) fn add_toc_object(&mut self, id: NodeId) {
        let doc = self.doc;
        let name = self.custom(id, "toc");
        let has_id = doc.attribute(id, AttributeKey::Id).is_some();
        let hidden = doc.parent_of(id).is_some_and(|desc| self.flag(desc, "no-contents-entry"));
        if name.is_empty() || !has_id || hidden {
            return;
        }
        let enclosing = self.descs.len().saturating_sub(1);
        let parent = self.descs[..enclosing].iter().rev().find_map(|o| o.entry);
        let first = self.d.sections.is_empty() && self.d.toc_objects.is_empty();
        let entry = TocObjectInfo {
            title: name,
            anchor: if first { String::new() } else { self.anchor(id) },
            section: self.current_section(),
            parent,
            position: count(self.d.sections.len()),
            toctrees: count(self.d.toctrees.len()),
            only: self.guard(),
        };
        self.d.toc_objects.push(entry);
        let index = count(self.d.toc_objects.len() - 1);
        if let Some(open) = self.descs.last_mut() {
            open.entry = Some(index);
        }
    }

    pub(super) fn add_index(&mut self, id: NodeId) {
        let info = IndexInfo {
            entries: self.custom(id, "entries"),
            anchor: self.anchor(id),
            targets: self.custom(id, "targets"),
            key: self.custom(id, "key"),
        };
        self.d.index.push(info);
    }

    /// A module a domain declared: an object that :py:mod: (and the other domains'
    /// module roles) reach, and an entry of the module index. Odin's are packages.
    pub(super) fn add_module(&mut self, id: NodeId) {
        let mut m = ModuleInfo {
            name: self.custom(id, "module"),
            domain: self.custom(id, "domain"),
            anchor: self.anchor(id),
            synopsis: self.custom(id, "synopsis"),
            platform: self.custom(id, "platform"),
            deprecated: self.flag(id, "deprecated"),
        };
        if m.domain.is_empty() {
            m.domain = "py".into();
        }
        let kind = if m.domain == "odin" { "package" } else { "module" };
        self.d.objects.push(ObjectInfo {
            domain: m.domain.clone(),
            objtype: kind.into(),
            fullname: m.name.clone(),
            anchor: m.anchor.clone(),
            aliased: false,
            line: 0,
        });
        self.d.modules.push(m);
    }

    /// A grammar token, named "group:token" in a named productionlist, as Sphinx names it.
    pub(super) fn add_production(&mut self, id: NodeId) {
        let doc = self.doc;
        let name = doc.node_text(id);
        if name.is_empty() {
            return;
        }
        let group = doc.parent_of(id).map(|p| self.custom(p, "group")).unwrap_or_default();
        let fullname = if group.is_empty() { name.to_owned() } else { format!("{group}:{name}") };
        let (anchor, line) = (self.anchor(id), self.source_line(id));
        let object = ObjectInfo {
            domain: "std".into(),
            objtype: "token".into(),
            fullname,
            anchor,
            aliased: false,
            line,
        };
        self.d.objects.push(object);
    }

    /// An image the document shows from the project (not a URL or data).
    pub(super) fn add_image(&mut self, id: NodeId) {
        let doc = self.doc;
        let Some(l) = doc.link_info(id) else { return };
        let uri = doc.text(l.destination);
        if !uri.is_empty() && !uri.contains("://") && !uri.starts_with("data:") {
            self.d.images.push(uri.to_owned());
        }
    }

    pub(super) fn add_ref(&mut self, id: NodeId) {
        let doc = self.doc;
        let reftype = self.custom(id, "reftype");
        let l = doc.link_info(id).unwrap_or(guidedoc::LinkInfo::UNRESOLVED);
        let target = doc.text(l.name).to_owned();
        if reftype == "download" {
            self.d.downloads.push(target.clone());
        }
        self.d.refs.push(RefInfo { reftype, target });
    }

    /// A meta directive's fields ("description", "keywords lang=en", "property=og:title")
    /// with their contents, for the page's head.
    pub(super) fn add_meta(&mut self, id: NodeId) {
        let doc = self.doc;
        for a in doc.attributes(id).iter().filter(|a| a.key == AttributeKey::Custom) {
            self.d.meta.push((doc.text(a.name).to_owned(), doc.text(a.value).to_owned()));
        }
    }
}

/// Lowercases a label or term and collapses its white space, as Sphinx does.
pub(crate) fn normalize_name(name: &str) -> String {
    name.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// An option's name as Sphinx files it: the program, its words joined by hyphens, then
/// the option.
pub(crate) fn option_key(program: &str, option: &str) -> String {
    if program.is_empty() {
        return option.to_owned();
    }
    let words: Vec<&str> = program.split_whitespace().collect();
    format!("{} {option}", words.join("-"))
}
