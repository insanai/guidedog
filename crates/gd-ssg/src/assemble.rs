//! The existing output journal commits every mount and shared asset as one website.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use gd_conf::Config;
use gd_env::{BuilderKind, Environment, ObjectStore};
use gd_host::{Budget, Problem, io_problem};
use gd_outputs::{CommitRecords, CommitState, Generation, GenerationOptions, UnitRecord};

use crate::mounts::{Mounted, SiteBuildOptions, confined_source, join, problem};

pub(crate) fn publish(
    config: &Config,
    options: &SiteBuildOptions,
    mounted: &[Mounted],
    budget: &Budget,
) -> Result<PathBuf, Problem> {
    let output = options.output.clone().unwrap_or_else(|| config.build_dir.join("site"));
    let paths = gd_outputs::output_paths(config, BuilderKind::Html, Some(&output))?;
    let store = ObjectStore::new(paths.doctrees.clone());
    let mut reports = Vec::new();
    let mut generation =
        Generation::begin(paths, GenerationOptions { all: false, budget }, &mut reports)?;
    let mut outputs = BTreeMap::new();
    for mount in mounted {
        let files = tree(&mount.built, &mount.exclude, mount.pdf)?;
        copy(&mut generation, &mount.key, &mount.output, &files, budget, &mut outputs)?;
    }
    for (index, asset) in config.site.assets.iter().enumerate() {
        let source = confined_source(&config.source_dir, &asset.source)?;
        let files = if source.is_file() {
            let name = source
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| problem(&source, "Asset name is not UTF-8."))?;
            vec![(source.clone(), name.into())]
        } else {
            tree(&source, &[], false)?
        };
        copy(
            &mut generation,
            &format!("asset:{index}"),
            &asset.output,
            &files,
            budget,
            &mut outputs,
        )?;
    }
    validate_mount_links(mounted, &outputs, budget)?;
    generation.check_written()?;
    let outcome =
        generation.commit(CommitRecords { environment: &Environment::default(), store: &store });
    if let Some(problem) = outcome.problem {
        return Err(problem);
    }
    if outcome.state != CommitState::Published {
        return Err(problem(&output, "Site commit did not complete."));
    }
    Ok(output)
}

fn validate_mount_links(
    mounts: &[Mounted],
    outputs: &BTreeMap<String, PathBuf>,
    budget: &Budget,
) -> Result<(), Problem> {
    for mount in mounts {
        let bytes =
            gd_host::read_file(&mount.manifest, Some(&gd_host::Ledger::new(budget)), 64 << 20)?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| problem(&mount.manifest, "Invalid output manifest."))?;
        let manifest = gd_outputs::Manifest::from_json(text)
            .ok_or_else(|| problem(&mount.manifest, "Invalid output manifest."))?;
        for record in manifest.units.values() {
            for (lookup, _) in &record.lookups {
                if lookup.kind != gd_env::LookupKind::Mount {
                    continue;
                }
                let target = lookup.key.split('#').next().unwrap_or("");
                if !outputs.contains_key(target) {
                    return Err(problem(
                        Path::new(&lookup.key),
                        "A cross-mount link has no eligible published target.",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn copy(
    generation: &mut Generation,
    key: &str,
    prefix: &str,
    files: &[(PathBuf, String)],
    budget: &Budget,
    outputs: &mut BTreeMap<String, PathBuf>,
) -> Result<(), Problem> {
    let mut seen = BTreeSet::new();
    for (_, file) in files {
        let target = join(prefix, file);
        generation.check_available(&target)?;
        if !seen.insert(target.to_ascii_lowercase()) {
            return Err(problem(
                Path::new(&target),
                "Two mounted files have the same portable output path.",
            ));
        }
    }
    let mut unit = generation.unit(key);
    for path in &seen {
        if path.match_indices('/').any(|(i, _)| seen.contains(&path[..i])) {
            return Err(problem(
                Path::new(path),
                "A mounted file overlaps a directory on case-insensitive filesystems.",
            ));
        }
    }
    for (source, file) in files {
        let target = join(prefix, file);
        unit.emit_copy(source, &target, budget)?;
        outputs.insert(target, source.clone());
    }
    unit.finish(UnitRecord::default());
    Ok(())
}

fn tree(
    root: &Path,
    exclude: &[String],
    pdf_only: bool,
) -> Result<Vec<(PathBuf, String)>, Problem> {
    let mut files = Vec::new();
    let mut pending = vec![(root.to_path_buf(), String::new(), 0usize)];
    let mut visited = 0usize;
    while let Some((dir, prefix, depth)) = pending.pop() {
        if depth > 64 {
            return Err(problem(&dir, "Mounted output exceeds the directory depth limit."));
        }
        let mut entries = std::fs::read_dir(&dir)
            .map_err(|e| io_problem(&dir, &e))?
            .take(100_001)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| io_problem(&dir, &e))?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            visited += 1;
            if visited > 100_000 {
                return Err(problem(root, "Mounted output exceeds the entry limit."));
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| problem(&entry.path(), "Output names must be UTF-8."))?;
            if name.starts_with('.') && name != ".nojekyll" {
                continue;
            }
            let relative = join(&prefix, &name);
            if exclude.iter().any(|pattern| gd_env::glob_match(pattern, &relative)) {
                continue;
            }
            let kind = entry.file_type().map_err(|e| io_problem(&entry.path(), &e))?;
            if kind.is_dir() {
                pending.push((entry.path(), relative, depth + 1));
            } else if kind.is_file() {
                if !pdf_only || name.ends_with(".pdf") {
                    files.push((entry.path(), relative));
                }
            } else {
                return Err(problem(
                    &entry.path(),
                    "Mounted output must contain only regular files and directories.",
                ));
            }
            if files.len() + pending.len() > 100_000 {
                return Err(problem(root, "Mounted output exceeds the file limit."));
            }
        }
    }
    files.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(files)
}
