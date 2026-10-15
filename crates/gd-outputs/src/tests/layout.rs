//! A custom output leaves one hidden folder beside it, its cache folder, which also
//! holds the spare output and the previous output; folders earlier versions left beside
//! it move there, or go, when they are clearly Guidedog's.

use std::path::Path;

use super::support::{Options, Project, Run, put, text_of};
use crate::hook::{StepAction, set_hook, set_renames_only};

fn custom(p: &Project) -> Options {
    Options { output: Some(p.dir.join("site/out")), ..Options::default() }
}

/// The names in a folder, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The titles of the notes about the output's layout.
fn layout_notes(run: &Run) -> Vec<&str> {
    let notes = run.reports.iter().filter(|r| r.code == "build.output.layout");
    notes.map(|r| r.title.as_str()).collect()
}

fn built(name: &str) -> (Project, Options) {
    let p = Project::new(name);
    p.put("index.rst", "Demo\n\nFirst.\n");
    let options = custom(&p);
    for i in 0..2 {
        p.put("index.rst", &format!("Demo\n\nVersion {i}.\n"));
        let run = p.build(&options);
        assert!(run.ok() && run.published && run.reports.is_empty(), "{run:?}");
    }
    (p, options)
}

#[test]
fn custom_outputs_leave_one_hidden_folder() {
    let (p, _) = built("one-folder");
    assert_eq!(names(&p.dir.join("site")), [".out.guidedog", "out"]);
    assert!(p.dir.join("site/.out.guidedog/.out.next").is_dir());
}

/// A build stopped at any step of its commit, by either kind of switch, is finished by
/// the next one, which leaves nothing beside the output but its cache folder.
#[test]
fn interrupted_commits_leave_nothing_beside_a_custom_output() {
    let (p, options) = built("interrupted");
    for renames in [false, true] {
        for step in 1..=3 {
            set_renames_only(renames);
            set_hook(Some(Box::new(
                move |n| {
                    if n == step { StepAction::Crash } else { StepAction::Go }
                },
            )));
            p.put("index.rst", &format!("Demo\n\nChange {step} {renames}.\n"));
            let stopped = p.build(&options);
            set_hook(None);
            set_renames_only(false);
            assert!(stopped.unsettled, "step {step}: {stopped:?}");
            let resumed = p.build(&options);
            assert!(resumed.ok() && resumed.published, "{resumed:?}");
            assert_eq!(names(&p.dir.join("site")), [".out.guidedog", "out"]);
            let page = text_of(&p.dir.join("site/out/index.html"));
            assert!(page.contains(&format!("Change {step} {renames}.")));
        }
    }
}

/// The spare output and the previous outputs an earlier version left beside the output
/// (an interrupted two-rename commit left `.out.old`, and one that could not remove it
/// `.out.old-2`) move into the cache folder, or go, each with a note.
#[test]
fn old_layout_folders_move_into_the_cache_folder() {
    let (p, options) = built("migrate");
    let site = p.dir.join("site");
    std::fs::rename(site.join(".out.guidedog/.out.next"), site.join(".out.next")).unwrap();
    put(&site.join(".out.old/index.html"), "an older page\n");
    put(&site.join(".out.old/_static/style.css"), "body {}\n");
    put(&site.join(".out.old/.guidedog-stage-0123456789.html"), "staged\n");
    std::fs::create_dir_all(site.join(".out.old-2/empty")).unwrap();
    p.put("index.rst", "Demo\n\nMigrated.\n");
    let run = p.build(&options);
    assert!(run.ok() && run.published, "{run:?}");
    assert_eq!(
        layout_notes(&run),
        ["SPARE OUTPUT MOVED", "OLD OUTPUT REMOVED", "OLD OUTPUT REMOVED"]
    );
    assert_eq!(names(&site), [".out.guidedog", "out"]);
    assert!(site.join(".out.guidedog/.out.next/index.html").is_file());
    assert!(text_of(&site.join("out/index.html")).contains("Migrated."));
    let again = p.build(&options);
    assert!(again.ok() && again.reports.is_empty(), "{again:?}");
}

/// Folders that only look like those earlier versions left are not touched: one with a
/// file Guidedog did not make, and a file with such a name.
#[test]
fn user_folders_beside_the_output_are_left_alone() {
    let (p, options) = built("foreign");
    let site = p.dir.join("site");
    put(&site.join(".out.next/index.html"), "a page\n");
    put(&site.join(".out.next/notes/mine.txt"), "my notes\n");
    put(&site.join(".out.old"), "a file\n");
    p.put("index.rst", "Demo\n\nOnce more.\n");
    let run = p.build(&options);
    assert!(run.ok() && run.published, "{run:?}");
    assert_eq!(
        layout_notes(&run),
        ["FOLDER BESIDE THE OUTPUT KEPT", "FOLDER BESIDE THE OUTPUT KEPT"]
    );
    assert_eq!(text_of(&site.join(".out.next/notes/mine.txt")), "my notes\n");
    assert_eq!(text_of(&site.join(".out.next/index.html")), "a page\n");
    assert_eq!(text_of(&site.join(".out.old")), "a file\n");
    assert!(text_of(&site.join("out/index.html")).contains("Once more."));
}

/// A default output keeps its folders where they were: beside it, in the build folder.
#[test]
fn default_outputs_keep_their_layout() {
    let p = Project::new("default-layout");
    for i in 0..2 {
        p.put("index.rst", &format!("Demo\n\nVersion {i}.\n"));
        let run = p.build(&Options::default());
        assert!(run.ok() && run.reports.is_empty(), "{run:?}");
    }
    assert_eq!(names(&p.dir.join("_build")), [".doctrees", ".html.next", "html"]);
}
