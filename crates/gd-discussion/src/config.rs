//! A catalog's process and its `discussion.toml`: identity, labels, record types, the
//! lifecycle workflow, and numbering.

use crate::metadata::is_valid_slug;
use crate::{DRAFT_NUMBER, MAX_NUMBER, Prefix, Problem, Result, TYPES, contains};
use gd_toml::{Table, Value};
use std::fmt::Write;

/// How promotion numbers records: at least `width` digits, zero-padded (4: `0007`), and
/// wider when a number needs it (10000 follows 9999); from `start`; and up to `limit`,
/// or [`MAX_NUMBER`] when there is none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Numbering {
    /// The fewest digits a number is written with, 1 to 9.
    pub width: u32,
    /// The first number promotion gives.
    pub start: u32,
    /// The last number promotion may give, when the catalog sets one.
    pub limit: Option<u32>,
}

impl Default for Numbering {
    /// GDS 0001's numbering: four digits from 1, without a limit.
    fn default() -> Numbering {
        Numbering { width: 4, start: 1, limit: None }
    }
}

/// One lifecycle state: the states the state command may move a record to from it,
/// whether moving into it needs evidence, and the memo status of a record in it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    /// The state's name, a lowercase slug such as `in-review`.
    pub name: String,
    /// The states a record in this one may move to.
    pub next: Vec<String>,
    /// Whether a move into this state needs `--evidence`.
    pub evidence: bool,
    /// The text of a record's optional status field in this state; may be empty.
    pub status: String,
}

/// How a catalog decides: its record types, its lifecycle states, the state of a new
/// draft, the state promotion opens it in, and its numbering. The default is the
/// process of GDS 0001.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    /// The kinds of record, such as `Standards Track`.
    pub types: Vec<String>,
    /// The lifecycle states, in their usual order.
    pub states: Vec<State>,
    /// The state of a new draft.
    pub draft: String,
    /// The state promotion moves a draft to, giving it its number.
    pub open: String,
    /// How promotion numbers records.
    pub numbering: Numbering,
}

impl Default for Process {
    /// GDS 0001's process: six states, three types, four-digit numbers from 1.
    fn default() -> Process {
        let state = |name: &str, next: &[&str], evidence: bool, status: &str| State {
            name: name.to_string(),
            next: next.iter().map(|s| s.to_string()).collect(),
            evidence,
            status: status.to_string(),
        };
        Process {
            types: TYPES.iter().map(|t| t.to_string()).collect(),
            states: vec![
                state("prediscussion", &["abandoned"], false, "Working draft"),
                state("discussion", &["accepted", "published", "abandoned"], false, OPEN),
                state("accepted", &["published", "committed", "abandoned"], false, ACCEPTED),
                state("published", &["committed", "abandoned"], false, "Adopted design"),
                state("committed", &[], true, "Implemented design"),
                state("abandoned", &[], false, "Closed proposal"),
            ],
            draft: "prediscussion".to_string(),
            open: "discussion".to_string(),
            numbering: Numbering::default(),
        }
    }
}

const OPEN: &str = "Open for discussion";
const ACCEPTED: &str = "Accepted direction";

impl Process {
    /// The state called `name`, borrowed, or `None`.
    pub fn state(&self, name: &str) -> Option<&State> {
        self.states.iter().find(|s| s.name == name)
    }

    /// The names of the states, in order, borrowed.
    pub fn state_names(&self) -> impl Iterator<Item = &str> {
        self.states.iter().map(|s| s.name.as_str())
    }

    /// `n` as this process writes numbers: zero-padded to the width (`0007`).
    pub fn format_number(&self, n: u32) -> String {
        format!("{n:0width$}", width = self.numbering.width.max(1) as usize)
    }

    /// The value of a permanent number written as this process writes it, or `None`: a
    /// draft, zero, more than nine digits, or a number with other padding. `start` and
    /// `limit` govern new numbers only, so changing them keeps every record valid.
    pub fn number_value(&self, number: &str) -> Option<u32> {
        if number.is_empty() || number.len() > 9 || !number.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let width = self.numbering.width.max(1) as usize;
        let canonical =
            number.len() == width || (number.len() > width && !number.starts_with('0'));
        let n: u32 = number.parse().ok()?;
        (canonical && n >= 1).then_some(n)
    }

    /// The highest number promotion may give.
    pub fn number_limit(&self) -> u32 {
        self.numbering.limit.unwrap_or(MAX_NUMBER)
    }

    /// Tells whether a record without a number may be in `state`: the draft state, or one
    /// a draft may move to without promotion.
    pub fn is_unnumbered(&self, state: &str) -> bool {
        state == self.draft || self.state(&self.draft).is_some_and(|d| contains(&d.next, state))
    }
}

/// A catalog's configuration: its prefix, display name, default author, the labels its
/// records may carry (empty: the prefix's default, see [`crate::Catalog::vocabulary`]),
/// and its process. A command run in or under the catalog reads it, so it needs neither
/// `--prefix` nor `--dir`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Definition {
    /// The catalog's prefix.
    pub prefix: Prefix,
    /// Its display name, such as `Enso Discussions`.
    pub name: String,
    /// The author of new records when the command line and Git name none; may be empty.
    pub author: String,
    /// The labels records may carry; empty for the prefix's default vocabulary.
    pub labels: Vec<String>,
    /// The catalog's process.
    pub process: Process,
}

/// The problem of a configuration that is not valid, at `line` of `source` when known.
pub(crate) fn config_problem(
    path: &str,
    source: Option<&str>,
    line: usize,
    message: impl Into<String>,
    hint: impl Into<String>,
) -> Problem {
    let p = Problem::new(
        "discussion.config",
        "The catalog configuration is not valid",
        path,
        message,
        hint,
    )
    .exit(2);
    match source {
        Some(source) if line > 0 => p.at_line(source, line),
        _ => p.with_line(line),
    }
}

/// Reads a `discussion.toml` whose text is `source`; `path` names it in problems.
/// Unknown settings, missing ones, and a process that does not hold together (a move to
/// a state that does not exist, a draft that could skip promotion) are problems that
/// name the line and the fix.
pub fn read_config(path: &str, source: &str) -> Result<Definition> {
    let root = gd_toml::parse(source).map_err(|e| {
        config_problem(
            path,
            Some(source),
            e.line,
            e.message.clone(),
            "Correct the TOML at that line; dsc init writes a complete example.",
        )
    })?;
    let r = Reader { path, source };
    r.known(&root, "", ROOT_KEYS)?;
    let prefix_text = r.text(&root, 0, "prefix", true, "")?;
    let prefix = Prefix::new(&prefix_text).map_err(|_| {
        r.fail(
            config_line(&root, &["prefix"]),
            format!(
                "prefix = {prefix_text:?} is not a prefix: a prefix is 2 to 16 lowercase \
                ASCII letters."
            ),
            "Write a short lowercase prefix, such as prefix = \"eds\".",
        )
    })?;
    let mut d = Definition {
        prefix,
        name: r.text(&root, 0, "name", true, "")?,
        author: r.text(&root, 0, "author", false, "")?,
        labels: r.list(&root, "labels", "")?.unwrap_or_default(),
        process: Process::default(),
    };
    if let Some(types) = r.list(&root, "types", "")? {
        d.process.types = types;
    }
    if let Some(v) = root.get("numbering") {
        r.numbering(v, &mut d.process.numbering)?;
    }
    if let Some(v) = root.get("workflow") {
        r.workflow(v, &mut d.process)?;
    }
    check_config(path, &d, Some((&root, source)))?;
    Ok(d)
}

const ROOT_KEYS: &[&str] =
    &["prefix", "name", "author", "labels", "types", "numbering", "workflow"];

/// Reads settings and reports the first mistake with its line.
struct Reader<'a> {
    path: &'a str,
    source: &'a str,
}

impl Reader<'_> {
    fn fail(&self, line: usize, message: impl Into<String>, hint: impl Into<String>) -> Problem {
        config_problem(self.path, Some(self.source), line, message, hint)
    }

    /// Refuses a setting the configuration does not have, so a misspelt one is reported
    /// rather than ignored.
    fn known(&self, table: &Table, prefix: &str, names: &[&str]) -> Result<()> {
        for (key, value) in table.iter() {
            if !names.contains(&key) {
                let hint = format!("Use one of: {}.", names.join(", "));
                let p = self.fail(value.line, format!("There is no setting {prefix}{key}."), hint);
                return Err(p.suggest(gd_report::suggest(key, names.iter().copied())));
            }
        }
        Ok(())
    }

    fn table<'t>(&self, v: &'t Value, name: &str) -> Result<&'t Table> {
        v.as_table().ok_or_else(|| {
            self.fail(
                v.line,
                format!("{name} must be a table."),
                format!("Write it as a [{name}] section."),
            )
        })
    }

    /// A one-line text setting; "" when it is missing and not required.
    fn text(
        &self,
        t: &Table,
        at: usize,
        key: &str,
        required: bool,
        prefix: &str,
    ) -> Result<String> {
        let Some(v) = t.get(key) else {
            if required {
                return Err(self.fail(
                    at,
                    format!("{prefix}{key} is missing."),
                    format!("Add {key} = \"...\" to the configuration."),
                ));
            }
            return Ok(String::new());
        };
        match v.as_str() {
            Some(text) if is_valid_name(text) => Ok(text.to_string()),
            _ => Err(self.fail(
                v.line,
                format!("{prefix}{key} must be one line of text."),
                format!("Write {key} = \"...\" in quotes."),
            )),
        }
    }

    fn list(&self, t: &Table, key: &str, prefix: &str) -> Result<Option<Vec<String>>> {
        let Some(v) = t.get(key) else { return Ok(None) };
        let Some(items) = v.as_array() else {
            return Err(self.fail(
                v.line,
                format!("{prefix}{key} must be a list."),
                format!("Write {key} = [\"a\", \"b\"]."),
            ));
        };
        let mut values = Vec::with_capacity(items.len());
        for item in items {
            match item.as_str() {
                Some(text) if is_valid_name(text) => values.push(text.to_string()),
                _ => {
                    return Err(self.fail(
                        item.line,
                        format!("{prefix}{key} must hold quoted one-line names."),
                        format!("Quote each item of {key}."),
                    ));
                }
            }
        }
        Ok(Some(values))
    }

    fn number(&self, t: &Table, name: &str, fallback: u32) -> Result<u32> {
        let Some(v) = t.get(name) else { return Ok(fallback) };
        match v.as_integer().and_then(|n| u32::try_from(n).ok()) {
            Some(n) if n <= MAX_NUMBER => Ok(n),
            _ => Err(self.fail(
                v.line,
                format!("numbering.{name} must be a whole number from 0 to {MAX_NUMBER}."),
                format!("Write {name} = {fallback}, without quotes."),
            )),
        }
    }

    fn boolean(&self, t: &Table, key: &str, prefix: &str) -> Result<bool> {
        let Some(v) = t.get(key) else { return Ok(false) };
        v.as_bool().ok_or_else(|| {
            self.fail(
                v.line,
                format!("{prefix}{key} must be true or false."),
                format!("Write {key} = true, without quotes."),
            )
        })
    }

    fn numbering(&self, v: &Value, n: &mut Numbering) -> Result<()> {
        let t = self.table(v, "numbering")?;
        self.known(t, "numbering.", &["width", "start", "limit"])?;
        n.width = self.number(t, "width", 4)?;
        n.start = self.number(t, "start", 1)?;
        n.limit = Some(self.number(t, "limit", 0)?).filter(|&l| l > 0);
        Ok(())
    }

    fn workflow(&self, v: &Value, p: &mut Process) -> Result<()> {
        let t = self.table(v, "workflow")?;
        self.known(t, "workflow.", &["draft", "open", "states"])?;
        p.draft = self.text(t, v.line, "draft", true, "workflow.")?;
        p.open = self.text(t, v.line, "open", true, "workflow.")?;
        let states = t.get("states").and_then(Value::as_table).filter(|s| !s.is_empty());
        let Some(states) = states else {
            return Err(self.fail(
                v.line,
                "[workflow] has no states.",
                "Add a [workflow.states.NAME] table for each state, as dsc config prints.",
            ));
        };
        p.states.clear();
        for (name, value) in states.iter() {
            let t = self.table(value, &format!("workflow.states.{name}"))?;
            let prefix = format!("workflow.states.{name}.");
            self.known(t, &prefix, &["next", "evidence", "status"])?;
            p.states.push(State {
                name: name.to_string(),
                next: self.list(t, "next", &prefix)?.unwrap_or_default(),
                evidence: self.boolean(t, "evidence", &prefix)?,
                status: self.text(t, value.line, "status", false, &prefix)?,
            });
        }
        Ok(())
    }
}

/// Tells whether `name` is a one-line name: not blank, at most 160 bytes, no controls.
pub(crate) fn is_valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= 160 && !name.chars().any(|c| c.is_control())
}

/// Tells whether `labels` can be a catalog's labels: at most 256 slugs.
pub(crate) fn is_valid_labels(labels: &[String]) -> bool {
    labels.len() <= 256 && labels.iter().all(|l| is_valid_slug(l))
}

/// The line where the setting at `parts` was written, or 0 for a configuration built in
/// code, so semantic problems point where syntax problems do.
fn config_line(root: &Table, parts: &[&str]) -> usize {
    let Some((first, rest)) = parts.split_first() else { return 0 };
    let mut v = root.get(first);
    for part in rest {
        v = v.and_then(|v| v.get(part));
    }
    v.map_or(0, |v| v.line)
}

/// Checks that a configuration holds together; `root` is its source, when it has one.
pub(crate) fn check_config(
    path: &str,
    d: &Definition,
    root: Option<(&Table, &str)>,
) -> Result<()> {
    let fail = |parts: &[&str], message: String, hint: &str| {
        let line = root.map_or(0, |(t, _)| config_line(t, parts));
        Err(config_problem(path, root.map(|(_, s)| s), line, message, hint))
    };
    if !is_valid_labels(&d.labels) {
        return fail(
            &["labels"],
            "labels must be lowercase words joined by hyphens.".into(),
            "Rename the labels, such as release-notes.",
        );
    }
    let p = &d.process;
    if p.types.is_empty() {
        return fail(
            &["types"],
            "types is empty.".into(),
            "List the kinds of record, such as types = [\"Standards Track\", \"Process\"].",
        );
    }
    let n = p.numbering;
    if !(1..=9).contains(&n.width) {
        return fail(
            &["numbering", "width"],
            format!("numbering.width is {}; numbers have 1 to 9 digits.", n.width),
            "Set width = 4 for numbers such as 0007.",
        );
    }
    if n.start < 1 || n.limit.is_some_and(|l| l < n.start) {
        let key = if n.start < 1 { "start" } else { "limit" };
        return fail(
            &["numbering", key],
            format!("numbering.start is {} and limit is {}.", n.start, n.limit.unwrap_or(0)),
            "Set the start setting to 1 or more, and the limit setting above it, or leave \
             it out.",
        );
    }
    check_workflow(p, &fail)
}

type Fail<'a> = dyn Fn(&[&str], String, &str) -> Result<()> + 'a;

fn check_workflow(p: &Process, fail: &Fail<'_>) -> Result<()> {
    for s in &p.states {
        if !is_valid_slug(&s.name) {
            return fail(
                &["workflow", "states", &s.name],
                format!("The state {:?} is not a lowercase name.", s.name),
                "Name states with lowercase letters and hyphens, such as in-review.",
            );
        }
        for to in &s.next {
            if p.state(to).is_none() || *to == s.name {
                return fail(
                    &["workflow", "states", &s.name, "next"],
                    format!(
                        "workflow.states.{}.next names {to:?}, which is not another \
                        state.",
                        s.name
                    ),
                    "List only states defined under [workflow.states].",
                );
            }
        }
    }
    for (key, name) in [("draft", &p.draft), ("open", &p.open)] {
        if p.state(name).is_none() {
            return fail(
                &["workflow", key],
                format!("The workflow names {name:?}, which is not a state."),
                "Set workflow.draft and workflow.open to states defined under \
                 [workflow.states].",
            );
        }
    }
    if p.draft == p.open {
        return fail(
            &["workflow", "open"],
            "workflow.draft and workflow.open are the same state.".into(),
            "Give promotion a state to open drafts in, such as open = \"discussion\".",
        );
    }
    if p.state(&p.draft).is_some_and(|d| contains(&d.next, &p.open)) {
        return fail(
            &["workflow", "states", &p.draft, "next"],
            format!("workflow.states.{}.next lists {}, the open state.", p.draft, p.open),
            "Remove it: the promote command opens a draft, and gives it its number.",
        );
    }
    Ok(())
}

/// Writes `d` as a `discussion.toml`: every setting, with a comment on each, so a
/// catalog's process can be read and changed in one place. `dsc config` prints it and
/// `init` writes it; [`read_config`] reads it back as `d`.
pub fn render_config(d: &Definition) -> String {
    let mut out = String::with_capacity(2048);
    out.push_str(
        "# A discussion catalog: its identity, the process its records follow, and \
        how\n# they are numbered. dsc reads it from the catalog folder.\n\n",
    );
    let _ = writeln!(
        out,
        "prefix = {}\nname = {}",
        toml_quote(d.prefix.as_str()),
        toml_quote(&d.name)
    );
    if !d.author.is_empty() {
        let _ = writeln!(out, "author = {}", toml_quote(&d.author));
    }
    out.push_str("\n# The labels records may carry; leave labels out to allow any.\n");
    if d.labels.is_empty() {
        out.push_str("# labels = [\"architecture\", \"process\"]\n");
    } else {
        let _ = writeln!(out, "labels = {}", toml_list(&d.labels));
    }
    let p = &d.process;
    let _ = writeln!(out, "# The kinds of record.\ntypes = {}", toml_list(&p.types));
    let n = p.numbering;
    let _ = write!(
        out,
        "\n# Numbers have at least width digits, zero-padded ({}); a number \
        that needs\n# more gets them. Promotion numbers from start, up to limit.\n\
        [numbering]\nwidth = {}\nstart = {}\n",
        p.format_number(7),
        n.width,
        n.start
    );
    match n.limit {
        Some(limit) => _ = writeln!(out, "limit = {limit}"),
        None => out.push_str("# limit = 99999\n"),
    }
    let _ = write!(
        out,
        "\n# A new draft is in the draft state, numbered {DRAFT_NUMBER}. \
        Promotion gives it a number\n# and moves it to the open state. The state command \
        moves a record to one of\n# its state's next states, with --reason, and --evidence \
        where the state asks for\n# it. status is the text a record's optional status field \
        takes.\n[workflow]\ndraft = {}\nopen = {}\n",
        toml_quote(&p.draft),
        toml_quote(&p.open)
    );
    for s in &p.states {
        let _ = writeln!(out, "\n[workflow.states.{}]", s.name);
        if !s.next.is_empty() {
            let _ = writeln!(out, "next = {}", toml_list(&s.next));
        }
        if s.evidence {
            out.push_str("evidence = true\n");
        }
        if !s.status.is_empty() {
            let _ = writeln!(out, "status = {}", toml_quote(&s.status));
        }
    }
    out
}

/// `value` as a TOML basic string.
pub(crate) fn toml_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    gd_toml::write_string(&mut out, value);
    out
}

/// `values` as a TOML array, on one line, or one item a line when long.
fn toml_list(values: &[String]) -> String {
    let width: usize = values.iter().map(|v| v.len() + 4).sum();
    let items = values.iter().map(|v| toml_quote(v));
    if width <= 72 {
        format!("[{}]", items.collect::<Vec<_>>().join(", "))
    } else {
        let mut out = String::from("[\n");
        for item in items {
            let _ = writeln!(out, "    {item},");
        }
        out.push(']');
        out
    }
}
