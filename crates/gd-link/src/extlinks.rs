//! sphinx.ext.extlinks' ExternalLinksChecker, which `extlinks_detect_hardcoded_links`
//! turns on: a URL written out in full that an extlink could write warns, with the role
//! to write instead.

use guidedoc::{AttributeKey, Document, LinkKind, NodeId};

use crate::page::{Page, Warning};

/// Warns when the link `id` is a URL an extlink of the configuration could write.
pub(crate) fn check_hardcoded_link(page: &mut Page<'_>, doc: Document<'_>, id: NodeId) {
    let config = page.config();
    if !config.extlinks_detect_hardcoded_links {
        return;
    }
    let Some(link) = doc.link_info(id).filter(|l| l.kind == LinkKind::External) else {
        return;
    };
    if doc.attribute(id, AttributeKey::Class).is_some_and(|c| c.contains("extlink-")) {
        return; // written with an extlink role.
    }
    let uri = doc.text(link.destination);
    // extlinks is a sorted map, so the first role by name wins, as in Sphinx.
    for (name, (pattern, _)) in &config.extlinks {
        let Some(value) = extlink_value(pattern, uri) else { continue };
        let mut title = String::new();
        doc.plain_text(id, &mut title);
        let role = if title == uri {
            format!(":{name}:`{value}`")
        } else {
            format!(":{name}:`{title} <{value}>`")
        };
        let hint = format!("Write {role} instead.");
        let warning = Warning {
            code: "build.extlink.hardcoded",
            title: "LINK AN EXTLINK COULD WRITE",
            category: "extlink.hardcoded",
            message: format!("The link to {uri} could be written {role}."),
            hint: &hint,
        };
        page.warn(doc, doc.node(id).span, warning);
        return;
    }
}

/// The value an extlink pattern (`https://host/%s`) gives `uri`: the part `%s` stands
/// for, when it holds no `/`.
fn extlink_value<'u>(pattern: &str, uri: &'u str) -> Option<&'u str> {
    let (before, after) = pattern.split_once("%s")?;
    if uri.len() <= before.len() + after.len() {
        return None;
    }
    let value = uri.strip_prefix(before)?.strip_suffix(after)?;
    if value.contains('/') { None } else { Some(value) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extlink_values_are_one_path_segment() {
        let pattern = "https://github.com/x/issues/%s";
        assert_eq!(extlink_value(pattern, "https://github.com/x/issues/12"), Some("12"));
        assert_eq!(extlink_value(pattern, "https://github.com/x/issues/1/2"), None);
        assert_eq!(extlink_value(pattern, "https://github.com/x/issues/"), None);
        assert_eq!(extlink_value("no placeholder", "no placeholder"), None);
    }
}
