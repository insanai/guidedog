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
}

pub(crate) fn read(value: &Value, path: &str) -> Result<SiteConfig, Problem> {
    let Data::Table(table) = &value.data else {
        return Err(invalid(path, value.line, "site must be a table."));
    };
    let mut site = SiteConfig::default();
    for (key, value) in table.iter() {
        match (key, &value.data) {
            ("base_url", Data::String(url)) => site.base_url = url.trim_end_matches('/').into(),
            ("sitemap", Data::Boolean(flag)) => site.sitemap = *flag,
            ("feeds", Data::Boolean(flag)) => site.feeds = *flag,
            _ => {
                return Err(invalid(
                    path,
                    value.line,
                    &format!("Unknown site setting or incorrect value type: {key}."),
                ));
            }
        }
    }
    if (site.sitemap || site.feeds) && !valid_url(&site.base_url) {
        return Err(invalid(
            path,
            value.line,
            "site.base_url must be an absolute HTTP or HTTPS URL.",
        ));
    }
    Ok(site)
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

fn invalid(path: &str, line: usize, message: &str) -> Problem {
    config_problem(
        path,
        line,
        message.into(),
        "Use [site] with base_url, sitemap = true and feeds = true.",
    )
}
