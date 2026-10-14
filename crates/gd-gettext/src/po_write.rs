//! Writing PO text, laid out and wrapped as msgmerge writes it.

use std::borrow::Cow;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::catalog::{Catalog, Message};
use crate::error::{Error, ErrorKind};

/// How `#:` locations are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReferenceStyle {
    /// `#: a.rst:1 b.rst:2`, wrapped at the width, as msgmerge writes.
    #[default]
    Wrapped,
    /// One `#:` line per location, as Sphinx's gettext builder writes.
    OnePerLine,
    /// No locations (`gettext_location = False`).
    None,
}

/// How [`write_po`] lays a catalog out. The default wraps at 76 columns, as sphinx-intl
/// (through Babel) does, and writes previous msgids and obsolete messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteOptions {
    /// Lines are wrapped to this many characters; 0: no wrapping.
    pub width: usize,
    /// How locations are written.
    pub references: ReferenceStyle,
    /// Write `#|` previous msgids of fuzzy messages.
    pub previous: bool,
    /// Write `#~` obsolete messages.
    pub obsolete: bool,
}

impl Default for WriteOptions {
    fn default() -> WriteOptions {
        WriteOptions {
            width: 76,
            references: ReferenceStyle::Wrapped,
            previous: true,
            obsolete: true,
        }
    }
}

/// Returns a catalog as PO text: the header, then the messages in order, then the
/// obsolete ones, each entry laid out as msgmerge lays it out. The output depends on
/// nothing but the catalog and the options, so files diff cleanly.
pub fn write_po(cat: &Catalog, options: &WriteOptions) -> String {
    let mut out = String::new();
    write_po_into(&mut out, cat, options);
    out
}

/// [`write_po`] appending to `out`, so a caller can reuse one buffer for many catalogs.
pub fn write_po_into(out: &mut String, cat: &Catalog, options: &WriteOptions) {
    for (k, m) in entries_in_order(cat, options).enumerate() {
        if k > 0 {
            out.push('\n');
        }
        write_entry(out, m, options);
    }
}

/// [`write_po`] streaming into `out` one entry at a time, through one reused buffer, so
/// the whole text is never held. Pass a buffered writer for a file. Fails with
/// [`ErrorKind::Io`] when writing fails.
pub fn write_po_to(
    out: &mut impl Write,
    cat: &Catalog,
    options: &WriteOptions,
) -> Result<(), Error> {
    let mut entry = String::new();
    for (k, m) in entries_in_order(cat, options).enumerate() {
        entry.clear();
        if k > 0 {
            entry.push('\n');
        }
        write_entry(&mut entry, m, options);
        out.write_all(entry.as_bytes()).map_err(|err| write_error(&cat.name, &err))?;
    }
    Ok(())
}

/// The header, the live messages, then the obsolete ones when the options keep them.
fn entries_in_order<'c>(
    cat: &'c Catalog,
    options: &WriteOptions,
) -> impl Iterator<Item = &'c Message> + 'c {
    let keep_obsolete = options.obsolete;
    let live = cat.messages.iter().filter(|m| !m.obsolete);
    let obsolete = cat.messages.iter().filter(move |m| m.obsolete && keep_obsolete);
    cat.header.iter().chain(live).chain(obsolete)
}

/// Writes a catalog to a file as [`write_po`] formats it, entry by entry through a
/// buffer. Fails with [`ErrorKind::Io`] when the file cannot be written.
pub fn save_po(
    cat: &Catalog,
    path: impl AsRef<Path>,
    options: &WriteOptions,
) -> Result<(), Error> {
    let path = path.as_ref();
    let name = path.display().to_string();
    let file = File::create(path).map_err(|err| write_error(&name, &err))?;
    let mut out = BufWriter::new(file);
    write_po_to(&mut out, cat, options)?;
    out.flush().map_err(|err| write_error(&name, &err))
}

fn write_error(name: &str, err: &io::Error) -> Error {
    let message = format!("Cannot write {name}: {err}.");
    Error::new(ErrorKind::Io, name, &message, "Check that the directory exists and is writable.")
}

fn write_entry(b: &mut String, m: &Message, o: &WriteOptions) {
    for (mark, lines) in [("#", &m.comments), ("#.", &m.notes)] {
        for c in lines {
            b.push_str(mark);
            if !c.is_empty() {
                b.push(' ');
            }
            b.push_str(c);
            b.push('\n');
        }
    }
    if !m.obsolete {
        write_references(b, &m.references, o);
    }
    if m.fuzzy || !m.flags.is_empty() {
        b.push_str("#,");
        let fuzzy = m.fuzzy.then_some("fuzzy");
        for (i, f) in fuzzy.into_iter().chain(m.flags.iter().map(String::as_str)).enumerate() {
            b.push_str(if i > 0 { ", " } else { " " });
            b.push_str(f);
        }
        b.push('\n');
    }
    let prefix = if m.obsolete { "#~ " } else { "" };
    if let Some(p) = m.previous.as_ref().filter(|_| o.previous) {
        let prev = if m.obsolete { "#~| " } else { "#| " };
        if let Some(ctx) = &p.ctx {
            write_field(b, prev, "msgctxt", ctx, o.width);
        }
        write_field(b, prev, "msgid", &p.id, o.width);
        if let Some(plural) = &p.id_plural {
            write_field(b, prev, "msgid_plural", plural, o.width);
        }
    }
    if let Some(ctx) = &m.ctx {
        write_field(b, prefix, "msgctxt", ctx, o.width);
    }
    write_field(b, prefix, "msgid", &m.id, o.width);
    let Some(plural) = &m.id_plural else {
        let s = m.strs.first().map_or("", String::as_str);
        return write_field(b, prefix, "msgstr", s, o.width);
    };
    write_field(b, prefix, "msgid_plural", plural, o.width);
    let mut keyword = String::new();
    for (i, s) in m.strs.iter().enumerate() {
        keyword.clear();
        let _ = write!(keyword, "msgstr[{i}]");
        write_field(b, prefix, &keyword, s, o.width);
    }
    if m.strs.is_empty() {
        write_field(b, prefix, "msgstr[0]", "", o.width);
        write_field(b, prefix, "msgstr[1]", "", o.width);
    }
}

fn write_references(b: &mut String, refs: &[String], o: &WriteOptions) {
    match o.references {
        ReferenceStyle::None => {}
        ReferenceStyle::OnePerLine => {
            for r in refs {
                b.push_str("#: ");
                b.push_str(r);
                b.push('\n');
            }
        }
        ReferenceStyle::Wrapped => {
            if refs.is_empty() {
                return;
            }
            let mut width = 0;
            for r in refs {
                let n = r.chars().count();
                if width > 0 && o.width > 0 && width + 1 + n > o.width {
                    b.push('\n');
                    width = 0;
                }
                if width == 0 {
                    b.push_str("#:");
                    width = 2;
                }
                b.push(' ');
                b.push_str(r);
                width += 1 + n;
            }
            b.push('\n');
        }
    }
}

/// Writes `keyword "value"`, wrapping as GNU gettext does: a value that holds a newline
/// other than at its end, or does not fit, starts with `keyword ""` and continues with
/// one string per line, broken after newlines and after spaces.
fn write_field(b: &mut String, prefix: &str, keyword: &str, value: &str, width: usize) {
    let escaped = escape_string(value);
    let multiline = value.find('\n').is_some_and(|inner| inner < value.len() - 1);
    let prefix_width = prefix.chars().count();
    let one_line = prefix_width + keyword.len() + 3 + escaped.chars().count();
    if !multiline && (width == 0 || one_line <= width) {
        let _ = writeln!(b, "{prefix}{keyword} \"{escaped}\"");
        return;
    }
    let _ = writeln!(b, "{prefix}{keyword} \"\"");
    let room = width as isize - prefix_width as isize - 2;
    for piece in value.split_inclusive('\n') {
        write_wrapped(b, prefix, &escape_string(piece), room, width);
    }
}

/// Writes one line of a value as strings of at most `room` characters, breaking after a
/// space; a word longer than `room` stays whole. Each piece looks at no more than `room`
/// characters past what it writes, so a long line costs its length.
fn write_wrapped(b: &mut String, prefix: &str, escaped: &str, room: isize, width: usize) {
    let mut rest = escaped;
    while !rest.is_empty() {
        let cut = if width > 0 && longer_than(rest, room) {
            break_point(rest, room)
        } else {
            rest.len()
        };
        let _ = writeln!(b, "{prefix}\"{}\"", &rest[..cut]);
        rest = &rest[cut..];
    }
}

/// Tells whether `s` has more than `n` characters, reading at most `n + 1` of them.
fn longer_than(s: &str, n: isize) -> bool {
    n < 0 || s.chars().nth(n as usize).is_some()
}

/// Returns the byte length of the longest prefix of at most `room` characters that ends
/// after a space, or of the first word when even that is longer.
fn break_point(s: &str, room: isize) -> usize {
    let room = room.max(0) as usize;
    if let Some((i, _)) = s.char_indices().take(room).filter(|&(_, c)| c == ' ').last() {
        return i + 1;
    }
    s.char_indices().find(|&(i, c)| c == ' ' && i > 0).map_or(s.len(), |(i, _)| i + 1)
}

/// Escapes a value for a PO string body: backslash, quote, and the control characters C
/// names (others as octal). Borrows `s` when nothing needs escaping.
pub fn escape_string(s: &str) -> Cow<'_, str> {
    if !s.bytes().any(|c| c < 0x20 || c == 0x7f || c == b'\\' || c == b'"') {
        return Cow::Borrowed(s);
    }
    let mut b = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => b.push_str("\\\\"),
            '"' => b.push_str("\\\""),
            '\n' => b.push_str("\\n"),
            '\t' => b.push_str("\\t"),
            '\r' => b.push_str("\\r"),
            '\x07' => b.push_str("\\a"),
            '\x08' => b.push_str("\\b"),
            '\x0b' => b.push_str("\\v"),
            '\x0c' => b.push_str("\\f"),
            c if (c as u32) < 0x20 || c == '\x7f' => {
                let _ = write!(b, "\\{:03o}", c as u32);
            }
            c => b.push(c),
        }
    }
    Cow::Owned(b)
}
