//! The Rust code the manual shows is code that compiles: every `.. code-block:: rust` in
//! `docs/manual` has a twin in this file, between a `// manual: <file>#<n>` marker (the
//! n-th Rust block of that file) and `// end manual`, and the test checks that the twin
//! and the manual's text are the same. The compiler checks the twin. A block that only
//! sketches an API (a trait's signatures, say) carries `:class: sketch` and is exempt.
//!
//! Programs (`fn main`) become modules; a fragment becomes the body of a function whose
//! parameters declare what the prose around it names. The engine examples also run.
//! This is the Odin suite's examples test; the example programs themselves are the
//! `guidedoc-examples` crate's, built and run by its own tests.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.
#![allow(dead_code)] // the documented programs' `main`s are compiled, not all called.

use guidedog_tests::{repository_root, rst_code_block_list};

/// The linked build plugin of the plugin chapter.
#[path = "../../../crates/gd-plugins/examples/build_plugin/badge.rs"]
mod badge;

#[rustfmt::skip] // a twin keeps the manual's exact text
mod install_embed {
    // manual: docs/manual/howto/install.rst#1
    use guidedoc::{Registry, Workspace, convert_text};

    fn main() {
        let registry = Registry {
            readers: vec![guidedoc_commonmark::READER],
            renderers: vec![guidedoc_html::renderer()],
        };
        let mut ws = Workspace::new(); // reuse it: its buffers are kept
        match convert_text(&mut ws, &registry, "commonmark", "html", "# Hello\n") {
            Ok(html) => println!("{html}"),
            Err(fail) => eprintln!("{fail}"), // title, message, and hint
        }
    }
    // end manual

    #[test]
    fn the_embedding_example_runs() {
        main();
    }
}

#[rustfmt::skip] // a twin keeps the manual's exact text
mod plugins_linked {
    use super::badge;

    // manual: docs/manual/plugins.rst#1
    use std::process::ExitCode;

    use gd_plugins::Registry;

    fn main() -> ExitCode {
        // Registry::new validates IDs, versions and hooks, so a bad list fails here.
        let linked = match Registry::new(vec![badge::plugin()]) {
            Ok(registry) => registry,
            Err(problem) => {
                let style = gd_report::Style::default();
                eprint!("{}", gd_report::render_text(&problem.report(), &style));
                return ExitCode::from(2);
            }
        };
        gd_cli::run_with(linked)
    }
    // end manual
}

#[rustfmt::skip] // a twin keeps the manual's exact text
mod cli_linked {
    use super::badge;

    // manual: docs/manual/reference/cli.rst#1
    use std::process::ExitCode;

    fn main() -> ExitCode {
        match gd_plugins::Registry::new(vec![badge::plugin()]) {
            Ok(linked) => gd_cli::run_with(linked),
            Err(problem) => {
                eprintln!("{problem}");
                ExitCode::from(2)
            }
        }
    }
    // end manual
}

/// The discussion how-to's fragment: `dir` is the catalog's folder.
#[rustfmt::skip] // a twin keeps the manual's exact text
fn discussions_state_change(dir: &std::path::Path) -> Result<(), gd_discussion::Problem> {
    // manual: docs/manual/howto/discussions.rst#1
    use gd_discussion as ds;

    let prefix = ds::Prefix::new("eds")?;
    let lock = ds::lock_catalog(dir, &prefix)?;     // released on drop
    let catalog = ds::load_catalog(dir, &prefix)?;  // metadata only, never record text
    let plan = ds::state_plan(&catalog, &ds::Decision {
        identifier: "0003".into(),
        to: "accepted".into(),
        reason: "Converged".into(),
        ..Default::default()
    })?;                                            // changes nothing yet
    ds::apply(&lock, &plan)?;                       // journaled; ds::recover rolls back
    // end manual
    Ok(())
}

#[test]
#[rustfmt::skip] // a twin keeps the manual's exact text
fn the_engine_example_runs() {
    // manual: docs/manual/internals/engine.rst#1
    use guidedoc::{convert_text, Workspace};

    let registry = guidedoc_defaults::registry();   // the built-in readers and renderers
    let mut ws = Workspace::new();                  // reuse it: buffers are kept
    match convert_text(&mut ws, &registry, "commonmark", "html", "# Hello\n") {
        Ok(html) => println!("{html}"),             // borrows ws until the next convert
        Err(fail) => eprintln!("{fail}"),           // title, message, and hint
    }
    // end manual
}

/// The twins in this file: (`file#n`, code without the common indentation).
fn twins() -> Vec<(String, String)> {
    let source = include_str!("manual_code.rs");
    let mut out = Vec::new();
    let mut current: Option<(String, Vec<&str>)> = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("// manual: ") {
            current = Some((name.to_owned(), Vec::new()));
        } else if trimmed == "// end manual" {
            if let Some((name, lines)) = current.take() {
                out.push((name, dedent(&lines)));
            }
        } else if let Some((_, lines)) = current.as_mut() {
            lines.push(line);
        }
    }
    out
}

fn dedent(lines: &[&str]) -> String {
    let common = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    lines.iter().map(|l| format!("{}\n", l.get(common..).unwrap_or(""))).collect()
}

/// The manual's `.rst` files, below `docs/manual` (build output left out).
fn manual_files() -> Vec<std::path::PathBuf> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if path.is_dir() && !name.to_string_lossy().starts_with('_') {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rst") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repository_root().join("docs/manual"), &mut out);
    out.sort();
    out
}

#[test]
fn every_rust_block_of_the_manual_is_compiled_here() {
    let root = repository_root();
    let twins = twins();
    let mut problems = Vec::new();
    let mut seen = 0;
    for path in manual_files() {
        let rel = path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
        let text = std::fs::read_to_string(&path).unwrap();
        for (n, block) in rst_code_block_list(&text, "rust").into_iter().enumerate() {
            if block.options.iter().any(|o| o == ":class: sketch") {
                continue;
            }
            seen += 1;
            let name = format!("{rel}#{}", n + 1);
            match twins.iter().find(|(twin, _)| *twin == name) {
                None => problems.push(format!(
                    "{rel}:{}: no twin; copy the block into manual_code.rs between \
                     `// manual: {name}` and `// end manual`, or mark it `:class: sketch`",
                    block.line
                )),
                Some((_, code)) if code.trim_end() != block.code.trim_end() => {
                    problems.push(format!(
                        "{rel}:{}: the manual and its twin differ:\n{}\n---\n{code}",
                        block.line, block.code
                    ))
                }
                Some(_) => {}
            }
        }
    }
    assert!(seen >= 5, "found only {seen} Rust blocks in the manual: is the scan broken?");
    let documented = twins.len();
    assert_eq!(documented, seen, "twins without a block in the manual (renumbered?)");
    assert!(problems.is_empty(), "{}", problems.join("\n\n"));
}
