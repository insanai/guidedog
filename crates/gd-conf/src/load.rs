//! Finding and loading a project's configuration: `find_project`, `load_config`,
//! `apply_override` (`-D name=value`) and `finish_config`.

use std::path::{Path, PathBuf};

use gd_host::Problem;
use gd_report::Report;
use gd_toml::{Data, Table};

use crate::check::check_config;
use crate::config::{CONFIG_FILE, Config, PdfDocument};
use crate::digest::{Digest, Digester};
use crate::paths::absolute;
use crate::settings::{SettingKind, find_setting};
use crate::store::{apply_table, config_problem};

/// The largest conf.toml read (1 MiB).
pub const MAX_CONFIG_BYTES: usize = 1 << 20;

/// The command line's say in where a project is and how it is set. Fill it with
/// struct-update syntax: `LoadOptions { no_config: true, ..Default::default() }`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadOptions {
    /// `-c`: the folder conf.toml is in, when not the source folder.
    pub conf_dir: Option<PathBuf>,
    /// `-C`: build with the defaults only.
    pub no_config: bool,
    /// `-D name=value`, applied after conf.toml, in order.
    pub overrides: Vec<String>,
    /// The sphinx-build form's explicit source folder.
    pub source: Option<PathBuf>,
    /// The sphinx-build form's explicit output folder.
    pub output: Option<PathBuf>,
}

/// A loaded configuration.
#[derive(Debug, Clone)]
pub struct Loaded {
    /// The settings, checked and finished.
    pub config: Config,
    /// What loading warned about: ignored Sphinx settings, a substituted theme, ...
    pub warnings: Vec<Report>,
    /// The digest of conf.toml's text and the `-D` overrides: what every output uses.
    pub settings: Digest,
}

/// Finds a project from `dir`: conf.toml in `dir`, in `dir/source` (the separated
/// layout), or in the nearest parent folder that has one. Returns the source folder and
/// the build folder (`_build` beside conf.toml, or `build` beside `source`), both
/// absolute; `None` when there is no project.
pub fn find_project(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let mut current = absolute(dir);
    loop {
        if current.join(CONFIG_FILE).is_file() {
            let parent = current.parent().map(Path::to_path_buf).unwrap_or_default();
            let separated = current.file_name().is_some_and(|n| n == "source");
            if separated && parent.join("build").is_dir() {
                return Some((current, parent.join("build")));
            }
            let build = current.join("_build");
            return Some((current, build));
        }
        let source = current.join("source");
        if source.join(CONFIG_FILE).is_file() {
            return Some((source, current.join("build")));
        }
        current = current.parent()?.to_path_buf();
    }
}

/// Loads the project found from `dir` (see [`find_project`]), or the explicit folders
/// in `options`: Sphinx's defaults, then conf.toml, then the `-D` overrides, checked
/// together once all are applied (so `-D html_extra_path=[]` keeps conf.toml's missing
/// folder from being reported, as with sphinx-build), then the derived settings.
///
/// Fails with `project.missing` (exit 2) when there is no conf.toml and `no_config` is
/// not set, with the problem of reading conf.toml, or with `config.invalid` /
/// `cli.define` (exit 2) for a setting that is not right.
pub fn load_config(dir: &Path, options: &LoadOptions) -> Result<Loaded, Problem> {
    let found = match &options.source {
        Some(source) => Some((absolute(source), options.output.clone().unwrap_or_default())),
        None => find_project(dir),
    };
    let (source, build) = match found {
        Some(folders) => folders,
        None if options.no_config => (absolute(dir), absolute(dir).join("_build")),
        None => {
            return Err(Problem::new(
                "project.missing",
                "NO DOCUMENTATION PROJECT HERE",
                &dir.display().to_string(),
                "No conf.toml was found here, in a source/ folder, or in a parent directory.",
                "Run guidedog quickstart to create a project, or pass the project folder.",
                2,
            ));
        }
    };
    let build = if build.as_os_str().is_empty() { build } else { absolute(&build) };
    let mut config = Config::new(source.clone(), build);
    let mut warnings = Vec::new();
    let mut digest = Digester::new();
    let later = !options.overrides.is_empty();
    let mut origin = "-D".to_string();
    if !options.no_config {
        let path = options.conf_dir.as_deref().unwrap_or(&source).join(CONFIG_FILE);
        origin = path.display().to_string();
        let text = read_config(&mut config, &path, &mut warnings, !later)?;
        digest.text(&text);
    }
    for o in &options.overrides {
        apply_override(&mut config, o, &mut warnings, false)?;
    }
    if later {
        check_config(&mut config, &origin, &mut warnings)?;
    }
    finish_config(&mut config);
    digest.texts(&options.overrides);
    Ok(Loaded { config, warnings, settings: digest.finish() })
}

/// Reads conf.toml at `path` into `config` and returns its text.
fn read_config(
    config: &mut Config,
    path: &Path,
    warnings: &mut Vec<Report>,
    check: bool,
) -> Result<String, Problem> {
    let shown = path.display().to_string();
    let text = gd_host::read_source(&shown, MAX_CONFIG_BYTES)?.text;
    config.config_dir = absolute(path).parent().map(Path::to_path_buf).unwrap_or_default();
    let table = gd_toml::parse(&text).map_err(|e| {
        config_problem(
            &shown,
            e.line,
            e.message.clone(),
            "Fix the line; guidedog quickstart writes a complete example.",
        )
    })?;
    apply_table(config, &table, &shown, warnings, check)?;
    Ok(text)
}

fn define_problem(assignment: &str, message: String, hint: &str) -> Problem {
    Problem::new("cli.define", "INVALID SETTING OVERRIDE", assignment, message, hint, 2)
}

/// Applies `-D name=value` to `config`. The value is read as TOML when it parses as
/// one, so `-D numfig=true` and `-D exclude_patterns='["drafts"]'` work; otherwise it
/// is a string. A text setting takes the value as written (`-D version=0.2.0` is the
/// text `0.2.0`), and a boolean also takes 0 and 1, as with sphinx-build.
/// `-D html_context.key=value` sets one key of the table and keeps the others.
///
/// With `check`, the settings are checked together afterwards. Fails with `cli.define`
/// (exit 2) for an assignment that is not `name=value` or a value that does not parse,
/// and like [`apply_table`] for a setting that is not right.
pub fn apply_override(
    config: &mut Config,
    assignment: &str,
    warnings: &mut Vec<Report>,
    check: bool,
) -> Result<(), Problem> {
    let Some((name, value)) = assignment.split_once('=') else {
        return Err(define_problem(
            assignment,
            "-D takes name=value.".into(),
            "Write -D numfig=true, for example.",
        ));
    };
    let kind = find_setting(name).map(|s| s.kind);
    let value = match (kind, value) {
        (Some(SettingKind::Bool), "0") => "false",
        (Some(SettingKind::Bool), "1") => "true",
        _ => value,
    };
    let quoted = || {
        let mut text = format!("{name} = ");
        gd_toml::write_string(&mut text, value);
        gd_toml::parse(&text)
    };
    let as_text = kind == Some(SettingKind::String) && !value.starts_with(['"', '\'']);
    let parsed = if as_text {
        quoted()
    } else {
        gd_toml::parse(&format!("{name} = {value}")).or_else(|_| quoted())
    };
    let table = parsed.map_err(|e| define_problem(assignment, e.message, "Quote the value."))?;
    if merge_into_values(config, name, &table) {
        return if check { check_config(config, "-D", warnings) } else { Ok(()) };
    }
    apply_table(config, &table, "-D", warnings, check)
}

/// Merges `-D table.key=value` into a table setting kept as written (`html_context`);
/// false when `name` is not such an assignment.
fn merge_into_values(config: &mut Config, name: &str, table: &Table) -> bool {
    let Some((base, _)) = name.split_once('.') else { return false };
    let Some(setting) = find_setting(base) else { return false };
    if setting.kind != SettingKind::Values || table.len() != 1 {
        return false;
    }
    let Some(Data::Table(src)) = table.get(base).map(|v| &v.data) else { return false };
    let crate::settings::Store::Values(field) = setting.store else { return false };
    merge_table(field(config), src);
    true
}

/// Sets each key of `src` in `dst`, merging the tables both have key by key.
pub fn merge_table(dst: &mut Table, src: &Table) {
    for (key, value) in src.iter() {
        if let (Some(existing), Data::Table(inner)) = (dst.get_mut(key), &value.data)
            && let Data::Table(target) = &mut existing.data
        {
            merge_table(target, inner);
            continue;
        }
        dst.insert(key, value.clone());
    }
}

/// Fills the settings Sphinx derives from others: `.md` documents with `myst_parser`,
/// `release` from `version`, the HTML titles from the project, and one book of the root
/// document when `pdf_documents` is unset.
pub fn finish_config(config: &mut Config) {
    if config.has_extension("myst_parser") && !config.source_suffix.iter().any(|s| s == ".md") {
        config.source_suffix.push(".md".into());
    }
    if config.release.is_empty() {
        config.release = config.version.clone();
    }
    let html = &mut config.html;
    html.title_derived = html.title.is_empty();
    html.short_title_derived = html.short_title.is_empty();
    if html.title.is_empty() {
        html.title = if config.release.is_empty() {
            format!("{} documentation", config.project)
        } else {
            format!("{} {} documentation", config.project, config.release)
        };
    }
    if html.short_title.is_empty() {
        html.short_title = html.title.clone();
    }
    if config.pdf.documents.is_empty() {
        let mut file = String::new();
        guidedoc::write_id(&mut file, &config.project);
        file.push_str(".pdf");
        config.pdf.documents.push(PdfDocument {
            root: config.root_doc.clone(),
            file,
            title: config.project.clone(),
            author: config.author.clone(),
        });
    }
}
