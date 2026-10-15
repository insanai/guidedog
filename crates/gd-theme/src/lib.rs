//! Guidedog's built-in theme: the files `guidedog quickstart` copies into a new project,
//! and that the HTML and PDF builders use when a project has no copy of its own.
//!
//! The texts are embedded at compile time, so the `guidedog` binary needs no data folder.
//! This crate has no dependencies; quickstart (`gd-migrate`), the site builder
//! (`gd-site`) and the book builder (`gd-book`) all read the same files from here.

/// `_templates/layout.html`: the Jinja page layout, with Sphinx's block names.
pub const LAYOUT_HTML: &str = include_str!("../assets/layout.html");

/// `_static/guidedog.css`: the page styles.
pub const GUIDEDOG_CSS: &str = include_str!("../assets/guidedog.css");

/// `_static/guidedog.js`: the theme switch, live search, copy buttons and section
/// highlighting; pages work without it.
pub const GUIDEDOG_JS: &str = include_str!("../assets/guidedog.js");

/// `_templates/book.typ`: the Typst template of the PDF book.
pub const BOOK_TYP: &str = include_str!("../assets/book.typ");

/// One theme file: where a project keeps its copy, and the built-in text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeFile {
    /// The path relative to the project's source folder, such as `_static/guidedog.css`.
    pub path: &'static str,
    /// The built-in contents.
    pub text: &'static str,
}

/// Every theme file, in the order quickstart writes them.
pub const FILES: [ThemeFile; 4] = [
    ThemeFile { path: "_templates/layout.html", text: LAYOUT_HTML },
    ThemeFile { path: "_templates/book.typ", text: BOOK_TYP },
    ThemeFile { path: "_static/guidedog.css", text: GUIDEDOG_CSS },
    ThemeFile { path: "_static/guidedog.js", text: GUIDEDOG_JS },
];

/// The built-in text for a project-relative theme path, or `None` for other paths.
pub fn builtin(path: &str) -> Option<&'static str> {
    FILES.iter().find(|f| f.path == path).map(|f| f.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_file_is_present_and_found_by_path() {
        for file in FILES {
            assert!(!file.text.is_empty(), "{} is empty", file.path);
            assert_eq!(builtin(file.path), Some(file.text));
        }
        assert!(LAYOUT_HTML.contains("{% block"), "the layout keeps Sphinx's blocks");
        assert_eq!(builtin("_static/other.css"), None);
    }
}
