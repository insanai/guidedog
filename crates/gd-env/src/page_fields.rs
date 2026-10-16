//! One shared AST contract for build metadata and authoring tools.

use guidedoc::{Document, NodeId, NodeKind, ROOT, SourceSpan};

use crate::PageMeta;

/// Whether a normalized leading field name configures static publication.
pub fn is_publication_field(name: &str) -> bool {
    matches!(
        name,
        "template"
            | "date"
            | "updated"
            | "publish-at"
            | "expires-at"
            | "draft"
            | "author"
            | "authors"
            | "tags"
            | "category"
            | "series"
            | "summary"
            | "pdf"
            | "nosidebar"
    )
}

/// Whether a field node belongs to the first root publication metadata block.
pub fn publication_field(doc: Document<'_>, id: NodeId) -> bool {
    if doc.kind(id) != NodeKind::Field {
        return false;
    }
    let Some(list) = doc.parent_of(id) else { return false };
    if doc.first_child(ROOT) != Some(list) || doc.kind(list) != NodeKind::FieldList {
        return false;
    }
    let mut name = String::new();
    if let Some(name_node) = doc.first_child(id) {
        doc.plain_text(name_node, &mut name);
    }
    is_publication_field(&name.trim().to_ascii_lowercase())
}

/// An invalid or repeated publication field and its source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageMetadataError {
    /// The field's normalized name.
    pub field: String,
    /// What needs correction.
    pub message: String,
    /// The field's location; MyST may identify the entire front matter block.
    pub span: SourceSpan,
}

impl std::fmt::Display for PageMetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for PageMetadataError {}

/// Reads only the first root field list. Returns the field name and explanation on failure.
pub fn page_metadata(doc: Document<'_>) -> Result<PageMeta, PageMetadataError> {
    let mut page = PageMeta::default();
    let mut seen = std::collections::BTreeSet::new();
    let Some(list) = doc.first_child(ROOT).filter(|&id| doc.kind(id) == NodeKind::FieldList)
    else {
        return Ok(page);
    };
    for field in doc.children(list) {
        let mut name = String::new();
        doc.plain_text(field + 1, &mut name);
        let body = doc.subtree_end(field + 1);
        if name.trim().eq_ignore_ascii_case("authors")
            && doc.custom_attribute(body, "metadata-value").is_none()
            && doc.children(body).count() > 1
        {
            for author in doc.children(body) {
                let mut text = String::new();
                doc.plain_text(author, &mut text);
                page.authors.push(text.trim().to_owned());
            }
            continue;
        }
        let mut value = String::new();
        if let Some(raw) = doc.custom_attribute(body, "metadata-value") {
            value.push_str(raw);
        } else {
            doc.plain_text(body, &mut value);
        }
        let name = name.trim().to_lowercase();
        if matches!(
            name.as_str(),
            "date"
                | "updated"
                | "publish-at"
                | "expires-at"
                | "draft"
                | "template"
                | "pdf"
                | "nosidebar"
        ) && !seen.insert(name.clone())
        {
            return Err(PageMetadataError {
                field: name,
                message: "the field must occur only once".into(),
                span: doc.node(field).span,
            });
        }
        let value = value.replace('\n', " ");
        page.set_field(&name, &value).map_err(|why| PageMetadataError {
            field: name,
            message: format!("{why}: {value:?}"),
            span: doc.node(field).span,
        })?;
    }
    Ok(page)
}
