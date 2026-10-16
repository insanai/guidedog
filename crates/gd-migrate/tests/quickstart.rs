//! The project quickstart writes loads with gd-conf, as a build loads it.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use gd_migrate::{QuickstartOptions, quickstart};

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir()
            .join(format!("gd-migrate quickstart {name} {}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn quickstart_lays_out_a_sphinx_project() {
    for separate in [false, true] {
        let temp = TempDir::new(if separate { "separate" } else { "joined" });
        let dir = &temp.0;
        let o = QuickstartOptions {
            dir: dir.clone(),
            project: "Demo \"Docs\"".into(),
            author: "Ada".into(),
            version: "0.1".into(),
            release: "0.1.0".into(),
            separate,
            year: Some(2026),
            date: Some("Tue Sep 29 2026".into()),
            ..Default::default()
        };
        let created = quickstart(&o).unwrap();
        assert_eq!(created.len(), 12);
        let source = if separate { dir.join("source") } else { dir.clone() };
        for f in [
            "conf.toml",
            "index.rst",
            "_static/guidedog.css",
            "_static/guidedog.js",
            "_templates/layout.html",
            "_templates/book.typ",
            "_templates/discussion.typ",
            "_templates/theme.typ",
            "assets/images/README.txt",
            "assets/video/README.txt",
            "assets/audio/README.txt",
        ] {
            assert!(source.join(f).is_file(), "missing {f}");
        }
        let build = if separate { dir.join("build") } else { dir.join("_build") };
        assert!(build.is_dir());
        let gitignore = std::fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert!(gitignore.contains(if separate { "/build/" } else { "/_build/" }));
        let index = std::fs::read_to_string(source.join("index.rst")).unwrap();
        assert!(index.contains("Demo \"Docs\" documentation\n=========================\n"));
        assert!(index.contains(".. toctree::\n   :maxdepth: 2\n"));

        let loaded = gd_conf::load_config(dir, &gd_conf::LoadOptions::default()).unwrap();
        assert_eq!(loaded.config.project, "Demo \"Docs\"");
        assert_eq!(loaded.config.copyright, "2026, Ada");
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        let real = gd_conf::absolute(&build);
        assert_eq!(loaded.config.build_dir, real);
        // A build folder that does not exist yet is still where the output goes.
        std::fs::remove_dir_all(&build).unwrap();
        let fresh = gd_conf::load_config(dir, &gd_conf::LoadOptions::default()).unwrap();
        assert_eq!(fresh.config.build_dir, real);

        let again = quickstart(&o).unwrap_err();
        assert_eq!(again.code, "quickstart.exists");
    }
}

#[test]
fn quickstart_writes_markdown_roots() {
    let temp = TempDir::new("markdown");
    let o = QuickstartOptions {
        dir: temp.0.clone(),
        project: "Notes".into(),
        author: "Ada".into(),
        suffix: ".md".into(),
        ..Default::default()
    };
    quickstart(&o).unwrap();
    let index = std::fs::read_to_string(temp.0.join("index.md")).unwrap();
    assert!(index.contains("# Notes documentation"));
    let loaded = gd_conf::load_config(&temp.0, &gd_conf::LoadOptions::default()).unwrap();
    assert_eq!(loaded.config.source_suffix, [".md"]);
}
