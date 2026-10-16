//! Mounts publish one coherent website, with real project builds behind the callback.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use gd_host::Problem;
use gd_plugins::BuildRequest;
use gd_ssg::{SiteBuildOptions, build_site};

struct Site(PathBuf);

impl Site {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("gd-ssg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        let site = Self(path.canonicalize().unwrap());
        site.put(
            "conf.toml",
            concat!(
                "project = \"Website\"\n[site]\nbase_url = \"https://example.org/project\"\n",
                "[[site.mounts]]\nname = \"home\"\nsource = \"home\"\noutput = \"\"\n",
                "[[site.mounts]]\nname = \"manual\"\nsource = \"manual\"\noutput = \"manual\"\n",
                "editions = [{language=\"en\",output=\"\"},",
                "{language=\"de\",output=\"de\",source=\"german\"}]\n",
                "[[site.assets]]\nsource = \"shared\"\noutput = \"assets\"\n"
            ),
        );
        for source in ["home", "manual", "german"] {
            site.put(&format!("{source}/conf.toml"), "project = \"Example\"\n");
        }
        site.put("home/index.rst", "Home\n====\n\n`Manual <mount://manual/index.html>`_.\n");
        site.put("manual/index.rst", "Manual\n======\n\n`Home <mount://home/index.html>`_.\n");
        site.put("german/index.rst", "Handbuch\n========\n");
        site.put("shared/logo.txt", "Shared asset");
        site
    }

    fn put(&self, file: &str, text: &str) {
        let path = self.0.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn read(&self, file: &str) -> String {
        std::fs::read_to_string(self.0.join(file)).unwrap()
    }

    fn build(&self) -> Result<PathBuf, Problem> {
        let options =
            SiteBuildOptions { now: "2026-10-16T00:00:00Z".into(), ..Default::default() };
        build_site(&self.0, &options, &mut build)
    }
}

impl Drop for Site {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn build(request: &BuildRequest) -> Result<(), Problem> {
    let load = gd_project::LoadOptions {
        config: gd_conf::LoadOptions {
            overrides: request.overrides.clone(),
            ..Default::default()
        },
        ..Default::default()
    };
    let project = gd_project::load(&request.project, &load)?;
    let kind = gd_project::BuilderKind::from_name(&request.builder).unwrap();
    let options = gd_project::BuildOptions {
        output_dir: Some(request.output.clone()),
        warnings_as_errors: request.strict,
        now: Some(gd_clock::parse_timestamp(&request.now).unwrap()),
        preview: request.preview,
        ..Default::default()
    };
    let result = gd_project::build(&project, kind, &options);
    if let Some(p) = result.problem {
        return Err(p);
    }
    assert!(result.published, "{:?}", result.reports);
    Ok(())
}

#[test]
fn mounts_editions_shared_assets_and_cross_links_build_together() {
    let site = Site::new("mounted-editions");
    let out = site.build().unwrap();
    assert_eq!(out, site.0.join("_build/site"));
    assert!(site.read("_build/site/index.html").contains("href=\"manual/index.html\""));
    assert!(site.read("_build/site/manual/index.html").contains("href=\"../index.html\""));
    assert!(site.read("_build/site/manual/de/index.html").contains("Handbuch"));
    assert_eq!(site.read("_build/site/assets/logo.txt"), "Shared asset");
    assert!(!out.join(".doctrees").exists());
    assert!(!out.join("manual/.doctrees").exists());
    site.build().unwrap();
}

#[test]
fn a_later_mount_failure_preserves_the_entire_previous_website() {
    let site = Site::new("mounted-failure");
    site.build().unwrap();
    let old = site.read("_build/site/index.html");
    site.put("home/index.rst", "Changed home\n============\n");
    site.put("german/index.rst", ":draft: perhaps\n\nBroken\n======\n");
    assert!(site.build().is_err());
    assert_eq!(site.read("_build/site/index.html"), old);
    assert!(site.read("_build/site/manual/de/index.html").contains("Handbuch"));
}

#[test]
fn missing_cross_mount_targets_cannot_be_satisfied_by_old_output() {
    let site = Site::new("mounted-hidden");
    site.put("manual/secret.rst", ":date: 2026-10-15\n\nArticle\n=======\n");
    site.put("home/index.rst", "Home\n====\n\n`Article <mount://manual/secret.html>`_.\n");
    site.build().unwrap();
    let old = site.read("_build/site/index.html");
    site.put("manual/secret.rst", ":draft: true\n:date: 2026-10-15\n\nArticle\n=======\n");
    let error = site.build().unwrap_err();
    assert_eq!(error.code, "ssg.site");
    assert!(error.message.contains("no eligible published target"));
    assert_eq!(site.read("_build/site/index.html"), old);
}

#[test]
fn overlapping_mounts_and_escaping_sources_are_refused() {
    let site = Site::new("mounted-collision");
    let original = site.read("conf.toml");
    site.put("conf.toml", &original.replace("output = \"manual\"", "output = \"\""));
    assert_eq!(site.build().unwrap_err().code, "site.collision");
    assert!(!site.0.join("_build/site/index.html").exists());
    let outside = Site::new("mounted-outside");
    let escaped = original.replace("source = \"home\"", &format!("source = {:?}", outside.0));
    site.put("conf.toml", &escaped);
    assert!(site.build().unwrap_err().message.contains("escapes its root"));
}

#[test]
fn invalid_clock_and_mount_paths_fail_before_publication() {
    let site = Site::new("mounted-invalid");
    let options = SiteBuildOptions { now: "tomorrow".into(), ..Default::default() };
    assert!(build_site(&site.0, &options, &mut |_| panic!("must not build")).is_err());
    let original = site.read("conf.toml");
    site.put("conf.toml", &original.replace("output = \"manual\"", "output = \"../manual\""));
    assert!(site.build().is_err());
    assert!(!site.0.join("_build/site/index.html").exists());
}

#[test]
fn html_only_previews_skip_pdf_mounts_and_unlink_their_cross_mount_downloads() {
    let site = Site::new("mounted-no-pdf");
    let config = format!(
        concat!(
            "{}\n[[site.mounts]]\nname=\"book\"\nsource=\"manual\"\n",
            "output=\"books\"\nbuilder=\"pdf\"\n"
        ),
        site.read("conf.toml")
    );
    site.put("conf.toml", &config);
    site.put("home/index.rst", "Home\n====\n\n`Download <mount://book/example-en.pdf>`_.\n");
    let options = SiteBuildOptions {
        no_pdf: true,
        now: "2026-10-16T00:00:00Z".into(),
        ..Default::default()
    };
    let output = build_site(&site.0, &options, &mut |request| {
        assert_ne!(request.builder, "pdf");
        build(request)
    })
    .unwrap();
    let html = std::fs::read_to_string(output.join("index.html")).unwrap();
    assert!(html.contains("Download"));
    assert!(!html.contains("example-en.pdf"));
    assert!(!output.join("books").exists());
}
