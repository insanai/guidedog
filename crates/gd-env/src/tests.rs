//! Tests of the source patterns, discovery, the environment's JSON and the object
//! store.

use std::path::{Path, PathBuf};

use gd_conf::Config;

use super::*;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("gd-env-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn put(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn patterns_match_like_sphinx() {
    let yes = [
        ("_build", "_build"),
        ("*.rst", "index.rst"),
        ("drafts/*", "drafts/a.rst"),
        ("**/.DS_Store", "a/b/.DS_Store"),
        ("**", "a/b/c.rst"),
        ("a/**/z.rst", "a/b/c/z.rst"),
        ("a/**/z.rst", "a/z.rst"),
        ("doc?.rst", "doc1.rst"),
        ("[a-c]*.rst", "b.rst"),
        ("[!x]*", "y.rst"),
        ("Thumbs.db", "Thumbs.db"),
    ];
    let no = [
        ("*.rst", "a/index.rst"),
        ("drafts/*", "drafts/a/b.rst"),
        ("doc?.rst", "doc12.rst"),
        ("[a-c]*.rst", "d.rst"),
        ("[!x]*", "x.rst"),
        ("_build", "_build2"),
    ];
    for (p, n) in yes {
        assert!(glob_match(p, n), "{p} should match {n}");
    }
    for (p, n) in no {
        assert!(!glob_match(p, n), "{p} matched {n}");
    }
    let mut c = Config::new("/p", "/p/_build");
    c.exclude_patterns = vec!["_build".into(), "drafts".into()];
    assert!(is_excluded(&c, "drafts/a/b.rst"));
    assert!(!is_excluded(&c, "guide/drafts.rst"));
}

#[test]
fn discovery_lists_documents_by_docname() {
    let dir = TempDir::new("discover");
    dir.put("index.rst", "Index\n");
    dir.put("guide/intro.md", "# Intro\n");
    dir.put("guide/notes.txt", "not a document\n");
    dir.put("drafts/wip.rst", "WIP\n");
    dir.put("_templates/page.rst", "a template folder\n");
    dir.put(".hidden/x.rst", "hidden\n");
    dir.put("_build/html/old.rst", "output\n");
    let mut c = Config::new(dir.0.clone(), dir.0.join("_build"));
    c.source_suffix = vec![".rst".into(), ".md".into()];
    c.exclude_patterns = vec!["drafts".into()];
    c.templates_path = vec!["_templates".into()];
    let found = discover(&c).unwrap();
    let names: Vec<&str> = found.iter().map(|f| f.docname.as_str()).collect();
    assert_eq!(names, ["guide/intro", "index"]);
    assert_eq!(found[0].kind, SourceKind::Markdown);
    assert_eq!(found[0].relative, "guide/intro.md");
    dir.put("index.md", "# Twice\n");
    let p = discover(&c).unwrap_err();
    assert_eq!(p.code, "project.duplicate");
}

#[test]
fn contents_is_the_root_without_an_index() {
    let mut c = Config::new("/p", "/p/_build");
    gd_conf::finish_config(&mut c);
    let source = |name: &str| SourceFile { docname: name.into(), ..SourceFile::default() };
    assert!(contents_root(&mut c, &[source("index"), source("contents")]).is_none());
    let report = contents_root(&mut c, &[source("contents")]).unwrap();
    assert_eq!(report.code, "build.root.contents");
    assert_eq!(c.root_doc, "contents");
    assert_eq!(c.pdf.documents[0].root, "contents");
}

fn sample_doc() -> DocInfo {
    DocInfo {
        docname: "guide/intro".into(),
        relative: "guide/intro.rst".into(),
        digest: Digest::of_text("source"),
        deps: vec![Dependency {
            path: "/p/inc.rst".into(),
            digest: Digest::of_text("inc"),
            included: true,
        }],
        object: Some(CachedFile { path: "objects/x.gdo".into(), digest: Digest::of_text("o") }),
        title: "Intro".into(),
        sections: vec![SectionInfo {
            level: 1,
            title: "Intro".into(),
            anchor: "intro".into(),
            parent: None,
            ids: vec!["intro".into()],
            ..SectionInfo::default()
        }],
        toctrees: vec![ToctreeInfo {
            section: Some(0),
            entries: vec![TocEntry { target: "a".into(), title: String::new() }],
            ..ToctreeInfo::default()
        }],
        labels: vec![LabelInfo {
            name: "intro".into(),
            kind: LabelKind::Section,
            ..Default::default()
        }],
        meta: vec![("description".into(), "text".into())],
        reports: vec![gd_report::Report::error("x.y", "TITLE", "message").hint("hint")],
        ..DocInfo::default()
    }
}

#[test]
fn environments_round_trip_through_json() {
    let env = Environment {
        settings: Digest::of_text("settings"),
        docs: vec![sample_doc()],
        ..Default::default()
    };
    let text = env.to_json();
    let back = Environment::from_json(&text).unwrap();
    assert_eq!(back, env);
    assert_eq!(back.id_of("guide/intro"), Some(DocId(0)));
    let other = text.replace(&format!("\"version\":{ENVIRONMENT_VERSION}"), "\"version\":1");
    assert!(Environment::from_json(&other).is_none());
}

#[test]
fn saved_environments_are_reused_only_with_the_same_settings() {
    let dir = TempDir::new("env");
    let env = Environment {
        settings: Digest::of_text("a"),
        docs: vec![sample_doc()],
        ..Default::default()
    };
    let path = dir.put("environment.json", &env.to_json());
    let budget = gd_host::Budget::new(BUILD_BUDGET);
    let same = load_environment(&path, Digest::of_text("a"), false, &budget).unwrap();
    assert_eq!(same.docs.len(), 1);
    let other = load_environment(&path, Digest::of_text("b"), false, &budget).unwrap();
    assert!(other.docs.is_empty());
    let fresh = load_environment(&path, Digest::of_text("a"), true, &budget).unwrap();
    assert!(fresh.docs.is_empty());
    let missing = load_environment(Path::new("/no/such"), Digest::default(), false, &budget);
    assert!(missing.unwrap().docs.is_empty());
}

#[test]
fn cached_files_are_named_by_their_digest_and_swept() {
    let dir = TempDir::new("store");
    let store = ObjectStore::new(&dir.0);
    let kept = store.save_search("guide/intro", "alpha beta").unwrap();
    let gone = store.save_search("old", "gamma").unwrap();
    assert!(kept.path.starts_with("search/guide/intro.") && kept.path.ends_with(".txt"));
    assert_eq!(store.load_bytes(&kept).unwrap(), b"alpha beta");
    assert_eq!(file_digest(&store.path_of(&kept)), Some(kept.digest));
    let wrong = CachedFile { digest: Digest::of_text("x"), ..kept.clone() };
    assert!(store.load_bytes(&wrong).unwrap_err().stale);
    store.sweep([kept.path.as_str()]);
    assert!(store.path_of(&kept).is_file());
    assert!(!store.path_of(&gone).exists());
}

#[test]
fn reading_settings_follow_the_configuration() {
    let a = Config::new("/p", "/p/_build");
    let mut b = a.clone();
    assert_eq!(reading_settings(&a), reading_settings(&b));
    b.rust_api.member_order = "alphabetical".into();
    assert_ne!(reading_settings(&a), reading_settings(&b));
    let mut c = a.clone();
    c.html.theme = "alabaster".into(); // an output setting, not a reading one.
    assert_eq!(reading_settings(&a), reading_settings(&c));
}

#[test]
fn reading_settings_follow_the_extensions_that_change_documents() {
    let a = Config::new("/p", "/p/_build");
    let changed = |change: fn(&mut Config)| {
        let mut b = a.clone();
        change(&mut b);
        reading_settings(&a) != reading_settings(&b)
    };
    assert!(changed(|c| c.extensions.push("sphinx.ext.napoleon".into())));
    assert!(changed(|c| c.ext.napoleon.use_ivar = true));
    assert!(changed(|c| c.ext.autosummary.generate = gd_conf::FlagOrList::Flag(false)));
    assert!(changed(|c| c.ext.apidoc.max_depth = 2));
    assert!(changed(|c| c.ext.linkcode.url = "https://x/{path}".into()));
    assert!(changed(|c| c.ext.viewcode.line_numbers = true));
    assert!(changed(|c| {
        c.ext.inheritance.graph_attrs.insert("rankdir".into(), "LR".into());
    }));
    // Reports on a build change no document.
    assert!(!changed(|c| c.ext.duration.n_slowest = 10));
    assert!(!changed(|c| c.ext.coverage.write_headline = false));
}
