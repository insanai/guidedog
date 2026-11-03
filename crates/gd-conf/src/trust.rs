//! Trust: what a build may read. A trusted build reads files below the source folder
//! and the folders conf.toml names (`include_roots`, `autodoc_source_paths`, ...), since
//! the project is the user's own. A confined build (`--untrusted`, or a program that
//! builds a project it did not write) does not let the project widen what it reads:
//! `confine` leaves out every setting that leads outside the given folders, each with a
//! warning, before the build discovers or reads anything (GDS 0004, GDS 0018).

use std::path::{Path, PathBuf};

use gd_host::Confinement;
use gd_report::Report;

use crate::config::Config;
use crate::paths::source_path;
use crate::store::warning;

/// The `typst` setting that selects the embedded compiler.
const EMBEDDED_TYPST: &str = "typst";

/// Confines `config` to the source folder: [`confine`] with no other folder and no
/// linked plugin. This is what an `--untrusted` build reads.
pub fn confine_untrusted(config: &mut Config) -> Vec<Report> {
    confine(config, &[], &[])
}

/// Confines `config` so that a build reads only below the source folder and `roots`
/// (absolute, or relative to the source folder; links are followed), and returns a
/// `build.untrusted` warning for each setting it changes or drops:
///
/// - `include_roots`, the folder settings (`html_static_path`, `html_extra_path`,
///   `templates_path`, `locale_dirs`, `pdf_font_paths`, `autodoc_source_paths`,
///   `odin_autoapi_dirs`, `rust_autoapi_crates`) and the file settings (`html_logo`,
///   `html_favicon`, `pdf_logo`, `mathjax_config_path`, `pdf_preamble`, and each file
///   of `plugin_configs`) that lead outside those folders;
/// - `typst` naming a program: a confined build runs the embedded compiler only;
/// - each of `plugins` not in `linked_plugins` (the plugins linked into the calling
///   program): a plugin from the machine registry is a program that reads anything.
///
/// It also sets `pdf_packages = "offline"`, so Typst downloads no package. Borrows
/// `config` for the call; never fails. The caller builds with the result alone.
pub fn confine(config: &mut Config, roots: &[PathBuf], linked_plugins: &[String]) -> Vec<Report> {
    let conf = config.config_dir.join(crate::CONFIG_FILE).display().to_string();
    let mut allowed = Vec::with_capacity(1 + roots.len());
    allowed.push(config.source_dir.clone());
    allowed.extend(roots.iter().map(|root| config.source_dir.join(root)));
    let allowed = gd_host::canonical_roots(&allowed);
    let mut reports = confine_folders(config, &allowed, &conf);
    reports.extend(confine_files(config, &allowed, &conf));
    reports.extend(drop_machine_plugins(config, linked_plugins));
    if !config.pdf.typst.is_empty() && config.pdf.typst != EMBEDDED_TYPST {
        let message = format!(
            "typst names the program {}, which a confined build does not run; it uses the \
             embedded Typst compiler.",
            config.pdf.typst
        );
        reports.push(confinement_warning(&conf, message));
        config.pdf.typst = EMBEDDED_TYPST.into();
    }
    config.pdf.packages = "offline".into();
    reports
}

/// Drops each of `plugins` that is not in `linked_plugins` and returns a
/// `build.untrusted` warning for each: a build that runs only the plugins linked into
/// its program starts none from the machine registry. Borrows `config` for the call.
pub fn drop_machine_plugins(config: &mut Config, linked_plugins: &[String]) -> Vec<Report> {
    let conf = config.config_dir.join(crate::CONFIG_FILE).display().to_string();
    let mut reports = Vec::new();
    config.plugins.ids.retain(|id| {
        let linked = linked_plugins.contains(id);
        if !linked {
            let message = format!(
                "plugins lists {id}, which is not linked into this program; a confined build \
                 starts no plugin from the machine registry."
            );
            reports.push(confinement_warning(&conf, message));
        }
        linked
    });
    reports
}

/// Keeps the folder settings and the include roots that lead inside `allowed`.
fn confine_folders(config: &mut Config, allowed: &[PathBuf], conf: &str) -> Vec<Report> {
    let mut reports = Vec::new();
    for (name, field) in CONFINED_FOLDERS {
        let mut kept = std::mem::take(field(config));
        kept.retain(|folder| {
            let keep = gd_host::confined(allowed, &source_path(config, folder));
            if !keep {
                reports.push(outside_warning(conf, name, folder));
            }
            keep
        });
        *field(config) = kept;
    }
    reports
}

/// Keeps the file settings that lead inside `allowed` (or name a URL).
fn confine_files(config: &mut Config, allowed: &[PathBuf], conf: &str) -> Vec<Report> {
    let mut reports = Vec::new();
    for (name, field) in CONFINED_FILES {
        let file = field(config).clone();
        if file.is_empty() || file.contains("://") {
            continue;
        }
        if !gd_host::confined(allowed, &source_path(config, &file)) {
            reports.push(outside_warning(conf, name, &file));
            field(config).clear();
        }
    }
    let config_dir = config.config_dir.clone();
    config.plugins.configs.retain(|id, file| {
        let keep = gd_host::confined(allowed, &crate::clean_path(&config_dir.join(&*file)));
        if !keep {
            reports.push(outside_warning(conf, &format!("plugin_configs.{id}"), file));
        }
        keep
    });
    reports
}

/// A file setting's field.
type FileField = fn(&mut Config) -> &mut String;
const CONFINED_FILES: [(&str, FileField); 5] = [
    ("html_logo", |c| &mut c.html.logo),
    ("html_favicon", |c| &mut c.html.favicon),
    ("pdf_logo", |c| &mut c.pdf.logo),
    ("mathjax_config_path", |c| &mut c.mathjax.config_path),
    ("pdf_preamble", |c| &mut c.pdf.preamble),
];

/// The folder settings a confined build keeps only inside its folders.
type FolderField = fn(&mut Config) -> &mut Vec<String>;
const CONFINED_FOLDERS: [(&str, FolderField); 9] = [
    ("include_roots", |c| &mut c.include_roots),
    ("html_static_path", |c| &mut c.html.static_path),
    ("html_extra_path", |c| &mut c.html.extra_path),
    ("templates_path", |c| &mut c.templates_path),
    ("locale_dirs", |c| &mut c.intl.locale_dirs),
    ("pdf_font_paths", |c| &mut c.pdf.font_paths),
    ("autodoc_source_paths", |c| &mut c.autodoc.source_paths),
    ("odin_autoapi_dirs", |c| &mut c.odin_api.dirs),
    ("rust_autoapi_crates", |c| &mut c.rust_api.crates),
];

fn outside_warning(conf: &str, setting: &str, value: &str) -> Report {
    let message = format!(
        "{setting} names {value}, outside the folders this build may read, so the build does \
         not read it."
    );
    confinement_warning(conf, message)
}

fn confinement_warning(conf: &str, message: String) -> Report {
    let mut r = warning(
        conf,
        0,
        "build.untrusted",
        "SETTING NOT USED",
        message,
        "Keep what the project reads below its source folder, or build without --untrusted \
         if we trust this project.",
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
