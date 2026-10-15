//! objects.inv, the inventory other projects' intersphinx reads (inventory.odin).

use gd_env::{DocId, LabelKind, ObjectInfo};
use gd_inventory::Item;

use crate::cpp::CppSymbolId;
use crate::tables::object_key;
use crate::view::BuildView;

/// Appends the project's `objects.inv` (Sphinx's version 2 format) to `out`: labels,
/// documents, terms and objects with their priorities (modules 0, other objects 1),
/// the C++ symbols with declarations, and the generated pages' labels.
///
/// As Sphinx's domains list them in `get_objects`: every document; every object,
/// options as `program.option`; every label once, with its title as the display
/// (anonymous labels display their name); glossary terms. Documents, labels, terms,
/// grammar tokens and the canonical names of Python objects have priority -1. Python
/// modules, Odin packages, Rust crates and modules have 0.
pub fn write_inventory(view: &BuildView<'_>, out: &mut Vec<u8>) {
    let mut items = Vec::new();
    for (id, d) in view.env.iter().filter(|(_, d)| !d.failed) {
        let page = view.page_name(&d.docname);
        let display = if d.title.is_empty() {
            view.ui.gettext("<no title>").into_owned()
        } else {
            d.title.clone()
        };
        items.push(item(
            &d.docname,
            "std:doc",
            -1,
            anchored(&page, &view.page_anchor(id, "")),
            display,
        ));
        for o in &d.objects {
            if let Some(entry) = object_item(view, id, o, &page) {
                items.push(entry);
            }
        }
        for t in &d.terms {
            let uri = anchored(&page, &view.page_anchor(id, &t.anchor));
            items.push(item(&t.name, "std:term", -1, uri, "-".into()));
        }
    }
    for (name, l) in &view.tables.labels {
        if l.kind == LabelKind::Equation {
            continue; // the math domain lists none.
        }
        let page = view.page_name(&view.doc(l.doc).docname);
        let uri = anchored(&page, &view.page_anchor(l.doc, &l.anchor));
        let display = if l.title.is_empty() { "-".into() } else { l.title.clone() };
        items.push(item(name, "std:label", -1, uri, display));
    }
    cpp_items(view, &mut items);
    generated_labels(view, &mut items);
    gd_inventory::sort_items(&mut items);
    let c = view.config;
    out.extend_from_slice(&gd_inventory::write_inventory(&c.project, &c.version, &items));
}

fn item(name: &str, kind: &str, priority: i64, uri: String, display: String) -> Item {
    Item { name: name.into(), kind: kind.into(), priority, uri, display }
}

/// An object's entry; `None` for a description a later one of the same name replaced,
/// and for an alias an object of the same name took.
fn object_item(view: &BuildView<'_>, id: DocId, o: &ObjectInfo, page: &str) -> Option<Item> {
    let held = view.tables.objects.get(&object_key(&o.domain, &o.objtype, &o.fullname));
    let own = held.is_some_and(|r| r.doc == id && r.anchor == o.anchor);
    if o.aliased && !(own && held.is_some_and(|r| r.aliased)) {
        return None;
    }
    // Sphinx's domains keep one object per name; C++ keeps every overload.
    let overridden = held.is_some_and(|r| r.objtype == o.objtype && !own);
    if !o.aliased && o.domain != "cpp" && overridden {
        return None;
    }
    let mut name = o.fullname.clone();
    let priority = match (o.domain.as_str(), o.objtype.as_str()) {
        _ if o.aliased => -1, // Sphinx lists aliases but does not search them.
        ("std", "cmdoption") => {
            if let Some((program, option)) = o.fullname.split_once(' ') {
                name = format!("{program}.{option}");
            }
            1
        }
        ("std", "token") => -1,
        ("py", "module") | ("odin", "package") | ("rust", "module") | ("rust", "crate") => 0,
        _ => 1,
    };
    let display = if o.domain == "c" { anonymous_display(&o.fullname) } else { "-".into() };
    let uri = anchored(page, &view.page_anchor(id, &o.anchor));
    Some(item(&name, &format!("{}:{}", o.domain, o.objtype), priority, uri, display))
}

/// Every C++ symbol with a declaration, as the C++ domain's `get_objects`: its full
/// name, displayed with `[anonymous]`, at its declaration's newest id.
fn cpp_items(view: &BuildView<'_>, items: &mut Vec<Item>) {
    let t = &view.tables.cpp;
    for (i, s) in t.symbols.iter().enumerate().skip(1) {
        let Some(declaration) = &s.declaration else { continue };
        let id = CppSymbolId(i as u32);
        let name = t.full_name(id, false).trim_start_matches(':').to_string();
        let display = t.full_name(id, true).trim_start_matches(':').to_string();
        let page = view.page_name(&view.doc(declaration.doc).docname);
        let uri = anchored(&page, &view.page_anchor(declaration.doc, &declaration.id));
        let display = if display == name { "-".into() } else { display };
        items.push(item(&name, &format!("cpp:{}", declaration.objtype), 1, uri, display));
    }
}

/// The labels Sphinx's standard and Python domains give the generated pages, whether
/// or not the pages have anything in them.
fn generated_labels(view: &BuildView<'_>, items: &mut Vec<Item>) {
    let labels = [
        ("genindex", "genindex.html", "Index"),
        ("search", "search.html", "Search Page"),
        ("modindex", "py-modindex.html", "Module Index"),
        ("py-modindex", "py-modindex.html", "Python Module Index"),
    ];
    for (name, uri, title) in labels {
        items.push(item(name, "std:label", -1, uri.into(), view.ui.gettext(title).into_owned()));
    }
}

fn anchored(page: &str, anchor: &str) -> String {
    if anchor.is_empty() { page.to_string() } else { format!("{page}#{anchor}") }
}

/// How Sphinx's C domain displays a name with anonymous parts (`A.@data`): each is
/// `[anonymous]`; `-` when the name has none, as it displays itself.
pub(crate) fn anonymous_display(name: &str) -> String {
    if !name.contains('@') {
        return "-".into();
    }
    let mut out = String::with_capacity(name.len() + 16);
    let mut skipping = false;
    for c in name.chars() {
        if c == '@' {
            out.push_str("[anonymous]");
            skipping = true;
        } else if skipping && c != '.' && c != ':' {
            continue;
        } else {
            skipping = false;
            out.push(c);
        }
    }
    out
}
