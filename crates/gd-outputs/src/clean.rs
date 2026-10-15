//! `guidedog clean` (build.odin's clean).

use gd_conf::Config;
use gd_host::{PathKind, Problem, io_problem, remove_tree};

/// Removes everything in the project's build folder, as `make clean` does for Sphinx,
/// and leaves the folder itself; a missing one is nothing to clean. Fails with
/// `project.clean-root` (exit 4) when the build path is a link or a file (nothing is
/// removed), and with `host.io` at the first entry it cannot remove.
pub fn clean(config: &Config) -> Result<(), Problem> {
    let dir = &config.build_dir;
    if dir.as_os_str().is_empty() {
        return Ok(());
    }
    match gd_host::path_type(dir).map_err(|e| io_problem(dir, &e))? {
        None => return Ok(()),
        Some(PathKind::Folder) => {}
        Some(_) => {
            return Err(Problem::new(
                "project.clean-root",
                "BUILD PATH IS NOT A DIRECTORY",
                &dir.display().to_string(),
                "Cleaning would follow a link or remove a file instead of a build directory. \
                 Nothing was removed.",
                "Replace the build path with a real directory, then run guidedog clean again.",
                4,
            ));
        }
    }
    for entry in gd_host::list_folder(dir).map_err(|e| io_problem(dir, &e))? {
        let path = dir.join(&entry.name);
        remove_tree(&path).map_err(|e| io_problem(&path, &e))?;
    }
    Ok(())
}
