//! Font-file discovery for builds without the embedded compiler. No program runs.

use std::path::PathBuf;
use std::sync::Arc;

use crate::{Options, TextFace};

pub(crate) fn text_fonts(options: &Options<'_>) -> Vec<TextFace> {
    let mut roots = options.font_paths.clone();
    if !options.embedded_fonts_only {
        for path in [
            "/usr/share/fonts",
            "/usr/local/share/fonts",
            "/System/Library/Fonts",
            "/Library/Fonts",
        ] {
            roots.push(PathBuf::from(path));
        }
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(&home).join(".local/share/fonts"));
            roots.push(PathBuf::from(&home).join(".fonts"));
            roots.push(PathBuf::from(home).join("Library/Fonts"));
        }
        if let Some(windows) = std::env::var_os("WINDIR") {
            roots.push(PathBuf::from(windows).join("Fonts"));
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut pending: Vec<_> = roots.into_iter().map(|p| (p, 0usize)).collect();
    let mut faces = Vec::new();
    while let Some((path, depth)) = pending.pop() {
        if depth > 16 || seen.len() >= 10_000 {
            continue;
        }
        let Ok(path) = path.canonicalize() else {
            continue;
        };
        if !seen.insert(path.clone()) {
            continue;
        }
        if path.is_dir() {
            if let Ok(children) = path.read_dir() {
                let mut children: Vec<_> =
                    children.filter_map(Result::ok).map(|p| p.path()).collect();
                children.sort();
                pending.extend(children.into_iter().rev().map(|p| (p, depth + 1)));
            }
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| ["otf", "ttf", "ttc"].iter().any(|s| e.eq_ignore_ascii_case(s)))
            && let Ok(data) = crate::fonts::read_font(&path)
        {
            append(&mut faces, &data, crate::fonts::FontFile::new(path));
        }
    }
    faces
}

fn append(faces: &mut Vec<TextFace>, data: &[u8], file: Arc<crate::fonts::FontFile>) {
    let count = ttf_parser::fonts_in_collection(data).unwrap_or(1).min(256);
    for index in 0..count {
        let Ok(face) = ttf_parser::Face::parse(data, index) else {
            continue;
        };
        let family = face
            .names()
            .into_iter()
            .find(|n| n.name_id == ttf_parser::name_id::FAMILY)
            .and_then(|n| n.to_string())
            .unwrap_or_default();
        faces.push(TextFace {
            family,
            index,
            bold: face.is_bold(),
            italic: face.is_italic(),
            data: crate::fonts::Data::File(Arc::clone(&file)),
            coverage: coverage(&face),
        });
    }
}

fn coverage(face: &ttf_parser::Face<'_>) -> Vec<(u32, u32)> {
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    let Some(cmap) = face.tables().cmap else { return ranges };
    for subtable in cmap.subtables.into_iter().filter(|table| table.is_unicode()) {
        subtable.codepoints(|code| {
            if code > 0x10ffff || ranges.len() >= 65536 {
                return;
            }
            if let Some((_, last)) = ranges.last_mut()
                && last.checked_add(1) == Some(code)
            {
                *last = code;
            } else {
                ranges.push((code, code));
            }
        });
    }
    ranges.sort_unstable();
    ranges
}
