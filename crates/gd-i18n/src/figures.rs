//! figure_language_filename: the localized file of an image, as Sphinx finds it.

use std::fmt;

/// Sphinx's default `figure_language_filename`.
pub const DEFAULT_FIGURE_FILENAME: &str = "{root}.{language}{ext}";

/// Why a `figure_language_filename` format cannot be filled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FigureFormatError {
    /// A "{" is not closed.
    Unclosed,
    /// A field the format names does not exist; it holds the field's name.
    UnknownField(String),
}

impl fmt::Display for FigureFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FigureFormatError::Unclosed => f.write_str("a \"{\" is not closed")?,
            FigureFormatError::UnknownField(name) => {
                write!(f, "{{{name}}} is not a field of figure_language_filename")?;
            }
        }
        f.write_str(
            "\nHint: Use the fields {root}, {path}, {basename}, {ext}, {docpath}, and \
             {language}; write {{ and }} for braces.",
        )
    }
}

impl std::error::Error for FigureFormatError {}

/// Fills a `figure_language_filename` format for an image path as written in a
/// document, appending to `out`, as Sphinx's `get_image_filename_for_language` does:
///
/// - `{root}`: the path without its extension ("img/logo" of "img/logo.png");
/// - `{path}`: its directory with a trailing slash, or "" ("img/");
/// - `{basename}`: the file name without directory or extension ("logo");
/// - `{ext}`: the extension with its dot (".png");
/// - `{docpath}`: the document's directory with a trailing slash, or "";
/// - `{language}`: the language.
///
/// `{{` and `}}` stand for braces. Fails on an unclosed brace or an unknown field, after
/// appending part of the result.
pub fn write_figure_filename(
    out: &mut String,
    format: &str,
    filename: &str,
    docname: &str,
    language: &str,
) -> Result<(), FigureFormatError> {
    let slash = filename.rfind('/');
    let name_start = slash.map_or(0, |s| s + 1);
    // No extension, or a leading dot that is part of the name.
    let dot = filename.rfind('.').filter(|&d| d > name_start).unwrap_or(filename.len());
    let (root, ext) = filename.split_at(dot);
    let docpath = docname.rfind('/').map_or("", |s| &docname[..=s]);
    let field = |name: &str| match name {
        "root" => Some(root),
        "path" => Some(&filename[..name_start]),
        "basename" => Some(&root[name_start..]),
        "ext" => Some(ext),
        "docpath" => Some(docpath),
        "language" => Some(language),
        _ => None,
    };
    let mut rest = format;
    while let Some(c) = rest.chars().next() {
        if (c == '{' || c == '}') && rest[1..].starts_with(c) {
            out.push(c);
            rest = &rest[2..];
        } else if c == '{' {
            let close = rest.find('}').ok_or(FigureFormatError::Unclosed)?;
            let name = &rest[1..close];
            let value =
                field(name).ok_or_else(|| FigureFormatError::UnknownField(name.to_owned()))?;
            out.push_str(value);
            rest = &rest[close + 1..];
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    Ok(())
}

/// Returns [`write_figure_filename`]'s result as a new string.
pub fn figure_filename(
    format: &str,
    filename: &str,
    docname: &str,
    language: &str,
) -> Result<String, FigureFormatError> {
    let mut out = String::with_capacity(format.len() + filename.len() + language.len());
    write_figure_filename(&mut out, format, filename, docname, language)?;
    Ok(out)
}

/// Answers whether an image path, as a document would write it, names a file; the
/// host resolves it against the document's directory. Any
/// `Fn(docname, path) -> bool` closure is one.
pub trait ImageExists {
    /// Tells whether `path`, written in document `docname`, names a file.
    fn image_exists(&self, docname: &str, path: &str) -> bool;
}

impl<F: Fn(&str, &str) -> bool> ImageExists for F {
    fn image_exists(&self, docname: &str, path: &str) -> bool {
        self(docname, path)
    }
}

/// Returns the localized image when it exists, else the image as written, as Sphinx's
/// `search_image_for_language` does. An empty `format` is the default; without
/// `exists`, no localized image exists. URLs and inline data are left alone.
pub fn figure_for_language(
    format: &str,
    filename: &str,
    docname: &str,
    language: &str,
    exists: Option<&dyn ImageExists>,
) -> String {
    let localization = ImageLocalization { format, docname, language, exists };
    let mut out = String::new();
    if localization.localize(filename, &mut out) { out } else { filename.to_owned() }
}

/// figure_language_filename for the translation pass: where localized images are.
/// `Default` localizes nothing.
#[derive(Clone, Copy, Default)]
pub struct ImageLocalization<'a> {
    /// The format; "" is [`DEFAULT_FIGURE_FILENAME`].
    pub format: &'a str,
    /// The document's name, e.g. "guide/usage".
    pub docname: &'a str,
    /// The language; "" localizes nothing.
    pub language: &'a str,
    /// Answers whether a localized file exists; `None` localizes nothing.
    pub exists: Option<&'a dyn ImageExists>,
}

impl fmt::Debug for ImageLocalization<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageLocalization")
            .field("format", &self.format)
            .field("docname", &self.docname)
            .field("language", &self.language)
            .field("exists", &self.exists.is_some())
            .finish()
    }
}

impl ImageLocalization<'_> {
    /// Tells whether this can localize any image.
    pub fn is_active(&self) -> bool {
        !self.language.is_empty() && self.exists.is_some()
    }

    /// Writes the localized path of image `uri` into `out` (cleared first) and returns
    /// true when that file exists.
    pub fn localize(&self, uri: &str, out: &mut String) -> bool {
        out.clear();
        let Some(exists) = self.exists.filter(|_| !self.language.is_empty()) else {
            return false;
        };
        if is_remote(uri) {
            return false;
        }
        let format = if self.format.is_empty() { DEFAULT_FIGURE_FILENAME } else { self.format };
        write_figure_filename(out, format, uri, self.docname, self.language).is_ok()
            && exists.image_exists(self.docname, out)
    }
}

/// Tells whether an image is a URL or inline data, which is never localized.
fn is_remote(uri: &str) -> bool {
    uri.is_empty() || uri.contains("://") || uri.starts_with("data:")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(format: &str, file: &str) -> String {
        figure_filename(format, file, "guide/usage", "de").unwrap_or_default()
    }

    #[test]
    fn figure_filenames_like_sphinx() {
        assert_eq!(fill(DEFAULT_FIGURE_FILENAME, "img/logo.png"), "img/logo.de.png");
        assert_eq!(fill("{path}{language}/{basename}{ext}", "img/logo.png"), "img/de/logo.png");
        assert_eq!(fill("{docpath}{basename}{{x}}{ext}", "logo.png"), "guide/logo{x}.png");
        assert_eq!(fill(DEFAULT_FIGURE_FILENAME, "Makefile"), "Makefile.de");
        assert_eq!(fill(DEFAULT_FIGURE_FILENAME, "img/.hidden"), "img/.hidden.de");
        let err = figure_filename("{nope}", "a.png", "", "de");
        assert!(err.is_err_and(|e| e.to_string().contains("{nope}")));
        assert_eq!(figure_filename("{root", "a.png", "", "de"), Err(FigureFormatError::Unclosed));
    }

    #[test]
    fn localized_images_exist_or_fall_back() {
        let exists = |_: &str, path: &str| path == "logo.de.png";
        let f = |image: &str, exists: Option<&dyn ImageExists>| {
            figure_for_language("", image, "index", "de", exists)
        };
        assert_eq!(f("logo.png", Some(&exists)), "logo.de.png");
        assert_eq!(f("other.png", Some(&exists)), "other.png");
        assert_eq!(f("https://example.com/a.png", Some(&exists)), "https://example.com/a.png");
        // Without a way to ask, no localized image exists.
        assert_eq!(f("logo.png", None), "logo.png");
    }
}
