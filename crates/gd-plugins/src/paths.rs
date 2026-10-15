//! Checks on names that cross the plugin boundary: bounded strings, servable site
//! paths and portable asset paths.

use crate::MAX_PATH;

/// Whether `text` is at most `limit` bytes and holds no NUL byte.
pub(crate) fn is_bounded(text: &str, limit: usize) -> bool {
    text.len() <= limit && !text.contains('\0')
}

/// Whether a static host and Git can serve `path`: at most [`MAX_PATH`] bytes, no
/// control character or backslash, and `/`-separated parts that are not empty, `.`,
/// `..` or `.git` in any case.
pub(crate) fn is_servable(path: &str) -> bool {
    if path.len() > MAX_PATH || path.chars().any(|c| c < ' ' || c == '\x7f' || c == '\\') {
        return false;
    }
    path.split('/').all(|part| {
        !part.is_empty() && part != "." && part != ".." && !part.eq_ignore_ascii_case(".git")
    })
}

/// Whether `path` is a portable relative asset path: servable, and also valid on
/// Windows (no `:*?"<>|`, no part ending in a dot or space, no device names).
pub(crate) fn is_portable_asset(path: &str) -> bool {
    if !is_servable(path) || path.contains([':', '*', '?', '"', '<', '>', '|']) {
        return false;
    }
    path.split('/').all(is_portable_part)
}

fn is_portable_part(part: &str) -> bool {
    if part.len() > 255 || part.ends_with('.') || part.ends_with(' ') {
        return false;
    }
    let stem = part.split('.').next().unwrap_or(part);
    !is_device_name(stem)
}

/// Windows reserves these names with any extension: CON, PRN, AUX, NUL, COM1-9,
/// LPT1-9, and COM/LPT with a superscript 1, 2 or 3.
fn is_device_name(stem: &str) -> bool {
    if ["CON", "PRN", "AUX", "NUL"].iter().any(|d| stem.eq_ignore_ascii_case(d)) {
        return true;
    }
    let (Some(head), Some(suffix)) = (stem.get(..3), stem.get(3..)) else {
        return false;
    };
    if !head.eq_ignore_ascii_case("COM") && !head.eq_ignore_ascii_case("LPT") {
        return false;
    }
    let digit = suffix.len() == 1 && (b'1'..=b'9').contains(&suffix.as_bytes()[0]);
    digit || matches!(suffix, "¹" | "²" | "³")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servable_paths_refuse_traversal_and_git() {
        for good in ["index.html", "_static/a.css", "a/b/c"] {
            assert!(is_servable(good), "{good}");
        }
        for bad in ["", "/a", "a//b", "a/../b", "./a", ".GIT/x", "a\\b", "a\tb"] {
            assert!(!is_servable(bad), "{bad}");
        }
    }

    #[test]
    fn portable_assets_refuse_windows_names() {
        for bad in ["CON.css", "a/aux", "com1.txt", "LPT²", "a.", "a ", "a?b", "C:/x"] {
            assert!(!is_portable_asset(bad), "{bad}");
        }
        for good in ["css/badge.css", "com10.txt", "console.css", "lpt"] {
            assert!(is_portable_asset(good), "{good}");
        }
    }
}
