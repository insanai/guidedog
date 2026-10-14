//! File tags (`#+build`, `#+private`, `#+ignore`, and the legacy `//+build` comments
//! above the package clause), read as Odin's parser reads them (`parse_file_tags` and
//! `match_build_tags` in core:odin/parser).

use crate::ast::File;
use crate::platform::{Arch, EVERY_ARCH, Os, Targets};

/// A file's privacy from `#+private`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum FilePrivacy {
    #[default]
    Public,
    Package,
    File,
}

/// One `#+build` alternative: systems and architectures as bit sets.
#[derive(Debug, Clone, Copy)]
struct BuildKind {
    os: u16,
    arch: u8,
}

const EVERY_OS: u16 = 0b111_1111_1110;

#[derive(Debug, Default)]
struct FileTags {
    /// Lines of alternatives; every line must hold.
    lines: Vec<Vec<BuildKind>>,
    private: FilePrivacy,
    ignore: bool,
}

/// The targets a file's tags allow, and its privacy.
pub(crate) fn tag_targets(file: &File, src: &str) -> (Targets, FilePrivacy) {
    let tags = read_tags(file, src);
    if tags.ignore {
        return (Targets::NONE, tags.private);
    }
    let mut out = Targets::SUPPORTED;
    for line in &tags.lines {
        let mut allowed = Targets::NONE;
        for kind in line {
            allowed = allowed.or(Targets::from_bits(kind.os, kind.arch));
        }
        out = out.and(allowed);
    }
    (out, tags.private)
}

fn read_tags(file: &File, src: &str) -> FileTags {
    let mut tags = FileTags::default();
    let text = |s: crate::ast::Span| src.get(s.start as usize..s.end as usize).unwrap_or("");
    if let Some(docs) = file.package_docs {
        for &span in &file.group(docs).list {
            let comment = text(span);
            if comment.len() >= 3 && comment.starts_with("//") {
                parse_tag(&comment[2..], &mut tags);
            }
        }
    }
    for &span in &file.tags {
        let tag = text(span);
        if tag.len() >= 3 && tag.starts_with("#+") {
            parse_tag(&tag[1..], &mut tags);
        }
    }
    tags
}

/// A word up to white space, a comma, or the end.
fn scan_value(text: &str) -> (&str, &str) {
    let end = text.find([' ', '\t', '\n', '\r', '\0', ',']).unwrap_or(text.len());
    text.split_at(end)
}

fn parse_tag(text: &str, tags: &mut FileTags) {
    let Some(rest) = text.trim_start_matches([' ', '\t']).strip_prefix('+') else {
        return;
    };
    let (name, rest) = scan_value(rest);
    match name {
        "ignore" => tags.ignore = true,
        "private" => {
            let (value, _) = scan_value(rest.trim_start_matches([' ', '\t']));
            match value {
                "file" => tags.private = FilePrivacy::File,
                "package" | "" => tags.private = FilePrivacy::Package,
                _ => {}
            }
        }
        "build" => parse_build(rest, tags),
        _ => {}
    }
}

/// A `#+build` line: alternatives separated by commas, each of names that must all
/// hold, `!name` excluding one.
fn parse_build(line: &str, tags: &mut FileTags) {
    let line = line.split(['\n', '\r', '\0']).next().unwrap_or("");
    let mut kinds = Vec::new();
    for alternative in line.split(',') {
        let (mut os_yes, mut os_no, mut arch_yes, mut arch_no) = (0u16, 0u16, 0u8, 0u8);
        for word in alternative.split([' ', '\t']).filter(|w| !w.is_empty()) {
            let (notted, value) = match word.strip_prefix('!') {
                Some(v) => (true, v),
                None => (false, word),
            };
            if value == "ignore" {
                tags.ignore = true;
            } else if let Some(os) = build_os(value) {
                let bit = 1 << (os as u16);
                if notted { os_no |= bit } else { os_yes |= bit }
            } else if let Some(arch) = build_arch(value) {
                let bit = 1 << (arch as u8);
                if notted { arch_no |= bit } else { arch_yes |= bit }
            }
        }
        let os = if os_yes == 0 { EVERY_OS } else { os_yes };
        let arch = if arch_yes == 0 { EVERY_ARCH } else { arch_yes };
        kinds.push(BuildKind { os: os & !os_no, arch: arch & !arch_no });
    }
    tags.lines.push(kinds);
}

/// The system a build tag names, in any case, before any `:subtarget`.
fn build_os(value: &str) -> Option<Os> {
    let name = value.split(':').next().unwrap_or("");
    Os::named(name)
}

fn build_arch(value: &str) -> Option<Arch> {
    Arch::ALL.into_iter().find(|a| a.name().eq_ignore_ascii_case(value))
}
