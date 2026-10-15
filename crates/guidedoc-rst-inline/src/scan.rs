//! The scanner works on the lines joined by "\n", with every escaping backslash
//! replaced by a NUL byte. Lengths do not change, so a position maps back to a source
//! offset through the line table. A NUL stands for "the next character is escaped": it
//! is in no character class, so escaped characters never start or end markup.

use guidedoc::{Fail, ReadContext, SourceSpan, decode_last_rune, decode_rune};

use crate::chars::start_context;
use crate::chars::{end_context, is_name_separator, is_space, is_word, matching_pair};
use crate::extension::RoleHook;
use crate::uri::UriMemo;
use crate::{InlineState, Line};

/// A piece of text starts here (see `Scanner::marks`).
pub(crate) const MARK_BOUNDARY: u8 = 1;
/// The piece starting here is a problematic start-string.
pub(crate) const MARK_BLOCKED: u8 = 2;

/// The scanner's reusable storage, kept in `InlineState` between calls.
#[derive(Debug, Default)]
pub(crate) struct Buffers {
    text: String,
    starts: Vec<u32>,
    marks: Vec<u8>,
    temp: String,
}

/// The constructs whose end-strings are searched for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndKind {
    Emphasis,
    Strong,
    Literal,
    Target,
    Substitution,
    Interpreted,
}

/// Remembers the first end-string at or after `from`. Whether a position is an
/// end-string does not depend on where the search began, so a later search that
/// starts between `from` and `found` has the same answer.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EndCache {
    pub from: usize,
    pub found: Option<usize>,
    pub valid: bool,
}

/// One inline parse: the joined text, where each line starts, and the builder.
pub(crate) struct Scanner<'x, 'a> {
    pub ctx: &'x mut ReadContext<'a>,
    pub state: &'x InlineState,
    pub hook: &'x mut dyn RoleHook,
    /// The joined lines, escapes as NUL.
    pub text: String,
    /// Start of each line in `text`.
    pub starts: Vec<u32>,
    pub lines: &'x [Line<'x>],
    /// Length of the source, for clamping spans.
    pub limit: u32,
    /// Per position: MARK_BOUNDARY, MARK_BLOCKED.
    pub marks: Vec<u8>,
    /// Scratch text for role calls and names.
    pub temp: String,
    /// Start of the text not yet emitted.
    pub pos: usize,
    /// Start of the text still to parse; it counts as a text start.
    pub base: usize,
    /// Positions before this cannot start a simple reference.
    pub no_ref: usize,
    pub uri: UriMemo,
    pub cache: [EndCache; 6],
}

impl<'x, 'a> Scanner<'x, 'a> {
    /// Joins `lines` into buffers taken from `buffers`; `escapes` turns escaping
    /// backslashes into NUL. Give the buffers back with `finish`.
    pub fn new(
        ctx: &'x mut ReadContext<'a>,
        state: &'x InlineState,
        hook: &'x mut dyn RoleHook,
        lines: &'x [Line<'x>],
        buffers: Buffers,
        escapes: bool,
    ) -> Result<Scanner<'x, 'a>, Fail> {
        let Buffers { text, mut starts, mut marks, mut temp } = buffers;
        let mut bytes = text.into_bytes();
        bytes.clear();
        starts.clear();
        temp.clear();
        let total: usize = lines.iter().map(|l| l.text.len() + 1).sum();
        bytes.reserve(total);
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                bytes.push(b'\n');
            }
            starts.push(bytes.len() as u32);
            bytes.extend_from_slice(line.text.as_bytes());
        }
        if escapes {
            escape_to_nul(&mut bytes);
        }
        // Only ASCII bytes changed, so the text is still UTF-8.
        let text = String::from_utf8(bytes)
            .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
        marks.clear();
        marks.resize(text.len() + 1, 0);
        let limit = ctx.source_of(state.source).len() as u32;
        ctx.b.spend(text.len() as u64 + 1)?;
        Ok(Scanner {
            ctx,
            state,
            hook,
            text,
            starts,
            lines,
            limit,
            marks,
            temp,
            pos: 0,
            base: 0,
            no_ref: 0,
            uri: UriMemo::default(),
            cache: [EndCache::default(); 6],
        })
    }

    /// Returns the buffers for the next parse.
    pub fn finish(self) -> Buffers {
        Buffers { text: self.text, starts: self.starts, marks: self.marks, temp: self.temp }
    }

    /// The joined text's bytes.
    pub fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    fn line_of(&self, p: usize) -> usize {
        self.starts.partition_point(|&s| s as usize <= p).saturating_sub(1)
    }

    /// Maps a text position to a source offset.
    pub fn offset(&self, p: usize) -> u32 {
        if self.lines.is_empty() {
            return 0;
        }
        let k = self.line_of(p);
        let column = (p - self.starts[k] as usize).min(self.lines[k].text.len());
        (self.lines[k].offset + column as u32).min(self.limit)
    }

    pub fn span(&self, a: usize, b: usize) -> SourceSpan {
        let start = self.offset(a);
        SourceSpan::new(self.state.source, start, start.max(self.offset(b)))
    }

    /// Decodes the character at `p`; NUL and out-of-range positions give '\0'.
    pub fn rune_at(&self, p: usize) -> (char, usize) {
        let Some(&c) = self.bytes().get(p) else {
            return ('\0', 0);
        };
        if c < 0x80 {
            return (c as char, 1);
        }
        decode_rune(&self.text, p)
    }

    pub fn rune_before(&self, p: usize) -> (char, usize) {
        if p == 0 || p > self.len() {
            return ('\0', 0);
        }
        let c = self.bytes()[p - 1];
        if c < 0x80 {
            return (c as char, 1);
        }
        decode_last_rune(&self.text, p)
    }

    pub fn byte_at(&self, p: usize) -> u8 {
        self.bytes().get(p).copied().unwrap_or(0)
    }

    /// The start-string at `p` begins the remaining text or follows a start context.
    pub fn start_ok(&self, p: usize, base: usize) -> bool {
        if p == base || self.state.options.character_level {
            return true;
        }
        let (r, n) = self.rune_before(p);
        n > 0 && start_context(r)
    }

    /// The end-string finishing at `p` ends the text or precedes an end context.
    pub fn end_ok(&self, p: usize, bound: usize) -> bool {
        if p >= bound || self.state.options.character_level {
            return true;
        }
        end_context(self.rune_at(p).0)
    }

    /// The character at `p` exists and is white space.
    pub fn space_after(&self, p: usize) -> bool {
        let (r, n) = self.rune_at(p);
        n > 0 && is_space(r)
    }

    pub fn space_before(&self, p: usize) -> bool {
        let (r, n) = self.rune_before(p);
        n > 0 && is_space(r)
    }

    /// The start-string `[p, q)` sits between matching brackets or quotes, or ends the
    /// text, so it is not markup.
    pub fn quoted(&self, p: usize, q: usize) -> bool {
        if q >= self.len() {
            return true;
        }
        if p == self.base {
            return false;
        }
        matching_pair(self.rune_before(p).0, self.rune_at(q).0)
    }

    /// Scans a simplename at `p`: words of alphanumerics joined by single characters
    /// from "-._+:". Returns the end of the longest name, or `p` when there is none.
    pub fn name_end(&self, p: usize) -> usize {
        let mut last = p;
        let mut i = p;
        loop {
            let j = self.word_end(i);
            if j == i {
                break;
            }
            last = j;
            if !is_name_separator(self.byte_at(j)) {
                break;
            }
            let (r, n) = self.rune_at(j + 1);
            if n == 0 || !is_word(r) {
                break;
            }
            i = j + 1;
        }
        last
    }

    /// Returns the end of the run of word characters at `p`.
    pub fn word_end(&self, p: usize) -> usize {
        let mut j = p;
        loop {
            let (r, n) = self.rune_at(j);
            if n == 0 || !is_word(r) {
                return j;
            }
            j += n;
        }
    }
}

/// Marks escapes: a backslash escapes the following character, which keeps its
/// identity; the backslash itself becomes NUL.
fn escape_to_nul(text: &mut [u8]) {
    let mut i = 0;
    while i < text.len() {
        if text[i] == b'\\' {
            text[i] = 0;
            i += 2;
        } else {
            i += 1;
        }
    }
}
