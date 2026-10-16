//! Collection queries use their full encoded value as the verifying-trace key.

use gd_json::Value;
use guidedoc::{Document, NodeId};

use crate::{DocId, DocInfo, Environment, ObjectBuilder, ObjectReader};

/// A malformed collection query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostlistError(pub &'static str);

impl std::fmt::Display for PostlistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for PostlistError {}

/// A chronological view of dated documents. Empty filters accept every value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PostlistSpec {
    /// Glob over docnames; empty accepts all documents.
    pub path: String,
    /// Required category.
    pub category: String,
    /// Required tags, all of which must match.
    pub tags: Vec<String>,
    /// Required author.
    pub author: String,
    /// Required series.
    pub series: String,
    /// Maximum items; zero means unlimited.
    pub limit: usize,
    /// Oldest articles first instead of newest first.
    pub ascending: bool,
    /// Include plain-text summaries.
    pub excerpts: bool,
}

impl PostlistSpec {
    /// Reads the attributes of a postlist node; refuses invalid limits and sort keys.
    pub fn from_node(doc: Document<'_>, id: NodeId) -> Result<Self, PostlistError> {
        let value = |name| doc.custom_attribute(id, name).unwrap_or("");
        let limit = value("limit");
        let limit = if limit.is_empty() {
            0
        } else {
            limit
                .parse::<usize>()
                .map_err(|_| PostlistError("expected a non-negative postlist limit"))?
        };
        let ascending = match value("sort") {
            "" | "-date" => false,
            "date" => true,
            _ => return Err(PostlistError("postlist sort must be date or -date")),
        };
        Ok(Self {
            path: value("path").into(),
            category: value("category").into(),
            tags: value("tags")
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
            author: value("author").into(),
            series: value("series").into(),
            limit,
            ascending,
            excerpts: value("excerpts") == "1",
        })
    }

    /// The complete query as a stable trace key, without hash collisions.
    pub fn encode(&self) -> String {
        gd_json::to_string(
            &ObjectBuilder::default()
                .text("path", &self.path)
                .text("category", &self.category)
                .texts("tags", &self.tags)
                .text("author", &self.author)
                .text("series", &self.series)
                .number("limit", self.limit as u64)
                .flag("ascending", self.ascending)
                .flag("excerpts", self.excerpts)
                .done(),
        )
    }

    /// Reads a query saved by `encode`; `None` for malformed records.
    pub fn decode(text: &str) -> Option<Self> {
        let value: Value = gd_json::parse(text).ok()?;
        let r = ObjectReader::of(&value)?;
        Some(Self {
            path: r.text("path")?,
            category: r.text("category")?,
            tags: r.texts("tags")?,
            author: r.text("author")?,
            series: r.text("series")?,
            limit: usize::try_from(r.number("limit")?).ok()?,
            ascending: r.flag("ascending")?,
            excerpts: r.flag("excerpts")?,
        })
    }

    /// Selects eligible articles in date order, breaking ties by docname before limiting.
    pub fn select(&self, env: &Environment) -> Vec<DocId> {
        let mut posts: Vec<_> = env.iter().filter(|(_, doc)| self.matches(doc)).collect();
        posts.sort_by(|(_, a), (_, b)| {
            let a_date = crate::publication_time(&a.page.date).ok();
            let b_date = crate::publication_time(&b.page.date).ok();
            let dates = if self.ascending { a_date.cmp(&b_date) } else { b_date.cmp(&a_date) };
            dates.then_with(|| a.docname.cmp(&b.docname))
        });
        if self.limit > 0 {
            posts.truncate(self.limit);
        }
        posts.into_iter().map(|(id, _)| id).collect()
    }

    fn matches(&self, doc: &DocInfo) -> bool {
        let p = &doc.page;
        !doc.failed
            && !p.date.is_empty()
            && (self.path.is_empty() || crate::glob_match(&self.path, &doc.docname))
            && (self.category.is_empty() || self.category == p.category)
            && (self.series.is_empty() || self.series == p.series)
            && (self.author.is_empty() || p.authors.contains(&self.author))
            && self.tags.iter().all(|tag| p.tags.contains(tag))
    }
}
