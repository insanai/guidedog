//! The sidebar (the root document's toctrees, as Sphinx's `toctree()` template function
//! writes them) and the page outline (the "On this page" rail), from gd-link's
//! navigation pieces (nav.odin's `Toc_Writer`, `sidebar_html`, `outline_html`).

use gd_env::{DocId, DocInfo};
use gd_link::{TocChild, doc_limit, only_shown, toc_order, top_sections, untitled, within};
use gd_resolve::{BuildView, TreeEntry};

use crate::escape::push_escaped;

/// The arguments of Sphinx's `toctree()` template function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TocOptions {
    /// Levels below the root's toctrees; 0 is unlimited (Sphinx's -1).
    pub maxdepth: u32,
    /// Expand only the entries on the way to the current page.
    pub collapse: bool,
    /// Documents only, without their sections.
    pub titles_only: bool,
    /// Toctrees with `:hidden:` too.
    pub include_hidden: bool,
}

impl TocOptions {
    /// `toctree()`'s defaults in Sphinx's templates: collapsed, unlimited.
    pub const TOCTREE: TocOptions =
        TocOptions { maxdepth: 0, collapse: true, titles_only: false, include_hidden: false };

    /// The Guidedog theme's sidebar: hidden toctrees are how projects fill the
    /// navigation without repeating it on the root page, so the sidebar shows them.
    pub const SIDEBAR: TocOptions =
        TocOptions { maxdepth: 0, collapse: true, titles_only: false, include_hidden: true };
}

/// The root document's toctrees as nested lists of links, each document with its
/// sections and the toctrees within them where they stand among those sections, as
/// seen from the page of `current` (`None`: a page at the output root, such as
/// genindex.html). Entries on the way to the current page are marked `current`.
/// Empty when the project has no root document.
pub fn sidebar_html(view: &BuildView<'_>, current: Option<DocId>, options: TocOptions) -> String {
    let Some(root) = view.doc_named(&view.config.root_doc) else { return String::new() };
    let n = view.env.docs.len();
    let mut w = TocWriter {
        view,
        current,
        options,
        path: vec![false; n],
        seen: vec![false; n],
        out: String::new(),
    };
    let mut up = current;
    while let Some(id) = up {
        // A cycle in the parents cannot happen (the tables walk a tree), but a marked
        // document ends the walk anyway.
        if w.path[id.index()] {
            break;
        }
        w.path[id.index()] = true;
        up = view.tables.parent.get(id.index()).copied().flatten();
    }
    w.seen[root.index()] = true;
    for (ti, tree) in view.doc(root).toctrees.iter().enumerate() {
        if tree.hidden && !options.include_hidden {
            continue;
        }
        if !tree.caption.is_empty() {
            w.out.push_str("<p class=\"caption\" role=\"heading\"><span class=\"caption-text\">");
            push_escaped(&mut w.out, &tree.caption);
            w.out.push_str("</span></p>");
        }
        w.out.push_str("<ul>");
        for e in gd_link::toctree_entries(view, root, ti as u32) {
            w.entry(e, 1);
        }
        w.out.push_str("</ul>");
    }
    w.out
}

/// Writes one global toctree.
struct TocWriter<'v, 'b> {
    view: &'v BuildView<'b>,
    current: Option<DocId>,
    options: TocOptions,
    /// Documents from the root to the current page.
    path: Vec<bool>,
    /// Documents being expanded, so a cycle ends.
    seen: Vec<bool>,
    out: String,
}

impl TocWriter<'_, '_> {
    /// One toctree entry: a URL or generated page, or a document with what lies beneath
    /// it, followed by the document's other top-level sections at the same level.
    fn entry(&mut self, e: &TreeEntry, level: u32) {
        let Some(id) = e.doc else {
            self.item_open(level, false);
            let (class, url) = if e.generated {
                ("internal", self.view.generated_url(self.current, &e.url))
            } else {
                ("external", e.url.clone())
            };
            self.out.push_str(&format!("<a class=\"reference {class}\" href=\""));
            push_escaped(&mut self.out, &url);
            self.out.push_str("\">");
            push_escaped(&mut self.out, if e.title.is_empty() { &e.url } else { &e.title });
            self.out.push_str("</a></li>");
            return;
        };
        if untitled(self.view, e) {
            self.untitled(e, id, level);
            return;
        }
        let d = self.view.doc(id);
        let title = if e.title.is_empty() { &d.title } else { &e.title };
        let number = self.number(id, 0);
        self.link(level, id, "", &number, title, self.path[id.index()], false);
        let limit = doc_limit(d, level);
        if self.seen[id.index()] {
            self.out.push_str("</li>");
            return;
        }
        self.seen[id.index()] = true;
        if self.expands(id, level, limit) {
            self.children(id, 0, level + 1, limit);
        }
        self.out.push_str("</li>");
        for i in top_sections(self.view, d) {
            let s = &d.sections[i as usize];
            let number = self.number(id, i);
            self.link(level, id, &s.anchor, &number, &s.title, false, false);
            if self.expands(id, level, limit) {
                self.children(id, i, level + 1, limit);
            }
            self.out.push_str("</li>");
        }
        self.seen[id.index()] = false;
    }

    /// An untitled document's entries at the entry's level: its objects' entries and
    /// its toctrees; an explicit title names its only entry when that is the page.
    fn untitled(&mut self, e: &TreeEntry, id: DocId, level: u32) {
        if self.seen[id.index()] {
            return;
        }
        let d = self.view.doc(id);
        self.seen[id.index()] = true;
        let limit = doc_limit(d, level);
        let children = toc_order(self.view, d, Some(0), false, false);
        for child in &children {
            match *child {
                TocChild::Object(o) => {
                    let t = &d.toc_objects[o as usize];
                    if children.len() == 1 && !e.title.is_empty() && t.anchor.is_empty() {
                        self.link(level, id, "", "", &e.title, self.path[id.index()], false);
                        self.out.push_str("</li>");
                    } else {
                        self.object(id, o, level, limit);
                    }
                }
                TocChild::Toctree(t) => self.toctree(id, t, level),
                TocChild::Section(_) => {}
            }
        }
        self.seen[id.index()] = false;
    }

    /// The entries of a toctree within a document, unless it is hidden.
    fn toctree(&mut self, id: DocId, tree: u32, level: u32) {
        let hidden = self.view.doc(id).toctrees.get(tree as usize).is_some_and(|t| t.hidden);
        if hidden && !self.options.include_hidden {
            return;
        }
        for e in gd_link::toctree_entries(self.view, id, tree) {
            self.entry(e, level);
        }
    }

    /// Whether an entry at this level shows what lies beneath it.
    fn expands(&self, id: DocId, level: u32, limit: u32) -> bool {
        if !within(self.options.maxdepth, level) || !within(limit, level) {
            return false;
        }
        !self.options.collapse || self.path[id.index()]
    }

    /// What stands in a section, in the document's order: subsections, objects'
    /// entries, and toctrees (with `titles_only`, the toctrees anywhere below it).
    fn children(&mut self, id: DocId, section: u32, level: u32, limit: u32) {
        let d = self.view.doc(id);
        let titles_only = self.options.titles_only;
        let mut wrote = false;
        for child in toc_order(self.view, d, Some(section), titles_only, titles_only) {
            match child {
                TocChild::Section(s) => {
                    self.open_list(&mut wrote);
                    let info = &d.sections[s as usize];
                    let number = self.number(id, s);
                    self.link(level, id, &info.anchor, &number, &info.title, false, false);
                    if self.expands(id, level, limit) {
                        self.children(id, s, level + 1, limit);
                    }
                    self.out.push_str("</li>");
                }
                TocChild::Object(o) => {
                    self.open_list(&mut wrote);
                    self.object(id, o, level, limit);
                }
                TocChild::Toctree(t) => {
                    if d.toctrees[t as usize].hidden && !self.options.include_hidden {
                        continue;
                    }
                    self.open_list(&mut wrote);
                    self.toctree(id, t, level);
                }
            }
        }
        if wrote {
            self.out.push_str("</ul>");
        }
    }

    fn open_list(&mut self, wrote: &mut bool) {
        if !*wrote {
            self.out.push_str("<ul>");
        }
        *wrote = true;
    }

    /// An object's entry, its name as code, with the entries of the objects its
    /// description holds.
    fn object(&mut self, id: DocId, object: u32, level: u32, limit: u32) {
        let d = self.view.doc(id);
        let t = &d.toc_objects[object as usize];
        self.link(level, id, &t.anchor, "", &t.title, false, true);
        let mut wrote = false;
        if self.expands(id, level, limit) {
            for (k, other) in d.toc_objects.iter().enumerate().skip(object as usize + 1) {
                if other.parent != Some(object) || !only_shown(self.view, &other.only) {
                    continue;
                }
                self.open_list(&mut wrote);
                self.object(id, k as u32, level + 1, limit);
            }
        }
        if wrote {
            self.out.push_str("</ul>");
        }
        self.out.push_str("</li>");
    }

    fn item_open(&mut self, level: u32, current: bool) {
        let mark = if current { " current" } else { "" };
        self.out.push_str(&format!("<li class=\"toctree-l{level}{mark}\">"));
    }

    /// Opens an entry's item and writes its link, marked as Sphinx marks the current
    /// page and the entries leading to it.
    #[allow(clippy::too_many_arguments)] // one call shape for every kind of entry.
    fn link(
        &mut self,
        level: u32,
        id: DocId,
        anchor: &str,
        number: &str,
        title: &str,
        on_path: bool,
        literal: bool,
    ) {
        let here = Some(id) == self.current && anchor.is_empty();
        self.item_open(level, on_path);
        self.out.push_str("<a class=\"reference internal");
        self.out.push_str(if here { " current\" aria-current=\"page\"" } else { "\"" });
        self.out.push_str(" href=\"");
        push_escaped(&mut self.out, &self.view.page_url(self.current, id, anchor));
        self.out.push_str("\">");
        if !number.is_empty() {
            push_escaped(&mut self.out, number);
            push_escaped(&mut self.out, &self.view.config.html.secnumber_suffix);
        }
        if literal {
            self.out.push_str("<code>");
        }
        push_escaped(&mut self.out, title);
        if literal {
            self.out.push_str("</code>");
        }
        self.out.push_str("</a>");
    }

    /// A section's number when a numbered toctree numbers it.
    fn number(&self, id: DocId, section: u32) -> String {
        gd_link::section_number(self.view, id, section).to_string()
    }
}

/// The page's contents for the on-page rail: two levels of its sections and object
/// entries, in the document's order; `:tocdepth:` limits the levels below the title.
/// Empty when there is nothing to list.
pub fn outline_html(view: &BuildView<'_>, id: DocId) -> String {
    let d = view.doc(id);
    // Without a promoted title, the first section is the page's own title.
    let root = (d.sections.first().is_some_and(|s| s.title == d.title)).then_some(0);
    // The title is the first level; :tocdepth: limits the levels below it.
    let last = if d.tocdepth == 0 { 3 } else { d.tocdepth.min(3) };
    if last < 2 {
        return String::new();
    }
    let outline = Outline { view, d, last };
    let mut out = String::new();
    for child in toc_order(view, d, root, false, false) {
        match child {
            TocChild::Section(s) => outline.section(&mut out, s),
            TocChild::Object(o) => outline.objects(&mut out, o, 2),
            TocChild::Toctree(_) => {}
        }
    }
    // Other top-level sections stand beside the title's.
    if root.is_some() {
        for s in top_sections(view, d) {
            outline.section(&mut out, s);
        }
    }
    if out.is_empty() {
        return out;
    }
    format!("<ul>{out}</ul>")
}

/// Writes one page's outline.
struct Outline<'v, 'b> {
    view: &'v BuildView<'b>,
    d: &'v DocInfo,
    /// The deepest level listed (2 or 3).
    last: u32,
}

impl Outline<'_, '_> {
    /// A section's entry and, at the second level, what it holds.
    fn section(&self, out: &mut String, section: u32) {
        let s = &self.d.sections[section as usize];
        outline_item(out, &s.anchor, &s.title, false, 2);
        if self.last < 3 {
            return;
        }
        for sub in toc_order(self.view, self.d, Some(section), false, false) {
            match sub {
                TocChild::Section(i) => {
                    let s = &self.d.sections[i as usize];
                    outline_item(out, &s.anchor, &s.title, false, 3);
                }
                TocChild::Object(o) => self.objects(out, o, 3),
                TocChild::Toctree(_) => {}
            }
        }
    }

    /// An object's entry and, at the second level, those it holds.
    fn objects(&self, out: &mut String, object: u32, level: u32) {
        let t = &self.d.toc_objects[object as usize];
        outline_item(out, &t.anchor, &t.title, true, level);
        if level >= self.last {
            return;
        }
        for (k, other) in self.d.toc_objects.iter().enumerate().skip(object as usize + 1) {
            if other.parent == Some(object) && only_shown(self.view, &other.only) {
                self.objects(out, k as u32, level + 1);
            }
        }
    }
}

fn outline_item(out: &mut String, anchor: &str, title: &str, literal: bool, level: u32) {
    out.push_str(&format!("<li class=\"level-{level}\"><a href=\"#"));
    push_escaped(out, anchor);
    out.push_str("\">");
    if literal {
        out.push_str("<code>");
    }
    push_escaped(out, title);
    if literal {
        out.push_str("</code>");
    }
    out.push_str("</a></li>");
}
