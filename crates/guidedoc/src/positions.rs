//! Source positions in preview output (GDS 0017): a block's position is written as
//! `<source>:<start>-<end>`, the byte span of its markup in the file it was read from,
//! so an editor maps a click in a preview back to the source. The HTML and Typst
//! renderers share these rules.

use crate::{Document, SourceId, SourceSpan};

/// How a preview writes block source positions.
#[derive(Debug, Clone, Copy)]
pub struct SourcePositions<'a> {
    /// The folder source names are written relative to (a project's source folder);
    /// empty: names as read.
    pub root: &'a str,
    /// The number of bytes a source file holds before the text it was read as, by
    /// source name: 3 for a file that starts with a byte order mark, which the readers
    /// drop. Positions add it, so they count the file's own bytes. `None`: 0 for every
    /// source.
    pub offset: Option<fn(&str) -> usize>,
}

/// Remembers each source's offset, so a page asks `SourcePositions::offset` once per
/// source rather than once per block.
#[derive(Debug, Default)]
pub struct PositionCache {
    offsets: Vec<(SourceId, usize)>,
}

impl PositionCache {
    /// The position of `span` in `doc` as `<source>:<start>-<end>`, with the source
    /// named relative to `positions.root`. `None` for an empty span and for text a
    /// reader generated rather than read from a file, whose source name is empty or
    /// starts with `<` (such as `<rst_epilog>` or `<autodoc mod.Obj>`).
    pub fn position(
        &mut self,
        doc: &Document<'_>,
        span: SourceSpan,
        positions: &SourcePositions<'_>,
    ) -> Option<String> {
        if span.is_zero() {
            return None;
        }
        let name = doc.source_entry(span.source).map_or("", |e| e.name.as_str());
        if name.is_empty() || name.starts_with('<') {
            return None;
        }
        let offset = self.offset(span.source, name, positions);
        let start = offset.saturating_add(span.start as usize);
        let end = offset.saturating_add(span.end as usize);
        Some(format!("{}:{start}-{end}", relative_source(name, positions.root)))
    }

    fn offset(&mut self, id: SourceId, name: &str, positions: &SourcePositions<'_>) -> usize {
        if let Some(&(_, offset)) = self.offsets.iter().find(|(s, _)| *s == id) {
            return offset;
        }
        let offset = positions.offset.map_or(0, |f| f(name));
        self.offsets.push((id, offset));
        offset
    }
}

/// `name` relative to the folder `root`, with `/` separators. Only Windows names use
/// `\` as a separator, so only there is it turned into `/`.
pub fn relative_source(name: &str, root: &str) -> String {
    let (name, root) = if cfg!(windows) {
        (name.replace('\\', "/"), root.replace('\\', "/"))
    } else {
        (name.to_string(), root.to_string())
    };
    let root = root.trim_end_matches('/');
    match name.strip_prefix(root) {
        Some(rest) if !root.is_empty() && rest.starts_with('/') => rest[1..].to_string(),
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_relative_to_the_root() {
        assert_eq!(relative_source("/w/docs/a/b.rst", "/w/docs"), "a/b.rst");
        assert_eq!(relative_source("/w/docs/a/b.rst", "/w/docs/"), "a/b.rst");
        assert_eq!(relative_source("/w/docsx/b.rst", "/w/docs"), "/w/docsx/b.rst");
        assert_eq!(relative_source("b.rst", ""), "b.rst");
        // A POSIX file name may hold a backslash; it is not a separator there.
        let expected = if cfg!(windows) { "a/b.rst" } else { "a\\b.rst" };
        assert_eq!(relative_source("/w/docs/a\\b.rst", "/w/docs"), expected);
    }
}
