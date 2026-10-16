//! Static publication settings stay separate from the Sphinx-compatible flat settings.

use gd_host::Problem;
use gd_toml::{Data, Value};

use crate::store::config_problem;

/// Optional static publication outputs. Disabled unless configured.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SiteConfig {
    /// Absolute public root URL, including any hosting prefix.
    pub base_url: String,
    /// Emit sitemap.xml.
    pub sitemap: bool,
    /// Emit rss.xml and atom.xml for all eligible dated documents.
    pub feeds: bool,
    /// Generated archive kinds: tags, category, year, authors and series.
    pub taxonomies: Vec<String>,
    /// Articles per generated archive page; zero means unlimited.
    pub page_size: usize,
    /// Named collection outputs and their filters.
    pub collections: Vec<CollectionConfig>,
    /// Projects assembled by guidedog ssg build.
    pub mounts: Vec<crate::SiteMount>,
    /// Shared files and directories copied once after mounted builds succeed.
    pub assets: Vec<crate::SiteAsset>,
    /// Load the installed theme only after verifying guidedog-theme.lock offline.
    pub theme: bool,
}

/// A named article collection, rendered under its own output directory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollectionConfig {
    /// Relative output directory; never a source path.
    pub output: String,
    /// Page heading, defaulting to the output directory.
    pub title: String,
    /// Docname glob; empty accepts every dated document.
    pub path: String,
    /// Required category.
    pub category: String,
    /// Required tags, all of which must match.
    pub tags: Vec<String>,
    /// Required author.
    pub author: String,
    /// Required series.
    pub series: String,
    /// Required publication year; empty accepts all years.
    pub year: String,
    /// Articles per page; zero means unlimited.
    pub page_size: usize,
    /// Emit RSS and Atom beneath this collection's output directory.
    pub feeds: bool,
    /// Optional HTML template; empty selects collection.html.
    pub template: String,
}

pub(crate) fn read(value: &Value, path: &str) -> Result<SiteConfig, Problem> {
    let mut site = SiteConfig::default();
    merge(&mut site, value, path)?;
    Ok(site)
}

pub(crate) fn merge(site: &mut SiteConfig, value: &Value, path: &str) -> Result<(), Problem> {
    let Data::Table(table) = &value.data else {
        return Err(invalid(path, value.line, "site must be a table."));
    };
    for (key, value) in table.iter() {
        match (key, &value.data) {
            ("base_url", Data::String(url)) => site.base_url = url.trim_end_matches('/').into(),
            ("sitemap", Data::Boolean(flag)) => site.sitemap = *flag,
            ("feeds", Data::Boolean(flag)) => site.feeds = *flag,
            ("theme", Data::Boolean(flag)) => site.theme = *flag,
            ("page_size", Data::Integer(n)) if *n >= 0 => site.page_size = *n as usize,
            ("taxonomies", Data::Array(items)) => {
                site.taxonomies = strings(items, path)?;
                for kind in &site.taxonomies {
                    if !matches!(
                        kind.as_str(),
                        "tags" | "category" | "year" | "authors" | "series"
                    ) {
                        return Err(invalid(path, value.line, "Unknown taxonomy kind."));
                    }
                }
                site.taxonomies.sort();
                site.taxonomies.dedup();
            }
            ("collections", Data::Array(items)) => {
                site.collections =
                    items.iter().map(|v| collection(v, path)).collect::<Result<_, _>>()?;
            }
            ("mounts", Data::Array(items)) => site.mounts = crate::mounts::mounts(items, path)?,
            ("assets", Data::Array(items)) => site.assets = crate::mounts::assets(items, path)?,
            _ => {
                return Err(invalid(
                    path,
                    value.line,
                    &format!("Unknown site setting or incorrect value type: {key}."),
                ));
            }
        }
    }
    if (site.sitemap || site.feeds || site.collections.iter().any(|c| c.feeds))
        && !valid_url(&site.base_url)
    {
        return Err(invalid(
            path,
            value.line,
            "site.base_url must be an absolute HTTP or HTTPS URL.",
        ));
    }
    Ok(())
}

fn collection(value: &Value, path: &str) -> Result<CollectionConfig, Problem> {
    let Data::Table(table) = &value.data else {
        return Err(invalid(path, value.line, "Each site collection must be a table."));
    };
    let mut c = CollectionConfig::default();
    for (key, v) in table.iter() {
        match (key, &v.data) {
            ("output", Data::String(s)) => c.output.clone_from(s),
            ("title", Data::String(s)) => c.title.clone_from(s),
            ("path", Data::String(s)) => c.path.clone_from(s),
            ("category", Data::String(s)) => c.category.clone_from(s),
            ("author", Data::String(s)) => c.author.clone_from(s),
            ("series", Data::String(s)) => c.series.clone_from(s),
            ("year", Data::String(s)) => c.year.clone_from(s),
            ("template", Data::String(s)) => c.template.clone_from(s),
            ("tags", Data::Array(items)) => c.tags = strings(items, path)?,
            ("page_size", Data::Integer(n)) if *n >= 0 => c.page_size = *n as usize,
            ("feeds", Data::Boolean(b)) => c.feeds = *b,
            _ => {
                return Err(invalid(path, v.line, &format!("Invalid collection setting: {key}.")));
            }
        }
    }
    if !valid_output(&c.output) {
        return Err(invalid(
            path,
            value.line,
            "A collection output must be a safe relative directory.",
        ));
    }
    if !c.year.is_empty() && (c.year.len() != 4 || !c.year.bytes().all(|b| b.is_ascii_digit())) {
        return Err(invalid(path, value.line, "A collection year must have four digits."));
    }
    Ok(c)
}

pub(crate) fn strings(items: &[Value], path: &str) -> Result<Vec<String>, Problem> {
    items
        .iter()
        .map(|v| match &v.data {
            Data::String(s) => Ok(s.clone()),
            _ => Err(invalid(path, v.line, "Expected an array of strings.")),
        })
        .collect()
}

/// Whether an output directory consists of portable relative path components.
pub fn valid_output(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 512
        && path.split('/').all(|part| {
            !part.is_empty()
                && part.len() <= 100
                && part != "."
                && part != ".."
                && part.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
                && !matches!(
                    part.to_ascii_lowercase().as_str(),
                    "con"
                        | "prn"
                        | "aux"
                        | "nul"
                        | "com1"
                        | "com2"
                        | "com3"
                        | "com4"
                        | "com5"
                        | "com6"
                        | "com7"
                        | "com8"
                        | "com9"
                        | "lpt1"
                        | "lpt2"
                        | "lpt3"
                        | "lpt4"
                        | "lpt5"
                        | "lpt6"
                        | "lpt7"
                        | "lpt8"
                        | "lpt9"
                )
        })
}

fn valid_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) else {
        return false;
    };
    !rest.is_empty()
        && !rest.starts_with('/')
        && !rest.contains(['?', '#', '\\'])
        && !rest.chars().any(char::is_whitespace)
}

pub(crate) fn invalid(path: &str, line: usize, message: &str) -> Problem {
    config_problem(
        path,
        line,
        message.into(),
        "Use [site] with base_url, sitemap = true and feeds = true.",
    )
}
