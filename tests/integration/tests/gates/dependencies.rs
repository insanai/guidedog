//! Dependency rules, read from `cargo metadata`: the engine stays std-only and
//! embeddable, the supporting libraries stay independent of the engine, Typst stays
//! inside `gd-typst`, external crates are the few the project chose, dependencies point
//! one way (core <- readers/renderers <- defaults <- host <- project <- cli <- programs),
//! and the reusable crates are published and need only each other. This replaces the
//! Odin suite's import scans (independence and boundary tests): cargo already knows
//! every crate's dependencies, so the gate asks it.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use gd_json::Value;

/// Each workspace package's layer. A package may depend only on packages of its own
/// layer or a lower one; a new package must be placed here.
const LAYERS: &[(&str, u8)] = &[
    // 0: the engine core and the supporting libraries, which never use the engine.
    ("guidedoc", 0),
    ("gd-report", 0),
    ("gd-toml", 0),
    ("gd-json", 0),
    ("gd-flate", 0),
    ("gd-sha", 0),
    ("gd-jinja", 0),
    ("gd-highlight", 0),
    ("gd-gettext", 0),
    ("gd-graphdog", 0),
    ("gd-inventory", 0),
    ("gd-theme", 0),
    ("gd-fetch", 0),
    ("gd-git", 0),
    ("gd-pyscan", 0),
    ("gd-autodoc", 0),
    ("gd-odindoc", 0),
    ("gd-rustdoc", 0),
    ("gd-typst", 0),
    ("gd-plugins", 0),
    ("gd-discussion", 0),
    ("gd-clock", 0),
    // 1: readers and renderers.
    ("guidedoc-rst-inline", 1),
    ("guidedoc-rst", 1),
    ("guidedoc-commonmark", 1),
    ("guidedoc-cpp", 1),
    ("guidedoc-sphinx", 1),
    ("guidedoc-html", 1),
    ("guidedoc-typst", 1),
    ("guidedoc-text", 1),
    ("guidedoc-rst-writer", 1),
    // 2: the built-in registry and its highlighter adapter.
    ("guidedoc-defaults", 2),
    // 3: the host: files, configuration, environment, outputs, serving, translation.
    ("gd-host", 3),
    ("gd-conf", 3),
    ("gd-env", 3),
    ("gd-outputs", 3),
    ("gd-serve", 3),
    ("gd-i18n", 3),
    // 4: the project: reading, resolving, linking, building sites and books, publishing.
    ("gd-read", 4),
    ("gd-resolve", 4),
    ("gd-link", 4),
    ("gd-site", 4),
    ("gd-book", 4),
    ("gd-intl", 4),
    ("gd-migrate", 4),
    ("gd-pages", 4),
    ("gd-project", 4),
    // 5: the command line.
    ("gd-cli", 5),
    // 6: programs, examples, tools and tests.
    ("guidedog-cmd", 6),
    ("graphdog-cmd", 6),
    ("guidedog-pages-cmd", 6),
    ("guidedoc-examples", 6),
    ("guidedog-tests", 6),
    ("check", 6),
    ("linkcheck", 6),
    ("xtask", 6),
];

/// The crates other projects may use (docs/manual/internals/crates.rst): published to
/// crates.io, so they may depend only on each other. Every other package is internal and
/// sets `publish = false`.
const REUSABLE: &[&str] = &[
    // The engine.
    "guidedoc",
    "guidedoc-rst",
    "guidedoc-rst-inline",
    "guidedoc-commonmark",
    "guidedoc-sphinx",
    "guidedoc-cpp",
    "guidedoc-html",
    "guidedoc-typst",
    "guidedoc-text",
    "guidedoc-rst-writer",
    "guidedoc-defaults",
    // Libraries.
    "gd-report",
    "gd-toml",
    "gd-json",
    "gd-sha",
    "gd-flate",
    "gd-jinja",
    "gd-highlight",
    "gd-gettext",
    "gd-graphdog",
    "gd-inventory",
    // API generators.
    "gd-pyscan",
    "gd-autodoc",
    "gd-odindoc",
    "gd-rustdoc",
    // The plugin SDK.
    "gd-plugins",
];

/// The external crates a workspace package may depend on directly, and which packages
/// may. Anything else needs a reason in the package's Cargo.toml and an entry here.
const EXTERNAL: &[(&str, &[&str])] = &[
    ("typst", &["gd-typst"]),
    ("typst-layout", &["gd-typst"]),
    ("typst-pdf", &["gd-typst"]),
    ("typst-html", &["gd-typst"]),
    ("typst-kit", &["gd-typst"]),
    ("tree-sitter", &["gd-pyscan", "gd-rustdoc"]),
    ("tree-sitter-python", &["gd-pyscan"]),
    ("tree-sitter-rust", &["gd-rustdoc"]),
    ("serde_json", &["gd-json"]),
    ("serde", &["gd-json"]),
    ("ureq", &["gd-fetch"]),
];

/// The workspace's packages and their compiled (normal and build) dependencies.
struct Graph {
    /// Package id -> name.
    names: BTreeMap<String, String>,
    /// Package ids of the workspace's own packages.
    members: BTreeSet<String>,
    /// Package id -> ids of its normal and build dependencies.
    deps: BTreeMap<String, Vec<String>>,
    /// Package ids whose manifest allows publishing (no `publish = false`).
    publishable: BTreeSet<String>,
    /// Every dependency of a workspace package on another one, by path.
    path_deps: Vec<PathDep>,
}

/// A dependency on a workspace package by path, with the version it asks for.
struct PathDep {
    package: String,
    version: String,
    dependency: String,
    requirement: String,
}

impl Graph {
    fn load() -> Graph {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let root = guidedog_tests::repository_root();
        let output = Command::new(cargo)
            .args(["metadata", "--format-version", "1", "--offline"])
            .current_dir(&root)
            .output()
            .expect("cargo metadata runs");
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let text = String::from_utf8(output.stdout).expect("cargo metadata writes UTF-8");
        let metadata = gd_json::parse(&text).expect("cargo metadata writes JSON");
        Graph::from_metadata(&metadata)
    }

    fn from_metadata(m: &Value) -> Graph {
        let list = |v: &Value, key: &str| v.get(key).and_then(Value::as_array).cloned();
        let text = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(str::to_owned);
        let mut names = BTreeMap::new();
        let mut publishable = BTreeSet::new();
        let mut path_deps = Vec::new();
        for p in list(m, "packages").unwrap_or_default() {
            let id = text(&p, "id").unwrap_or_default();
            // `publish` is null when the manifest does not restrict it.
            if p.get("publish").is_none_or(Value::is_null) {
                publishable.insert(id.clone());
            }
            for d in list(&p, "dependencies").unwrap_or_default() {
                if d.get("path").is_some_and(|path| !path.is_null()) {
                    path_deps.push(PathDep {
                        package: text(&p, "name").unwrap_or_default(),
                        version: text(&p, "version").unwrap_or_default(),
                        dependency: text(&d, "name").unwrap_or_default(),
                        requirement: text(&d, "req").unwrap_or_default(),
                    });
                }
            }
            names.insert(id, text(&p, "name").unwrap_or_default());
        }
        let members = list(m, "workspace_members")
            .unwrap_or_default()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        let mut deps = BTreeMap::new();
        let resolve = m.get("resolve").cloned().unwrap_or(Value::Null);
        for node in list(&resolve, "nodes").unwrap_or_default() {
            let compiled = list(&node, "deps")
                .unwrap_or_default()
                .iter()
                .filter(|d| is_compiled(d))
                .filter_map(|d| text(d, "pkg"))
                .collect();
            deps.insert(text(&node, "id").unwrap_or_default(), compiled);
        }
        Graph { names, members, deps, publishable, path_deps }
    }

    fn name<'a>(&'a self, id: &'a str) -> &'a str {
        self.names.get(id).map_or(id, String::as_str)
    }

    /// Every package `id` needs to compile, itself excluded.
    fn closure(&self, id: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![id.to_owned()];
        while let Some(next) = stack.pop() {
            for dep in self.deps.get(&next).into_iter().flatten() {
                if seen.insert(dep.clone()) {
                    stack.push(dep.clone());
                }
            }
        }
        seen
    }

    fn member_names(&self) -> Vec<(&str, &str)> {
        self.members.iter().map(|id| (id.as_str(), self.name(id))).collect()
    }
}

/// Whether a resolved dependency is compiled into the package (a normal or build
/// dependency), not only into its tests.
fn is_compiled(dep: &Value) -> bool {
    let kinds = dep.get("dep_kinds").and_then(Value::as_array).cloned().unwrap_or_default();
    kinds.iter().any(|k| k.get("kind").and_then(Value::as_str) != Some("dev"))
}

fn layer(name: &str) -> Option<u8> {
    LAYERS.iter().find(|(n, _)| *n == name).map(|(_, l)| *l)
}

fn is_engine(name: &str) -> bool {
    name.starts_with("guidedoc") && layer(name).is_some_and(|l| l <= 2)
}

/// A rule: the problems it finds in a graph.
type Check = fn(&Graph) -> Vec<String>;

/// Packages without a layer, and dependencies on a higher layer.
fn direction_problems(graph: &Graph) -> Vec<String> {
    let mut problems = Vec::new();
    for (id, name) in graph.member_names() {
        let Some(own) = layer(name) else {
            problems.push(format!("{name} has no layer: add it to LAYERS in this gate"));
            continue;
        };
        for dep in graph.deps.get(id).into_iter().flatten().filter(|d| graph.members.contains(*d))
        {
            let dep_name = graph.name(dep);
            if layer(dep_name).is_some_and(|l| l > own) {
                problems
                    .push(format!("{name} (layer {own}) depends on {dep_name}, a higher layer"));
            }
        }
    }
    problems
}

/// External crates the engine needs, directly or not.
fn engine_problems(graph: &Graph) -> Vec<String> {
    let mut problems = Vec::new();
    for (id, name) in graph.member_names().into_iter().filter(|(_, n)| is_engine(n)) {
        for dep in graph.closure(id) {
            if !graph.members.contains(&dep) {
                problems.push(format!("{name} needs the external crate {}", graph.name(&dep)));
            }
        }
    }
    problems
}

/// Engine crates a supporting library needs, directly or not.
fn support_problems(graph: &Graph) -> Vec<String> {
    let mut problems = Vec::new();
    let support =
        graph.member_names().into_iter().filter(|(_, n)| layer(n) == Some(0) && *n != "guidedoc");
    for (id, name) in support {
        for dep in graph.closure(id) {
            if graph.name(&dep).starts_with("guidedoc") {
                problems.push(format!("{name} depends on {}", graph.name(&dep)));
            }
        }
    }
    problems
}

/// Direct external dependencies that are not chosen, or not chosen for that package.
fn external_problems(graph: &Graph) -> Vec<String> {
    let mut problems = Vec::new();
    for (id, name) in graph.member_names() {
        for dep in graph.deps.get(id).into_iter().flatten() {
            if graph.members.contains(dep) {
                continue;
            }
            let dep_name = graph.name(dep);
            match EXTERNAL.iter().find(|(n, _)| *n == dep_name) {
                Some((_, owners)) if owners.contains(&name) => {}
                Some((_, owners)) => problems.push(format!(
                    "{name} depends on {dep_name}, which only {} may use",
                    owners.join(", ")
                )),
                None => problems.push(format!(
                    "{name} depends on {dep_name}, which is not one of the chosen external \
                     crates (record the reason in Cargo.toml and add it to EXTERNAL)"
                )),
            }
        }
    }
    problems
}

/// Internal packages that could be published, and reusable ones that cannot be or that
/// need an internal package or a path dependency without the workspace version.
fn publication_problems(graph: &Graph) -> Vec<String> {
    let mut problems = Vec::new();
    for (id, name) in graph.member_names() {
        let reusable = REUSABLE.contains(&name);
        match (reusable, graph.publishable.contains(id)) {
            (true, false) => problems.push(format!("{name} is reusable but not published")),
            (false, true) => problems.push(format!(
                "{name} is internal: set `publish = false` in its Cargo.toml, or add it to \
                 REUSABLE in this gate and docs/manual/internals/crates.rst"
            )),
            _ => {}
        }
        if !reusable {
            continue;
        }
        for dep in graph.deps.get(id).into_iter().flatten().filter(|d| graph.members.contains(*d))
        {
            let dep_name = graph.name(dep);
            if !REUSABLE.contains(&dep_name) {
                problems.push(format!("{name} is reusable but needs the internal {dep_name}"));
            }
        }
    }
    for dep in graph.path_deps.iter().filter(|d| REUSABLE.contains(&d.package.as_str())) {
        if dep.requirement != format!("^{}", dep.version) {
            problems.push(format!(
                "{} asks for {} {}, not ^{}: set `version` in [workspace.dependencies]",
                dep.package, dep.dependency, dep.requirement, dep.version
            ));
        }
    }
    problems
}

/// Plugins the command line must not need: programs link them and hand gd-cli their
/// registry, so gd-cli knows only gd-plugins' interface.
const PROGRAM_PLUGINS: &[&str] = &["gd-pages", "gd-discussion"];

/// The command line needing a plugin crate, directly or not.
fn plugin_problems(graph: &Graph) -> Vec<String> {
    let mut problems = Vec::new();
    for (id, name) in graph.member_names().into_iter().filter(|(_, n)| *n == "gd-cli") {
        for dep in graph.closure(id) {
            if PROGRAM_PLUGINS.contains(&graph.name(&dep)) {
                problems.push(format!("{name} depends on the plugin {}", graph.name(&dep)));
            }
        }
    }
    problems
}

#[test]
fn the_workspace_follows_the_dependency_rules() {
    let graph = Graph::load();
    let checks: [(&str, Check); 6] = [
        ("dependencies point one way", direction_problems),
        ("the engine stays std-only", engine_problems),
        ("supporting libraries never use the engine", support_problems),
        ("external crates are the chosen few, in their own crates", external_problems),
        ("reusable crates are published and need only each other", publication_problems),
        ("programs link plugins; the command line knows only their interface", plugin_problems),
    ];
    let mut report = String::new();
    for (rule, check) in checks {
        for problem in check(&graph) {
            report.push_str(&format!("{rule}: {problem}\n"));
        }
    }
    assert!(report.is_empty(), "{report}");
}

/// A metadata document with the given packages (`(name, workspace member?)`) and
/// normal dependencies (`(from, to)`).
fn synthetic(packages: &[(&str, bool)], deps: &[(&str, &str)]) -> Graph {
    let mut m = gd_json::Object::new();
    let package = |name: &str| {
        let mut p = gd_json::Object::new();
        p.insert("id".into(), format!("id-{name}").into());
        p.insert("name".into(), name.into());
        Value::Object(p)
    };
    m.insert("packages".into(), Value::Array(packages.iter().map(|p| package(p.0)).collect()));
    let members = packages.iter().filter(|p| p.1).map(|p| format!("id-{}", p.0).into());
    m.insert("workspace_members".into(), Value::Array(members.collect()));
    let node = |name: &str| {
        let edges = deps.iter().filter(|d| d.0 == name).map(|d| {
            gd_json::parse(&format!(r#"{{"pkg": "id-{}", "dep_kinds": [{{"kind": null}}]}}"#, d.1))
                .unwrap()
        });
        let mut n = gd_json::Object::new();
        n.insert("id".into(), format!("id-{name}").into());
        n.insert("deps".into(), Value::Array(edges.collect()));
        Value::Object(n)
    };
    let mut resolve = gd_json::Object::new();
    resolve.insert("nodes".into(), Value::Array(packages.iter().map(|p| node(p.0)).collect()));
    m.insert("resolve".into(), Value::Object(resolve));
    Graph::from_metadata(&Value::Object(m))
}

#[test]
fn the_rules_find_what_they_forbid() {
    let graph = synthetic(
        &[
            ("guidedoc", true),
            ("guidedoc-html", true),
            ("gd-toml", true),
            ("gd-cli", true),
            ("gd-mystery", true),
            ("gd-pages", true),
            ("regex", false),
            ("typst", false),
        ],
        &[
            ("guidedoc-html", "gd-toml"),
            ("gd-toml", "guidedoc-html"),
            ("guidedoc", "regex"),
            ("guidedoc-html", "gd-cli"),
            ("gd-cli", "typst"),
            ("gd-cli", "gd-pages"),
        ],
    );
    let direction = direction_problems(&graph);
    assert!(direction.iter().any(|p| p.contains("gd-mystery has no layer")), "{direction:?}");
    assert!(direction.iter().any(|p| p.contains("guidedoc-html (layer 1) depends on gd-cli")));
    let engine = engine_problems(&graph);
    assert!(engine.iter().any(|p| p == "guidedoc needs the external crate regex"), "{engine:?}");
    assert!(engine.iter().any(|p| p == "guidedoc-html needs the external crate typst"));
    let support = support_problems(&graph);
    assert!(support.iter().any(|p| p == "gd-toml depends on guidedoc-html"), "{support:?}");
    let external = external_problems(&graph);
    assert!(external.iter().any(|p| p.contains("guidedoc depends on regex, which is not")));
    assert!(external.iter().any(|p| p.contains("gd-cli depends on typst, which only gd-typst")));
    let publication = publication_problems(&graph);
    assert!(publication.iter().any(|p| p.starts_with("gd-cli is internal")), "{publication:?}");
    assert!(
        publication.iter().any(|p| p == "guidedoc-html is reusable but needs the internal gd-cli")
    );
    let plugins = plugin_problems(&graph);
    assert_eq!(plugins, ["gd-cli depends on the plugin gd-pages"]);
}
