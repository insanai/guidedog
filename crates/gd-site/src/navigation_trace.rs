//! HTML records the navigation values actually read by a template and its native calls.

use gd_env::{Digest, Digester, Lookup, LookupKind};
use gd_link::{NavItem, NavOptions};
use gd_resolve::BuildView;

use crate::context::{Page, page_context};
use crate::nav::{TocOptions, sidebar_html};

pub(crate) fn context_key(name: &str) -> bool {
    matches!(name, "toc" | "outline" | "prev" | "next" | "parents" | "rellinks" | "root_url")
}

pub(crate) fn lookup(key: &str) -> Lookup {
    Lookup::new(LookupKind::Navigation, key)
}

pub(crate) fn answer(
    view: &BuildView<'_>,
    page: &str,
    lookup: &Lookup,
    articles: &crate::article_context::ArticleIndex,
) -> Digest {
    let mut digest = Digester::new();
    let id = view.doc_named(page);
    let Some((kind, key)) = lookup.key.split_once('\0') else { return digest.finish() };
    match kind {
        // Keep old whole-page traces valid while new renders record individual neighbours.
        "ctx" | "page" if key == "page" || matches!(key, "older" | "newer") => {
            if let Some(id) = id {
                digest.digest(&crate::article_context::inputs(view, id, articles, key));
            }
        }
        "ctx" if context_key(key) => {
            let page = id.map_or(Page::Special(page), Page::Doc);
            let (ctx, _) = page_context(view, page, "", articles);
            digest.text(&format!("{:?}", ctx.get(key)));
        }
        "hasdoc" => {
            digest.flag(view.doc_named(key).is_some());
        }
        "pathto" => {
            let Some((resource, target)) = key.split_once('\0') else { return digest.finish() };
            let page = id.map_or(Page::Special(page), Page::Doc);
            let (_, state) = page_context(view, page, "", articles);
            digest.text(&crate::templates::path_to(view, &state, target, resource == "true"));
        }
        "toctree" => {
            let parts: Vec<_> = key.split('/').collect();
            if parts.len() == 4 {
                let options = TocOptions {
                    maxdepth: parts[0].parse().unwrap_or(0),
                    collapse: parts[1] == "true",
                    titles_only: parts[2] == "true",
                    include_hidden: parts[3] == "true",
                };
                digest.text(&sidebar_html(view, id, options));
            }
        }
        "tree" => {
            if let Some((id, index, info)) = id.and_then(|id| {
                let index: usize = key.parse().ok()?;
                Some((id, index, view.doc(id).toctrees.get(index)?))
            }) {
                let options = NavOptions {
                    maxdepth: info.maxdepth,
                    titles_only: info.titlesonly,
                    include_hidden: info.includehidden,
                };
                let items = gd_link::nav_for_toctree(view, id, index as u32, options);
                hash_items(view, &items, &mut digest);
            }
        }
        _ => {}
    }
    digest.finish()
}

fn hash_items(view: &BuildView<'_>, items: &[NavItem], digest: &mut Digester) {
    // An explicit work list preserves child boundaries without recursive hashing.
    let mut pending = vec![items];
    while let Some(items) = pending.pop() {
        digest.number(items.len() as i64);
        for item in items {
            digest.text(item.doc.map_or("", |id| view.doc(id).docname.as_str()));
            digest.text(&item.anchor).text(&item.url).text(&item.title).text(&item.number);
            digest.flag(item.literal).flag(item.generated);
        }
        for item in items.iter().rev() {
            pending.push(&item.children);
        }
    }
}
