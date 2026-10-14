//! Reading PO and POT text: a streaming reader of entries, and catalogs built from it.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::catalog::{Catalog, Message, charset_in, header_field_in, is_utf8_charset};
use crate::error::{Error, ErrorKind, read_error};
use crate::load::{LoadOptions, check_size};
use crate::plural::{PluralRule, parse_plural_forms};
use crate::po_parse::Parser;

/// Reads PO or POT entries one at a time from any buffered source, so a large file
/// need not be held whole.
///
/// It is an iterator of `Result<Message, Error>`: the header entry first when the file
/// has one ([`Message::is_header`]), then the messages, obsolete ones included, in file
/// order. After an error it yields nothing more. It keeps only the current entry and
/// the keys seen so far (to report duplicates), plus one reused line buffer.
///
/// ```
/// use gd_gettext::{PoReader, Stats};
/// let text = "msgid \"a\"\nmsgstr \"b\"\n\nmsgid \"c\"\nmsgstr \"\"\n";
/// let mut stats = Stats::default();
/// for message in PoReader::new(text.as_bytes(), "de.po") {
///     stats.add(&message?);
/// }
/// assert_eq!((stats.translated, stats.untranslated), (1, 1));
/// # Ok::<(), gd_gettext::Error>(())
/// ```
#[derive(Debug)]
pub struct PoReader<R> {
    source: R,
    name: String,
    parser: Parser,
    line: Vec<u8>,
    line_no: usize,
    bytes_read: u64,
    max_bytes: u64,
    plural: PluralRule,
    done: bool,
}

impl<R: BufRead> PoReader<R> {
    /// Returns a reader of `source`; `name` labels errors. There is no size limit.
    pub fn new(source: R, name: &str) -> PoReader<R> {
        PoReader {
            source,
            name: name.to_string(),
            parser: Parser::new(name),
            line: Vec::new(),
            line_no: 0,
            bytes_read: 0,
            max_bytes: u64::MAX,
            plural: PluralRule::default(),
            done: false,
        }
    }

    /// Returns the Plural-Forms rule of the header read so far (`n != 1` before it).
    pub fn plural_rule(&self) -> &PluralRule {
        &self.plural
    }

    /// Bytes gathered into messages so far: what the cost test checks.
    pub(crate) fn bytes_gathered(&self) -> usize {
        self.parser.copied
    }

    /// Reads lines until a message ends; `None` at the end of the input.
    fn read_message(&mut self) -> Result<Option<Message>, Error> {
        loop {
            self.line.clear();
            let n = self.source.read_until(b'\n', &mut self.line);
            let n = n.map_err(|e| read_error(&self.name, &e))?;
            if n == 0 {
                return self.parser.finish();
            }
            self.bytes_read += n as u64;
            check_size(&self.name, self.bytes_read, self.max_bytes)?;
            self.line_no += 1;
            let text = line_text(&self.line, self.line_no, &self.name)?;
            if let Some(m) = self.parser.feed(self.line_no, text)? {
                return Ok(Some(m));
            }
        }
    }

    /// Checks a header entry and takes its Plural-Forms rule.
    fn read_header(&mut self, header: &Message) -> Result<(), Error> {
        let text = header.strs.first().map_or("", String::as_str);
        let set = charset_in(text);
        if !is_utf8_charset(set) {
            let excerpt = format!("\"Content-Type: text/plain; charset={set}\\n\"");
            let message =
                format!("The catalog says it is encoded in {set}; Guidedog reads UTF-8 only.");
            let hint = "Convert it with `msgconv --to-code=UTF-8` (or iconv) and set \
                        charset=UTF-8 in the header.";
            let error = Error::new(ErrorKind::Charset, &self.name, &message, hint);
            return Err(error.at(header.line, 0, &excerpt));
        }
        let Some(forms) = header_field_in(text, "Plural-Forms") else { return Ok(()) };
        self.plural = parse_plural_forms(forms).map_err(|mut err| {
            self.name.clone_into(&mut err.file);
            err.line = header.line;
            err
        })?;
        Ok(())
    }
}

impl PoReader<BufReader<File>> {
    /// Opens a PO or POT file for reading entry by entry. Fails when it cannot be
    /// opened or is larger than `options.max_bytes`; reading stops with the same error
    /// if it grows past the limit.
    pub fn open(path: impl AsRef<Path>, options: &LoadOptions) -> Result<Self, Error> {
        let path = path.as_ref();
        let name = path.display().to_string();
        let file = File::open(path).map_err(|e| read_error(&name, &e))?;
        let size = file.metadata().map_err(|e| read_error(&name, &e))?.len();
        check_size(&name, size, options.max_bytes)?;
        let mut reader = PoReader::new(BufReader::new(file), &name);
        reader.max_bytes = options.max_bytes;
        Ok(reader)
    }
}

impl<R: BufRead> Iterator for PoReader<R> {
    type Item = Result<Message, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let result = self.read_message().and_then(|m| match m {
            Some(m) if m.is_header() => self.read_header(&m).map(|()| Some(m)),
            other => Ok(other),
        });
        if !matches!(result, Ok(Some(_))) {
            self.done = true;
        }
        result.transpose()
    }
}

/// Returns a line as text without its line break (and a byte-order mark on line 1).
/// Fails at the first byte that does not start a UTF-8 character.
fn line_text<'l>(bytes: &'l [u8], line_no: usize, name: &str) -> Result<&'l str, Error> {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    let bytes = if line_no == 1 {
        bytes.strip_prefix("\u{feff}".as_bytes()).unwrap_or(bytes)
    } else {
        bytes
    };
    std::str::from_utf8(bytes).map_err(|err| {
        let at = err.valid_up_to();
        let excerpt = String::from_utf8_lossy(bytes).replace('\u{fffd}', "?");
        let message = format!(
            "The byte 0x{:02x} at column {} does not start a UTF-8 character.",
            bytes[at],
            at + 1
        );
        let hint = "Convert the file to UTF-8, for example with `msgconv --to-code=UTF-8`.";
        Error::new(ErrorKind::Encoding, name, &message, hint).at(line_no, 0, &excerpt)
    })
}

/// Reads a PO or POT file into an editable catalog, entry by entry through a buffer.
///
/// Fails when the file cannot be read or is larger than `options.max_bytes`, or as
/// [`parse_po`] fails.
pub fn load_po(path: impl AsRef<Path>, options: &LoadOptions) -> Result<Catalog, Error> {
    let path = path.as_ref();
    let reader = PoReader::open(path, options)?;
    let mut cat = collect_catalog(reader, &path.display().to_string())?.0;
    cat.allow_fuzzy = options.allow_fuzzy;
    Ok(cat)
}

/// Parses PO or POT text into an editable catalog; `name` labels errors and the
/// catalog, and `text` is only borrowed.
///
/// Fails on the first syntax error, a duplicate message, a header naming a charset other
/// than UTF-8, or an invalid Plural-Forms header; the error names the line and column.
pub fn parse_po(text: &str, name: &str) -> Result<Catalog, Error> {
    parse_po_bytes(text.as_bytes(), name)
}

/// [`parse_po`] for bytes that may not be UTF-8: fails with [`ErrorKind::Encoding`] at
/// the first byte that does not start a character.
pub fn parse_po_bytes(data: &[u8], name: &str) -> Result<Catalog, Error> {
    Ok(collect_catalog(PoReader::new(data, name), name)?.0)
}

/// Gathers a reader's entries into a catalog; also returns the bytes gathered.
pub(crate) fn collect_catalog<R: BufRead>(
    mut reader: PoReader<R>,
    name: &str,
) -> Result<(Catalog, usize), Error> {
    let mut cat = Catalog::new(name);
    for m in reader.by_ref() {
        let m = m?;
        if m.is_header() {
            cat.header = Some(m);
        } else {
            cat.messages.push(m);
        }
    }
    cat.plural = reader.plural.clone();
    Ok((cat, reader.bytes_gathered()))
}
