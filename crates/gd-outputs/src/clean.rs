//! `guidedog clean` (build.odin's clean).

use gd_conf::Config;
use gd_host::Problem;

/// Removes everything in the project's build folder, as `make clean` does for Sphinx,
/// and leaves the folder itself; a missing one is nothing to clean. Fails with
/// `project.clean-root` (exit 4) when the build path is a link or a file (nothing is
/// removed), and with `host.io` at the first entry it cannot remove.
pub fn clean(config: &Config) -> Result<(), Problem> {
    let _ = config;
    todo!("wave 1: port clean with gd_host::remove_path")
}
