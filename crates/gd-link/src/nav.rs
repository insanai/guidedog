//! Navigation: the items of a toctree, as Sphinx renders it in place, and
//! the order of what stands in a document's sections. gd-site writes the sidebar and
//! the page outline from the same pieces.

use gd_env::{DocId, DocInfo, OutputFormat};
use gd_resolve::{BuildView, TreeEntry};
use guidedoc::{Fail, LinkKind, ListInfo, ListStyle, NodeId, NodeKind, SourceSpan};

use crate::only::{Tags, evaluate_only};
use crate::page::{Indexed, Page};
use crate::rewrite::Rewriter;

/// One line of navigation: a document, one of its sections, or a URL, with the items
/// below it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NavItem {
    /// The document; `None` for a URL or a generated page.
    pub doc: Option<DocId>,
    /// The section's id; empty for a document's page.
    pub anchor: String,
    /// A URL, or a generated page's file relative to the output root.
    pub url: String,
    /// What the line shows.
    pub title: String,
    /// The section number, when numbered.
    pub number: String,
    /// An object's entry, whose title is shown as code.
    pub literal: bool,
    /// A generated page (genindex, modindex, search).
    pub generated: bool,
    /// The items below it.
    pub children: Vec<NavItem>,
}

/// How deep a toctree's navigation goes and what it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NavOptions {
    /// Levels shown; 0 is unlimited.
    pub maxdepth: u32,
    /// Documents only, without their sections.
    pub titles_only: bool,
    /// Toctrees with `:hidden:` too.
    pub include_hidden: bool,
}

/// What stands directly in a section of a document's table of contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TocChild {
    /// A section, by its index in `DocInfo::sections`.
    Section(u32),
    /// An object's entry, by its index in `DocInfo::toc_objects`.
    Object(u32),
    /// A toctree, by its index in `DocInfo::toctrees`.
    Toctree(u32),
}

/// The items of toctree `tree` of document `doc`.
pub fn nav_for_toctree(
    view: &BuildView<'_>,
    doc: DocId,
    tree: u32,
    options: NavOptions,
) -> Vec<NavItem> {
    let mut nav = Nav { view, options, seen: vec![false; view.env.docs.len()] };
    nav.seen[doc.index()] = true;
    let mut items = Vec::new();
    for entry in toctree_entries(view, doc, tree) {
        nav.entry_items(entry, 1, &mut items);
    }
    items
}

/// The resolved entries of a document's toctree; none when the tables have none.
pub fn toctree_entries<'b>(view: &BuildView<'b>, doc: DocId, tree: u32) -> &'b [TreeEntry] {
    let trees = view.tables.trees.get(doc.index());
    trees.and_then(|t| t.get(tree as usize)).map_or(&[], Vec::as_slice)
}

/// A section's number from a numbered toctree, or "".
pub fn section_number<'b>(view: &BuildView<'b>, doc: DocId, section: u32) -> &'b str {
    let numbers = view.tables.secnumbers.get(doc.index());
    numbers.and_then(|n| n.get(section as usize)).map_or("", String::as_str)
}

/// Whether entries at `depth` may show what lies beneath them under `limit`; 0 is no
/// limit.
pub fn within(limit: u32, depth: u32) -> bool {
    limit == 0 || depth < limit
}

/// The depth a document's own entries end at when its entry stands at `depth`: its
/// `:tocdepth:` counts its levels from its title, as Sphinx cuts the document's table
/// of contents before toctrees take it. 0 is no limit.
pub fn doc_limit(d: &DocInfo, depth: u32) -> u32 {
    if d.tocdepth > 0 { depth + d.tocdepth - 1 } else { 0 }
}

/// Whether the `only` expressions an entry stands in keep it on the website, as Sphinx
/// keeps or drops its table of contents' only nodes.
pub fn only_shown(view: &BuildView<'_>, guard: &str) -> bool {
    let tags = Tags { kind: view.kind, format: OutputFormat::Html, tags: &view.options.tags };
    guard.is_empty() || evaluate_only(guard, &tags)
}

/// Whether a toctree entry is a document without a title: Sphinx then lists what its
/// contents hold instead, the objects it describes and its toctrees.
pub fn untitled(view: &BuildView<'_>, entry: &TreeEntry) -> bool {
    entry.doc.is_some_and(|id| {
        let d = view.env.doc(id);
        d.sections.is_empty() && d.title.is_empty()
    })
}

/// A document's top-level sections after its title's, when its first section is its
/// title.
pub fn top_sections(view: &BuildView<'_>, d: &DocInfo) -> Vec<u32> {
    if d.sections.first().is_none_or(|s| s.title != d.title) {
        return Vec::new();
    }
    (1..d.sections.len())
        .filter(|&i| {
            let s = &d.sections[i];
            s.parent.is_none() && !s.in_object && only_shown(view, &s.only)
        })
        .map(|i| i as u32)
        .collect()
}

/// What stands directly in `section` of a document (`Some(0)` also takes what precedes
/// the first section; `None` takes the top level), in the document's order, as Sphinx's
/// build_toc finds it: subsections, the entries of objects described there, and
/// toctrees. `titles_only` leaves out sections and objects; `deep_toctrees` then takes
/// the toctrees of subsections too.
pub fn toc_order(
    view: &BuildView<'_>,
    d: &DocInfo,
    section: Option<u32>,
    titles_only: bool,
    deep_toctrees: bool,
) -> Vec<TocChild> {
    let here = |s: Option<u32>| s == section || (section == Some(0) && s.is_none());
    let mut out = Vec::new();
    let (mut tree, mut object) = (0usize, 0usize);
    for i in 0..=d.sections.len() {
        let position = i as u32;
        loop {
            let next_tree = d.toctrees.get(tree).filter(|t| t.position <= position);
            let next_object = d.toc_objects.get(object).filter(|o| o.position <= position);
            let take_object = match (next_tree, next_object) {
                (None, None) => break,
                (None, Some(_)) => true,
                (Some(_), None) => false,
                (Some(_), Some(o)) => o.toctrees as usize <= tree,
            };
            if take_object {
                let o = &d.toc_objects[object];
                let shown = !titles_only && o.parent.is_none() && here(o.section);
                if shown && only_shown(view, &o.only) {
                    out.push(TocChild::Object(object as u32));
                }
                object += 1;
            } else {
                let t = &d.toctrees[tree];
                let deep = deep_toctrees && t.section >= section;
                if (here(t.section) || deep) && only_shown(view, &t.only) {
                    out.push(TocChild::Toctree(tree as u32));
                }
                tree += 1;
            }
        }
        if let Some(s) = d.sections.get(i)
            && !titles_only
            && s.parent == section
            && !s.in_object
            && only_shown(view, &s.only)
        {
            out.push(TocChild::Section(position));
        }
    }
    out
}

/// Builds navigation items, keeping the documents on the way so a cycle ends.
struct Nav<'v, 'b> {
    view: &'v BuildView<'b>,
    options: NavOptions,
    seen: Vec<bool>,
}

impl Nav<'_, '_> {
    /// An entry's items: the document's, then the other top-level sections it has (as
    /// `only` may give it), each an entry of its own at the same depth, as Sphinx lists
    /// every top-level entry of a document's contents.
    fn entry_items(&mut self, e: &TreeEntry, depth: u32, out: &mut Vec<NavItem>) {
        if untitled(self.view, e) {
            self.untitled_items(e, depth, out);
            return;
        }
        out.push(self.entry_item(e, depth));
        let Some(id) = e.doc.filter(|id| !self.seen[id.index()]) else { return };
        let d = self.view.env.doc(id);
        let limit = doc_limit(d, depth);
        self.seen[id.index()] = true;
        for i in top_sections(self.view, d) {
            let mut item = self.section_item(id, i);
            if within(self.options.maxdepth, depth) && within(limit, depth) {
                item.children = self.section_items(id, i, depth + 1, limit);
            }
            out.push(item);
        }
        self.seen[id.index()] = false;
    }

    /// One entry's item, with the document's sections beneath it.
    fn entry_item(&mut self, e: &TreeEntry, depth: u32) -> NavItem {
        let Some(id) = e.doc else {
            let title = if e.title.is_empty() { &e.url } else { &e.title };
            return NavItem {
                url: e.url.clone(),
                title: title.clone(),
                generated: e.generated,
                ..NavItem::default()
            };
        };
        let d = self.view.env.doc(id);
        let title = if e.title.is_empty() { &d.title } else { &e.title };
        let mut item = NavItem { doc: Some(id), title: title.clone(), ..NavItem::default() };
        if !d.sections.is_empty() {
            item.number = section_number(self.view, id, 0).to_string();
        }
        let limit = doc_limit(d, depth);
        if self.seen[id.index()] || !within(self.options.maxdepth, depth) || !within(limit, depth)
        {
            return item;
        }
        self.seen[id.index()] = true;
        item.children = self.section_items(id, 0, depth + 1, limit);
        self.seen[id.index()] = false;
        item
    }

    /// An untitled document's entries at the entry's depth; an explicit title names its
    /// only entry when that entry is the page itself, as Sphinx does.
    fn untitled_items(&mut self, e: &TreeEntry, depth: u32, out: &mut Vec<NavItem>) {
        let Some(id) = e.doc.filter(|id| !self.seen[id.index()]) else { return };
        let d = self.view.env.doc(id);
        self.seen[id.index()] = true;
        let start = out.len();
        for child in toc_order(self.view, d, Some(0), false, false) {
            match child {
                TocChild::Object(o) => {
                    let mut item = self.object_item(id, o, depth, doc_limit(d, depth));
                    if self.options.titles_only {
                        item.children.clear();
                    }
                    out.push(item);
                }
                TocChild::Toctree(t) => self.toctree_items(id, t, depth, out),
                TocChild::Section(_) => {}
            }
        }
        self.seen[id.index()] = false;
        if let [only] = &mut out[start..]
            && !e.title.is_empty()
            && only.doc == Some(id)
            && only.anchor.is_empty()
        {
            only.title = e.title.clone();
            only.literal = false;
        }
    }

    /// The items of a toctree within a document, unless it is hidden.
    fn toctree_items(&mut self, id: DocId, tree: u32, depth: u32, out: &mut Vec<NavItem>) {
        let hidden = self.view.env.doc(id).toctrees.get(tree as usize).is_some_and(|t| t.hidden);
        if hidden && !self.options.include_hidden {
            return;
        }
        for entry in toctree_entries(self.view, id, tree) {
            self.entry_items(entry, depth, out);
        }
    }

    /// A section's item, without children.
    fn section_item(&self, id: DocId, section: u32) -> NavItem {
        let s = &self.view.env.doc(id).sections[section as usize];
        NavItem {
            doc: Some(id),
            anchor: s.anchor.clone(),
            title: s.title.clone(),
            number: section_number(self.view, id, section).to_string(),
            ..NavItem::default()
        }
    }

    /// What stands in a section of a document, in the document's order: its
    /// subsections, the entries of its objects, and the entries of its toctrees.
    fn section_items(&mut self, id: DocId, parent: u32, depth: u32, limit: u32) -> Vec<NavItem> {
        let d = self.view.env.doc(id);
        let deeper = within(self.options.maxdepth, depth) && within(limit, depth);
        let mut out = Vec::new();
        for child in toc_order(self.view, d, Some(parent), self.options.titles_only, false) {
            match child {
                TocChild::Section(s) => {
                    let mut item = self.section_item(id, s);
                    if deeper {
                        item.children = self.section_items(id, s, depth + 1, limit);
                    }
                    out.push(item);
                }
                TocChild::Object(o) => out.push(self.object_item(id, o, depth, limit)),
                TocChild::Toctree(t) => self.toctree_items(id, t, depth, &mut out),
            }
        }
        out
    }

    /// An object's entry, with the entries of the objects its description holds when
    /// the depth allows.
    fn object_item(&self, id: DocId, object: u32, depth: u32, limit: u32) -> NavItem {
        let d = self.view.env.doc(id);
        let t = &d.toc_objects[object as usize];
        let mut item = NavItem {
            doc: Some(id),
            anchor: t.anchor.clone(),
            title: t.title.clone(),
            literal: true,
            ..NavItem::default()
        };
        if !within(self.options.maxdepth, depth) || !within(limit, depth) {
            return item;
        }
        for (k, other) in d.toc_objects.iter().enumerate().skip(object as usize + 1) {
            if other.parent == Some(object) && only_shown(self.view, &other.only) {
                item.children.push(self.object_item(id, k as u32, depth + 1, limit));
            }
        }
        item
    }
}

/// Writes a toctree's navigation into the page, as Sphinx's toctree wrapper: an
/// optional caption, then nested lists of links.
pub(crate) fn replace_toctree(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
) -> Result<(), Fail> {
    let Indexed::Toctree(tree) = page.indexed(id) else { return Ok(()) };
    let Some(info) = page.info.toctrees.get(tree as usize) else { return Ok(()) };
    let options = NavOptions {
        maxdepth: info.maxdepth,
        titles_only: info.titlesonly,
        include_hidden: info.includehidden,
    };
    let items = nav_for_toctree(&page.view, page.id, tree, options);
    let doc = rw.doc;
    let span = doc.node(id).span;
    let wrapper = rw.open(NodeKind::Container, span)?;
    rw.add_class(wrapper, "toctree-wrapper compound")?;
    // :name: names the wrapper, as Sphinx's add_name gives it the id.
    if let Some(name) = doc.custom_attribute(id, "name") {
        rw.add_custom(wrapper, "name", name)?;
    }
    if !info.caption.is_empty() {
        let caption = rw.open(NodeKind::Paragraph, span)?;
        rw.add_class(caption, "caption")?;
        rw.add_plain(&info.caption, span)?;
        rw.close(caption)?;
    }
    if !items.is_empty() {
        emit_nav_list(page, rw, &items, span)?;
    }
    rw.close(wrapper)
}

/// Writes items as a bullet list of links, nested as the items are.
fn emit_nav_list(
    page: &Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    items: &[NavItem],
    span: SourceSpan,
) -> Result<(), Fail> {
    let list = rw.open(NodeKind::List, span)?;
    rw.b.set_list(list, ListInfo { style: ListStyle::Bullet, delimiter: b'*', start: 1 })?;
    for item in items {
        let li = rw.open(NodeKind::ListItem, span)?;
        let p = rw.open(NodeKind::Paragraph, span)?;
        let url = match item.doc {
            Some(doc) => page.view.page_url(Some(page.id), doc, &item.anchor),
            None if item.generated => page.view.generated_url(Some(page.id), &item.url),
            None => item.url.clone(),
        };
        let link = rw.open_link(&url, "", LinkKind::External, span)?;
        let title = if item.number.is_empty() {
            item.title.clone()
        } else {
            format!("{}{}{}", item.number, page.config().html.secnumber_suffix, item.title)
        };
        let kind = if item.literal { NodeKind::Code } else { NodeKind::Text };
        rw.b.add_text_leaf(kind, span, &title)?;
        rw.close(link)?;
        rw.close(p)?;
        if !item.children.is_empty() {
            emit_nav_list(page, rw, &item.children, span)?;
        }
        rw.close(li)?;
    }
    rw.close(list)
}
