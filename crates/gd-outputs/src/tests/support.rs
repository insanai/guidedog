//! A simulated builder: every `*.rst` file of a project folder is a page, its `extras`
//! folder is copied whole (as `html_extra_path`), and a site unit holds an index and a
//! style sheet. Builds go through the real `Generation`, so the tests see what a
//! project build would publish.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use gd_conf::Config;
use gd_env::{
    BUILD_BUDGET, BuilderKind, Digest, DocInfo, Environment, Lookup, LookupKind, ObjectStore,
};
use gd_host::{Budget, Problem};
use gd_report::Report;

use crate::{
    CommitRecords, CommitState, Generation, GenerationOptions, OutputPaths, SITE_UNIT, UnitRecord,
    output_paths,
};

/// A folder removed when dropped.
#[derive(Debug)]
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> TempDir {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("gd-outputs-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(std::fs::canonicalize(&dir).unwrap())
    }

    pub fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        crate::tree::unlock(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Writes a file, making its folders.
pub fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A file's text, or "" when it cannot be read.
pub fn text_of(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// How a simulated build runs.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub all: bool,
    pub warnings_as_errors: bool,
    pub output: Option<PathBuf>,
}

/// What a simulated build did.
#[derive(Debug, Default)]
pub struct Run {
    pub problem: Option<Problem>,
    pub published: bool,
    pub unsettled: bool,
    pub written: usize,
    pub reused: usize,
    pub reports: Vec<Report>,
}

impl Run {
    pub fn title(&self) -> &str {
        self.problem.as_ref().map_or("", |p| p.title)
    }

    pub fn ok(&self) -> bool {
        self.problem.is_none()
    }

    pub fn has_report(&self, title: &str) -> bool {
        self.reports.iter().any(|r| r.title == title)
    }
}

/// A project folder with its configuration.
#[derive(Debug)]
pub struct Project {
    pub dir: TempDir,
    pub config: Config,
}

impl Project {
    pub fn new(name: &str) -> Project {
        let dir = TempDir::new(name);
        let config = Config::new(&dir.0, dir.join("_build"));
        Project { dir, config }
    }

    pub fn put(&self, rel: &str, text: &str) {
        put(&self.dir.join(rel), text);
    }

    pub fn remove(&self, rel: &str) {
        let _ = crate::tree::remove_tree(&self.dir.join(rel));
    }

    pub fn out(&self) -> PathBuf {
        self.dir.join("_build/html")
    }

    pub fn paths(&self, options: &Options) -> Result<OutputPaths, Problem> {
        output_paths(&self.config, BuilderKind::Html, options.output.as_deref())
    }

    /// Builds the project's HTML through a generation.
    pub fn build(&self, options: &Options) -> Run {
        let mut run = Run::default();
        let paths = match self.paths(options) {
            Ok(paths) => paths,
            Err(p) => return Run { problem: Some(p), ..run },
        };
        let budget = Budget::new(BUILD_BUDGET);
        let store = ObjectStore::new(&paths.doctrees);
        let begun = Generation::begin(
            paths,
            GenerationOptions { all: options.all, budget: &budget },
            &mut run.reports,
        );
        let mut generation = match begun {
            Ok(g) => g,
            Err(p) => return Run { problem: Some(p), unsettled: true, ..run },
        };
        let pages = self.pages();
        let warned = self.write_pages(&mut generation, &pages, &mut run);
        if let Err(p) = self.write_site(&mut generation, &pages, &mut run.reports) {
            return Run { problem: Some(p), ..run };
        }
        if options.warnings_as_errors && warned {
            run.problem =
                Some(Problem::new("build.warnings", "WARNINGS TREATED AS ERRORS", "", "", "", 1));
            return run; // the generation is dropped: abandoned.
        }
        let environment = environment_of(&pages);
        let outcome =
            generation.commit(CommitRecords { environment: &environment, store: &store });
        run.reports.extend(outcome.reports);
        run.problem = outcome.problem;
        run.published = outcome.state == CommitState::Published;
        run.unsettled = outcome.state == CommitState::Unsettled;
        run
    }

    /// The pages: docname to text, from the `*.rst` files of the project folder.
    fn pages(&self) -> BTreeMap<String, String> {
        let mut pages = BTreeMap::new();
        for entry in std::fs::read_dir(&self.dir.0).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(docname) = name.strip_suffix(".rst") {
                pages.insert(docname.to_string(), text_of(&entry.path()));
            }
        }
        pages
    }

    /// Writes or reuses every page; returns whether a page warned.
    fn write_pages(
        &self,
        generation: &mut Generation,
        pages: &BTreeMap<String, String>,
        run: &mut Run,
    ) -> bool {
        let index_title = pages.get("index").map_or("", |t| title_of(t)).to_string();
        let answer = |lookup: &Lookup| match lookup.kind {
            LookupKind::Doc if lookup.key == "index" => Digest::of_text(&index_title),
            _ => Digest::default(),
        };
        let mut warned = false;
        for (docname, text) in pages {
            let inputs = Digest::of_text(text);
            if let Some(old) = generation.reuse(docname, inputs, answer) {
                warned |= !old.reports.is_empty();
                run.reports.extend(old.reports.iter().cloned());
                run.reused += 1;
                continue;
            }
            let mut reports = Vec::new();
            if text.contains(":ref:`missing`") {
                reports.push(Report::error("ref.missing", "UNDEFINED LABEL", "missing"));
            }
            warned |= !reports.is_empty();
            run.reports.extend(reports.iter().cloned());
            let mut unit = generation.unit(docname);
            let html = format!("<h1>{}</h1>\n{text}", title_of(text));
            unit.emit_text(&format!("{docname}.html"), &html).unwrap();
            let lookups = vec![(
                Lookup::new(LookupKind::Doc, "index"),
                answer(&Lookup::new(LookupKind::Doc, "index")),
            )];
            unit.finish(UnitRecord { inputs, lookups, reports, ..UnitRecord::default() });
            run.written += 1;
        }
        warned
    }

    fn write_site(
        &self,
        generation: &mut Generation,
        pages: &BTreeMap<String, String>,
        reports: &mut Vec<Report>,
    ) -> Result<(), Problem> {
        let budget = Budget::new(BUILD_BUDGET);
        let roots = gd_host::canonical_roots(&[&self.dir.0]);
        let mut unit = generation.unit(SITE_UNIT);
        let names: Vec<&str> = pages.keys().map(String::as_str).collect();
        unit.emit_text("genindex.html", &names.join("\n"))?;
        unit.emit_text("_static/style.css", "body { margin: 0 }\n")?;
        unit.emit_tree(&self.dir.join("extras"), "", &roots, &budget, reports)?;
        unit.finish(UnitRecord::default());
        Ok(())
    }
}

fn title_of(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

fn environment_of(pages: &BTreeMap<String, String>) -> Environment {
    let docs = pages.iter().map(|(docname, text)| DocInfo {
        docname: docname.clone(),
        relative: format!("{docname}.rst"),
        digest: Digest::of_text(text),
        title: title_of(text).to_string(),
        ..DocInfo::default()
    });
    Environment { settings: Digest::of_text("settings"), docs: docs.collect() }
}

/// A folder's files and folders, each with its contents ("/" for a folder), and, with
/// `records`, the build records, so two states can be compared whole.
pub fn snapshot(project: &Project, records: bool) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut pending = vec![(String::new(), project.out())];
    while let Some((prefix, dir)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let rel = format!("{prefix}{}", e.file_name().to_string_lossy());
            if e.file_type().unwrap().is_dir() {
                out.insert(rel.clone(), "/".to_string());
                pending.push((format!("{rel}/"), e.path()));
            } else {
                out.insert(rel, text_of(&e.path()));
            }
        }
    }
    if records {
        for name in ["environment.json", "html.outputs.json"] {
            out.insert(
                name.to_string(),
                text_of(&project.dir.join("_build/.doctrees").join(name)),
            );
        }
    }
    out
}

/// Checks that nothing a commit prepares is left beside the output but the spare
/// output (`.html.next`).
pub fn expect_settled(project: &Project) {
    for left in [
        "_build/.doctrees/html.commit.json",
        "_build/.html.old",
        "_build/.doctrees/html.environment.json.next",
        "_build/.doctrees/html.environment.json.previous",
        "_build/.doctrees/html.outputs.json.next",
        "_build/.doctrees/html.outputs.json.previous",
        "_build/.doctrees/html.outputs.json.restore",
        "_build/.doctrees/environment.json.restore",
    ] {
        assert!(!project.dir.join(left).exists(), "{left} is left");
    }
}
