//! Paths in included Markdown (MyST's include options): an image path, or a link path
//! that starts with the relative-docs prefix, is made relative to the main source again
//! by prefixing the included source's directory relative to the main source's.

use guidedoc::SourceId;

use super::inline::url_scheme;
use crate::host::Host;

/// Returns `path`, written in `source`, as seen from the main source when the include
/// options ask for it; otherwise `path` itself.
pub(crate) fn relocate(h: &Host<'_, '_>, source: SourceId, path: &str, image: bool) -> String {
    if source == h.ctx.source || path.is_empty() {
        return path.to_owned();
    }
    let wanted =
        if image { h.rel_images } else { !h.rel_docs.is_empty() && path.starts_with(&h.rel_docs) };
    if !wanted || path.starts_with('/') || !url_scheme(path).is_empty() {
        return path.to_owned();
    }
    let name_of = |id| h.ctx.source_entry(id).map_or("", |e| e.name.as_str());
    let main = dir_of(name_of(h.ctx.source));
    let here = dir_of(name_of(source));
    let Some(rel) = here.strip_prefix(main) else { return path.to_owned() };
    let rel = rel.trim_start_matches('/');
    if rel.is_empty() {
        return path.to_owned();
    }
    normal_path(&format!("{rel}/{path}"))
}

fn dir_of(path: &str) -> &str {
    path.rfind('/').map_or("", |k| &path[..k])
}

/// Removes "x/.." and "." segments.
fn normal_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for s in path.split('/') {
        match s {
            "" | "." => {}
            ".." if parts.last().is_some_and(|p| *p != "..") => {
                parts.pop();
            }
            _ => parts.push(s),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_normalize() {
        assert_eq!(normal_path("sub/../a/./b.png"), "a/b.png");
        assert_eq!(normal_path("../../x"), "../../x");
        assert_eq!(dir_of("docs/sub/x.md"), "docs/sub");
        assert_eq!(dir_of("x.md"), "");
    }
}
