//! Small, complete sites built from the same templates and pipeline as existing projects.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use gd_host::{Problem, io_problem};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Distinct starting structures; none selects a separate rendering engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// A project home and a documentation guide in reading order.
    Project,
    /// A chronological journal, feeds and taxonomy archives.
    Journal,
    /// A paper collection with abstracts, mathematics and standalone PDFs.
    Research,
}

impl Preset {
    /// Parses the command's preset name; None for unknown names.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "project" => Some(Self::Project),
            "journal" => Some(Self::Journal),
            "research" => Some(Self::Research),
            _ => None,
        }
    }
}

/// Creates a new project directory with editable theme copies and a working example.
/// Refuses an existing destination and invalid title/date before writing project files.
pub fn create_site(
    path: &Path,
    preset: Preset,
    title: &str,
    author: &str,
    date: &str,
) -> Result<(), Problem> {
    if title.trim().is_empty() || title.chars().chain(author.chars()).any(char::is_control) {
        return Err(problem(path, "The site title and author must be single-line text."));
    }
    gd_clock::parse_date(date).map_err(|e| problem(path, &e.to_string()))?;
    if std::fs::symlink_metadata(path).is_ok() {
        return Err(problem(path, "The destination already exists."));
    }
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| io_problem(parent, &e))?;
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let stage = parent.join(format!(".ssg-init-{}-{serial}", std::process::id()));
    std::fs::create_dir(&stage).map_err(|e| io_problem(&stage, &e))?;
    let outcome = populate(&stage, preset, title, author, date).and_then(|()| {
        if std::fs::symlink_metadata(path).is_ok() {
            return Err(problem(path, "The destination was created during initialization."));
        }
        std::fs::rename(&stage, path).map_err(|e| io_problem(path, &e))?;
        gd_host::sync_each(&[parent.to_path_buf()])
    });
    if stage.exists() {
        let _ = std::fs::remove_dir_all(stage);
    }
    outcome
}

fn populate(
    root: &Path,
    preset: Preset,
    title: &str,
    author: &str,
    date: &str,
) -> Result<(), Problem> {
    let mut files: Vec<(String, String)> =
        gd_theme::FILES.iter().map(|f| (f.path.into(), f.text.into())).collect();
    files.push(("conf.toml".into(), config(preset, title, author)));
    files.push((".gitignore".into(), "_build/\n_themes/\n.guidedog-ssg.lock\n".into()));
    files.push(("index.rst".into(), index(preset, title)));
    files.push(("_static/ssg.css".into(), stylesheet(preset).into()));
    match preset {
        Preset::Project => files.push((
            "guide.rst".into(),
            "Guide\n=====\n\nWe start with a small example and explain why it works.\n".into(),
        )),
        Preset::Journal => files.push((
            "posts/welcome.rst".into(),
            include_str!("../assets/welcome.rst").replace("@DATE@", date),
        )),
        Preset::Research => files.push((
            "papers/first-paper.rst".into(),
            include_str!("../assets/paper.rst").replace("@DATE@", date),
        )),
    }
    let mut synced = Vec::new();
    for (file, text) in files {
        let path = root.join(file);
        let parent =
            path.parent().ok_or_else(|| problem(&path, "A generated path has no parent."))?;
        std::fs::create_dir_all(parent).map_err(|e| io_problem(parent, &e))?;
        gd_host::write_unsynced(&path, text.as_bytes())?;
        synced.push(path.clone());
        let mut parent = path.parent();
        while let Some(dir) = parent {
            synced.push(dir.to_path_buf());
            if dir == root {
                break;
            }
            parent = dir.parent();
        }
    }
    synced.sort();
    synced.dedup();
    gd_host::sync_each(&synced)
}

fn config(preset: Preset, title: &str, author: &str) -> String {
    let mut out = String::from("project = ");
    gd_toml::write_basic_string(&mut out, title);
    out.push_str("\nauthor = ");
    gd_toml::write_basic_string(&mut out, author);
    out.push_str(concat!(
        "\nlanguage = \"en\"\nextensions = [\"myst_parser\", \"sphinx.ext.mathjax\"]\n",
        "source_suffix = [\".rst\", \".md\"]\ntemplates_path = [\"_templates\"]\n",
        "exclude_patterns = [\"_build\", \"_themes/**\", \"README*\"]\n",
        "html_static_path = [\"_static\"]\nhtml_css_files = [\"ssg.css\"]\n"
    ));
    let (accent, size, output) = match preset {
        Preset::Project => ("#126d78", 0, ""),
        Preset::Journal => ("#9b4130", 10, "journal"),
        Preset::Research => ("#394b80", 12, "papers"),
    };
    out.push_str(&format!("\n[html_theme_options]\naccent = \"{accent}\"\n"));
    if !output.is_empty() {
        out.push_str(&format!(
            concat!(
                "\n[site]\nbase_url = \"https://example.org\"\nsitemap = true\nfeeds = true\n",
                "taxonomies = [\"tags\", \"category\", \"year\"]\npage_size = {size}\n",
                "\n[[site.collections]]\noutput = \"{output}\"\ntitle = \"{title}\"\n",
                "page_size = {size}\nfeeds = true\n"
            ),
            size = size,
            output = output,
            title = if preset == Preset::Research { "Papers" } else { "Journal" }
        ));
    }
    out
}

fn index(preset: Preset, title: &str) -> String {
    let mut out =
        format!(":template: landing.html\n\n{title}\n{}\n\n", "=".repeat(title.len().max(3)));
    out.push_str(match preset {
        Preset::Project => include_str!("../assets/project.rst"),
        Preset::Journal => include_str!("../assets/journal.rst"),
        Preset::Research => include_str!("../assets/research.rst"),
    });
    out
}

fn stylesheet(preset: Preset) -> &'static str {
    match preset {
        Preset::Project => ".gd-body > h1 { max-width: 22ch; }\n",
        Preset::Journal => {
            ".gd-body { max-width: 58rem; }\n.postlist > ul > li { margin-block: 1.5rem; }\n"
        }
        Preset::Research => {
            concat!(
                ".gd-body { max-width: 48rem; }\n",
                ".gd-article-meta { border-block-end: 1px solid #aaa; }\n"
            )
        }
    }
}

fn problem(path: &Path, message: &str) -> Problem {
    Problem::new(
        "ssg.init",
        "SITE INITIALIZATION REFUSED",
        &path.display().to_string(),
        message,
        "Choose a new directory and a project, journal or research preset.",
        2,
    )
}
