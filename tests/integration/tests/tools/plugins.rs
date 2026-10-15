//! Installed build plugins through the program: the example badge registered on a
//! private machine registry transforms and stages a project for every HTML builder,
//! rebuilds keep their output, and failures (bad settings, untrusted builds, asset
//! conflicts, crashing or stalling programs) leave the last good site in place.
//!
//! The protocol itself, publishers and their failures are covered by gd-plugins'
//! `build_plugins.rs` and `publishers.rs`; the registry commands by guidedog's
//! `process.rs`. The example programs are the ones cargo builds for gd-plugins
//! (`target/<profile>/examples`).

use std::path::{Path, PathBuf};
use std::process::Command;

use guidedog_tests::{Run, Scratch, has, profile_dir, read, run_command};

use crate::flat;

/// Links the example program `example` (built by cargo for gd-plugins) into `root` as
/// `name`, and returns its path.
fn example_program(root: &Scratch, example: &str, name: &str) -> String {
    let suffix = std::env::consts::EXE_SUFFIX;
    let built = profile_dir().join("examples").join(format!("{example}{suffix}"));
    assert!(
        built.is_file(),
        "{} is not built. Run `cargo test --workspace` (or `cargo build -p gd-plugins \
         --examples`) first.",
        built.display()
    );
    let program = root.join(&format!("{name}{suffix}"));
    if std::fs::hard_link(&built, &program).is_err() {
        std::fs::copy(&built, &program).unwrap();
    }
    program.to_string_lossy().into_owned()
}

/// Runs guidedog with the machine registry kept in `root/state`.
fn cli(root: &Scratch, args: &[&str]) -> Run {
    let mut command = Command::new(guidedog_tests::program("guidedog"));
    command
        .args(args)
        .current_dir(&root.path)
        .env("NO_COLOR", "1")
        .env("GUIDEDOG_CONFIG_DIR", root.join("state"));
    run_command(&mut command)
}

/// A quickstart project in `root/docs` with `conf` as its configuration.
fn docs_project(root: &Scratch, conf: &str) -> PathBuf {
    cli(root, &["quickstart", "docs", "-q", "-p", "Demo", "-a", "Team"]).expect(0);
    root.write("docs/conf.toml", conf);
    root.join("docs")
}

fn arg(prefix: &str, path: &Path) -> String {
    format!("{prefix}{}", path.display())
}

#[test]
fn an_installed_build_plugin_transforms_and_stages_a_project() {
    let root = Scratch::new("badge");
    let program = example_program(&root, "build_plugin", "badge");
    let docs = docs_project(
        &root,
        "project = \"Orbit\"\nplugins = [\"product-badge\"]\n\
         source_suffix = [\".rst\", \".md\"]\nextensions = [\"myst_parser\"]\n\
         html_static_path = [\"_static\"]\n[plugin_configs]\nproduct-badge = \"badge.toml\"\n",
    );
    root.write("docs/badge.toml", "product = \"Orbit\"\n");
    let project = arg("--project=", &docs);
    cli(&root, &["plugin", "check", &program, &project, "--config=badge.toml"]).expect(0);
    assert!(!root.exists("state/plugins.toml"), "checking registered the plugin");
    let register = [
        "plugin",
        "register",
        "product-badge",
        &program,
        "--yes",
        &project,
        "--config=badge.toml",
    ];
    cli(&root, &register).expect(0);
    root.write(
        "docs/index.rst",
        "Welcome\n=======\n\nHello {{product}}.\n\n.. toctree::\n\n   nested/page\n",
    );
    root.write("docs/nested/page.md", "# Details\n\nHello {{product}}.\n");
    let docs_arg = docs.to_string_lossy().into_owned();
    for builder in ["html", "dirhtml", "singlehtml"] {
        cli(&root, &["build", builder, &docs_arg, "-W", "-j", "4", "--color=never"]).expect(0);
        let output = docs.join("_build").join(builder);
        has(&read(&output.join("index.html")), "Hello Orbit.");
        has(&read(&output.join("search.html")), "product-badge");
        assert!(output.join("_static/plugins/product-badge/banner.css").is_file());
        if builder != "singlehtml" {
            let (page, prefix) = match builder {
                "dirhtml" => ("nested/page/index.html", "../../"),
                _ => ("nested/page.html", "../"),
            };
            let css = format!("{prefix}_static/plugins/product-badge/banner.css");
            has(&read(&output.join(page)), &css);
        }
        let modified =
            || std::fs::metadata(output.join("index.html")).unwrap().modified().unwrap();
        let before = modified();
        let again = cli(&root, &["build", builder, &docs_arg, "-W", "--color=never"]).expect(0);
        if builder != "singlehtml" {
            has(&again.out, "0 output(s) written");
        }
        assert_eq!(modified(), before, "{builder}: an unchanged page was written again");
    }
    badge_failures_preserve_the_site(&root, &docs_arg);
    badge_settings_folder(&root, &docs_arg);
}

/// Bad settings, an untrusted build and a project file in the plugin's asset folder
/// each stop the build, and the published page stays as it was.
fn badge_failures_preserve_the_site(root: &Scratch, docs: &str) {
    root.write("docs/badge.toml", "product = \"Comet\"\n");
    let r = cli(root, &["build", "html", docs, "-W", "--color=never"]).expect(0);
    has(&r.out, "2 document(s) parsed");
    let before = root.read("docs/_build/html/index.html");
    has(&before, "Hello Comet.");
    root.write("docs/badge.toml", "product = 42\n");
    let r = cli(root, &["build", "html", docs, "--color=never"]).expect(2);
    has(&flat(&r.err), "product = \"Orbit\"");
    assert_eq!(root.read("docs/_build/html/index.html"), before);
    root.write("docs/badge.toml", "product = \"Comet\"\n");
    let r = cli(root, &["build", "html", docs, "--untrusted", "--color=never"]).expect(2);
    has(&flat(&r.err), "without --untrusted");
    assert_eq!(root.read("docs/_build/html/index.html"), before);
    root.write("docs/_static/plugins/product-badge/banner.css", "project file");
    let r = cli(root, &["build", "html", docs, "--color=never"]).expect(2);
    has(&flat(&r.err), "Move the project file");
    assert_eq!(root.read("docs/_build/html/index.html"), before);
    std::fs::remove_file(root.join("docs/_static/plugins/product-badge/banner.css")).unwrap();
    // Source hooks apply to every builder, not only HTML.
    cli(root, &["build", "text", docs, "-W", "--color=never"]).expect(0);
    has(&root.read("docs/_build/text/index.txt"), "Hello Comet.");
}

/// `-c` names another settings folder, whose plugin settings are the ones used; an
/// extra argument never silently turns a builder into the default one.
fn badge_settings_folder(root: &Scratch, docs: &str) {
    root.write("other-settings/conf.toml", &root.read("docs/conf.toml"));
    root.write("other-settings/badge.toml", "product = \"Away\"\n");
    let settings = root.join("other-settings").to_string_lossy().into_owned();
    let output = root.join("other-html").to_string_lossy().into_owned();
    cli(root, &["build", "-b", "html", docs, &output, "-c", &settings, "-W"]).expect(0);
    has(&root.read("other-html/index.html"), "Hello Away.");
    let r = cli(root, &["build", docs, "dirhtml", "--color=never"]).expect(2);
    has(&flat(&r.err), "Put the builder first");
}

/// Crashes, stalls, malformed problems and plugin-expanded markup become diagnostics;
/// none replaces the last good site, and a healthy next build still works.
#[test]
fn failed_build_plugin_processes_leave_the_site_in_place() {
    let root = Scratch::new("broken");
    let program = example_program(&root, "protocol_fixture", "broken-build");
    let docs = docs_project(
        &root,
        "project = \"Demo\"\nplugins = [\"broken-build\"]\nplugin_timeout = 1\n\
         [plugin_configs]\nbroken-build = \"mode.txt\"\n",
    );
    let project = arg("--project=", &docs);
    root.write("docs/mode.txt", "bounds");
    let register =
        ["plugin", "register", "broken-build", &program, "--yes", &project, "--config=mode.txt"];
    let refused = cli(&root, &register);
    assert!((1..=4).contains(&refused.status), "{refused:?}");
    assert!(!root.exists("state/plugins.toml"), "a failing plugin was registered");
    root.write("docs/mode.txt", "ok");
    cli(&root, &register).expect(0);
    let docs_arg = docs.to_string_lossy().into_owned();
    cli(&root, &["build", "html", &docs_arg, "-W"]).expect(0);
    let before = root.read("docs/_build/html/index.html");
    for mode in ["exit", "bounds", "stall", "bad-problem", "markup"] {
        root.write("docs/mode.txt", mode);
        let r = cli(&root, &["build", "html", &docs_arg, "-W", "--color=never"]);
        assert!((1..=4).contains(&r.status), "{mode}: {r:?}");
        if mode == "markup" {
            has(&flat(&r.err), "plugin-expanded input");
            has(&r.err, "not-a-directive");
        } else {
            has(&r.err, "broken-build");
        }
        assert_eq!(root.read("docs/_build/html/index.html"), before, "{mode}");
    }
    root.write("docs/mode.txt", "ok");
    cli(&root, &["build", "html", &docs_arg, "-W"]).expect(0);
}
