//! Explicit downloads install a pin; subsequent project builds remain offline.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use gd_theme::pack::{self, Files};

struct Project(PathBuf);
impl Project {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("gd-theme-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("conf.toml"),
            "project=\"Theme\"\ntemplates_path=[\"_templates\"]\n[site]\ntheme=true\n",
        )
        .unwrap();
        std::fs::write(root.join("index.rst"), "Theme\n=====\n\nText.\n").unwrap();
        Self(root)
    }
    fn build(&self) -> String {
        let project = gd_project::load(&self.0, &Default::default()).unwrap();
        let result =
            gd_project::build(&project, gd_project::BuilderKind::Html, &Default::default());
        assert!(result.published, "{:?} {:?}", result.problem, result.reports);
        std::fs::read_to_string(self.0.join("_build/html/index.html")).unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn archive(label: &str) -> Vec<u8> {
    pack::encode(&Files::from([
        ("_templates/page.html".into(), format!("{label}: {{{{ body }}}}").into_bytes()),
        ("_static/theme.txt".into(), label.as_bytes().to_vec()),
    ]))
    .unwrap()
}

#[test]
fn explicit_http_install_is_pinned_and_builds_make_no_network_requests() {
    let project = Project::new("http");
    let data = archive("Remote theme");
    let digest = pack::archive_digest(&data);
    let server = gd_fetch::testserver::start(move |_| gd_fetch::testserver::Reply {
        status: 200,
        body: data.clone(),
        ..Default::default()
    })
    .unwrap();
    let url = server.url("/theme.gdt");
    let lock = gd_ssg::install_theme(&project.0, &url, &digest).unwrap();
    assert_eq!(server.served_count(), 1);
    assert!(project.build().contains("Remote theme:"));
    assert_eq!(server.served_count(), 1);
    drop(server);
    assert!(project.build().contains("Remote theme:"));
    assert_eq!(pack::verify(&project.0).unwrap(), lock);
    std::fs::create_dir(project.0.join("_templates")).unwrap();
    std::fs::write(project.0.join("_templates/page.html"), "Project override").unwrap();
    assert_eq!(project.build(), "Project override");
}

#[test]
fn failed_pins_and_tampered_trees_preserve_the_selected_theme_or_fail_closed() {
    let project = Project::new("pins");
    let path = project.0.join("theme.gdt");
    let data = archive("First");
    std::fs::write(&path, &data).unwrap();
    let lock =
        gd_ssg::install_theme(&project.0, path.to_str().unwrap(), &pack::archive_digest(&data))
            .unwrap();
    let original = std::fs::read(project.0.join(pack::LOCK_FILE)).unwrap();
    std::fs::write(&path, archive("Second")).unwrap();
    assert!(
        gd_ssg::install_theme(&project.0, path.to_str().unwrap(), &lock.archive_sha256).is_err()
    );
    assert_eq!(std::fs::read(project.0.join(pack::LOCK_FILE)).unwrap(), original);
    assert!(project.build().contains("First:"));
    std::fs::write(lock.directory(&project.0).join("_static/theme.txt"), "Tampered").unwrap();
    assert!(pack::verify(&project.0).is_err());
    assert!(gd_project::load(&project.0, &Default::default()).is_err());
}

#[test]
fn static_only_theme_changes_update_settings_with_and_without_overrides() {
    let project = Project::new("static-settings");
    let path = project.0.join("theme.gdt");
    let load = |overrides| {
        gd_conf::load_config(&project.0, &gd_conf::LoadOptions { overrides, ..Default::default() })
            .unwrap()
    };
    let mut previous = None;
    for label in ["First", "Second"] {
        let files = Files::from([("_static/theme.txt".into(), label.as_bytes().to_vec())]);
        let data = pack::encode(&files).unwrap();
        std::fs::write(&path, &data).unwrap();
        gd_ssg::install_theme(&project.0, path.to_str().unwrap(), &pack::archive_digest(&data))
            .unwrap();
        let settings = [load(Vec::new()).settings, load(vec!["version=1".into()]).settings];
        if let Some(previous) = previous {
            for (before, after) in std::iter::zip(previous, settings) {
                assert_ne!(before, after, "a static-only theme is a settings input");
            }
        }
        previous = Some(settings);
        project.build();
        assert_eq!(
            std::fs::read_to_string(project.0.join("_build/html/_static/theme.txt")).unwrap(),
            label
        );
    }
}

#[test]
fn archive_file_and_depth_limits_survive_installation_and_offline_verification() {
    let project = Project::new("theme-limits");
    let mut files = Files::new();
    for index in 0..pack::MAX_FILES {
        files.insert(format!("_static/d{index:04}/file.txt"), b"data".to_vec());
    }
    let archive = pack::encode(&files).unwrap();
    let path = project.0.join("theme.gdt");
    std::fs::write(&path, &archive).unwrap();
    let lock =
        gd_ssg::install_theme(&project.0, path.to_str().unwrap(), &pack::archive_digest(&archive))
            .unwrap();
    assert_eq!(pack::read_tree(&lock.directory(&project.0)).unwrap().len(), pack::MAX_FILES);
    assert_eq!(pack::verify(&project.0).unwrap(), lock);
    let deepest = format!("_static/{}file.txt", "d/".repeat(31));
    let archive = pack::encode(&Files::from([(deepest, b"deep".to_vec())])).unwrap();
    std::fs::write(&path, &archive).unwrap();
    gd_ssg::install_theme(&project.0, path.to_str().unwrap(), &pack::archive_digest(&archive))
        .unwrap();
    pack::verify(&project.0).unwrap();
}

#[cfg(unix)]
#[test]
fn packing_and_verification_refuse_symbolic_links() {
    let project = Project::new("links");
    let source = project.0.join("source");
    std::fs::create_dir_all(source.join("_static")).unwrap();
    std::os::unix::fs::symlink(project.0.join("index.rst"), source.join("_static/linked"))
        .unwrap();
    assert!(pack::read_tree(&source).is_err());
    let data = archive("Linked");
    let path = project.0.join("theme.gdt");
    std::fs::write(&path, &data).unwrap();
    let lock =
        gd_ssg::install_theme(&project.0, path.to_str().unwrap(), &pack::archive_digest(&data))
            .unwrap();
    let file = lock.directory(&project.0).join("_static/theme.txt");
    std::fs::remove_file(&file).unwrap();
    std::os::unix::fs::symlink(project.0.join("index.rst"), &file).unwrap();
    assert!(pack::verify(&project.0).is_err());
}
