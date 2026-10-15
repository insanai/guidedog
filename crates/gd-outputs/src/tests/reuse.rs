//! `Generation::reuse`, the cutoff point of incremental writing, and the unit API.

use gd_env::{Asset, BUILD_BUDGET, Digest, Environment, Lookup, LookupKind, ObjectStore};
use gd_host::Budget;
use gd_report::Report;

use super::support::{Options, Project, put, text_of};
use crate::{CommitRecords, CommitState, Generation, GenerationOptions, Manifest, UnitRecord};

fn begin(p: &Project, all: bool, budget: &Budget) -> Generation {
    let paths = p.paths(&Options::default()).unwrap();
    let mut reports = Vec::new();
    Generation::begin(paths, GenerationOptions { all, budget }, &mut reports).unwrap()
}

fn commit(generation: Generation) -> CommitState {
    let store = ObjectStore::new(&generation.paths().doctrees);
    let env = Environment::default();
    let outcome = generation.commit(CommitRecords { environment: &env, store: &store });
    assert!(outcome.problem.is_none(), "{:?}", outcome.problem);
    outcome.state
}

fn label(key: &str) -> Lookup {
    Lookup::new(LookupKind::Label, key)
}

/// Publishes one unit `page` with a lookup of the label `intro` answered `answer`.
fn publish_page(p: &Project, budget: &Budget, answer: Digest) {
    let mut generation = begin(p, false, budget);
    let mut unit = generation.unit("page");
    unit.emit_text("page.html", "page").unwrap();
    unit.finish(UnitRecord {
        inputs: Digest::of_text("inputs"),
        assets: vec![Asset { source: p.dir.join("img.png"), target: "_images/img.png".into() }],
        lookups: vec![(label("intro"), answer)],
        reports: vec![Report::error("ref.x", "A REPORT", "kept")],
    });
    assert_eq!(commit(generation), CommitState::Published);
}

#[test]
fn a_unit_is_reused_only_while_its_inputs_and_answers_hold() {
    let p = Project::new("reuse");
    let budget = Budget::new(BUILD_BUDGET);
    let answer = Digest::of_text("intro is in a");
    publish_page(&p, &budget, answer);
    let inputs = Digest::of_text("inputs");
    let mut g = begin(&p, false, &budget);
    let old =
        g.reuse("page", inputs, |l| if *l == label("intro") { answer } else { Digest::default() });
    let old = old.expect("unchanged unit is reused");
    assert_eq!(old.reports[0].message, "kept");
    assert_eq!(old.assets[0].target, "_images/img.png");
    assert_eq!(old.files, ["page.html"]);
    // A changed answer, changed inputs, or a new unit: written again.
    assert!(g.reuse("page", inputs, |_| Digest::of_text("intro moved")).is_none());
    assert!(g.reuse("page", Digest::of_text("other"), |_| answer).is_none());
    assert!(g.reuse("new", inputs, |_| answer).is_none());
    drop(g);
    // -a reuses nothing.
    let mut all = begin(&p, true, &budget);
    assert!(all.reuse("page", inputs, |_| answer).is_none());
    drop(all);
    // A file that is gone: written again.
    std::fs::remove_file(p.out().join("page.html")).unwrap();
    let mut g = begin(&p, false, &budget);
    assert!(g.reuse("page", inputs, |_| answer).is_none());
}

#[test]
fn a_reused_unit_keeps_its_files_in_the_next_generation() {
    let p = Project::new("kept");
    let budget = Budget::new(BUILD_BUDGET);
    let answer = Digest::of_text("a");
    publish_page(&p, &budget, answer);
    let mut g = begin(&p, false, &budget);
    assert!(g.reuse("page", Digest::of_text("inputs"), |_| answer).is_some());
    let mut unit = g.unit("other");
    unit.emit_text("other.html", "other").unwrap();
    unit.finish(UnitRecord::default());
    assert_eq!(commit(g), CommitState::Published);
    assert_eq!(text_of(&p.out().join("page.html")), "page");
    assert_eq!(text_of(&p.out().join("other.html")), "other");
    let manifest =
        Manifest::from_json(&text_of(&p.dir.join("_build/.doctrees/html.outputs.json")));
    let manifest = manifest.unwrap();
    assert_eq!(manifest.units["page"].lookups, [(label("intro"), answer)]);
    // A unit not written again is obsolete: its files go.
    let mut g = begin(&p, false, &budget);
    assert!(g.reuse("other", Digest::default(), |_| answer).is_none());
    let mut unit = g.unit("other");
    unit.emit_text("other.html", "other").unwrap();
    unit.finish(UnitRecord::default());
    assert_eq!(commit(g), CommitState::Published);
    assert!(!p.out().join("page.html").exists());
}

#[test]
fn unit_paths_stay_in_the_output_folder() {
    let p = Project::new("unit-paths");
    let budget = Budget::new(BUILD_BUDGET);
    let mut g = begin(&p, false, &budget);
    let mut unit = g.unit("u");
    assert_eq!(unit.emit_text("../escape.html", "x").unwrap_err().code, "build.output");
    assert_eq!(unit.emit_text("", "x").unwrap_err().code, "build.output");
    unit.emit_text("a/b.html", "x").unwrap();
    assert_eq!(unit.taken_path("A/B.HTML"), Some("a/b.html"));
    assert_eq!(unit.taken_path("a"), Some("a/b.html"));
    assert_eq!(unit.taken_path("a/b.html/c"), Some("a/b.html"));
    assert_eq!(unit.taken_path("a/bc.html"), None);
    // A file and files below it are written twice.
    let problem = unit.emit_text("a/b.html/c.html", "y").unwrap_err();
    assert_eq!(problem.title, "A FILE AND A FOLDER AT THE SAME PATH");
    unit.finish(UnitRecord::default());
    assert_eq!(g.taken_path("a/b.html"), Some("a/b.html"));
}

#[test]
fn a_staged_path_is_free_and_published() {
    let p = Project::new("stage-path");
    let budget = Budget::new(BUILD_BUDGET);
    let mut g = begin(&p, false, &budget);
    let mut unit = g.unit("book:manual.pdf");
    let path = unit.stage_path("manual.pdf").unwrap();
    assert!(!path.exists());
    put(&path, "%PDF");
    unit.keep_file("../outside"); // not a path of the output: ignored.
    unit.finish(UnitRecord::default());
    assert_eq!(commit(g), CommitState::Published);
    assert_eq!(text_of(&p.out().join("manual.pdf")), "%PDF");
}

#[test]
fn trees_are_copied_without_hidden_names_or_outside_links() {
    let p = Project::new("tree");
    let outside = super::support::TempDir::new("tree-outside");
    p.put("static/a.css", "a");
    p.put("static/sub/b.css", "b");
    p.put("static/.hidden", "h");
    put(&outside.join("secret.txt"), "secret");
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.join("secret.txt"), p.dir.join("static/link.txt")).unwrap();
    let budget = Budget::new(BUILD_BUDGET);
    let roots = gd_host::canonical_roots(&[&p.dir.0]);
    let mut reports = Vec::new();
    let mut g = begin(&p, false, &budget);
    let mut unit = g.unit("");
    unit.emit_tree(&p.dir.join("static"), "_static", &roots, &budget, &mut reports).unwrap();
    unit.emit_tree(&outside.0, "_other", &roots, &budget, &mut reports).unwrap();
    unit.finish(UnitRecord::default());
    assert_eq!(commit(g), CommitState::Published);
    assert_eq!(text_of(&p.out().join("_static/a.css")), "a");
    assert_eq!(text_of(&p.out().join("_static/sub/b.css")), "b");
    assert!(!p.out().join("_static/.hidden").exists());
    assert!(!p.out().join("_static/link.txt").exists());
    assert!(!p.out().join("_other").exists());
    let codes: Vec<&str> = reports.iter().map(|r| r.code.as_str()).collect();
    assert!(codes.contains(&"build.output.outside"), "{codes:?}");
    #[cfg(unix)]
    assert!(codes.contains(&"build.output.link"), "{codes:?}");
}
