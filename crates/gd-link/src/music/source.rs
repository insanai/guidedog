//! Confined lexical sources and their dependency identities.

use std::path::{Path, PathBuf};

use gd_env::LookupKind;
use gd_lilypond::{IncludeRefusal, IncludeRequest, IncludedFile};
use guidedoc::{Document, NodeId};

use crate::page::Page;

pub(super) fn origin(page: &Page<'_>, doc: Document<'_>, id: NodeId) -> PathBuf {
    page.config()
        .source_dir
        .join(doc.custom_attribute(id, "lilypond-source").unwrap_or(&page.info.relative))
}

pub(super) fn read(
    page: &mut Page<'_>,
    from: &Path,
    name: &str,
) -> Result<IncludedFile, IncludeRefusal> {
    if page.view.options.untrusted {
        return Err(refusal("Music file access is disabled in --untrusted builds."));
    }
    let mut candidates = vec![from.parent().unwrap_or(from).join(name)];
    candidates.push(page.config().source_dir.join(name));
    for root in &page.config().include_roots {
        candidates.push(page.config().source_dir.join(root).join(name));
    }
    for path in candidates {
        if !gd_host::confined(page.view.roots, &path) {
            continue;
        }
        page.trace.note_key(LookupKind::FileContent, &path.to_string_lossy());
        let Ok(path) = path.canonicalize() else { continue };
        page.trace.note_key(LookupKind::FileContent, &path.to_string_lossy());
        let bytes = gd_host::read_file(&path, Some(page.ledger), 256 * 1024)
            .map_err(|e| refusal(&e.to_string()))?;
        let mut text = String::from_utf8(bytes).map_err(|_| refusal("Music must be UTF-8."))?;
        if text.starts_with('\u{feff}') {
            text.drain(..3);
        }
        return Ok(IncludedFile { name: path.to_string_lossy().into_owned(), text });
    }
    Err(refusal(&format!("Cannot read music file `{name}` inside the source roots.")))
}

pub(super) fn include(
    page: &mut Page<'_>,
    files: &mut Vec<PathBuf>,
    request: IncludeRequest<'_>,
) -> Result<IncludedFile, IncludeRefusal> {
    let from = files
        .get(request.from.0 as usize)
        .ok_or_else(|| refusal("The include source has no file identity."))?;
    let file = read(page, from, request.name)?;
    files.push(PathBuf::from(&file.name));
    Ok(file)
}

fn refusal(message: &str) -> IncludeRefusal {
    IncludeRefusal {
        message: message.to_string(),
        hint: "Keep music files within the source directory or configured include_roots.".into(),
    }
}
