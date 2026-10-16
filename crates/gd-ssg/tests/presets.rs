//! Every starter is a buildable project, not a partially filled command template.
#![allow(clippy::unwrap_used)]

use gd_ssg::{Preset, SiteBuildOptions, build_site, create_site};

#[test]
fn all_presets_build_html_collections_and_real_research_pdfs() {
    for (name, preset) in [
        ("project", Preset::Project),
        ("journal", Preset::Journal),
        ("research", Preset::Research),
    ] {
        let root = std::env::temp_dir().join(format!("gd-preset-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        create_site(&root, preset, "Example", "Ada", "2026-10-16").unwrap();
        let options =
            SiteBuildOptions { now: "2026-10-16T00:00:00Z".into(), ..Default::default() };
        let output = build_site(&root, &options, &mut |r| {
            let load = gd_project::LoadOptions {
                config: gd_conf::LoadOptions {
                    overrides: r.overrides.clone(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let project = gd_project::load(&r.project, &load)?;
            let result = gd_project::build(
                &project,
                gd_project::BuilderKind::Html,
                &gd_project::BuildOptions {
                    output_dir: Some(r.output.clone()),
                    warnings_as_errors: true,
                    now: Some(gd_clock::parse_timestamp(&r.now).unwrap()),
                    ..Default::default()
                },
            );
            if let Some(problem) = result.problem {
                return Err(problem);
            }
            assert!(result.published);
            Ok(())
        })
        .unwrap();
        let index = std::fs::read_to_string(output.join("index.html")).unwrap();
        assert!(index.contains("Example"));
        match preset {
            Preset::Project => assert!(output.join("guide.html").exists()),
            Preset::Journal => {
                assert!(output.join("journal/index.html").exists());
                assert!(output.join("tags/journal/index.html").exists());
                assert!(output.join("atom.xml").exists());
            }
            Preset::Research => {
                assert!(output.join("papers/index.html").exists());
                let pdf = std::fs::read(output.join("papers/first-paper.pdf")).unwrap();
                assert!(pdf.starts_with(b"%PDF-"));
                assert!(index.contains("A first paper"));
            }
        }
        let config = std::fs::read(root.join("conf.toml")).unwrap();
        assert!(create_site(&root, preset, "Other", "", "2026-10-16").is_err());
        assert_eq!(std::fs::read(root.join("conf.toml")).unwrap(), config);
        std::fs::remove_dir_all(root).unwrap();
    }
}
