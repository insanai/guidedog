//! A document's translation while it is read (the `Translating` hook of intl.odin):
//! gd-i18n's pass with the catalog of the document's text domain (gd-intl), and
//! localized figures (`figure_language_filename`).

use std::path::Path;

use gd_i18n::{ImageExists, ImageLocalization, Translation};
use guidedoc::{ReadOptions, Reader};

use crate::context::ReadContext;

/// The translation of document `docname`, read by `reader` with `read`; `None` when the
/// build translates nothing. `source_name` names the translations where they are parsed
/// (`Intl::source_name`); `exists` answers whether a localized image is there.
pub(crate) fn translation<'a>(
    ctx: &ReadContext<'a>,
    docname: &'a str,
    reader: &Reader,
    read: ReadOptions<'a>,
    source_name: &'a str,
    exists: &'a dyn ImageExists,
) -> Option<Translation<'a>> {
    let intl = ctx.intl;
    if !intl.translate {
        return None;
    }
    let c = ctx.config;
    let mut t = Translation {
        catalog: intl.catalog_of(docname).map(|catalog| catalog as &dyn gd_gettext::Lookup),
        read_inline: reader.read_inline,
        read,
        policy: ctx.policy,
        limits: ctx.limits,
        targets: intl.targets,
        source_name,
        progress: intl.progress,
        images: ImageLocalization::default(),
    };
    if c.language != "en" {
        t.images = ImageLocalization {
            format: &c.intl.figure_language_filename,
            docname,
            language: &c.language,
            exists: Some(exists),
        };
    }
    Some(t)
}

/// Whether the image `path`, written in document `docname`, is a file of the source
/// folder: `/path` is below the source folder, any other path below the document's.
pub(crate) fn image_exists(source_dir: &Path, docname: &str, path: &str) -> bool {
    source_dir.join(file_target(docname, path)).is_file()
}

/// The source-folder path `target` names from document `docname`, `.` and `..`
/// resolved; `..` never leaves the source folder.
fn file_target(docname: &str, target: &str) -> String {
    let (base, rest) = match target.strip_prefix('/') {
        Some(rest) => ("", rest),
        None => (docname.rfind('/').map_or("", |i| &docname[..i]), target),
    };
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    for p in rest.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(p),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_paths_resolve_against_the_document_or_the_source_folder() {
        assert_eq!(file_target("guide/usage", "img/a.png"), "guide/img/a.png");
        assert_eq!(file_target("guide/usage", "/img/a.png"), "img/a.png");
        assert_eq!(file_target("guide/usage", "../../a.png"), "a.png");
        assert_eq!(file_target("index", "./a.png"), "a.png");
    }
}
