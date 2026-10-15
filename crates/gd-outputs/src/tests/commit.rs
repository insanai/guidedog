//! commit_test.odin: a generation is published whole or not at all, a failed step is
//! undone, and an interrupted commit is finished by the next build.

use std::collections::BTreeMap;
use std::path::Path;

use super::support::{Options, Project, Run, expect_settled, put, snapshot, text_of};
use crate::hook::{StepAction, set_hook, set_renames_only};

/// Makes step `at` of this thread's commits do `action`, after running `before`.
fn hook_at(at: usize, action: StepAction, before: impl Fn() + 'static) {
    set_hook(Some(Box::new(move |step| {
        if step != at {
            return StepAction::Go;
        }
        before();
        action
    })));
}

fn clear_hook() {
    set_hook(None);
}

fn build(p: &Project) -> Run {
    p.build(&Options::default())
}

/// A project built twice, so that the output the second build replaced is the spare
/// output (`.html.next`), and the state it published.
fn first_build() -> (Project, BTreeMap<String, String>) {
    let p = Project::new("commit");
    p.put("index.rst", "Demo\n\nZeroth.\n");
    p.put("old.rst", "Old\n\nOld page.\n");
    p.put("extras/z", "a file\n");
    let first = build(&p);
    assert!(first.ok(), "{:?}", first.problem);
    p.put("index.rst", "Demo\n\nFirst.\n");
    let second = build(&p);
    assert!(second.ok() && second.published, "{:?}", second.problem);
    assert!(p.dir.join("_build/.html.next").is_dir());
    let old = snapshot(&p, true);
    (p, old)
}

/// The second build's changes: a page changes, a document goes, another comes, and an
/// extra file becomes a folder (extras/z becomes extras/z/nested).
fn second_sources(p: &Project) {
    p.put("index.rst", "Demo\n\nSecond.\n");
    p.remove("old.rst");
    p.put("new.rst", "New\n\nNew page.\n");
    p.remove("extras/z");
    p.put("extras/z/nested", "now a folder\n");
}

/// The output the second build publishes, from a project that was never interrupted.
fn reference_output() -> BTreeMap<String, String> {
    let (p, _) = first_build();
    second_sources(&p);
    assert!(build(&p).ok());
    snapshot(&p, false)
}

#[test]
fn a_file_that_becomes_a_folder_is_published() {
    let (p, _) = first_build();
    second_sources(&p);
    let r = build(&p);
    assert!(r.ok() && r.published, "{:?}", r.problem);
    let out = p.out();
    assert_eq!(text_of(&out.join("z/nested")), "now a folder\n");
    assert!(text_of(&out.join("index.html")).contains("Second."));
    assert!(!out.join("old.html").exists());
    expect_settled(&p);
    // And back: the folder becomes a file again.
    p.remove("extras/z");
    p.put("extras/z", "a file again\n");
    assert!(build(&p).ok());
    assert_eq!(text_of(&out.join("z")), "a file again\n");
    expect_settled(&p);
    let again = build(&p);
    assert!(again.ok());
    assert_eq!(again.written, 0);
}

#[test]
fn the_spare_output_never_leaks_into_the_published_one() {
    let want = reference_output();
    let (p, _) = first_build();
    let spare = p.dir.join("_build/.html.next");
    put(&spare.join("stray.html"), "stray\n");
    std::fs::remove_file(spare.join("index.html")).unwrap();
    put(&spare.join("index.html/x"), "x\n");
    std::fs::remove_dir_all(spare.join("_static")).unwrap();
    put(&spare.join("_static"), "not a folder\n");
    // A new file, not the output's: the spare shares its files with the output.
    std::fs::remove_file(spare.join("genindex.html")).unwrap();
    put(&spare.join("genindex.html"), "changed\n");
    second_sources(&p);
    assert!(build(&p).ok());
    assert_eq!(want, snapshot(&p, false));
    expect_settled(&p);
}

#[test]
fn a_file_of_the_user_in_the_way_is_reported() {
    let (p, _) = first_build();
    let out = p.out();
    put(&out.join("CNAME"), "docs.example.org\n");
    p.put("index.rst", "Demo\n\nChanged.\n");
    assert!(build(&p).ok());
    assert_eq!(text_of(&out.join("CNAME")), "docs.example.org\n");
    // A user's file where an extra folder now goes.
    let kept = snapshot(&p, true);
    p.put("extras/CNAME/x", "x\n");
    let r = build(&p);
    assert_eq!(r.title(), "A FILE IN THE OUTPUT IS IN THE WAY");
    assert!(!r.published && !r.unsettled);
    assert_eq!(kept, snapshot(&p, true));
    expect_settled(&p);
}

/// A commit step that fails, at any step, is undone: the previous output and records
/// are exactly as they were, the problem says so, and the next build publishes. Both
/// ways of switching the folders are tried. A step past the last is never reached.
#[test]
fn a_failed_commit_step_is_undone() {
    for renames in [false, true] {
        for step in 1..=6 {
            set_renames_only(renames);
            let (p, old) = first_build();
            second_sources(&p);
            hook_at(step, StepAction::Fail, || {});
            let r = build(&p);
            clear_hook();
            if r.ok() {
                // Past the last step: 3 with an exchange, 4 with two renames.
                assert!(step > if renames { 4 } else { 3 }, "step {step} was not reached");
            } else {
                assert_eq!(r.title(), "THE OUTPUT COULD NOT BE PUBLISHED", "step {step}");
                let problem = r.problem.as_ref().unwrap();
                assert!(problem.message.contains("failed: invalid file. Nothing was published"));
                assert!(!r.published && !r.unsettled);
                assert_eq!(old, snapshot(&p, true), "renames {renames}, step {step}");
                expect_settled(&p);
                assert!(build(&p).ok());
            }
            assert!(text_of(&p.out().join("index.html")).contains("Second."));
            assert_eq!(snapshot(&p, true)["z/nested"], "now a folder\n");
            expect_settled(&p);
            set_renames_only(false);
        }
    }
}

/// A commit stopped at any step, as when the machine goes down, is finished by the
/// next build before it does anything else, even when that build fails by -W: the
/// output and the records are all new, and nothing is left beside them.
#[test]
fn an_interrupted_commit_is_finished_by_the_next_build() {
    let want = reference_output();
    for renames in [false, true] {
        for step in 1..=if renames { 4 } else { 3 } {
            set_renames_only(renames);
            let (p, _) = first_build();
            second_sources(&p);
            hook_at(step, StepAction::Crash, || {});
            let stopped = build(&p);
            clear_hook();
            assert_eq!(stopped.title(), "COMMIT STOPPED");
            assert!(stopped.unsettled && !stopped.published);
            assert!(p.dir.join("_build/.doctrees/html.commit.json").is_file());
            // The next build fails by -W, but first finishes the commit.
            p.put("new.rst", "New\n\nNew page, see :ref:`missing`.\n");
            let failed = p.build(&Options { warnings_as_errors: true, ..Options::default() });
            assert_eq!(failed.title(), "WARNINGS TREATED AS ERRORS");
            expect_settled(&p);
            assert_eq!(want, snapshot(&p, false), "renames {renames}, step {step}");
            let manifest = text_of(&p.dir.join("_build/.doctrees/html.outputs.json"));
            assert!(manifest.contains("z/nested") && !manifest.contains("old.html"));
            let environment = text_of(&p.dir.join("_build/.doctrees/environment.json"));
            assert!(environment.contains("\"new\"") && !environment.contains("\"old\""));
            set_renames_only(false);
        }
    }
}

/// A build killed while writing a record leaves its stage file in the doctrees folder,
/// and one killed while writing a page leaves one in the next folder. The next build
/// settles the commit the killed one decided first, and then removes them, but no file
/// of the user's with a similar name.
#[test]
fn stages_left_by_a_killed_build_are_removed() {
    let (p, _) = first_build();
    second_sources(&p);
    hook_at(1, StepAction::Crash, || {});
    let stopped = build(&p);
    clear_hook();
    assert!(stopped.unsettled);
    let stale = [
        "_build/.doctrees/.guidedog-stage-0123456789.next",
        "_build/.doctrees/graphs/.guidedog-stage-0123456789.svg",
        "_build/.doctrees/objects/.guidedog-stage-0123456789.json",
    ];
    let mine = [
        "_build/.doctrees/notes.next",
        "_build/.doctrees/.guidedog-stage-notes",
        "_build/.doctrees/graphs/.guidedog-stage-01234.svg",
    ];
    for f in stale {
        p.put(f, "");
    }
    for f in mine {
        p.put(f, "mine");
    }
    assert!(build(&p).ok());
    expect_settled(&p);
    assert_eq!(text_of(&p.out().join("z/nested")), "now a folder\n");
    for f in stale {
        assert!(!p.dir.join(f).exists(), "{f} is left");
    }
    for f in mine {
        assert_eq!(text_of(&p.dir.join(f)), "mine");
    }
    // A page's stage in the spare output goes when the next change completes it.
    let page = p.dir.join("_build/.html.next/.guidedog-stage-0123456789.html");
    put(&page, "");
    p.put("new.rst", "New\n\nNew page, changed again.\n");
    assert!(build(&p).ok());
    assert!(!page.exists());
    assert!(!p.out().join(".guidedog-stage-0123456789.html").exists());
    assert!(text_of(&p.out().join("new.html")).contains("again"));
}

/// Whether a folder's mode stops this process writing in it; it does not stop an
/// administrator.
#[cfg(unix)]
fn modes_bind(dir: &Path) -> bool {
    lock(dir);
    let probe = dir.join("probe");
    let made = std::fs::create_dir(&probe);
    crate::tree::make_writable(dir);
    let _ = std::fs::remove_dir(&probe);
    made.is_err()
}

#[cfg(unix)]
fn lock(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555));
}

/// A real failure of the system: the build folder cannot be written when the output is
/// to be switched. The commit is undone, and the next build, once the folder can be
/// written, publishes.
#[cfg(unix)]
#[test]
fn a_read_only_build_folder_fails_the_commit_cleanly() {
    let (p, old) = first_build();
    let locked = p.dir.join("_build");
    if !modes_bind(&locked) {
        return;
    }
    second_sources(&p);
    let target = locked.clone();
    hook_at(1, StepAction::Go, move || lock(&target));
    let r = build(&p);
    clear_hook();
    crate::tree::make_writable(&locked);
    assert_eq!(r.title(), "THE OUTPUT COULD NOT BE PUBLISHED");
    assert!(!r.published && !r.unsettled);
    assert_eq!(old, snapshot(&p, true));
    assert!(build(&p).ok());
    assert_eq!(text_of(&p.out().join("z/nested")), "now a folder\n");
    expect_settled(&p);
}

/// A real failure that also stops the commit being undone: the records' folder cannot
/// be written once the output is switched. The build says the publication is not
/// finished, and the next build, once the folder can be written, finishes it.
#[cfg(unix)]
#[test]
fn a_commit_that_cannot_be_undone_is_finished_by_the_next_build() {
    let (p, _) = first_build();
    let locked = p.dir.join("_build/.doctrees");
    if !modes_bind(&locked) {
        return;
    }
    second_sources(&p);
    let target = locked.clone();
    hook_at(3, StepAction::Go, move || lock(&target));
    let r = build(&p);
    clear_hook();
    crate::tree::make_writable(&locked);
    assert_eq!(r.title(), "PUBLICATION NOT FINISHED");
    assert!(r.unsettled && !r.published);
    assert!(build(&p).ok());
    assert_eq!(text_of(&p.out().join("z/nested")), "now a folder\n");
    assert!(text_of(&p.out().join("index.html")).contains("Second."));
    expect_settled(&p);
}

/// A journal that cannot be read (damaged, or written by an older Guidedog) is
/// discarded with a warning, and the build writes every output again.
#[test]
fn an_unreadable_journal_is_discarded() {
    let (p, _) = first_build();
    let journal = p.dir.join("_build/.doctrees/html.commit.json");
    put(&journal, "{\"version\": 1, \"moves\": [\"z/nested\"]}");
    second_sources(&p);
    let r = build(&p);
    assert!(r.ok(), "{:?}", r.problem);
    assert!(r.has_report("UNREADABLE PUBLICATION JOURNAL DISCARDED"));
    assert_eq!(r.reused, 0);
    assert_eq!(text_of(&p.out().join("z/nested")), "now a folder\n");
    expect_settled(&p);
    // A journal of this version that names what no commit writes: more records than a
    // commit has, or a record without a destination. The output stays as it is.
    let kept = snapshot(&p, false);
    let record = "{\"from\": \"a\", \"to\": \"b\", \"previous\": null}";
    let start = "{\"version\": 2, \"out_dir\": \"x\", \"next\": null, \"old\": \"y\", \
                 \"next_id\": null, \"records\": ";
    for bad in [
        format!("{start}[{record}, {record}, {record}]}}"),
        format!("{start}[{{\"from\": \"a\"}}]}}"),
        format!("{start}[{{\"from\": \"a\", \"to\": \"\", \"previous\": null}}]}}"),
    ] {
        put(&journal, &bad);
        let r = build(&p);
        assert!(r.ok());
        assert!(r.has_report("UNREADABLE PUBLICATION JOURNAL DISCARDED"));
        assert_eq!(kept, snapshot(&p, false));
        expect_settled(&p);
    }
}

/// A build that changes nothing publishes nothing: the environment and the manifest
/// are not written again (the same files, with the same times), and nothing is staged.
#[cfg(unix)]
#[test]
fn an_unchanged_build_rewrites_no_records() {
    let p = Project::new("unchanged");
    p.put("index.rst", "Demo\n\nFirst.\n");
    p.put("a.rst", "A\n\nA page.\n");
    assert!(build(&p).ok());
    let records = [
        p.dir.join("_build/.doctrees/environment.json"),
        p.dir.join("_build/.doctrees/html.outputs.json"),
    ];
    let state = |path: &Path| {
        let info = std::fs::metadata(path).unwrap();
        (gd_host::identity(path).unwrap(), info.modified().unwrap())
    };
    let before: Vec<_> = records.iter().map(|r| state(r)).collect();
    let out_before = gd_host::identity(&p.out()).unwrap();
    let again = build(&p);
    assert!(again.ok());
    assert_eq!(again.written, 0);
    let after: Vec<_> = records.iter().map(|r| state(r)).collect();
    assert_eq!(before, after);
    assert_eq!(gd_host::identity(&p.out()).unwrap(), out_before);
    assert!(!p.dir.join("_build/.html.next").exists());
    // A change publishes again.
    p.put("a.rst", "A\n\nA changed page.\n");
    assert!(build(&p).ok());
    assert_ne!(state(&records[1]).0, before[1].0);
    assert!(text_of(&p.out().join("a.html")).contains("changed"));
}

/// An interrupted commit whose new output is gone (someone removed the next folder)
/// cannot be finished: the next build undoes it, says so, and publishes its own.
#[test]
fn an_interrupted_commit_whose_output_is_gone_is_undone() {
    let (p, old) = first_build();
    second_sources(&p);
    hook_at(1, StepAction::Crash, || {});
    assert!(build(&p).unsettled);
    clear_hook();
    std::fs::remove_dir_all(p.dir.join("_build/.html.next")).unwrap();
    // A build that stops before committing leaves what the undone commit kept.
    p.put("new.rst", "New\n\nSee :ref:`missing`.\n");
    let failed = p.build(&Options { warnings_as_errors: true, ..Options::default() });
    assert_eq!(failed.title(), "WARNINGS TREATED AS ERRORS");
    assert!(failed.has_report("INTERRUPTED PUBLICATION UNDONE"), "{failed:?}");
    assert_eq!(old, snapshot(&p, true));
    expect_settled(&p);
    p.put("new.rst", "New\n\nNew page.\n");
    assert!(build(&p).ok());
    assert_eq!(text_of(&p.out().join("z/nested")), "now a folder\n");
}
