//! The inputs digests of the units the HTML builders write (html_builder.odin's
//! `document_settings`, `html_settings_digest` and `page_inputs`). Every input is
//! listed in docs/manual/internals/incremental.rst; a new input changes that page too.

use gd_env::{Digest, Digester, DocId};
use gd_resolve::BuildView;

use crate::templates::Layout;

/// What every document's output shares: the output settings, the translations and
/// reading settings (`Environment::settings`), the inventories, every document's name
/// and title (a page may test whether any page exists), and in toctree order, per
/// document: its name, anchor key, parent, number of toctrees, and per section its
/// title, anchor and number. A change of navigation therefore rewrites every page.
pub(crate) fn document_settings(view: &BuildView<'_>) -> Digest {
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

/// What every page of the html and dirhtml builders shares: the document settings,
/// MathJax's configuration file, the built-in layout (used unless `templates_path` has
/// one), and every template file used. Files are hashed in chunks, never held.
pub(crate) fn html_settings(view: &BuildView<'_>, layout: &Layout) -> Digest {
    let mut d = Digester::new();
    d.digest(&document_settings(view));
    let config_path = &view.config.mathjax.config_path;
    if !config_path.is_empty() {
        let sum = gd_env::file_digest(&view.config.source_dir.join(config_path));
        d.digest(&sum.unwrap_or_default());
    }
    d.text(gd_theme::LAYOUT_HTML);
    for path in layout.files() {
        let sum = gd_env::file_digest(path).unwrap_or_default();
        d.text(&path.display().to_string()).digest(&sum);
    }
    d.finish()
}

/// A page's own inputs besides `common`: its source and object, its figure numbers,
/// what it was made from, and for a page with a todolist every document with todos.
/// What its references found is in its lookups, not here.
pub(crate) fn page_inputs(view: &BuildView<'_>, id: DocId, common: &Digest) -> Digest {
    let doc = view.doc(id);
    let mut d = Digester::new();
    d.digest(common).digest(&doc.digest);
    d.digest(&doc.object.as_ref().map_or(Digest::default(), |o| o.digest));
    if let Some(numbers) = view.tables.fignumbers.get(id.index()) {
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
