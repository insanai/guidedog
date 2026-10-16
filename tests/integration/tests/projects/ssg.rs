//! The linked SSG commands work through the actual guidedog executable.

use super::support::{build, cli, project};

#[test]
fn publication_commands_are_linked_and_drive_real_builds() {
    let dir = project("ssg-cli");
    dir.write(
        "conf.toml",
        "project = \"SSG\"\nsource_suffix = [\".rst\", \".md\"]\n\
        extensions = [\"myst_parser\"]\n",
    );
    cli(&dir.path, 0, &["ssg", "--help"]);
    for suffix in ["rst", "md"] {
        let path = format!("post.{suffix}");
        cli(
            &dir.path,
            0,
            &["ssg", "new", &path, "--title", "Article", "--now=2026-10-16T09:00:00Z"],
        );
        cli(&dir.path, 2, &["ssg", "new", &path]);
        cli(&dir.path, 0, &["ssg", "schedule", &path, "--at=2026-10-17T09:00:00Z"]);
        let status = cli(&dir.path, 0, &["ssg", "status", "--now=2026-10-16T09:00:00Z"]);
        assert!(status.out.contains("scheduled"), "{}", status.out);
        build(&dir.path, 0, &["html", "--now=2026-10-16T09:00:00Z"]);
        assert!(!dir.path.join("_build/html/post.html").exists());
        cli(&dir.path, 0, &["ssg", "publish", &path, "--now=2026-10-16T09:00:00Z"]);
        build(&dir.path, 0, &["html", "--now=2026-10-16T09:00:00Z"]);
        assert!(dir.path.join("_build/html/post.html").exists());
        cli(&dir.path, 0, &["ssg", "draft", &path]);
        build(&dir.path, 0, &["html", "--now=2026-10-16T09:00:00Z"]);
        assert!(!dir.path.join("_build/html/post.html").exists());
        std::fs::remove_file(dir.path.join(path)).unwrap();
    }
}
