//! Builds mounted projects first, then publishes the complete website transactionally.

use std::path::{Path, PathBuf};

use gd_conf::{Config, SiteEdition, SiteMount};
use gd_host::{Budget, Problem, io_problem};
use gd_plugins::BuildRequest;

/// Controls one whole-site build. Project and edition overrides remain in conf.toml.
#[derive(Debug, Clone, Default)]
pub struct SiteBuildOptions {
    /// Final output directory; None selects _build/site beneath the site project.
    pub output: Option<PathBuf>,
    /// Skip PDF mounts when inspecting HTML locally.
    pub no_pdf: bool,
    /// Include withheld sources; use a separate output for previews.
    pub preview: bool,
    /// Fixed RFC 3339 clock; empty fixes the clock once at command start.
    pub now: String,
}

pub(crate) struct Mounted {
    pub key: String,
    pub output: String,
    pub built: PathBuf,
    pub exclude: Vec<String>,
    pub pdf: bool,
    pub manifest: PathBuf,
}

/// Builds every configured mount through `build` and atomically replaces the assembled site.
/// A failed build, overlapping output or unsafe source keeps the preceding site intact.
pub fn build_site(
    root: &Path,
    options: &SiteBuildOptions,
    build: &mut dyn FnMut(&BuildRequest) -> Result<(), Problem>,
) -> Result<PathBuf, Problem> {
    let config = gd_conf::load_config(root, &Default::default())?.config;
    let root = &config.source_dir;
    let work = config.build_dir.join("ssg-projects");
    std::fs::create_dir_all(&work).map_err(|e| io_problem(&work, &e))?;
    let lock_path = work.join("site.lock");
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| io_problem(&lock_path, &e))?;
    lock.try_lock()
        .map_err(|_| problem(root, "Another site build holds the publication lock."))?;
    let now = fixed_clock(&options.now)?;
    let mut mounted = Vec::new();
    let mounts = if config.site.mounts.is_empty() {
        vec![SiteMount {
            name: "site".into(),
            source: ".".into(),
            builder: "html".into(),
            ..Default::default()
        }]
    } else {
        config.site.mounts.clone()
    };
    for mount in &mounts {
        if options.no_pdf && matches!(mount.builder.as_str(), "pdf" | "discussion-pdf") {
            continue;
        }
        for edition in editions(mount) {
            let instance = build_mount(&config, mount, &edition, &work, options, &now, build)?;
            mounted.push(instance);
        }
    }
    if mounted.is_empty() {
        return Err(problem(root, "No mounted projects were selected."));
    }
    crate::assemble::publish(&config, options, &mounted, &Budget::new(gd_env::BUILD_BUDGET))
}

fn editions(mount: &SiteMount) -> Vec<SiteEdition> {
    if mount.editions.is_empty() { vec![SiteEdition::default()] } else { mount.editions.clone() }
}

fn build_mount(
    config: &Config,
    mount: &SiteMount,
    edition: &SiteEdition,
    work: &Path,
    options: &SiteBuildOptions,
    now: &str,
    build: &mut dyn FnMut(&BuildRequest) -> Result<(), Problem>,
) -> Result<Mounted, Problem> {
    let source = if edition.source.is_empty() { &mount.source } else { &edition.source };
    let source = confined_source(&config.source_dir, source)?;
    let mut edition = edition.clone();
    if edition.language.is_empty() {
        edition.language = gd_conf::load_config(&source, &Default::default())?.config.language;
        if edition.language.is_empty() {
            edition.language = "en".into();
        }
    }
    let language = &edition.language;
    let key = format!("{}/{language}", mount.name);
    let built = work.join(&key);
    let output = join(&mount.output, &edition.output);
    let mut overrides = overrides(config, mount, &edition, &output);
    overrides.push(format!("html_context.ssg_no_pdf={}", options.no_pdf));
    let mut builder = mount.builder.clone();
    if builder == "discussion-pdf" {
        overrides.push(discussion_books(&source, &mount.prefix)?);
        builder = "pdf".into();
    }
    build(&BuildRequest {
        project: source,
        output: built.clone(),
        builder,
        overrides,
        strict: true,
        now: now.into(),
        preview: options.preview,
        ..Default::default()
    })?;
    let kind = if mount.builder.ends_with("pdf") {
        gd_env::BuilderKind::Pdf
    } else if mount.builder == "dirhtml" {
        gd_env::BuilderKind::Dirhtml
    } else {
        gd_env::BuilderKind::Html
    };
    let manifest = gd_outputs::output_paths(config, kind, Some(&built))?.manifest;
    Ok(Mounted {
        key,
        output,
        built,
        exclude: mount.exclude.clone(),
        pdf: mount.builder.ends_with("pdf"),
        manifest,
    })
}

fn overrides(
    config: &Config,
    mount: &SiteMount,
    edition: &SiteEdition,
    output: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    for (name, value) in [("release", &config.release), ("version", &config.version)] {
        if !value.is_empty() {
            out.push(format!("{name}={value}"));
        }
    }
    if !edition.language.is_empty() {
        out.push(format!("language={}", edition.language));
    }
    // The linker resolves mount://NAME/file against these public prefixes.
    out.push(setting("html_context.ssg_prefix", output));
    out.push(setting("html_context.ssg_base_url", &config.site.base_url));
    if !config.site.base_url.is_empty() {
        let url = join(&config.site.base_url, output);
        out.push(setting("site.base_url", &url));
    }
    let mut links = String::from("html_context.ssg_mounts={");
    for target in &config.site.mounts {
        let ed = target
            .editions
            .iter()
            .find(|e| {
                e.language
                    .replace('-', "_")
                    .eq_ignore_ascii_case(&edition.language.replace('-', "_"))
            })
            .or_else(|| target.editions.iter().find(|e| e.output.is_empty()));
        let prefix = join(&target.output, ed.map_or("", |e| e.output.as_str()));
        gd_toml::write_basic_string(&mut links, &target.name);
        links.push('=');
        gd_toml::write_basic_string(&mut links, &prefix);
        links.push(',');
    }
    if links.ends_with(',') {
        links.pop();
    }
    links.push('}');
    out.push(links);
    for value in mount.overrides.iter().chain(&edition.overrides) {
        out.push(expand(value, config, edition, output));
    }
    out
}

fn expand(text: &str, config: &Config, edition: &SiteEdition, output: &str) -> String {
    let root = "../".repeat(output.split('/').filter(|s| !s.is_empty()).count());
    text.replace("{language}", &edition.language)
        .replace("{edition}", &edition.output)
        .replace("{site_root}", &root)
        .replace("{base_url}", &config.site.base_url)
}

fn setting(name: &str, text: &str) -> String {
    let mut value = format!("{name}=");
    gd_toml::write_basic_string(&mut value, text);
    value
}

fn discussion_books(source: &Path, prefix: &str) -> Result<String, Problem> {
    let prefix = gd_discussion::Prefix::new(if prefix.is_empty() { "gds" } else { prefix })
        .map_err(|e| problem(source, &e.to_string()))?;
    let catalog = gd_discussion::load_catalog(source, &prefix)
        .map_err(|e| problem(source, &e.to_string()))?;
    gd_discussion::check_sources(&catalog).map_err(|e| problem(source, &e.to_string()))?;
    gd_discussion::check_index(&catalog).map_err(|e| problem(source, &e.to_string()))?;
    let mut out = String::from("pdf_documents=[");
    for record in catalog.records.iter().filter(|r| !r.historical) {
        out.push('{');
        for (key, text) in [
            ("root", format!("records/{}", record.stem())),
            ("file", format!("{}.pdf", gd_discussion::artifact_name(&prefix, record))),
            ("title", record.title.clone()),
            ("author", record.authors.join(", ")),
        ] {
            out.push_str(key);
            out.push('=');
            gd_toml::write_basic_string(&mut out, &text);
            out.push(',');
        }
        out.pop();
        out.push_str("},");
    }
    if out.ends_with(',') {
        out.pop();
    }
    out.push(']');
    Ok(out)
}

pub(crate) fn confined_source(root: &Path, source: &str) -> Result<PathBuf, Problem> {
    let path = root.join(source);
    let canonical = path.canonicalize().map_err(|e| io_problem(&path, &e))?;
    let root = root.canonicalize().map_err(|e| io_problem(root, &e))?;
    if !canonical.starts_with(&root) {
        return Err(problem(&path, "The site source escapes its root."));
    }
    Ok(canonical)
}

pub(crate) fn join(prefix: &str, path: &str) -> String {
    if prefix.is_empty() {
        path.into()
    } else if path.is_empty() {
        prefix.into()
    } else {
        format!("{prefix}/{path}")
    }
}

fn fixed_clock(value: &str) -> Result<String, Problem> {
    if !value.is_empty() {
        gd_clock::parse_timestamp(value)
            .map_err(|e| problem(Path::new("--now"), &e.to_string()))?;
        return Ok(value.into());
    }
    let time = gd_clock::build_time().utc();
    Ok(format!("{}T{:02}:{:02}:{:02}Z", time.date, time.hour, time.minute, time.second))
}

pub(crate) fn problem(path: &Path, message: &str) -> Problem {
    Problem::new(
        "ssg.site",
        "SITE BUILD REFUSED",
        &path.display().to_string(),
        message,
        "Correct the site mounts or output paths in conf.toml, then rebuild.",
        2,
    )
}
