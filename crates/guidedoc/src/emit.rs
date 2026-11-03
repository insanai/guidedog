//! The emitter is where a renderer writes. It never fails: in write mode it appends to
//! an output buffer until a byte limit and then only counts; in check mode it only
//! counts. The engine compares the count with the limit afterwards, so an output that
//! is too large is a `Capacity` result with the exact size needed.

use std::fmt;

use crate::options::MapEntry;
use crate::text::TextSink;
use crate::types::SourceSpan;

/// What an emitter does with the bytes it is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmitMode {
    /// Count them only (check_render).
    Check,
    /// Store them, up to the limit.
    Write,
}

/// A renderer's output sink. The output is bytes so a renderer may copy text byte by
/// byte; the engine checks the whole output is UTF-8 before returning it.
pub struct Emitter<'a> {
    out: Option<&'a mut Vec<u8>>,
    limit: usize,
    written: usize,
    map: Option<&'a mut Vec<MapEntry>>,
    mapped: usize,
}

impl fmt::Debug for Emitter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Emitter")
            .field("mode", &self.mode())
            .field("written", &self.written)
            .field("limit", &self.limit)
            .finish_non_exhaustive()
    }
}

impl<'a> Emitter<'a> {
    /// Makes a write-mode emitter appending to `out`, without a limit. Renderers and
    /// tests use it for side outputs (a dump, a highlighted fragment).
    pub fn new(out: &'a mut Vec<u8>) -> Emitter<'a> {
        Emitter { out: Some(out), limit: usize::MAX, written: 0, map: None, mapped: 0 }
    }

    /// Makes a write-mode emitter that stores at most `limit` bytes and counts the rest.
    pub fn with_limit(out: &'a mut Vec<u8>, limit: usize) -> Emitter<'a> {
        Emitter { limit, ..Emitter::new(out) }
    }

    /// Makes a check-mode emitter, which only counts.
    pub fn check() -> Emitter<'static> {
        Emitter { out: None, limit: 0, written: 0, map: None, mapped: 0 }
    }

    /// Records source map entries into `map` (see `emit_mark`).
    pub fn set_source_map(&mut self, map: Option<&'a mut Vec<MapEntry>>) {
        self.map = map;
    }

    /// Returns the mode.
    pub fn mode(&self) -> EmitMode {
        if self.out.is_some() { EmitMode::Write } else { EmitMode::Check }
    }

    /// Tells whether this emitter only counts.
    pub fn is_check(&self) -> bool {
        self.out.is_none()
    }

    /// Returns the bytes written (stored or counted).
    pub fn written(&self) -> usize {
        self.written
    }

    /// Returns the source map entries recorded.
    pub fn mapped(&self) -> usize {
        self.mapped
    }

    /// Writes raw bytes.
    pub fn emit_bytes(&mut self, bytes: &[u8]) {
        self.written = self.written.saturating_add(bytes.len());
        if let Some(out) = self.out.as_deref_mut()
            && self.written <= self.limit
            && out.try_reserve(bytes.len()).is_ok()
        {
            out.extend_from_slice(bytes);
        } else if self.out.is_some() {
            // Past the limit (or out of memory): stop storing, keep counting; the engine
            // reports the exact size from `written`.
            self.limit = 0;
        }
    }

    /// The bytes stored since the output was `from` bytes long (a `written` value);
    /// empty in check mode or past the limit.
    pub fn written_since(&self, from: usize) -> &[u8] {
        match self.out.as_deref() {
            Some(out) if self.written <= self.limit && from <= self.written => {
                let back = self.written - from;
                out.get(out.len().saturating_sub(back)..).unwrap_or(&[])
            }
            _ => &[],
        }
    }

    /// Inserts `bytes` where the output was `at` bytes long (a `written` value at or
    /// after which everything is still stored), moving what follows. Renderers use it
    /// to add an attribute to a tag they already wrote. Counts only, like every write,
    /// in check mode or past the limit.
    pub fn insert_bytes(&mut self, at: usize, bytes: &[u8]) {
        let stored = self.written <= self.limit && at <= self.written;
        let back = self.written.saturating_sub(at);
        self.written = self.written.saturating_add(bytes.len());
        let fits = stored && self.written <= self.limit;
        let Some(out) = self.out.as_deref_mut() else { return };
        if fits && out.try_reserve(bytes.len()).is_ok() {
            let index = out.len() - back;
            out.splice(index..index, bytes.iter().copied());
        } else {
            self.limit = 0;
        }
    }

    /// Writes `s`.
    pub fn emit(&mut self, s: &str) {
        self.emit_bytes(s.as_bytes());
    }

    /// Writes one byte.
    pub fn emit_byte(&mut self, c: u8) {
        self.emit_bytes(&[c]);
    }

    /// Writes `c` UTF-8 encoded.
    pub fn emit_char(&mut self, c: char) {
        let mut bytes = [0u8; 4];
        self.emit(c.encode_utf8(&mut bytes));
    }

    /// Writes `value` in decimal.
    pub fn emit_int(&mut self, value: i64) {
        let mut digits = [0u8; 20];
        let mut n = digits.len();
        let mut v = value.unsigned_abs();
        loop {
            n -= 1;
            digits[n] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        if value < 0 {
            self.emit_byte(b'-');
        }
        self.emit_bytes(&digits[n..]);
    }

    /// Records that output from here on originates at `span`.
    pub fn emit_mark(&mut self, span: SourceSpan) {
        let offset = u32::try_from(self.written).unwrap_or(u32::MAX);
        if let Some(map) = self.map.as_deref_mut()
            && map.try_reserve(1).is_ok()
        {
            map.push(MapEntry { offset, span });
        }
        self.mapped += 1;
    }
}

impl TextSink for Emitter<'_> {
    fn put_str(&mut self, s: &str) {
        self.emit(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitters_count_past_their_limit() {
        let mut out = Vec::new();
        let mut e = Emitter::with_limit(&mut out, 4);
        e.emit("ab");
        e.emit_int(-305);
        e.emit("c");
        assert_eq!(e.written(), 7);
        // What is stored is always a prefix of what was written.
        assert_eq!(out, b"ab-");
        let mut check = Emitter::check();
        check.emit_char('é');
        assert_eq!(check.written(), 2);
        let mut all = Vec::new();
        let mut e = Emitter::new(&mut all);
        e.emit_int(0);
        e.emit_int(i64::MIN);
        assert_eq!(all, b"0-9223372036854775808");
    }

    #[test]
    fn insertions_land_where_the_output_was() {
        let mut out = b"kept ".to_vec();
        let mut e = Emitter::new(&mut out);
        e.emit("<p>a</p><ul>");
        let at = e.written();
        e.emit("<li>b</li>");
        assert_eq!(e.written_since(at), b"<li>b</li>");
        e.insert_bytes(at + 3, b" id=x");
        e.insert_bytes(2, b" class=y");
        assert_eq!(e.written(), 35);
        assert_eq!(out, b"kept <p class=y>a</p><ul><li id=x>b</li>");
        let mut small = Vec::new();
        let mut e = Emitter::with_limit(&mut small, 4);
        e.emit("<p>");
        e.insert_bytes(2, b" too long");
        assert_eq!((e.written(), e.written_since(0)), (12, &b""[..]));
    }
}
