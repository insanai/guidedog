//! The C half of sphinx.ext.coverage: header files (`coverage_c_path`) read line by
//! line, each `coverage_c_regexes` pattern naming an item in its first group, and the
//! items no `c:` description names written to `c.txt`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use gd_conf::{Config, Pattern};
use gd_report::Report;

use crate::coverage::{Documented, warning};

/// A `coverage_c_regexes` pattern split at its first group, so the group's text can be
/// found with a matcher that only answers yes or no: `before(group)after`.
struct GroupPattern {
    before: Pattern,
    group: Pattern,
    after: Pattern,
}

impl GroupPattern {
    /// `None` when the pattern has no group, or a part does not compile alone.
    fn new(source: &str) -> Option<GroupPattern> {
        let (open, close) = first_group(source)?;
        Some(GroupPattern {
            before: Pattern::new(&source[..open]).ok()?,
            group: Pattern::new(&source[open + 1..close]).ok()?,
            after: Pattern::new(&format!("(?:{}).*", &source[close + 1..])).ok()?,
        })
    }

    /// `regex.match(line).groups()[0]`: the group as late as the part before it allows
    /// (as a greedy pattern places it), then as long as it can be.
    fn first_group<'a>(&self, line: &'a str) -> Option<&'a str> {
        let bounds: Vec<usize> = line.char_indices().map(|(i, _)| i).chain([line.len()]).collect();
        for &s in bounds.iter().rev() {
            if !self.before.full_match(&line[..s]) {
                continue;
            }
            for &e in bounds.iter().rev().filter(|&&e| e >= s) {
                if self.group.full_match(&line[s..e]) && self.after.full_match(&line[e..]) {
                    return Some(&line[s..e]);
                }
            }
        }
        None
    }
}

/// The byte span of the parentheses of the first capturing group of `source`.
fn first_group(source: &str) -> Option<(usize, usize)> {
    let b = source.as_bytes();
    let (mut i, mut in_class, mut open, mut depth) = (0, false, None, 0usize);
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            b'[' if !in_class => in_class = true,
            b']' if in_class => in_class = false,
            b'(' if !in_class => {
                if open.is_none() && b.get(i + 1) != Some(&b'?') {
                    open = Some(i);
                    depth = 0;
                } else if open.is_some() {
                    depth += 1;
                }
            }
            b')' if !in_class && open.is_some() => {
                if depth == 0 {
                    return open.map(|o| (o, i));
                }
                depth -= 1;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// The files `coverage_c_path` names: each pattern relative to the source folder, its
/// last part a glob.
fn c_files(c: &Config) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for pattern in &c.ext.coverage.c_path {
        let full = gd_conf::source_path(c, pattern);
        let (Some(dir), Some(name)) = (full.parent(), full.file_name()) else { continue };
        let name = name.to_string_lossy();
        if !name.contains(['*', '?', '[']) {
            out.push(full.clone());
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        let mut found: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| crate::apidoc::fnmatch(&e.file_name().to_string_lossy(), &name))
            .map(|e| e.path())
            .collect();
        found.sort();
        out.extend(found);
    }
    out
}

/// `build_c_coverage` and `write_c_coverage`: the text of `c.txt`.
pub(crate) fn c_txt(c: &Config, documented: &Documented, reports: &mut Vec<Report>) -> String {
    let k = &c.ext.coverage;
    let regexes: Vec<(&String, GroupPattern)> =
        k.c_regexes.iter().filter_map(|(key, p)| Some((key, GroupPattern::new(p)?))).collect();
    let ignores = |key: &str, name: &str| {
        k.ignore_c_items.get(key).is_some_and(|patterns| {
            patterns
                .iter()
                .filter_map(|p| Pattern::new(&format!("(?:{p}).*")).ok())
                .any(|p| p.full_match(name))
        })
    };
    let readable = gd_host::canonical_roots(&gd_conf::readable_roots(c));
    let mut out = String::new();
    if k.write_headline {
        out.push_str("Undocumented C API elements\n===========================\n\n");
    }
    out.push('\n');
    for file in c_files(c) {
        if !gd_host::confined(&readable, &file) {
            reports.push(outside(&file));
            continue;
        }
        let Ok(bytes) = gd_host::read_file(&file, None, gd_host::MAX_SOURCE_BYTES) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        let mut undoc: BTreeSet<(String, String)> = BTreeSet::new();
        for line in text.lines() {
            for (key, regex) in &regexes {
                let Some(name) = regex.first_group(line) else { continue };
                let described = documented.c_items.contains(&((*key).clone(), name.to_owned()));
                if !described && !ignores(key, name) {
                    undoc.insert(((*key).clone(), name.to_owned()));
                }
            }
        }
        if undoc.is_empty() {
            continue;
        }
        let shown = file.display().to_string();
        out.push_str(&format!("{shown}\n{}\n\n", "-".repeat(shown.chars().count())));
        for (typ, name) in &undoc {
            out.push_str(&format!(" * {name:<50} [{typ:>9}]\n"));
            if k.show_missing_items {
                reports.push(missing(&shown, typ, name));
            }
        }
        out.push('\n');
    }
    out
}

fn missing(file: &str, typ: &str, name: &str) -> Report {
    let message = format!("undocumented c api: {name} [{typ}] in file {file}");
    let report = Report::note("coverage.undocumented", "UNDOCUMENTED OBJECT", &message)
        .hint("Describe it with a c: directive, or ignore it with coverage_ignore_c_items.");
    Report { category: "coverage.undocumented".to_owned(), ..report }
}

fn outside(file: &std::path::Path) -> Report {
    warning(
        "coverage.c",
        "C HEADER OUTSIDE THE READABLE FOLDERS",
        format!("coverage_c_path names {}, outside the folders a build may read.", file.display()),
        "Move the header below the source folder, or add its folder to include_roots.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_group_is_found_as_python_finds_it() {
        let p =
            GroupPattern::new(r"^PyAPI_FUNC\(.*\)\s+([A-Za-z_][A-Za-z0-9_]+)").expect("compiles");
        assert_eq!(p.first_group("PyAPI_FUNC(int) Py_Main(int argc);"), Some("Py_Main"));
        assert_eq!(p.first_group("int other(void);"), None);
        let p = GroupPattern::new(r"(?:#define)\s+(\w+)").expect("compiles");
        assert_eq!(p.first_group("#define MAX_LEN 10"), Some("MAX_LEN"));
        assert!(GroupPattern::new(r"no group").is_none());
    }
}
