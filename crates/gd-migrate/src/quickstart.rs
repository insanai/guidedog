//! `guidedog quickstart`: a new documentation project laid out as Sphinx lays one out,
//! with conf.toml, a root document, and the default templates copied in to edit.

use std::path::{Path, PathBuf};

use gd_conf::CONFIG_FILE;
use gd_host::Problem;

use gd_theme::{
    ARTICLE_TYP, BOOK_TYP, DISCUSSION_TYP, GUIDEDOG_CSS, GUIDEDOG_JS, LAYOUT_HTML, PDF_THEME_TYP,
};

const QUICKSTART_CONF: &str = include_str!("../assets/quickstart/conf.toml");
const QUICKSTART_INDEX: &str = include_str!("../assets/quickstart/index.rst");
const QUICKSTART_INDEX_MD: &str = include_str!("../assets/quickstart/index.md");
const QUICKSTART_GITIGNORE: &str = include_str!("../assets/quickstart/gitignore");

// The media folders quickstart creates. Each holds a note, so version control keeps the
// folder; .txt files are not documents.
const README_IMAGES: &str = "Images for the documents: PNG, JPEG, GIF, SVG, or WebP.\n\n\
Show one with\n\n    .. image:: /assets/images/diagram.png\n       :alt: What it shows\n\n\
The path is relative to the document, or to the source folder when it starts with /.\n";
const README_VIDEO: &str = "Video for the documents: MP4, WebM, OGV, MOV, or M4V.\n\n\
Play one with\n\n    .. figure:: /assets/video/tour.mp4\n\n       A caption.\n\n\
Pages show a player; the PDF book names the recording.\n";
const README_AUDIO: &str = "Audio for the documents: MP3, OGG, M4A, Opus, WAV, FLAC, or AAC.\n\n\
Play one with\n\n    .. image:: /assets/audio/intro.mp3\n\n\
Pages show a player; the PDF book names the recording.\n";

/// The answers to `guidedog quickstart`. Empty texts take the defaults; fill it with
/// struct-update syntax: `QuickstartOptions { project: "Demo".into(), ..Default::default() }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickstartOptions {
    /// The folder the project goes in.
    pub dir: PathBuf,
    /// The project name.
    pub project: String,
    /// The author(s), also in the copyright line.
    pub author: String,
    /// The short version (`0.1`).
    pub version: String,
    /// The full release (`0.1.0`); empty takes `version`.
    pub release: String,
    /// The documents' language (`en`).
    pub language: String,
    /// The root document's suffix: `.rst` (default), `.md` or `.markdown`.
    pub suffix: String,
    /// The root document's name (`index`).
    pub root_doc: String,
    /// `source/` and `build/` instead of one folder with `_build`.
    pub separate: bool,
    /// The copyright year; `None` takes the system clock's.
    pub year: Option<i64>,
    /// The date named in the root document's comment; `None` takes today (`YYYY-MM-DD`).
    pub date: Option<String>,
}

impl Default for QuickstartOptions {
    fn default() -> QuickstartOptions {
        QuickstartOptions {
            dir: PathBuf::from("."),
            project: String::new(),
            author: String::new(),
            version: String::new(),
            release: String::new(),
            language: "en".into(),
            suffix: ".rst".into(),
            root_doc: "index".into(),
            separate: false,
            year: None,
            date: None,
        }
    }
}

/// Creates a documentation project in `options.dir`: conf.toml, the root document,
/// `.gitignore`, `_templates/layout.html`, the four PDF theme modules, `_static/guidedog.css` and
/// `.js`, notes in the media folders, and the (empty) build folder. Returns the files
/// it created, in order.
///
/// Fails with `quickstart.exists` (exit 2) when a project or one of the files is already
/// there (nothing is overwritten), or `host.io` (exit 4) when a file cannot be written;
/// files written before the failure stay.
pub fn quickstart(options: &QuickstartOptions) -> Result<Vec<PathBuf>, Problem> {
    let o = with_defaults(options);
    let source = if o.separate { o.dir.join("source") } else { o.dir.clone() };
    let build = if o.separate { "build" } else { "_build" };
    let root = source.join(format!("{}{}", o.root_doc, o.suffix));
    for existing in [source.join(CONFIG_FILE), root.clone()] {
        if existing.exists() {
            return Err(Problem::new(
                "quickstart.exists",
                "A PROJECT IS ALREADY HERE",
                &existing.display().to_string(),
                "guidedog quickstart does not overwrite an existing project.",
                "Choose an empty folder, or edit the existing conf.toml.",
                2,
            ));
        }
    }
    let mut written = Vec::new();
    for (path, text) in quickstart_files(&o, &source, &root, build) {
        write_new(&path, &text)?;
        written.push(path);
    }
    let build_path = if o.separate { o.dir.join(build) } else { source.join(build) };
    std::fs::create_dir_all(&build_path).map_err(|e| gd_host::io_problem(&build_path, &e))?;
    Ok(written)
}

/// The options with every empty text replaced by its default.
fn with_defaults(options: &QuickstartOptions) -> QuickstartOptions {
    let fallback = QuickstartOptions::default();
    let or = |given: &str, default: &str| {
        if given.is_empty() { default.to_string() } else { given.to_string() }
    };
    let mut o = options.clone();
    o.language = or(&o.language, &fallback.language);
    o.suffix = or(&o.suffix, &fallback.suffix);
    o.root_doc = or(&o.root_doc, &fallback.root_doc);
    o.release = or(&o.release, &o.version);
    o
}

/// Every file quickstart writes, with its text.
fn quickstart_files(
    o: &QuickstartOptions,
    source: &Path,
    root: &Path,
    build: &str,
) -> Vec<(PathBuf, String)> {
    let year = o.year.unwrap_or_else(|| gd_clock::now().utc().date.year);
    let date = o.date.clone().unwrap_or_else(|| gd_clock::now().utc().date.to_string());
    let title = format!("{} documentation", o.project);
    let values = [
        ("project", toml_text(&o.project)),
        ("author", toml_text(&o.author)),
        ("copyright", toml_text(&format!("{year}, {}", o.author))),
        ("version", toml_text(&o.version)),
        ("release", toml_text(&o.release)),
        ("language", toml_text(&o.language)),
        ("root", toml_text(&o.root_doc)),
        ("suffixes", format!("\"{}\"", toml_text(&o.suffix))),
        ("build", build.to_string()),
        ("date", date),
        ("heading", heading(&title)),
        ("title", title),
    ];
    let markdown = o.suffix == ".md" || o.suffix == ".markdown";
    let index = if markdown { QUICKSTART_INDEX_MD } else { QUICKSTART_INDEX };
    let assets = source.join("assets");
    vec![
        (source.join(CONFIG_FILE), substitute(QUICKSTART_CONF, &values)),
        (root.to_path_buf(), substitute(index, &values)),
        (o.dir.join(".gitignore"), substitute(QUICKSTART_GITIGNORE, &values)),
        (source.join("_static/guidedog.css"), GUIDEDOG_CSS.into()),
        (source.join("_static/guidedog.js"), GUIDEDOG_JS.into()),
        (source.join("_templates/layout.html"), LAYOUT_HTML.into()),
        (source.join("_templates/book.typ"), BOOK_TYP.into()),
        (source.join("_templates/discussion.typ"), DISCUSSION_TYP.into()),
        (source.join("_templates/article.typ"), ARTICLE_TYP.into()),
        (source.join("_templates/theme.typ"), PDF_THEME_TYP.into()),
        (assets.join("images/README.txt"), README_IMAGES.into()),
        (assets.join("video/README.txt"), README_VIDEO.into()),
        (assets.join("audio/README.txt"), README_AUDIO.into()),
    ]
}

/// Replaces `{{name}}` placeholders in quickstart files (they are not Jinja templates).
fn substitute(text: &str, values: &[(&str, String)]) -> String {
    let mut out = text.to_string();
    for (name, value) in values {
        out = out.replace(&format!("{{{{{name}}}}}"), value);
    }
    out
}

/// Escapes a value for a basic TOML string; line breaks and tabs become spaces.
fn toml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' | '\t' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Underlines a title with `=` as wide as it displays, wide characters counting two.
fn heading(title: &str) -> String {
    format!("{title}\n{}", "=".repeat(gd_report::display_width(title)))
}

/// Writes a new file, creating its folder; refuses to replace one.
fn write_new(path: &Path, text: &str) -> Result<(), Problem> {
    if path.exists() {
        return Err(Problem::new(
            "quickstart.exists",
            "FILE ALREADY EXISTS",
            &path.display().to_string(),
            "guidedog quickstart does not overwrite files.",
            "Move the file aside, or run quickstart in an empty folder.",
            2,
        ));
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| gd_host::io_problem(dir, &e))?;
    }
    std::fs::write(path, text).map_err(|e| gd_host::io_problem(path, &e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_count_wide_characters_twice() {
        assert_eq!(heading("Docs"), "Docs\n====");
        assert_eq!(heading("文書"), "文書\n====");
    }

    #[test]
    fn placeholders_are_replaced() {
        let values = [("project", "Demo".to_string())];
        assert_eq!(substitute("p = \"{{project}}\" {x}", &values), "p = \"Demo\" {x}");
        assert_eq!(toml_text("a \"b\"\\\n"), "a \\\"b\\\"\\\\ ");
    }
}
