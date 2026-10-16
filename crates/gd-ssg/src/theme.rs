//! Theme downloads are explicit commands; builds only verify the installed pin.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use gd_host::{Problem, io_problem};
use gd_plugins::{CommandContext, CommandInfo, CommandOption};
use gd_theme::pack::{self, Lock};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Fetches a pinned version-one archive, installs its immutable tree and replaces the lock.
/// A checksum mismatch or filesystem conflict preserves the preceding selected theme.
pub fn install_theme(project: &Path, source: &str, sha256: &str) -> Result<Lock, Problem> {
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(problem(project, "An explicit 64-digit archive SHA-256 is required."));
    }
    let archive = gd_fetch::get(
        source,
        &gd_fetch::Options { max_bytes: 2 * pack::MAX_BYTES, ..Default::default() },
    )
    .map_err(|e| problem(project, &e.to_string()))?;
    let actual = pack::archive_digest(&archive.data);
    if actual != sha256.to_ascii_lowercase() {
        return Err(problem(project, "Downloaded theme does not match the archive pin."));
    }
    let files = pack::decode(&archive.data).map_err(|e| problem(project, &e.to_string()))?;
    let lock = Lock {
        source: source.into(),
        archive_sha256: actual,
        tree_sha256: pack::tree_digest(&files),
    };
    install_tree(project, &lock, &files)?;
    let path = project.join(pack::LOCK_FILE);
    let before = if path.try_exists().map_err(|e| io_problem(&path, &e))? {
        Some(crate::lifecycle::read_source(&path)?)
    } else {
        None
    };
    crate::transaction::write_source(&path, before.as_deref(), &lock.encode())?;
    pack::verify(project).map_err(|e| problem(project, &e.to_string()))?;
    Ok(lock)
}

fn install_tree(project: &Path, lock: &Lock, files: &pack::Files) -> Result<(), Problem> {
    let root = project.join("_themes");
    std::fs::create_dir_all(&root).map_err(|e| io_problem(&root, &e))?;
    if !std::fs::symlink_metadata(&root).map_err(|e| io_problem(&root, &e))?.is_dir() {
        return Err(problem(&root, "The theme directory must not be a symbolic link."));
    }
    let installed = lock.directory(project);
    if installed.try_exists().map_err(|e| io_problem(&installed, &e))? {
        let tree = pack::read_tree(&installed).map_err(|e| problem(&installed, &e.to_string()))?;
        if pack::tree_digest(&tree) != lock.tree_sha256 {
            return Err(problem(
                &installed,
                "An existing theme directory has different contents.",
            ));
        }
        return Ok(());
    }
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let stage = root.join(format!(".install-{}-{serial}", std::process::id()));
    std::fs::create_dir(&stage).map_err(|e| io_problem(&stage, &e))?;
    let outcome = write_tree(&stage, files).and_then(|()| {
        std::fs::rename(&stage, &installed).map_err(|e| io_problem(&installed, &e))?;
        gd_host::sync_each(&[root])
    });
    if stage.exists() {
        let _ = std::fs::remove_dir_all(&stage);
    }
    outcome
}

fn write_tree(stage: &Path, files: &pack::Files) -> Result<(), Problem> {
    let mut paths = Vec::new();
    for (name, data) in files {
        let path = stage.join(name);
        let parent = path.parent().ok_or_else(|| problem(&path, "Theme file has no parent."))?;
        std::fs::create_dir_all(parent).map_err(|e| io_problem(parent, &e))?;
        gd_host::write_unsynced(&path, data)?;
        paths.push(path.clone());
        let mut directory = path.parent();
        while let Some(dir) = directory {
            paths.push(dir.to_path_buf());
            if dir == stage {
                break;
            }
            directory = dir.parent();
        }
    }
    paths.sort();
    paths.dedup();
    gd_host::sync_each(&paths)
}

pub(crate) fn commands() -> Vec<CommandInfo> {
    vec![
        CommandInfo::new("ssg theme pack", "Pack a portable theme directory")
            .with_help(concat!(
                "guidedog ssg theme pack DIR --out FILE\n",
                "Only _templates and _static are included."
            ))
            .with_option(CommandOption::with_value("--out", "FILE", "New theme archive")),
        CommandInfo::new("ssg theme install", "Install an explicitly pinned theme archive")
            .with_help("guidedog ssg theme install URL --sha256 HASH [--project DIR]")
            .with_option(CommandOption::with_value("--sha256", "HASH", "Required archive SHA-256"))
            .with_option(CommandOption::with_value("--project", "DIR", "Project root")),
        CommandInfo::new("ssg theme verify", "Verify the installed theme without network access")
            .with_help("guidedog ssg theme verify [PROJECT]"),
    ]
}

pub(crate) fn run(
    ctx: &mut CommandContext<'_>,
    action: &str,
    args: &[String],
) -> Result<i32, gd_plugins::Problem> {
    let (positional, options) = parse(action, args).map_err(super::command::host_problem)?;
    let result = match action {
        "pack" => {
            let source = ctx.dir.join(
                positional
                    .as_deref()
                    .ok_or_else(|| problem(Path::new("theme"), "A theme directory is required."))
                    .map_err(super::command::host_problem)?,
            );
            pack_to(&source, &ctx.dir, &options)
        }
        "install" => {
            let project = ctx.dir.join(options.get("--project").map_or(".", String::as_str));
            let source = positional
                .as_deref()
                .ok_or_else(|| problem(&project, "A theme archive URL or path is required."))
                .map_err(super::command::host_problem)?;
            let source = if source.contains("://") || Path::new(source).is_absolute() {
                source.into()
            } else {
                ctx.dir.join(source).to_string_lossy().into_owned()
            };
            let sha = options.get("--sha256").map_or("", String::as_str);
            install_theme(&project, &source, sha).map(|_| ())
        }
        "verify" => {
            let project = ctx.dir.join(positional.as_deref().unwrap_or("."));
            pack::verify(&project).map(|_| ()).map_err(|e| problem(&project, &e.to_string()))
        }
        _ => Err(problem(Path::new("theme"), "Unknown theme command.")),
    };
    result.map_err(super::command::host_problem)?;
    if !ctx.quiet {
        ctx.print(&format!("Theme {action} succeeded.\n"));
    }
    Ok(0)
}

fn pack_to(source: &Path, root: &Path, options: &BTreeMap<String, String>) -> Result<(), Problem> {
    let destination =
        options.get("--out").ok_or_else(|| problem(source, "theme pack requires --out."))?;
    let files = pack::read_tree(source).map_err(|e| problem(source, &e.to_string()))?;
    let bytes = pack::encode(&files).map_err(|e| problem(source, &e.to_string()))?;
    let text = std::str::from_utf8(&bytes).map_err(|e| problem(source, &e.to_string()))?;
    crate::transaction::write_source(&root.join(destination), None, text)
}

fn parse(
    action: &str,
    args: &[String],
) -> Result<(Option<String>, BTreeMap<String, String>), Problem> {
    let mut options = BTreeMap::new();
    let mut positional = None;
    let mut words = args.iter();
    while let Some(word) = words.next() {
        if word.starts_with("--") {
            let (key, value) =
                word.split_once('=').map_or((word.as_str(), None), |(k, v)| (k, Some(v)));
            let allowed = match action {
                "pack" => key == "--out",
                "install" => matches!(key, "--sha256" | "--project"),
                _ => false,
            };
            if !allowed {
                return Err(problem(Path::new("theme"), "Unknown theme option."));
            }
            let value = value
                .or_else(|| words.next().map(String::as_str))
                .ok_or_else(|| problem(Path::new("theme"), "Missing option value."))?;
            if options.insert(key.into(), value.into()).is_some() {
                return Err(problem(Path::new("theme"), "Duplicate theme option."));
            }
        } else if positional.replace(word.clone()).is_some() {
            return Err(problem(Path::new("theme"), "Expected one theme source or project."));
        }
    }
    Ok((positional, options))
}

fn problem(path: &Path, message: &str) -> Problem {
    Problem::new(
        "ssg.theme",
        "THEME OPERATION REFUSED",
        &path.display().to_string(),
        message,
        "Check the archive pin, portable paths and installed lock; builds never fetch themes.",
        2,
    )
}
