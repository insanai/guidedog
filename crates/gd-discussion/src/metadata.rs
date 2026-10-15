//! A record's metadata: reading it from a record's leading fields, checking it against
//! the catalog's process, and changing one field while keeping every other byte.

use crate::catalog::Catalog;
use crate::problem::shown;
use crate::scan::{MAX_HEADER_BYTES, MAX_LINE, metadata_line, metadata_size_problem};
use crate::{CONFIG_FILE, DRAFT_NUMBER, MAX_RECORD_BYTES, Prefix, Problem, Result, Process};
use crate::{contains, rst, too_large, typst};
use std::path::{Path, PathBuf};

/// The fields every record has.
pub(crate) const REQUIRED: [&str; 9] =
    ["number", "title", "state", "type", "authors", "created", "updated", "discussion", "labels"];
/// The memo fields a record may add, as the Sanin discussions do.
pub(crate) const OPTIONAL: [&str; 3] = ["category", "status", "summary"];

/// Which of the sections every record needs a record has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sections {
    /// An `Abstract` section.
    pub has_abstract: bool,
    /// A `References` (or `Sources and design provenance`) section.
    pub has_references: bool,
}

/// One discussion: its metadata as the record writes it, its file, and which required
/// sections it has. A catalog keeps records without their text; a command that changes
/// a record reads it whole.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Record {
    /// The permanent number as written (`0007`), or [`DRAFT_NUMBER`].
    pub number: String,
    /// The slug, from the file name after the number.
    pub slug: String,
    /// The title.
    pub title: String,
    /// The lifecycle state.
    pub state: String,
    /// The record type, such as `Standards Track`.
    pub kind: String,
    /// The date the record was created, `YYYY-MM-DD`.
    pub created: String,
    /// The date the record last changed, `YYYY-MM-DD`.
    pub updated: String,
    /// Where it is discussed.
    pub discussion: String,
    /// The optional memo category; may be empty.
    pub category: String,
    /// The optional memo status, which follows the state; may be empty.
    pub status: String,
    /// The optional one-line summary; may be empty.
    pub summary: String,
    /// The authors, at least one.
    pub authors: Vec<String>,
    /// The labels, at least one.
    pub labels: Vec<String>,
    /// The record's file.
    pub path: PathBuf,
    /// The required sections it has.
    pub sections: Sections,
    /// An inherited record of another process, kept as provenance and never changed.
    pub historical: bool,
}

impl Record {
    /// Tells whether the record is a draft, numbered [`DRAFT_NUMBER`].
    pub fn is_draft(&self) -> bool {
        self.number == DRAFT_NUMBER
    }

    /// The record's file name, borrowed: `0007-cache-policy.rst`.
    pub fn file_name(&self) -> &str {
        self.path.file_name().and_then(|n| n.to_str()).unwrap_or("")
    }

    /// The file name without its extension, borrowed: `0007-cache-policy`.
    pub fn stem(&self) -> &str {
        self.path.file_stem().and_then(|n| n.to_str()).unwrap_or("")
    }
}

/// A metadata value to write: one line of text, or a list (authors and labels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldValue<'a> {
    /// A text field, such as a state or a date.
    Text(&'a str),
    /// A list field: authors or labels.
    List(&'a [String]),
}

/// Reads the record at `path`, whose text is `source`, as the catalog `catalog` reads its
/// records: the metadata is checked against its prefix, labels, and process, and the
/// sections are found in `source`. A missing, repeated, unknown, or invalid field, a bad
/// slug, or dates out of order is a problem.
pub fn read_record(catalog: &Catalog, path: &Path, source: &str) -> Result<Record> {
    let mut r = parse_metadata(catalog, path, source)?;
    r.sections = sections_of(path, source);
    Ok(r)
}

/// Reads and checks the metadata at the start of `source`, without its sections.
pub(crate) fn parse_metadata(catalog: &Catalog, path: &Path, source: &str) -> Result<Record> {
    let prefix = &catalog.prefix;
    check_header_size(prefix, path, source)?;
    let r = if is_rst(path) {
        rst::parse_record(path, source, prefix)?
    } else {
        typst::parse_record(path, source, prefix)?
    };
    validate_record(&r, catalog)?;
    Ok(r)
}

/// Tells whether `path` is a reStructuredText record (else an older Typst one).
pub(crate) fn is_rst(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "rst")
}

/// The required sections `source` has.
pub(crate) fn sections_of(path: &Path, source: &str) -> Sections {
    if is_rst(path) {
        Sections {
            has_abstract: rst::has_section(source, "Abstract"),
            has_references: rst::has_section(source, "References")
                || rst::has_section(source, "Sources and design provenance"),
        }
    } else {
        Sections {
            has_abstract: source.contains("= Abstract"),
            has_references: source.contains("= References")
                || source.contains("= Sources and design provenance"),
        }
    }
}

/// Returns `source` with the metadata field `key` set to `value`. An RST record keeps
/// its own notation (plain text, JSON arrays for lists); an older Typst record gets
/// string literals and tuples. Only the field's line changes: every other byte, line
/// endings included, stays. A field the record does not have is `PREFIX.field`.
pub fn replace_field(source: &str, prefix: &Prefix, key: &str, value: FieldValue<'_>)
    -> Result<String> {
    let rst = source.trim_start().starts_with(&format!(":{prefix}-"));
    let Some((start, end)) = field_line(source, prefix, key, rst) else {
        return Err(Problem::new(prefix.code("field"), "The record has no such field", key,
            format!("No {prefix}-{key} field starts the record."),
            format!("Add the {prefix}-{key} field from the record template."))
        .exit(2));
    };
    let line = if rst {
        let trimmed = source[start..end].trim();
        let name = trimmed[1..].split(':').next().unwrap_or(key);
        format!(":{name}: {}", rst::field_text(value))
    } else {
        format!("#let {prefix}-{key} = {}", typst::field_text(value))
    };
    let mut out = String::with_capacity(source.len() + line.len());
    out.push_str(&source[..start]);
    out.push_str(&line);
    out.push_str(&source[end..]);
    Ok(out)
}

/// Finds the line of the field `key` in the metadata at the start of `source`: its byte
/// range without the line break. `updated` is also found as `last-updated`.
pub(crate) fn field_line(source: &str, prefix: &Prefix, key: &str, rst: bool)
    -> Option<(usize, usize)> {
    let marker = if rst { format!(":{prefix}-") } else { format!("#let {prefix}-") };
    let mut start = 0;
    while start < source.len() {
        let end = source[start..].find('\n').map_or(source.len(), |i| start + i);
        let line = source[start..end].strip_suffix('\r').unwrap_or(&source[start..end]);
        let trimmed = line.trim();
        if rst && !trimmed.is_empty() && !trimmed.starts_with(&marker) {
            return None;
        }
        if !rst && trimmed.starts_with("#show:") {
            return None;
        }
        if let Some(rest) = trimmed.strip_prefix(&marker) {
            let separator = if rst { ':' } else { '=' };
            let name = rest.split(separator).next().unwrap_or("").trim();
            if name == key || (rst && key == "updated" && name == "last-updated") {
                return Some((start, start + line.len()));
            }
        }
        start = end + 1;
    }
    None
}

/// Sets the record's field `key` to `value`.
pub(crate) fn set_value(r: &mut Record, key: &str, value: String) {
    match key {
        "number" => r.number = value,
        "title" => r.title = value,
        "state" => r.state = value,
        "type" => r.kind = value,
        "created" => r.created = value,
        "updated" => r.updated = value,
        "discussion" => r.discussion = value,
        "category" => r.category = value,
        "status" => r.status = value,
        "summary" => r.summary = value,
        _ => {}
    }
}

/// Reads the slug from the file name, which starts with the record's number.
pub(crate) fn take_slug(r: &mut Record, prefix: &Prefix) -> Result<()> {
    let stem = r.stem().to_string();
    match stem.strip_prefix(&format!("{}-", r.number)) {
        Some(slug) => {
            r.slug = slug.to_string();
            Ok(())
        }
        None => Err(metadata_problem(prefix, &r.path, "number")),
    }
}

/// The problem of a missing, repeated, or unreadable field.
pub(crate) fn metadata_problem(prefix: &Prefix, path: &Path, key: &str) -> Problem {
    Problem::new(prefix.code("metadata"), "Invalid discussion metadata", shown(path),
        format!("The {prefix}-{key} field is missing, repeated, or not a literal value."),
        format!("Put one :{prefix}-name: field per line at the start of the record. Use \
            plain text for values and JSON string arrays for authors and labels."))
}

/// Refuses metadata longer than 64 KiB, or with a line longer than 4 KiB, as load does.
fn check_header_size(prefix: &Prefix, path: &Path, source: &str) -> Result<()> {
    if source.len() as u64 > MAX_RECORD_BYTES {
        return Err(too_large(prefix.as_str(), path, "A record may be at most 16 MiB."));
    }
    let rst = is_rst(path);
    let mut size = 0;
    for raw in source.lines() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if !metadata_line(prefix, rst, line) {
            break;
        }
        size += line.len() + 1;
        if line.len() > MAX_LINE || size > MAX_HEADER_BYTES {
            return Err(metadata_size_problem(prefix, path));
        }
    }
    Ok(())
}

/// Tells whether `s` is a slug: lowercase letters, digits, and single inner hyphens, at
/// most 80 bytes.
pub(crate) fn is_valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 80
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

/// Tells whether `s` is a real calendar date written `YYYY-MM-DD`.
pub(crate) fn is_valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| -> Option<u32> {
        let part = &s[r];
        part.bytes().all(|c| c.is_ascii_digit()).then(|| part.parse().ok()).flatten()
    };
    let (Some(year), Some(month), Some(day)) = (digits(0..4), digits(5..7), digits(8..10))
    else {
        return false;
    };
    if year < 1 || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    (1..=days[month as usize - 1]).contains(&day)
}

/// Tells whether `s` looks like a permanent number: one to nine digits, not all zero.
pub(crate) fn is_number_shape(s: &str) -> bool {
    (1..=9).contains(&s.len())
        && s.bytes().all(|c| c.is_ascii_digit())
        && s.bytes().any(|c| c != b'0')
}

/// Checks a record's metadata against its catalog's process and vocabulary.
fn validate_record(r: &Record, c: &Catalog) -> Result<()> {
    let (prefix, process) = (&c.prefix, &c.process);
    if !is_valid_slug(&r.slug) {
        return Err(metadata_problem(prefix, &r.path, "slug"));
    }
    if process.state(&r.state).is_none() {
        return Err(state_problem(prefix, r, process));
    }
    if !contains(&process.types, &r.kind) {
        return Err(Problem::new(prefix.code("metadata"), "Unknown record type", shown(&r.path),
            format!("{prefix}-type is {:?}, which is not one of this catalog's types.", r.kind),
            format!("Use one of: {}; or add it to types in {CONFIG_FILE}.",
                process.types.join(", ")))
        .suggest(gd_report::suggest(&r.kind, process.types.iter().map(String::as_str))));
    }
    if r.is_draft() {
        if !process.is_unnumbered(&r.state) {
            return Err(state_problem(prefix, r, process));
        }
    } else if process.number_value(&r.number).is_none() {
        return Err(number_problem(prefix, r, process));
    }
    validate_dates(r, prefix)?;
    if r.title.trim().is_empty() {
        return Err(metadata_problem(prefix, &r.path, "title"));
    }
    if r.authors.iter().any(|a| a.trim().is_empty()) {
        return Err(metadata_problem(prefix, &r.path, "authors"));
    }
    for label in &r.labels {
        if !c.allows_label(label) {
            return Err(label_problem(c, &shown(&r.path), label));
        }
    }
    Ok(())
}

fn state_problem(prefix: &Prefix, r: &Record, process: &Process) -> Problem {
    let path = shown(&r.path);
    if r.is_draft() && process.state(&r.state).is_some() {
        return Problem::new(prefix.code("metadata"), "A draft has a state only a number allows",
            path,
            format!("The record is numbered {DRAFT_NUMBER} but its state is {}.", r.state),
            "Promote the draft first, which numbers it; or set its state back to the draft \
             state.");
    }
    let names: Vec<&str> = process.state_names().collect();
    Problem::new(prefix.code("metadata"), "Unknown lifecycle state", path,
        format!("{prefix}-state is {:?}, which is not one of this catalog's states.", r.state),
        format!("Use one of: {}.", names.join(", ")))
    .suggest(gd_report::suggest(&r.state, names))
}

fn number_problem(prefix: &Prefix, r: &Record, process: &Process) -> Problem {
    Problem::new(prefix.code("metadata"), "Invalid record number", shown(&r.path),
        format!("{prefix}-number is {:?}; this catalog writes numbers with at least {} \
            digits, such as {}, or {DRAFT_NUMBER} for a draft.", r.number,
            process.numbering.width, process.format_number(7)),
        format!("Correct the number, and the file name that starts with it; numbering.width \
            in {CONFIG_FILE} sets the digits."))
}

/// The problem of a label the catalog does not allow, about `path`.
pub(crate) fn label_problem(c: &Catalog, path: &str, label: &str) -> Problem {
    let prefix = &c.prefix;
    let Some(labels) = c.vocabulary() else {
        return Problem::new(prefix.code("label"), "Invalid discussion label", path,
            format!("{label:?} is not a label: labels use lowercase letters, digits, and \
                separating hyphens."),
            "Rename the label, such as release-notes.");
    };
    Problem::new(prefix.code("label"), "Unknown discussion label", path,
        format!("{label:?} is not one of this catalog's labels."),
        format!("Use labels from: {}. Add a label to the labels list of the catalog's \
            {CONFIG_FILE} to allow it.", labels.join(", ")))
    .suggest(gd_report::suggest(label, labels.iter().copied()))
}

fn validate_dates(r: &Record, prefix: &Prefix) -> Result<()> {
    for (name, value) in [("created", &r.created), ("updated", &r.updated)] {
        if !is_valid_date(value) {
            return Err(Problem::new(prefix.code("date"), "Invalid discussion date",
                shown(&r.path),
                format!("{prefix}-{name} is {value:?}, which is not a valid calendar date."),
                "Use a real date in YYYY-MM-DD format, such as 2026-09-29."));
        }
    }
    if r.updated < r.created {
        return Err(Problem::new(prefix.code("date-order"), "Discussion dates are out of order",
            shown(&r.path),
            format!("{prefix}-updated ({}) is earlier than {prefix}-created ({}).", r.updated,
                r.created),
            "Check both dates and correct the mistaken one; updated must be on or after \
             created."));
    }
    Ok(())
}

/// `value` as a quoted string literal, escaping quotes, backslashes, and line breaks.
pub(crate) fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Reads a quoted single-line string literal at the start of `input` (after blanks):
/// its value and the rest of the input. Metadata is data, never evaluated, so only
/// `\n`, `\r`, `\t`, `\"` and `\\` escapes are accepted.
pub(crate) fn parse_literal(input: &str) -> Option<(String, &str)> {
    let s = input.trim();
    let body = s.strip_prefix('"')?;
    let mut value = String::new();
    let mut chars = body.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((value, &body[i + 1..])),
            c if (c as u32) < 32 => return None,
            '\\' => value.push(match chars.next()?.1 {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                c @ ('"' | '\\') => c,
                _ => return None,
            }),
            c => value.push(c),
        }
    }
    None
}

/// Tells whether `rest` after a literal is blank or a `//` comment.
pub(crate) fn literal_end(rest: &str) -> bool {
    let s = rest.trim();
    s.is_empty() || s.starts_with("//")
}
