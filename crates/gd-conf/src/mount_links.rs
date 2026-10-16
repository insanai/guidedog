//! Public mount addressing shared by the link pass and whole-site validation.

use crate::Config;

/// Resolves a mount URI to its website-relative file and optional fragment.
/// Returns None for unknown mounts, traversal, query strings or nonportable paths.
pub fn mount_target(config: &Config, uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("mount://")?;
    let (name, file) = rest.split_once('/').unwrap_or((rest, "index.html"));
    let mappings = config.html.context.get("ssg_mounts")?.as_table()?;
    let prefix = mappings.get(name)?.as_str()?;
    let path = file.split('#').next()?;
    if path.is_empty()
        || path.contains(['\\', '?', '%', ':'])
        || path.chars().any(|c| c.is_control() || c.is_whitespace())
        || path.split('/').any(|p| p.is_empty() || p == "." || p == "..")
    {
        return None;
    }
    Some(if prefix.is_empty() { file.into() } else { format!("{prefix}/{file}") })
}
