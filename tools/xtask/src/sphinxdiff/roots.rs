//! `cargo xtask sphinxdiff-roots`: builds Sphinx's own test roots (those whose features
//! Guidedog has natively) with a pinned sphinx-build and with Guidedog, compares each
//! pair with [`compare`](super::compare), and writes a record of the result.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{Comparison, Logs, Options, compare};
use crate::args::{self, Spec};
use crate::fsutil::{self, absolute, copy_tree, create_dir, io_failure, remove, write};
use crate::{Result, evidence, fail, process, repository_root, site};

const HELP: &str = "\
Builds 42 of Sphinx's test roots with a pinned sphinx-build and with Guidedog, compares
each pair with `cargo xtask sphinxdiff`, and writes a record of the counts.

Usage: cargo xtask sphinxdiff-roots SPHINX_BUILD ROOTS_DIR WORK_DIR [RECORD]
                                    [--guidedog PATH]

  SPHINX_BUILD     a pinned sphinx-build, in a separate verification environment
  ROOTS_DIR        the tests/roots folder of the same Sphinx's source distribution
  WORK_DIR         where each root's copy, both outputs, both logs and diff.txt go
  RECORD           the record to write (default: print it)
  --guidedog PATH  the guidedog to build with (or set GUIDEDOG); without one, the task
                   builds target/release/guidedog with Cargo

GUIDEDOG_REVISION names the revision when the tree is not a git checkout. Example:

  cargo xtask sphinxdiff-roots ~/verify-venv/bin/sphinx-build \\
      ~/sphinx-9.1.0/tests/roots /tmp/sd tests/integration/data/sphinxdiff/results.txt
";

/// The roots compared, with `-D` settings after `@`. test-numfig is compared twice: as
/// Sphinx's tests build it by default (numfig off) and with numfig on, as the numfig
/// tests of test_build_html do.
const ROOTS: [&str; 43] = [
    "basic",
    "toctree",
    "toctree-glob",
    "toctree-maxdepth",
    "toctree-duplicated",
    "toctree-index",
    "tocdepth",
    "numfig",
    "numfig@numfig=1",
    "footnotes",
    "glossary",
    "productionlist",
    "directive-only",
    "domain-py",
    "domain-c",
    "domain-cpp",
    "domain-js",
    "ext-autosectionlabel",
    "ext-todo",
    "ext-math",
    "ext-extlinks-hardcoded-urls",
    "index_on_title",
    "markup-citation",
    "markup-rubric",
    "images",
    "default_role",
    "directive-code",
    "directive-include",
    "metadata",
    "prolog",
    "smartquotes",
    "roles-download",
    "numbered-circular",
    "object-description-sections",
    "reST-code-block",
    "reST-code-role",
    "nested-tables",
    "refonly_bullet_list",
    "html_entity",
    "keep_warnings",
    "ext-ifconfig",
    "toctree-domain-objects",
    "search",
];

/// What `sphinxdiff-roots` builds and compares.
#[derive(Debug, Clone, Default)]
pub(crate) struct RootsOptions {
    /// The pinned sphinx-build.
    pub sphinx_build: PathBuf,
    /// Sphinx's tests/roots folder.
    pub roots: PathBuf,
    /// Where each root's builds and comparison go.
    pub work: PathBuf,
    /// The record to write; `None` prints it.
    pub record: Option<PathBuf>,
    /// The guidedog to build with; `None` builds it with Cargo.
    pub guidedog: Option<PathBuf>,
}

/// Runs `cargo xtask sphinxdiff-roots`.
pub(crate) fn main(args: &[String]) -> Result<()> {
    let spec =
        Spec { command: "sphinxdiff-roots", flags: &[], options: &["--guidedog"], help: HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(4, &spec)?;
    let path = |i: usize, what: &str| -> Result<PathBuf> {
        Ok(absolute(Path::new(parsed.positional(i, what, &spec)?)))
    };
    let guidedog = parsed.option("--guidedog").map(String::from);
    let guidedog = guidedog.or_else(|| std::env::var("GUIDEDOG").ok()).filter(|g| !g.is_empty());
    let options = RootsOptions {
        sphinx_build: site::program_path(parsed.positional(0, "a sphinx-build", &spec)?),
        roots: path(1, "Sphinx's tests/roots folder")?,
        work: path(2, "a work folder")?,
        record: parsed.positionals.get(3).map(|r| absolute(Path::new(r))),
        guidedog: guidedog.map(|g| site::program_path(&g)),
    };
    compare_roots(&options)
}

/// Builds and compares every root as [`RootsOptions`] says, then writes the record.
/// Fails when a program cannot start or a file cannot be written; a build that fails is
/// part of the record, not a failure of the task.
pub(crate) fn compare_roots(options: &RootsOptions) -> Result<()> {
    let root = repository_root();
    let guidedog = match &options.guidedog {
        Some(guidedog) => guidedog.clone(),
        None => site::build_guidedog(&root, std::time::Instant::now())?,
    };
    if !fsutil::is_executable(&options.sphinx_build) {
        return Err(fail(
            "sphinxdiff-sphinx",
            "NO SPHINX-BUILD",
            &format!("{} is not an executable file.", options.sphinx_build.display()),
            "Install the pinned Sphinx in a virtual environment (pip install sphinx==9.1.0) \
             and pass its bin/sphinx-build.",
        ));
    }
    create_dir(&options.work)?;
    let mut lines = Vec::with_capacity(ROOTS.len());
    for spec in ROOTS {
        let line = compare_root(options, &guidedog, spec)?;
        println!("{line}");
        lines.push(line);
    }
    let record = record(options, &root, &lines);
    match &options.record {
        Some(path) => write(path, record),
        None => {
            print!("{record}");
            Ok(())
        }
    }
}

/// One root, named like `test-numfig@numfig=1`, and its `-D` arguments.
struct Root {
    name: String,
    source: PathBuf,
    defines: Vec<String>,
}

fn root(roots: &Path, spec: &str) -> Root {
    let (folder, settings) = spec.split_once('@').unwrap_or((spec, ""));
    let mut name = format!("test-{folder}");
    let source = roots.join(&name);
    let mut defines = Vec::new();
    if !settings.is_empty() {
        name = format!("{name}@{settings}");
        for setting in settings.split(',') {
            defines.push(String::from("-D"));
            defines.push(setting.to_string());
        }
    }
    Root { name, source, defines }
}

/// Builds one root with both builders in WORK_DIR/NAME and compares them; returns the
/// record's line, `NAME: counts`.
fn compare_root(options: &RootsOptions, guidedog: &Path, spec: &str) -> Result<String> {
    let root = root(&options.roots, spec);
    let out = options.work.join(&root.name);
    remove(&out)?;
    copy_tree(&root.source, &out.join("src"))?;
    let mut status = String::new();
    let mut sphinx = Command::new(&options.sphinx_build);
    sphinx.args(["-q", "-b", "html", "-D", "html_theme=basic"]).args(&root.defines);
    sphinx.arg(out.join("src")).arg(out.join("sphinx"));
    let code = run_logged(&mut sphinx, None, &out.join("sphinx.log"))?;
    status.push_str(&format!("sphinx-build exit {code}\n"));
    // The project's conf.py becomes the conf.toml Guidedog reads; a root without one
    // fails to migrate, and Guidedog's build then says what it lacks.
    let mut migrate = Command::new(guidedog);
    migrate.current_dir(out.join("src")).args(["migrate", "conf.py", "-o", "conf.toml"]);
    migrate.arg("--force").stdout(Stdio::null()).stderr(Stdio::null());
    let _ = process::capture(&mut migrate)?;
    let mut build = Command::new(guidedog);
    build.args(["build", "-b", "html"]).args(&root.defines);
    build.arg(out.join("src")).arg(out.join("guidedog"));
    build.args(["--color=never", "--diagnostics=json"]);
    let code = run_logged(&mut build, Some(&out.join("guidedog.out")), &out.join("guidedog.log"))?;
    status.push_str(&format!("guidedog exit {code}\n"));
    write(&out.join("status.txt"), status)?;
    let logs = Logs {
        sphinx: out.join("sphinx.log"),
        guidedog: out.join("guidedog.log"),
        source: out.join("src"),
    };
    let compared = Options {
        sphinx: out.join("sphinx"),
        guidedog: out.join("guidedog"),
        verbose: true,
        logs: Some(logs),
    };
    let comparison: Comparison = compare(&compared)?;
    write(&out.join("diff.txt"), &comparison.report)?;
    Ok(format!("{}: {}", root.name, comparison.summary()))
}

/// Runs a command with its standard error (and its output, when `stdout` is given)
/// written to files; returns its exit status as text (`1`, or `signal`).
fn run_logged(command: &mut Command, stdout: Option<&Path>, stderr: &Path) -> Result<String> {
    let create = |path: &Path| File::create(path).map_err(|err| io_failure("create", path, &err));
    command.stdin(Stdio::null()).stderr(create(stderr)?);
    match stdout {
        Some(path) => command.stdout(create(path)?),
        None => command.stdout(Stdio::null()),
    };
    let status = command.status().map_err(|err| process::spawn_failure(command, &err))?;
    Ok(status.code().map_or_else(|| String::from("signal"), |code| code.to_string()))
}

/// The record: what was compared, where, when and with what, then one line per root.
fn record(options: &RootsOptions, root: &Path, lines: &[String]) -> String {
    let mut text = String::from(
        "# Guidedog compared with Sphinx over Sphinx's test roots, written by\n\
         # cargo xtask sphinxdiff-roots. One line per root: the root (with -D settings\n\
         # after @), then sphinxdiff's counts as name=compared/differing. See\n\
         # docs/manual/internals/sphinx-comparison.rst.\n",
    );
    text.push_str(&format!("date: {}\n", evidence::utc_timestamp()));
    text.push_str(&format!("host: {}\n", evidence::host()));
    text.push_str(&format!("sphinx: {}\n", evidence::version(&options.sphinx_build)));
    text.push_str(&format!("guidedog revision: {}\n", evidence::revision(root)));
    text.push_str(&format!(
        "command: cargo xtask sphinxdiff-roots {} {} {}\n",
        options.sphinx_build.display(),
        options.roots.display(),
        options.work.display()
    ));
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    text
}
