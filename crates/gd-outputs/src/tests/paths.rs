//! build_paths_test.odin: where a generation's output and records go.

use std::path::Path;

use gd_env::BuilderKind;

use super::support::{Options, Project, put, text_of};
use crate::hook::{StepAction, set_hook};
use crate::{contained, output_absolute, output_paths};

/// In the sphinx-build form, the configured build folder is also the output. Its
/// records stay beside it: fresh and incremental builds settle with no warning, and an
/// interrupted commit resumes through the same journal.
#[test]
fn direct_outputs_keep_their_records_outside_the_swap() {
    let mut p = Project::new("direct");
    p.put("index.rst", "Demo\n\nFirst.\n");
    let output = p.dir.join("published");
    p.config.build_dir = output.clone();
    let options = Options { output: Some(output.clone()), ..Options::default() };
    let first = p.build(&options);
    assert!(first.ok() && first.published && first.reports.is_empty(), "{first:?}");
    let cache = p.dir.join(".published.guidedog/.doctrees");
    assert!(cache.join("html.outputs.json").is_file());
    assert!(!cache.join("html.commit.json").exists());
    assert!(!output.join(".doctrees").exists());
    let second = p.build(&options);
    assert!(second.ok() && second.reused == 1 && second.written == 0, "{second:?}");
    assert!(second.reports.is_empty());
    for step in 1..=3 {
        set_hook(Some(Box::new(
            move |n| {
                if n == step { StepAction::Crash } else { StepAction::Go }
            },
        )));
        p.put("index.rst", &format!("Demo\n\nChange {step}.\n"));
        let stopped = p.build(&options);
        set_hook(None);
        assert!(stopped.unsettled, "step {step}: {stopped:?}");
        let resumed = p.build(&options);
        assert!(resumed.ok() && resumed.published && resumed.reports.is_empty(), "{resumed:?}");
        assert!(!cache.join("html.commit.json").exists());
        assert!(text_of(&output.join("index.html")).contains(&format!("Change {step}.")));
    }
}

/// A manifest describes one destination: files with the same names in another custom
/// output are no evidence that this build already published their content.
#[test]
fn custom_outputs_do_not_share_manifests() {
    let p = Project::new("targets");
    p.put("index.rst", "Demo\n\nCorrect content.\n");
    let one = Options { output: Some(p.dir.join("one")), ..Options::default() };
    assert!(p.build(&one).ok());
    let other = p.dir.join("two");
    put(&other.join("index.html"), "Wrong content.");
    let second = p.build(&Options { output: Some(other.clone()), ..Options::default() });
    assert!(second.ok());
    assert_eq!(second.written, 1);
    assert!(text_of(&other.join("index.html")).contains("Correct content."));
    let refused = p.build(&Options { output: Some(p.dir.0.clone()), ..Options::default() });
    assert_eq!(refused.problem.as_ref().map(|p| p.code), Some("build.output.layout"));
    assert!(!refused.published && p.dir.join("index.rst").is_file());
}

#[test]
fn default_outputs_share_the_build_folders_doctrees() {
    let p = Project::new("default");
    let paths = output_paths(&p.config, BuilderKind::Text, None).unwrap();
    let build = p.dir.join("_build");
    assert_eq!(paths.out_dir, build.join("text"));
    assert_eq!(paths.next, build.join(".text.next"));
    assert_eq!(paths.old, build.join(".text.old"));
    assert_eq!(paths.legacy, build.join(".text.stage"));
    assert_eq!(paths.build_dir, build);
    assert_eq!(paths.doctrees, build.join(".doctrees"));
    assert_eq!(paths.manifest, build.join(".doctrees/text.outputs.json"));
    assert_eq!(paths.journal, build.join(".doctrees/text.commit.json"));
    assert_eq!(paths.builder(), "text");
}

#[test]
fn missing_folders_are_resolved_through_their_nearest_ancestor() {
    let p = Project::new("absolute");
    let path = p.dir.join("a/b/../c");
    assert_eq!(output_absolute(&path).unwrap(), p.dir.join("a/c"));
    assert_eq!(output_absolute(&p.dir.0).unwrap(), p.dir.0);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&p.dir.0, p.dir.join("link")).unwrap();
        assert_eq!(output_absolute(&p.dir.join("link/x")).unwrap(), p.dir.join("x"));
    }
    let relative = output_absolute(Path::new("some/output")).unwrap();
    assert!(relative.is_absolute() && relative.ends_with("some/output"));
}

/// `contained` splits a relative path at the system's separators, so an output below
/// the source, which is `..\..` on Windows, is not taken for a folder below.
#[test]
fn contained_paths_split_at_the_system_separators() {
    for rel in ["a", "a/b", "a/..b", "..a/b"] {
        assert!(contained(rel), "{rel:?} is below");
    }
    for rel in ["", "..", "../a", "a/../../b", "/a"] {
        assert!(!contained(rel), "{rel:?} is not below");
    }
    if cfg!(windows) {
        for rel in [r"..\..", r"a\..\..\b", r"\a", r"C:\a"] {
            assert!(!contained(rel), "{rel:?} is not below");
        }
    } else {
        assert!(contained(r"..\..")); // one name, with backslashes in it.
    }
}

#[test]
fn unit_paths_are_normalized() {
    use crate::paths::unit_path;
    assert_eq!(unit_path("a//b/./c.html").as_deref(), Some("a/b/c.html"));
    assert_eq!(unit_path("./x").as_deref(), Some("x"));
    assert_eq!(unit_path("."), None);
    assert_eq!(unit_path("../x"), None);
}
