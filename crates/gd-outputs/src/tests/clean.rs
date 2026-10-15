//! clean_test.odin: `guidedog clean` empties the build folder and follows no link.

use gd_conf::Config;

use super::support::{TempDir, put, text_of};
use crate::clean;

#[test]
fn clean_removes_files_and_links_without_touching_their_targets() {
    let dir = TempDir::new("clean");
    let build_dir = dir.join("_build");
    let outside = dir.join("outside");
    put(&build_dir.join(".DS_Store"), "metadata");
    put(&build_dir.join("html/index.html"), "page");
    put(&build_dir.join("html/deep/er/file.txt"), "deep");
    put(&outside.join("keep.txt"), "keep");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, build_dir.join("linked")).unwrap();
    let config = Config::new(&dir.0, &build_dir);
    clean(&config).unwrap();
    assert!(build_dir.is_dir());
    assert_eq!(std::fs::read_dir(&build_dir).unwrap().count(), 0);
    assert_eq!(text_of(&outside.join("keep.txt")), "keep");
    #[cfg(unix)]
    {
        std::fs::remove_dir(&build_dir).unwrap();
        std::os::unix::fs::symlink(&outside, &build_dir).unwrap();
        assert_eq!(clean(&config).unwrap_err().code, "project.clean-root");
        assert!(outside.join("keep.txt").is_file());
    }
}

#[test]
fn a_missing_build_folder_is_nothing_to_clean() {
    let dir = TempDir::new("clean-missing");
    clean(&Config::new(&dir.0, dir.join("_build"))).unwrap();
    clean(&Config::default()).unwrap();
}
