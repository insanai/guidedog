//! Page metadata shared by publication gates and renderers.

use std::time::SystemTime;

use gd_json::Value;

use crate::{ObjectBuilder, ObjectReader};

/// Publication and presentation fields; empty strings mean absent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageMeta {
    /// HTML template override.
    pub template: String,
    /// Publication date or instant.
    pub date: String,
    /// Last significant revision date or instant.
    pub updated: String,
    /// Scheduled release instant, overriding `date`.
    pub publish_at: String,
    /// Exclusive publication end instant.
    pub expires_at: String,
    /// Hidden from release builds.
    pub draft: bool,
    /// Authors in source order.
    pub authors: Vec<String>,
    /// Tags in source order.
    pub tags: Vec<String>,
    /// Primary category.
    pub category: String,
    /// Series name.
    pub series: String,
    /// Plain-text abstract.
    pub summary: String,
    /// PDF layout; empty or `none` disables per-page PDF.
    pub pdf: String,
    /// Suppress navigation sidebar.
    pub nosidebar: bool,
}

impl PageMeta {
    /// Digest of the metadata, excluding body text, for collection dependencies.
    pub fn digest(&self) -> crate::Digest {
        crate::Digest::of_text(&gd_json::to_string(&self.to_json()))
    }

    /// Records a field, ignoring unknown names; returns an explanation for invalid values.
    pub(crate) fn set_field(&mut self, name: &str, value: &str) -> Result<(), String> {
        let value = value.trim();
        match name {
            "date" | "updated" => {
                publication_time(value).map_err(|e| e.to_string())?;
                if name == "date" { self.date = value.into() } else { self.updated = value.into() }
            }
            "publish-at" | "expires-at" => {
                gd_clock::parse_timestamp(value).map_err(|e| e.to_string())?;
                if name == "publish-at" {
                    self.publish_at = value.into()
                } else {
                    self.expires_at = value.into()
                }
            }
            "draft" => self.draft = boolean(value)?,
            "nosidebar" => self.nosidebar = boolean(value)?,
            "template" => self.template = value.into(),
            "author" => self.authors.push(value.into()),
            "authors" => self.authors.extend(items(value)?),
            "tags" => self.tags.extend(items(value)?),
            "category" => self.category = value.into(),
            "series" => self.series = value.into(),
            "summary" => self.summary = value.into(),
            "pdf" => {
                if !matches!(value, "" | "none" | "article") {
                    return Err("expected none or article for the PDF layout".into());
                }
                self.pdf = value.into();
            }
            _ => {}
        }
        Ok(())
    }

    /// Why the page is hidden at `now`, or `None` when eligible. Invalid dates fail closed.
    pub fn ineligible_reason(&self, now: SystemTime) -> Option<&'static str> {
        if self.draft {
            return Some("draft");
        }
        let release = if self.publish_at.is_empty() { &self.date } else { &self.publish_at };
        if !release.is_empty() {
            match publication_time(release) {
                Ok(at) if at <= now => {}
                Ok(_) => return Some("scheduled"),
                Err(_) => return Some("invalid publication date"),
            }
        }
        if !self.expires_at.is_empty() {
            match gd_clock::parse_timestamp(&self.expires_at) {
                Ok(at) if now < at => {}
                Ok(_) => return Some("expired"),
                Err(_) => return Some("invalid expiration date"),
            }
        }
        None
    }

    /// The selected HTML layout, defaulting to page or article according to `date`.
    pub fn html_template(&self) -> &str {
        if !self.template.is_empty() {
            &self.template
        } else if self.date.is_empty() {
            "page.html"
        } else {
            "article.html"
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        ObjectBuilder::default()
            .text("template", &self.template)
            .text("date", &self.date)
            .text("updated", &self.updated)
            .text("publish_at", &self.publish_at)
            .text("expires_at", &self.expires_at)
            .flag("draft", self.draft)
            .texts("authors", &self.authors)
            .texts("tags", &self.tags)
            .text("category", &self.category)
            .text("series", &self.series)
            .text("summary", &self.summary)
            .text("pdf", &self.pdf)
            .flag("nosidebar", self.nosidebar)
            .done()
    }

    pub(crate) fn from_json(value: &Value) -> Option<Self> {
        let r = ObjectReader::of(value)?;
        Some(Self {
            template: r.text("template")?,
            date: r.text("date")?,
            updated: r.text("updated")?,
            publish_at: r.text("publish_at")?,
            expires_at: r.text("expires_at")?,
            draft: r.flag("draft")?,
            authors: r.texts("authors")?,
            tags: r.texts("tags")?,
            category: r.text("category")?,
            series: r.text("series")?,
            summary: r.text("summary")?,
            pdf: r.text("pdf")?,
            nosidebar: r.flag("nosidebar")?,
        })
    }
}

/// Parses a publication date at midnight UTC, or an RFC 3339 instant.
pub fn publication_time(text: &str) -> Result<SystemTime, gd_clock::ParseTimeError> {
    if text.len() == 10 {
        gd_clock::parse_date(text)?;
        gd_clock::parse_timestamp(&format!("{text}T00:00:00Z"))
    } else {
        gd_clock::parse_timestamp(text)
    }
}

fn boolean(value: &str) -> Result<bool, String> {
    match value {
        "true" | "True" => Ok(true),
        "false" | "False" => Ok(false),
        _ => Err("expected true or false".into()),
    }
}

fn items(value: &str) -> Result<Vec<String>, String> {
    if value.starts_with('[') {
        let parsed = gd_json::parse(value).map_err(|_| "expected a list of strings")?;
        let items = parsed.as_array().ok_or("expected a list of strings")?;
        return items
            .iter()
            .map(|v| {
                v.as_str().map(str::to_owned).ok_or_else(|| "expected a list of strings".into())
            })
            .collect();
    }
    Ok(value.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_boundaries_and_cache_roundtrip() {
        let mut page = PageMeta::default();
        page.set_field("date", "2026-10-16").unwrap();
        page.set_field("publish-at", "2026-10-16T09:00:00.5Z").unwrap();
        page.set_field("expires-at", "2026-10-17T09:00:00Z").unwrap();
        let start = publication_time(&page.publish_at).unwrap();
        assert_eq!(
            page.ineligible_reason(start - std::time::Duration::from_nanos(1)),
            Some("scheduled")
        );
        assert_eq!(page.ineligible_reason(start), None);
        assert_eq!(
            page.ineligible_reason(publication_time(&page.expires_at).unwrap()),
            Some("expired")
        );
        page.set_field("draft", "true").unwrap();
        assert_eq!(page.ineligible_reason(start), Some("draft"));
        assert_eq!(PageMeta::from_json(&page.to_json()), Some(page));
    }
}
