//! Article navigation follows publication dates rather than book reading order.

use gd_env::{Digest, Digester, DocId, PostlistSpec};
use gd_jinja::Value;
use gd_resolve::{BuildView, relative_url};

#[derive(Debug)]
pub(crate) struct ArticleIndex {
    neighbours: Vec<(Option<DocId>, Option<DocId>)>,
}

impl ArticleIndex {
    pub(crate) fn new(view: &BuildView<'_>) -> Self {
        let posts = PostlistSpec::default().select(view.env);
        let mut neighbours = vec![(None, None); view.env.docs.len()];
        for (index, &id) in posts.iter().enumerate() {
            let newer = index.checked_sub(1).and_then(|i| posts.get(i)).copied();
            let older = posts.get(index + 1).copied();
            neighbours[id.index()] = (older, newer);
        }
        Self { neighbours }
    }

    fn neighbours(&self, id: DocId) -> (Option<DocId>, Option<DocId>) {
        self.neighbours.get(id.index()).copied().unwrap_or_default()
    }
}

pub(crate) fn inputs(view: &BuildView<'_>, id: DocId, index: &ArticleIndex, key: &str) -> Digest {
    let mut digest = Digester::new();
    let (older, newer) = index.neighbours(id);
    for (name, other) in [("older", older), ("newer", newer)] {
        if key != "page" && key != name {
            continue;
        }
        digest.flag(other.is_some());
        if let Some(other) = other {
            let doc = view.doc(other);
            digest.text(&doc.docname).text(&doc.title).text(&doc.page.date);
        }
    }
    digest.finish()
}

pub(crate) fn fill(
    view: &BuildView<'_>,
    id: DocId,
    body: &str,
    page: &Value,
    index: &ArticleIndex,
) {
    let (older, newer) = index.neighbours(id);
    let entry = |other| {
        let Some(other) = other else { return Value::None };
        let value = Value::dict();
        let doc = view.doc(other);
        value.set("title", Value::from(doc.title.as_str()));
        value.set("url", Value::from(view.page_url(Some(id), other, "")));
        value.set("date", Value::from(doc.page.date.as_str()));
        value
    };
    page.set("older", entry(older));
    page.set("newer", entry(newer));
    let words = word_count(body);
    let rate = usize::try_from(view.config.myst.words_per_minute).unwrap_or(200).max(1);
    page.set("word_count", Value::from(words as i64));
    page.set("reading_minutes", Value::from(words.div_ceil(rate) as i64));
    let file = view.page_name(&view.doc(id).docname);
    let taxonomies = Value::dict();
    for kind in &view.config.site.taxonomies {
        let entries = crate::collections::terms(kind, &view.doc(id).page);
        taxonomies.set(
            kind,
            Value::list(entries.iter().map(|term| {
                let value = Value::dict();
                value.set("name", Value::from(term.as_str()));
                let target = crate::collections::page_file(
                    &format!("{kind}/{}", crate::collections::slug(term)),
                    1,
                );
                value.set("url", Value::from(relative_url(&file, &target)));
                value
            })),
        );
    }
    page.set("taxonomies", taxonomies);
}

/// Counts visible whitespace-delimited words, with one word per CJK character.
fn word_count(html: &str) -> usize {
    let mut tag = false;
    let mut word = false;
    let mut count = 0;
    for c in html.chars() {
        match c {
            '<' => {
                tag = true;
                word = false;
            }
            '>' => {
                tag = false;
                word = false;
            }
            _ if tag => {}
            '\u{3400}'..='\u{9fff}' | '\u{3040}'..='\u{30ff}' | '\u{ac00}'..='\u{d7af}' => {
                count += 1;
                word = false;
            }
            _ if c.is_whitespace() => word = false,
            _ if !word => {
                count += 1;
                word = true;
            }
            _ => {}
        }
    }
    count
}
