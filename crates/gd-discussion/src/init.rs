//! Making a new catalog: configuration, Guidedog project, index, workflow, record
//! template, and a process draft that describes the configured lifecycle.

use crate::catalog::{Catalog, check_sources, load_catalog};
use crate::config::{Definition, Process, check_config, is_valid_labels, is_valid_name};
use crate::config::{render_config, toml_quote};
use crate::edit::fill_draft;
use crate::index::{check_index, plan_index};
use crate::metadata::{FieldValue, read_record};
use crate::problem::{io_problem, shown};
use crate::transaction::{Change, Plan, replace_file, unique_name};
use crate::{CONFIG_FILE, DRAFT_NUMBER, Prefix, Problem, Result, contains, replace_field, rst};
use std::fmt::Write;
use std::path::{Path, PathBuf};

const TEMPLATE: &str = include_str!("../assets/template.rst");
const PROCESS: &str = include_str!("../assets/process.rst");
const WORKFLOW: &str = include_str!("../assets/workflow.rst");
const INDEX: &str = include_str!("../assets/index.rst");
const PROJECT: &str = include_str!("../assets/conf.toml");
const README: &str = include_str!("../assets/README.md");
const IGNORE: &str = include_str!("../assets/gitignore");

/// What a new catalog is made from: where, its identity, and its process.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Init {
    /// The catalog's folder, which must be missing or empty.
    pub dir: PathBuf,
    /// The catalog's prefix.
    pub prefix: Prefix,
    /// The display name, such as `Example Discussions`.
    pub name: String,
    /// The default author, also the process draft's author.
    pub author: String,
    /// The labels records may carry, or `None` for any well-formed label. An empty list
    /// is a problem: the process draft needs one.
    pub labels: Option<Vec<String>>,
    /// The process draft's text from its title on; empty for the built-in one.
    pub draft: String,
    /// The record template, with metadata fields in the prefix's name; empty for the
    /// built-in one.
    pub template: String,
    /// The catalog's workflow, types, and numbering.
    pub process: Process,
}

/// Plans a new catalog in `init.dir`, which must be missing or empty: its
/// configuration, Guidedog project, index, workflow, record template, and process draft.
/// It changes nothing; [`initialize`] makes the catalog.
pub fn init_plan(init: &Init) -> Result<Plan> {
    let labels = init.labels.clone().unwrap_or_default();
    if !is_valid_name(&init.name) || !is_valid_name(&init.author) || !is_valid_labels(&labels) {
        return Err(Problem::new(
            "discussion.init",
            "Invalid catalog identity",
            shown(&init.dir),
            "The name and author must be single lines of text, and labels lowercase words \
             with hyphens.",
            "Pass --name, --author, and --labels with plain values.",
        )
        .exit(2));
    }
    let d = Definition {
        prefix: init.prefix.clone(),
        name: init.name.clone(),
        author: init.author.clone(),
        labels,
        process: init.process.clone(),
    };
    check_config(CONFIG_FILE, &d, None)?;
    empty_destination(&init.dir)?;
    let template =
        if init.template.is_empty() { built_in_template(init) } else { init.template.clone() };
    let process = process_record(init, &template)?;
    let path = format!("records/{DRAFT_NUMBER}-{}-process.rst", init.prefix);
    let c = Catalog {
        dir: init.dir.clone(),
        prefix: init.prefix.clone(),
        labels: d.labels.clone(),
        process: d.process.clone(),
        ..Catalog::default()
    };
    let r = read_record(&c, &init.dir.join(&path), &process)?;
    let mut plan = plan_index(&c, &[&r])?;
    let files = [
        (CONFIG_FILE, render_config(&d)),
        ("conf.toml", fill(PROJECT, init)),
        ("index.rst", fill(INDEX, init)),
        ("workflow.rst", fill(WORKFLOW, init)),
        ("README.md", fill(README, init)),
        (".gitignore", fill(IGNORE, init)),
        ("template/rfc-template.rst", template),
        ("_templates/discussion.typ", gd_theme::DISCUSSION_TYP.to_string()),
        ("_templates/theme.typ", gd_theme::PDF_THEME_TYP.to_string()),
        (path.as_str(), process),
    ];
    for (name, text) in files {
        plan.changes.push(Change { path: name.to_string(), before: None, after: Some(text) });
    }
    plan.summary = format!("Create {} ({}) in {}", init.name, init.prefix, shown(&init.dir));
    Ok(plan)
}

/// Makes the catalog [`init_plan`] plans. It writes it whole into a folder beside
/// `init.dir`, checks it as the catalog commands would, and only then moves it into
/// place: a failure leaves nothing behind, and an existing file or a folder with
/// contents is never changed.
pub fn initialize(init: &Init) -> Result<()> {
    let plan = init_plan(init)?;
    let ns = init.prefix.as_str();
    let parent = match init.dir.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    std::fs::create_dir_all(parent).map_err(|e| io_problem(ns, parent, &e))?;
    let stage = StageDir(parent.join(unique_name(".discussion-init-")));
    std::fs::create_dir(&stage.0).map_err(|e| io_problem(ns, &stage.0, &e))?;
    for c in &plan.changes {
        let path = stage.0.join(&c.path);
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|e| io_problem(ns, folder, &e))?;
        }
        replace_file(ns, &path, c.after.as_deref().unwrap_or(""))?;
    }
    let c = load_catalog(&stage.0, &init.prefix)?;
    check_sources(&c)?;
    check_index(&c)?;
    empty_destination(&init.dir)?;
    if init.dir.is_dir() {
        std::fs::remove_dir(&init.dir).map_err(|e| io_problem(ns, &init.dir, &e))?;
    }
    std::fs::rename(&stage.0, &init.dir).map_err(|e| io_problem(ns, &init.dir, &e))
}

/// A staging folder, removed with its contents when dropped unless it was moved.
struct StageDir(PathBuf);

impl Drop for StageDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Puts the catalog's identity into a built-in text.
fn fill(text: &str, i: &Init) -> String {
    let year = gd_clock::build_time().local().date.year;
    let name_width = i.name.len();
    let pairs = [
        ("@NAME_TOML@", toml_quote(&i.name)),
        ("@AUTHOR_TOML@", toml_quote(&i.author)),
        ("@COPYRIGHT_TOML@", toml_quote(&format!("{year}, {}", i.author))),
        ("@PROCESS_UNDERLINE@", "=".repeat(name_width + " Process".len())),
        ("@PREFIX@", i.prefix.to_string()),
        ("@NAME@", i.name.clone()),
        ("@UNDERLINE@", "=".repeat(name_width)),
    ];
    let mut out = text.to_string();
    for (key, value) in pairs {
        out = out.replace(key, &value);
    }
    out
}

/// The process draft: the template's metadata, filled, and the process text.
fn process_record(i: &Init, template: &str) -> Result<String> {
    let process = &i.process;
    let mut labels = vec!["process".to_string()];
    if let Some(given) = &i.labels {
        let Some(first) = given.first() else {
            return Err(Problem::new(
                "discussion.init",
                "No labels to choose from",
                shown(&i.dir),
                "The catalog's labels list is empty.",
                "Pass --labels with at least one label, such as --labels=process.",
            )
            .exit(2));
        };
        if !contains(given, "process") {
            labels = vec![first.clone()];
        }
    }
    let title = format!("{} Process", i.name);
    let block = rst::metadata_block(template, &i.prefix);
    let metadata = fill_draft(block, &title, &i.author, &labels, &i.prefix, process)?;
    let kind = if contains(&process.types, "Process") { "Process" } else { &process.types[0] };
    let metadata = replace_field(&metadata, &i.prefix, "type", FieldValue::Text(kind))?;
    let body = if i.draft.is_empty() {
        fill(PROCESS, i).replace("@LIFECYCLE@", &lifecycle_text(process))
    } else {
        i.draft.clone()
    };
    Ok(format!(
        "{}\n\n{}",
        metadata.trim_end_matches(['\r', '\n']),
        body.trim_start_matches(['\r', '\n'])
    ))
}

/// Refuses a destination that exists and is not an empty folder.
fn empty_destination(dir: &Path) -> Result<()> {
    match std::fs::symlink_metadata(dir) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(io_problem("discussion", dir, &e)),
        Ok(info) if info.is_dir() => {
            let mut entries =
                std::fs::read_dir(dir).map_err(|e| io_problem("discussion", dir, &e))?;
            if entries.next().is_none() {
                return Ok(());
            }
        }
        Ok(_) => {}
    }
    Err(Problem::new(
        "discussion.init-exists",
        "The folder is not empty",
        shown(dir),
        "A new catalog is made only in a missing or empty folder; nothing was changed.",
        "Choose a new --dir, or use the new and index commands in an existing catalog.",
    )
    .exit(2))
}

/// The built-in record template, with a type the catalog allows.
fn built_in_template(i: &Init) -> String {
    let template = fill(TEMPLATE, i);
    let types = &i.process.types;
    match types.first() {
        Some(first) if !contains(types, "Standards Track") => {
            replace_field(&template, &i.prefix, "type", FieldValue::Text(first))
                .unwrap_or(template)
        }
        _ => template,
    }
}

/// Describes a workflow and its numbering in reStructuredText: how a draft is numbered,
/// a table of the states and their moves, and a diagram of them.
fn lifecycle_text(p: &Process) -> String {
    let mut out = String::with_capacity(2048);
    let n = p.numbering;
    let _ = write!(
        out,
        "A draft starts with the number ``{DRAFT_NUMBER}`` and the state \
        ``{}``.\nPromotion gives it the next number, from {}, with at least {} digits ({}), \
        and\nmoves it to ``{}``. A state change needs a reason.\n\n",
        p.draft,
        n.start,
        n.width,
        p.format_number(n.start.max(7)),
        p.open
    );
    out.push_str(
        ".. list-table:: The states and the moves between them\n   :header-rows: 1\n   \
        :widths: 30 70\n\n   * - State\n     - May move to\n",
    );
    for s in &p.states {
        let mut moves = match s.next.is_empty() {
            true => "none: the record is closed".to_string(),
            false => s.next.join(", "),
        };
        if s.name == p.draft {
            moves = format!("{} (by promotion), {moves}", p.open);
        }
        if s.evidence {
            moves.push_str("; moving here needs evidence");
        }
        let _ = write!(out, "   * - ``{}``\n     - {moves}\n", s.name);
    }
    out.push_str(
        "\n.. graphviz::\n   :caption: The moves this catalog allows.\n   :alt: The \
        lifecycle states of a record and the moves between them.\n\n   digraph lifecycle {\n     \
        graph [rankdir=TB, bgcolor=\"transparent\"];\n     node [shape=box, \
        fontname=\"Helvetica\"];\n",
    );
    let open_evidence = p.state(&p.open).is_some_and(|s| s.evidence);
    let promotion = if open_evidence { "promote, evidence" } else { "promote" };
    let _ = writeln!(out, "     \"{}\" -> \"{}\" [label=\"{promotion}\"];", p.draft, p.open);
    for s in &p.states {
        for to in &s.next {
            let evidence = p.state(to).is_some_and(|t| t.evidence);
            let label = if evidence { " [label=\"evidence\"]" } else { "" };
            let _ = writeln!(out, "     \"{}\" -> \"{to}\"{label};", s.name);
        }
    }
    out.push_str("   }\n");
    out
}
