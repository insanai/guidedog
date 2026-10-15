//! Trust: what a build may read. A trusted build reads files below the source folder
//! and the folders conf.toml names (`include_roots`, `autodoc_source_paths`, ...), since
//! the project is the user's own. An `--untrusted` build does not let the project widen
//! what it reads: `confine_untrusted` leaves out every setting that leads elsewhere,
//! each with a warning, before the build discovers or reads anything.

use std::path::{Path, PathBuf};

use gd_host::Confinement;
use gd_report::Report;

use crate::config::Config;
use crate::paths::source_path;
use crate::store::warning;

/// Confines `config` for an `--untrusted` build and returns a `build.untrusted` warning
/// for each setting it drops: `include_roots`; `pdf_preamble` (Typst the project wrote);
/// files (`html_logo`, `html_favicon`, `pdf_logo`, `mathjax_config_path`) and folders
/// (`html_static_path`, `html_extra_path`, `templates_path`, `locale_dirs`,
/// `pdf_font_paths`, `autodoc_source_paths`, `odin_autoapi_dirs`,
/// `rust_autoapi_crates`) that lead outside the source folder once every link is
/// followed. No PDF package is downloaded (`pdf_packages = "offline"`).
pub fn confine_untrusted(config: &mut Config) -> Vec<Report> {
    let conf = config.source_dir.join(crate::CONFIG_FILE).display().to_string();
    let mut reports = Vec::new();
    if !config.include_roots.is_empty() {
        reports.push(untrusted_warning(&conf, "include_roots", &config.include_roots.join(", ")));
        config.include_roots.clear();
    }
    if !config.pdf.preamble.is_empty() {
        let mut r = warning(
            &conf,
            0,
            "build.untrusted",
            "SETTING NOT USED",
            format!(
                "pdf_preamble names {}, Typst the project wrote, which an --untrusted build \
                 does not run.",
                config.pdf.preamble
            ),
            "Build without --untrusted if you trust this project.",
        );
        r.category = "build.untrusted".into();
        reports.push(r);
        config.pdf.preamble.clear();
    }
    config.pdf.packages = "offline".into();
    let source = gd_host::canonical_roots(std::slice::from_ref(&config.source_dir));
    let inside = |c: &Config, path: &str| gd_host::confined(&source, &source_path(c, path));
    let files: [(&str, FileField); 4] = [
        ("html_logo", |c| &mut c.html.logo),
        ("html_favicon", |c| &mut c.html.favicon),
        ("pdf_logo", |c| &mut c.pdf.logo),
        ("mathjax_config_path", |c| &mut c.mathjax.config_path),
    ];
    for (name, field) in files {
        let file = field(config).clone();
        if file.is_empty() || file.contains("://") || inside(config, &file) {
            continue;
        }
        reports.push(untrusted_warning(&conf, name, &file));
        field(config).clear();
    }
    for (name, field) in UNTRUSTED_FOLDERS {
        let mut kept = std::mem::take(field(config));
        kept.retain(|folder| {
            let keep = inside(config, folder);
            if !keep {
                reports.push(untrusted_warning(&conf, name, folder));
            }
            keep
        });
        *field(config) = kept;
    }
    reports
}

/// A file setting's field.
type FileField = fn(&mut Config) -> &mut String;

/// The folder settings an untrusted build keeps only below the source folder.
type FolderField = fn(&mut Config) -> &mut Vec<String>;
const UNTRUSTED_FOLDERS: [(&str, FolderField); 8] = [
    ("html_static_path", |c| &mut c.html.static_path),
    ("html_extra_path", |c| &mut c.html.extra_path),
    ("templates_path", |c| &mut c.templates_path),
    ("locale_dirs", |c| &mut c.intl.locale_dirs),
    ("pdf_font_paths", |c| &mut c.pdf.font_paths),
    ("autodoc_source_paths", |c| &mut c.autodoc.source_paths),
    ("odin_autoapi_dirs", |c| &mut c.odin_api.dirs),
    ("rust_autoapi_crates", |c| &mut c.rust_api.crates),
];

fn untrusted_warning(conf: &str, setting: &str, value: &str) -> Report {
    let mut r = warning(
        conf,
        0,
        "build.untrusted",
        "SETTING NOT USED",
        format!(
            "{setting} names {value}, outside the source folder, which an --untrusted build \
             does not read."
        ),
        "Build without --untrusted if you trust this project.",
    );
    r.category = "build.untrusted".into();
    r
}

/// Explains a file or folder conf.toml leads a build to (a static file, a template)
/// that it does not read, since it lies outside the source folder and the include
/// roots: named there (`Outside`, code `<code>.outside`) or reached through a link
/// (`ThroughLink`, code `<code>.link`). `what` names it ("folder", "file",
/// "templates_path folder"). Returns `None` for a path that is `Inside`.
pub fn unread_warning(
    code: &str,
    title: &str,
    path: &Path,
    what: &str,
    why: Confinement,
) -> Option<Report> {
    let (suffix, message, hint) = match why {
        Confinement::Inside => return None,
        Confinement::ThroughLink => (
            "link",
            format!(
                "This {what} is a link that leads outside the source folder and the include \
                 roots, which a build does not read."
            ),
            "Add the folder it leads to to include_roots, or copy it here.",
        ),
        Confinement::Outside => (
            "outside",
            format!(
                "This {what} is outside the source folder and the include roots, which a \
                 build does not read."
            ),
            "Add its folder to include_roots, or move it below the source folder.",
        ),
    };
    let code = format!("{code}.{suffix}");
    Some(Report {
        path: path.display().to_string(),
        ..Report::warning(&code, title, &message).hint(hint)
    })
}

/// The `rust_autoapi_crates` folders, relative to the source folder, cleaned: they
/// join the folders a trusted build reads (`gd_read` confines the crate reader to them).
pub fn rust_crate_roots(config: &Config) -> Vec<PathBuf> {
    config.rust_api.crates.iter().map(|c| source_path(config, c)).collect()
}
