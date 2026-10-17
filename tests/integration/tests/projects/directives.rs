//! Execute the manual's custom-directive examples, including their raw format branches.

use guidedog_tests::{Scratch, has, lacks, repository_text, rst_code_blocks};

use crate::support::{build, cli, project};

const TUTORIAL: &str = "docs/manual/tutorial/dynamic-content-and-directives.rst";
const GUIDE: &str = "docs/manual/howto/format-specific-content.rst";

#[test]
fn custom_directive_tutorial_builds_both_formats() {
    let dir = Scratch::new("directive-tutorial");
    cli(
        &dir.path,
        0,
        &["quickstart", "-q", "-p", "Cluster Telemetry", "-a", "Operations Team", "-v", "1.0"],
    );
    let text = repository_text(TUTORIAL);
    let conf = format!("{}\n{}", dir.read("conf.toml"), rst_code_blocks(&text, "toml")[0]);
    dir.write("conf.toml", &conf);
    dir.write("index.rst", &rst_code_blocks(&text, "rst")[0]);
    for builder in ["html", "pdf"] {
        build(&dir.path, 0, &[builder, "-n", "-W"]);
    }
    let html = dir.read("_build/html/index.html");
    has(&html, "href=\"#metric-p99_latency\"");
    has(&html, "id=\"rps-slider\"");
    has(&html, "oninput=");
    lacks(&html, "Telemetry SLA Specification");
    has(&dir.read("_build/html/genindex.html"), "cluster telemetry");
    let source = dir.read("_build/pdf/book-en.typ");
    has(&source, "Telemetry SLA Specification");
    has(&source, "metric-p99_latency");
    lacks(&source, "rps-slider");
    assert!(dir.exists("_build/pdf/cluster-telemetry-en.pdf"));
}

#[test]
fn tutorial_theme_function_is_visible_from_the_document() {
    let dir = project("directive-badge");
    let text = repository_text(TUTORIAL);
    let theme =
        format!("{}\n{}", dir.read("_templates/theme.typ"), rst_code_blocks(&text, "typst")[0]);
    dir.write("_templates/theme.typ", &theme);
    dir.write("index.rst", &format!("Badge\n=====\n\n{}", rst_code_blocks(&text, "rst")[1]));
    build(&dir.path, 0, &["pdf", "-W"]);
    has(&dir.read("_build/pdf/book-en.typ"), "#status-badge(\"ok\")");
    assert!(dir.exists("_build/pdf/demo-en.pdf"));
}

#[test]
fn guide_callouts_and_custom_objects_build_both_formats() {
    let dir = project("directive-guide");
    let text = repository_text(GUIDE);
    let toml = rst_code_blocks(&text, "toml");
    let conf = format!("{}\n{}\n{}\n{}", dir.read("conf.toml"), toml[0], toml[1], toml[2]);
    dir.write("conf.toml", &conf);
    dir.write("_static/interactive.js", &rst_code_blocks(&text, "javascript")[0]);
    dir.write("_static/interactive.css", "");
    let theme =
        format!("{}\n{}", dir.read("_templates/theme.typ"), rst_code_blocks(&text, "typst")[0]);
    dir.write("_templates/theme.typ", &theme);
    let rst = rst_code_blocks(&text, "rst");
    let source = format!(
        "Services\n========\n\n{}\n{}\n{}\n{}\n{}\n\n.. endpoint:: check_health()\n\n   Kept.\n",
        rst[0], rst[1], rst[2], rst[3], rst[4]
    );
    dir.write("index.rst", &source);
    for builder in ["html", "pdf"] {
        build(&dir.path, 0, &[builder, "-n", "-W"]);
    }
    let html = dir.read("_build/html/index.html");
    has(&html, "onclick=\"checkHealth()\"");
    has(&html, "href=\"#service-auth-proxy\"");
    lacks(&html, "Security Advisory");
    let source = dir.read("_build/pdf/book-en.typ");
    has(&source, "Security Advisory");
    has(&source, "#danger-box[");
    lacks(&source, "checkHealth()");
    assert!(dir.exists("_build/pdf/demo-en.pdf"));
}

#[test]
fn guide_source_plugin_compiles_and_builds_both_formats() {
    let dir = project("directive-plugin");
    let text = repository_text(GUIDE);
    dir.write("chart.rs", &rst_code_blocks(&text, "rust")[0]);
    let profile = guidedog_tests::profile_dir();
    let program = dir.join(&format!("chart{}", std::env::consts::EXE_SUFFIX));
    let mut rustc = std::process::Command::new("rustc");
    rustc.args(["--edition=2024", "--crate-name=chart"]);
    rustc.arg(dir.join("chart.rs")).arg("-o").arg(&program);
    rustc
        .arg("--extern")
        .arg(format!("gd_plugins={}", profile.join("libgd_plugins.rlib").display()));
    rustc.arg("-L").arg(format!("dependency={}", profile.join("deps").display()));
    guidedog_tests::run_command(&mut rustc).expect(0);
    cli(
        &dir.path,
        0,
        &["plugin", "register", "chart", program.to_str().unwrap(), "--yes", "--project=."],
    );
    let conf = dir.read("conf.toml");
    let conf = conf
        .lines()
        .filter(|line| !line.starts_with("extensions =") && !line.starts_with("source_suffix ="))
        .collect::<Vec<_>>()
        .join("\n");
    dir.write(
        "conf.toml",
        &format!(
            "plugins = [\"chart\"]\nextensions=[\"myst_parser\"]\n\
             source_suffix=[\".rst\", \".md\"]\n{}",
            conf
        ),
    );
    dir.write(
        "index.rst",
        concat!(
            "Charts\n======\n\n.. chart:: latency\n\n",
            ".. code-block:: rst\n\n   .. chart:: latency\n\n",
            ".. toctree::\n\n   notes\n"
        ),
    );
    dir.write("notes.md", "# Notes\n\n.. chart:: latency\n");
    for builder in ["html", "pdf"] {
        build(&dir.path, 0, &[builder, "-W"]);
    }
    has(&dir.read("_build/html/index.html"), "data-type=\"latency\"");
    let markdown = dir.read("_build/html/notes.html");
    has(&markdown, ".. chart:: latency");
    lacks(&markdown, "data-type=\"latency\"");
    has(&dir.read("_build/pdf/book-en.typ"), "Latency Chart");
    assert!(dir.exists("_build/pdf/demo-en.pdf"));
}

#[test]
fn format_guards_and_foreign_raw_content_match_the_manual() {
    let dir = project("directive-policies");
    dir.write(
        "index.rst",
        concat!(
            "Formats\n=======\n\nHalf is :math:`\\frac{1}{2}`.\n\n",
            ".. raw:: latex\n\n   \\documentclass{article}\n\n",
            ".. only:: latex\n\n   .. raw:: typst\n\n      *Compatibility marker*\n\n",
            ".. only:: typst\n\n   .. raw:: typst\n\n      *Unknown tag marker*\n"
        ),
    );
    let result = build(&dir.path, 0, &["pdf"]);
    has(&result.err, "typst.raw.foreign");
    let source = dir.read("_build/pdf/book-en.typ");
    has(&source, "frac(1, 2)");
    has(&source, "Compatibility marker");
    lacks(&source, "Unknown tag marker");
    lacks(&source, "documentclass");
    assert!(dir.exists("_build/pdf/demo-en.pdf"));
}
