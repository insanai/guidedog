//! The line-by-line PO parser behind [`crate::PoReader`].

use std::collections::HashMap;

use crate::catalog::{Message, Previous, message_key};
use crate::error::{Error, ErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Field {
    None,
    Ctx,
    Id,
    Plural,
    Str,
    PrevCtx,
    PrevId,
    PrevPlural,
}

const FIELDS: usize = 8;

/// Parses PO text one line at a time and hands out each message as it ends.
#[derive(Debug)]
pub(crate) struct Parser {
    name: String,
    line_no: usize,
    /// A copy of the line being parsed, for error excerpts (the buffer is reused).
    current: String,
    /// The first line of the current entry, for errors about the whole entry.
    entry_line: String,
    cur: Message,
    /// A message that ended on the line just parsed.
    finished: Option<Message>,
    seen_header: bool,
    field: Field,
    str_index: usize,
    seen_id: bool,
    seen_str: bool,
    /// msgctxt `\x04` msgid of live messages -> the line it was defined on.
    keys: HashMap<String, usize>,
    /// Each field's strings, gathered until the entry (or, for msgstr, the form) ends
    /// and copied into the message once, so a field of many continuation lines costs its
    /// length.
    pending: [String; FIELDS],
    /// Scratch for decoding one quoted string, reused.
    decoded: Vec<u8>,
    /// Bytes gathered into messages; the cost test checks it grows with the text.
    pub(crate) copied: usize,
}

type Step = Result<(), Error>;

impl Parser {
    pub(crate) fn new(name: &str) -> Parser {
        Parser {
            name: name.to_string(),
            line_no: 0,
            current: String::new(),
            entry_line: String::new(),
            cur: Message::default(),
            finished: None,
            seen_header: false,
            field: Field::None,
            str_index: 0,
            seen_id: false,
            seen_str: false,
            keys: HashMap::with_capacity(64),
            pending: Default::default(),
            decoded: Vec::new(),
            copied: 0,
        }
    }

    /// Parses line `line_no` (without its line break) and returns the message it ended,
    /// if any.
    pub(crate) fn feed(&mut self, line_no: usize, line: &str) -> Result<Option<Message>, Error> {
        self.line_no = line_no;
        self.current.clear();
        self.current.push_str(line);
        self.parse_line(line)?;
        Ok(self.finished.take())
    }

    /// Ends the input and returns the last message, if any.
    pub(crate) fn finish(&mut self) -> Result<Option<Message>, Error> {
        self.finish_entry()?;
        Ok(self.finished.take())
    }

    fn parse_line(&mut self, line: &str) -> Step {
        if line.trim().is_empty() {
            self.field = Field::None;
            return Ok(());
        }
        if let Some(after) = line.strip_prefix("#~") {
            let rest = after.trim_start_matches([' ', '\t']);
            let column = line.len() - rest.len();
            if rest.is_empty() {
                return Ok(()); // a bare "#~" says nothing.
            }
            // "#~|" is the previous msgid of an obsolete message; "#~ msgid" its fields.
            return match rest.strip_prefix('|') {
                Some(prev) => {
                    self.parse_previous(prev.trim_start_matches([' ', '\t']), column + 1)
                }
                None => self.parse_keyword_line(rest, column, true),
            };
        }
        if line.starts_with('#') {
            return self.parse_comment(line);
        }
        let trimmed = line.trim_start_matches([' ', '\t']);
        self.parse_keyword_line(trimmed, line.len() - trimmed.len(), false)
    }

    /// Notes that the current entry has begun on this line.
    fn touch(&mut self) {
        if self.cur.line == 0 {
            self.cur.line = self.line_no;
            self.entry_line.clone_from(&self.current);
        }
    }

    fn parse_comment(&mut self, line: &str) -> Step {
        if self.seen_str {
            self.finish_entry()?;
        }
        self.touch();
        let kind = line.as_bytes().get(1).copied().unwrap_or(b' ');
        let body = if matches!(kind, b'.' | b':' | b',' | b'|') { &line[2..] } else { "" };
        match kind {
            b'.' => self.cur.notes.push(body.strip_prefix(' ').unwrap_or(body).to_string()),
            b':' => self.cur.references.extend(body.split_whitespace().map(str::to_string)),
            b',' => {
                for flag in body.split(',').map(str::trim).filter(|f| !f.is_empty()) {
                    if flag == "fuzzy" {
                        self.cur.fuzzy = true;
                    } else {
                        self.cur.flags.push(flag.to_string());
                    }
                }
            }
            b'|' => {
                let column = line.len() - body.len() + 1;
                return self.parse_previous(body.trim_start_matches([' ', '\t']), column);
            }
            _ => {
                let text = &line[1..];
                self.cur.comments.push(text.strip_prefix(' ').unwrap_or(text).to_string());
            }
        }
        Ok(())
    }

    /// Reads a `#|` line: msgctxt, msgid, or msgid_plural of the previous message, or a
    /// continuation string.
    fn parse_previous(&mut self, text: &str, column: usize) -> Step {
        if self.seen_str {
            self.finish_entry()?;
        }
        self.touch();
        let previous = self.cur.previous.get_or_insert_with(Previous::default);
        let (field, rest) = if let Some(rest) = text.strip_prefix("msgctxt") {
            previous.ctx = Some(String::new());
            (Field::PrevCtx, rest)
        } else if let Some(rest) = text.strip_prefix("msgid_plural") {
            previous.id_plural = Some(String::new());
            (Field::PrevPlural, rest)
        } else if let Some(rest) = text.strip_prefix("msgid") {
            (Field::PrevId, rest)
        } else if text.starts_with('"') {
            if self.field < Field::PrevCtx {
                return Err(self.syntax_error(
                    column,
                    "This \"#|\" string continues no previous field.",
                    "Start the previous message with #| msgid \"...\".",
                ));
            }
            (self.field, text)
        } else {
            return Err(self.syntax_error(
                column,
                "A \"#|\" comment must hold msgctxt, msgid, or msgid_plural of the previous \
                 message.",
                "Write it as #| msgid \"old text\".",
            ));
        };
        self.field = field;
        self.parse_string(rest, column + text.len() - rest.len())
    }

    fn parse_keyword_line(&mut self, text: &str, column: usize, obsolete: bool) -> Step {
        if text.starts_with('"') {
            if self.field == Field::None || self.field >= Field::PrevCtx {
                return Err(self.syntax_error(
                    column + 1,
                    "This string continues no msgid or msgstr.",
                    "Put continuation strings right after the keyword line they extend.",
                ));
            }
            return self.parse_string(text, column);
        }
        let end = text.find([' ', '\t', '"']).unwrap_or(text.len());
        self.start_field(&text[..end], column, obsolete)?;
        self.parse_string(&text[end..], column + end)
    }

    /// Checks that a keyword may come here and starts its field.
    fn start_field(&mut self, keyword: &str, column: usize, obsolete: bool) -> Step {
        match keyword {
            "msgctxt" | "msgid" => self.start_id(keyword, column)?,
            "msgid_plural" => {
                if !self.seen_id || self.seen_str || self.cur.id_plural.is_some() {
                    return Err(self.syntax_error(
                        column + 1,
                        "msgid_plural must follow msgid, once, before msgstr.",
                        "Write msgid, then msgid_plural, then msgstr[0], msgstr[1], ...",
                    ));
                }
                self.cur.id_plural = Some(String::new());
                self.field = Field::Plural;
            }
            _ if keyword.starts_with("msgstr") => self.start_msgstr(keyword, column)?,
            _ => {
                return Err(self.syntax_error(
                    column + 1,
                    &format!("\"{keyword}\" is not a PO keyword."),
                    "Start the line with msgctxt, msgid, msgid_plural, msgstr, msgstr[N], a \
                     \"string\", or # for a comment.",
                ));
            }
        }
        self.touch();
        self.cur.obsolete |= obsolete;
        Ok(())
    }

    fn start_id(&mut self, keyword: &str, column: usize) -> Step {
        if self.seen_str {
            self.finish_entry()?;
        }
        if self.seen_id {
            return Err(self.syntax_error(
                column + 1,
                "The message before this line has no msgstr.",
                "Add msgstr \"\" after its msgid.",
            ));
        }
        if keyword == "msgid" {
            self.seen_id = true;
            self.field = Field::Id;
            return Ok(());
        }
        if self.cur.ctx.is_some() {
            return Err(self.syntax_error(
                column + 1,
                "This message has two msgctxt lines.",
                "Keep one msgctxt; continue long strings on the next line.",
            ));
        }
        self.cur.ctx = Some(String::new());
        self.field = Field::Ctx;
        Ok(())
    }

    fn start_msgstr(&mut self, keyword: &str, column: usize) -> Step {
        if !self.seen_id {
            return Err(self.syntax_error(
                column + 1,
                "This msgstr has no msgid before it.",
                "Put a msgid \"...\" line before the msgstr: every message starts with one.",
            ));
        }
        if keyword == "msgstr" {
            self.check_plain_msgstr(column)?;
        } else {
            self.check_msgstr_index(keyword, column)?;
        }
        if self.seen_str {
            self.settle(Some(Field::Str)); // the form before this one.
        }
        self.cur.strs.push(String::new());
        self.seen_str = true;
        self.field = Field::Str;
        self.str_index = self.cur.strs.len() - 1;
        Ok(())
    }

    fn check_plain_msgstr(&self, column: usize) -> Step {
        if self.cur.id_plural.is_some() {
            return Err(self.syntax_error(
                column + 1,
                "A message with msgid_plural needs msgstr[N] forms.",
                "Write msgstr[0] \"...\", msgstr[1] \"...\", one per plural form.",
            ));
        }
        if self.seen_str {
            return Err(self.syntax_error(
                column + 1,
                "This message has two msgstr lines.",
                "Keep one msgstr; continue long strings on the next line.",
            ));
        }
        Ok(())
    }

    fn check_msgstr_index(&self, keyword: &str, column: usize) -> Step {
        let Some(index) = msgstr_index(keyword) else {
            return Err(self.syntax_error(
                column + 1,
                &format!("\"{keyword}\" is not a PO keyword."),
                "Write plural forms as msgstr[0], msgstr[1], ...",
            ));
        };
        if self.cur.id_plural.is_none() {
            return Err(self.syntax_error(
                column + 1,
                "msgstr[N] needs a msgid_plural before it.",
                "Add msgid_plural \"...\", or write a plain msgstr \"...\".",
            ));
        }
        if index != self.cur.strs.len() {
            return Err(self.syntax_error(
                column + 1,
                &format!("Expected msgstr[{}] here, found msgstr[{index}].", self.cur.strs.len()),
                "Number the plural forms 0, 1, 2, ... in order.",
            ));
        }
        Ok(())
    }

    /// Copies the gathered strings into the current message: one field, or all.
    fn settle(&mut self, only: Option<Field>) {
        for (k, buf) in self.pending.iter_mut().enumerate() {
            let field = FIELD_ORDER[k];
            if buf.is_empty() || only.is_some_and(|f| f != field) {
                continue;
            }
            self.copied += buf.len();
            // Copy at the exact size and keep the buffer's capacity for the next entry:
            // the message holds no slack, and the buffer stops reallocating.
            let text = buf.as_str().to_owned();
            buf.clear();
            let m = &mut self.cur;
            match field {
                Field::None => {}
                Field::Ctx => m.ctx = Some(text),
                Field::Id => m.id = text,
                Field::Plural => m.id_plural = Some(text),
                Field::Str => m.strs[self.str_index] = text,
                Field::PrevCtx => previous(m).ctx = Some(text),
                Field::PrevId => previous(m).id = text,
                Field::PrevPlural => previous(m).id_plural = Some(text),
            }
        }
    }

    /// Reads one quoted C string into the current field's buffer and requires nothing
    /// but space after it.
    fn parse_string(&mut self, text: &str, column: usize) -> Step {
        let s = text.trim_start_matches([' ', '\t']);
        let at = column + text.len() - s.len();
        if !s.starts_with('"') {
            return Err(self.syntax_error(
                at + 1,
                "Expected a string in double quotes here.",
                "Write the text as \"...\"; an empty text is \"\".",
            ));
        }
        let end = self.decode(s, at)?;
        if !s[end + 1..].trim().is_empty() {
            return Err(self.syntax_error(
                at + end + 2,
                "Unexpected text after the closing quote.",
                "Escape quotes inside a string as \\\".",
            ));
        }
        let Ok(decoded) = std::str::from_utf8(&self.decoded) else {
            return Err(self.syntax_error(
                at + 1,
                "Escapes in this string make bytes that are not UTF-8.",
                "Write the characters themselves instead of octal or hex escapes.",
            ));
        };
        if self.field != Field::None {
            self.pending[self.field as usize].push_str(decoded);
        }
        Ok(())
    }

    /// Decodes the string starting at `s[0]` (a quote) into `self.decoded`; returns the
    /// byte offset of the closing quote.
    fn decode(&mut self, s: &str, at: usize) -> Result<usize, Error> {
        let bytes = s.as_bytes();
        self.decoded.clear();
        let mut i = 1;
        loop {
            let Some(&c) = bytes.get(i) else {
                return Err(self.syntax_error(
                    at + 1,
                    "This string has no closing quote.",
                    "End the string with \" on the same line; continue on the next line with \
                     a new \"...\".",
                ));
            };
            match c {
                b'"' => return Ok(i),
                b'\\' => match parse_escape(bytes, i) {
                    Some((byte, n)) => {
                        self.decoded.push(byte);
                        i += n;
                    }
                    None => {
                        return Err(self.syntax_error(
                            at + i + 1,
                            "This is not a C escape sequence.",
                            "Use \\n \\t \\r \\\" \\\\ \\a \\b \\f \\v \\? \\' \\ooo (octal) \
                             or \\xhh.",
                        ));
                    }
                },
                _ => {
                    self.decoded.push(c);
                    i += 1;
                }
            }
        }
    }

    /// Ends the current message: the header (once), a live message (checked for
    /// duplicates), or an obsolete one; then starts the next.
    fn finish_entry(&mut self) -> Step {
        self.settle(None);
        let m = std::mem::take(&mut self.cur);
        let (seen_id, seen_str) = (self.seen_id, self.seen_str);
        self.field = Field::None;
        self.seen_id = false;
        self.seen_str = false;
        if !seen_id {
            return Ok(());
        }
        if !seen_str {
            return Err(self.entry_error(
                &m,
                ErrorKind::Syntax,
                "This message has a msgid but no msgstr.",
                "Add msgstr \"\" after the msgid; translators fill it in.",
            ));
        }
        if m.id.is_empty() && m.ctx.is_none() && !m.obsolete {
            if self.seen_header {
                return Err(self.entry_error(
                    &m,
                    ErrorKind::Duplicate,
                    "The catalog has a second header (msgid \"\").",
                    "Keep one header entry at the top of the file.",
                ));
            }
            self.seen_header = true;
            self.finished = Some(m);
            return Ok(());
        }
        if !m.obsolete {
            let key = message_key(&m);
            if let Some(&first) = self.keys.get(&key) {
                return Err(self.entry_error(
                    &m,
                    ErrorKind::Duplicate,
                    &format!("This message is already defined on line {first}."),
                    "Merge the two entries, or give one a msgctxt.",
                ));
            }
            self.keys.insert(key, m.line);
        }
        self.finished = Some(m);
        Ok(())
    }

    fn syntax_error(&self, column: usize, message: &str, hint: &str) -> Error {
        let error = Error::new(ErrorKind::Syntax, &self.name, message, hint);
        error.at(self.line_no, column, &self.current)
    }

    fn entry_error(&self, m: &Message, kind: ErrorKind, message: &str, hint: &str) -> Error {
        Error::new(kind, &self.name, message, hint).at(m.line, 0, &self.entry_line)
    }
}

/// The fields in the order of their `pending` buffers.
const FIELD_ORDER: [Field; FIELDS] = [
    Field::None,
    Field::Ctx,
    Field::Id,
    Field::Plural,
    Field::Str,
    Field::PrevCtx,
    Field::PrevId,
    Field::PrevPlural,
];

fn previous(m: &mut Message) -> &mut Previous {
    m.previous.get_or_insert_with(Previous::default)
}

fn msgstr_index(keyword: &str) -> Option<usize> {
    let digits = keyword.strip_prefix("msgstr[")?.strip_suffix(']')?;
    if digits.is_empty() || digits.len() > 2 || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Decodes the escape at `s[i]` (a backslash); returns its byte and length.
fn parse_escape(s: &[u8], i: usize) -> Option<(u8, usize)> {
    let c = *s.get(i + 1)?;
    let simple = match c {
        b'n' => b'\n',
        b't' => b'\t',
        b'r' => b'\r',
        b'a' => 0x07,
        b'b' => 0x08,
        b'f' => 0x0c,
        b'v' => 0x0b,
        b'\\' | b'"' | b'\'' | b'?' => c,
        b'0'..=b'7' => {
            // Up to three octal digits; more than 255 is not a byte.
            let mut value = 0u32;
            let mut n = 0;
            while n < 3 {
                match s.get(i + 1 + n) {
                    Some(&d @ b'0'..=b'7') => value = value * 8 + u32::from(d - b'0'),
                    _ => break,
                }
                n += 1;
            }
            return u8::try_from(value).ok().map(|v| (v, 1 + n));
        }
        b'x' => {
            let mut value = 0u32;
            let mut n = 0;
            while let Some(d) = s.get(i + 2 + n).and_then(|&d| (d as char).to_digit(16)) {
                value = value * 16 + d;
                n += 1;
                if value > 255 {
                    return None;
                }
            }
            return (n > 0).then_some((value as u8, 2 + n));
        }
        _ => return None,
    };
    Some((simple, 2))
}
