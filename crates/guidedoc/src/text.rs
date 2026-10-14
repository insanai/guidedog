//! Text helpers readers and renderers share: character classes, case folding, label
//! normalization, UTF-8 checks, line and column positions, and `TextSink`, the one
//! interface every text writer of the crate writes through.

use crate::report::{Arg, CORE_UTF8, Fail, diagnostic};
use crate::tables::casefold::CASE_FOLDS;
use crate::tables::categories::{DECIMAL_DIGITS, LETTERS, PUNCTUATION_OR_SYMBOL};
use crate::types::{SourceId, SourceSpan};

/// Something text can be written into: a `String`, a byte buffer, a builder's text pool,
/// or an emitter. Helpers such as `normalize_label` and `make_id` write into any of them,
/// so callers reuse their own buffers instead of receiving fresh strings.
pub trait TextSink {
    /// Appends `s`.
    fn put_str(&mut self, s: &str);

    /// Appends one character.
    fn put_char(&mut self, c: char) {
        let mut bytes = [0u8; 4];
        self.put_str(c.encode_utf8(&mut bytes));
    }
}

impl TextSink for String {
    fn put_str(&mut self, s: &str) {
        self.push_str(s);
    }

    fn put_char(&mut self, c: char) {
        self.push(c);
    }
}

impl TextSink for Vec<u8> {
    fn put_str(&mut self, s: &str) {
        self.extend_from_slice(s.as_bytes());
    }
}

/// Decodes the character starting at byte `i` of `s`, with its width. A position that
/// is not a character boundary decodes as U+FFFD with width 1, as Odin's decoder does
/// for an invalid byte; past the end gives U+FFFD with width 0.
pub fn decode_rune(s: &str, i: usize) -> (char, usize) {
    match s.get(i..).and_then(|rest| rest.chars().next()) {
        Some(c) => (c, c.len_utf8()),
        None if i < s.len() => (char::REPLACEMENT_CHARACTER, 1),
        None => (char::REPLACEMENT_CHARACTER, 0),
    }
}

/// Decodes the character ending at byte `end` of `s`, as `decode_rune` does.
pub fn decode_last_rune(s: &str, end: usize) -> (char, usize) {
    match s.get(..end).and_then(|head| head.chars().next_back()) {
        Some(c) => (c, c.len_utf8()),
        None if end > 0 && end <= s.len() => (char::REPLACEMENT_CHARACTER, 1),
        None => (char::REPLACEMENT_CHARACTER, 0),
    }
}

/// Tells whether `c` is ASCII white space: space, tab, LF, CR, FF, or VT.
pub fn is_ascii_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0c | 0x0b)
}

/// Tells whether `c` is ASCII punctuation, as CommonMark defines it.
pub fn is_ascii_punct(c: u8) -> bool {
    c.is_ascii_punctuation()
}

/// Tells whether `c` is an ASCII letter or digit.
pub fn is_ascii_alnum(c: u8) -> bool {
    c.is_ascii_alphanumeric()
}

/// Tells whether `c` is an ASCII digit.
pub fn is_ascii_digit(c: u8) -> bool {
    c.is_ascii_digit()
}

/// Tells whether `r` is white space as CommonMark defines it: Zs characters, tab, LF,
/// FF, and CR.
pub fn is_unicode_whitespace(r: char) -> bool {
    match r {
        ' ' | '\t' | '\n' | '\u{0c}' | '\r' | '\u{a0}' => true,
        // White_Space minus the line and paragraph separators and NEL leaves Zs.
        _ => {
            (r as u32) > 0x7f
                && r.is_whitespace()
                && !matches!(r, '\u{85}' | '\u{2028}' | '\u{2029}')
        }
    }
}

/// Tells whether `r` is punctuation as CommonMark 0.31 defines it: general categories P
/// and S.
pub fn is_unicode_punctuation(r: char) -> bool {
    if r.is_ascii() {
        return is_ascii_punct(r as u8);
    }
    in_ranges(PUNCTUATION_OR_SYMBOL, r)
}

/// Tells whether `r` is a letter (general category L).
pub fn is_letter(r: char) -> bool {
    if r.is_ascii() {
        return r.is_ascii_alphabetic();
    }
    in_ranges(LETTERS, r)
}

/// Tells whether `r` is a decimal digit (general category Nd).
pub fn is_decimal_digit(r: char) -> bool {
    if r.is_ascii() {
        return r.is_ascii_digit();
    }
    in_ranges(DECIMAL_DIGITS, r)
}

/// Returns the simple lowercase mapping of `r` (one character), as Odin's
/// `unicode.to_lower`; characters whose full mapping has several characters keep the
/// first one, which is the simple mapping for every such character (U+0130 gives 'i').
pub fn to_lower(r: char) -> char {
    r.to_lowercase().next().unwrap_or(r)
}

fn in_ranges(ranges: &[(u32, u32)], r: char) -> bool {
    let c = r as u32;
    ranges
        .binary_search_by(|&(lo, hi)| {
            if hi < c {
                std::cmp::Ordering::Less
            } else if lo > c {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// Applies Unicode full case folding (statuses C and F) to `r`: up to three characters,
/// and how many.
pub fn fold_rune(r: char) -> ([char; 3], usize) {
    if r.is_ascii() {
        return ([r.to_ascii_lowercase(), '\0', '\0'], 1);
    }
    let c = r as u32;
    let Ok(i) = CASE_FOLDS.binary_search_by_key(&c, |row| row.0) else {
        return ([r, '\0', '\0'], 1);
    };
    let mut out = ['\0'; 3];
    let mut n = 0;
    for &code in CASE_FOLDS[i].1.iter().take_while(|&&code| code != 0) {
        out[n] = char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER);
        n += 1;
    }
    (out, n)
}

/// Writes the case-folded `label` with runs of white space collapsed to one space and
/// the ends trimmed. CommonMark labels and RST names are normalized this way.
pub fn normalize_label(out: &mut (impl TextSink + ?Sized), label: &str) {
    let mut space = false;
    let mut wrote = false;
    for r in label.chars() {
        if is_unicode_whitespace(r) {
            space = wrote;
            continue;
        }
        if space {
            out.put_char(' ');
            space = false;
        }
        let (folded, n) = fold_rune(r);
        for &f in &folded[..n] {
            out.put_char(f);
        }
        wrote = true;
    }
}

/// Checks that `bytes` are UTF-8 and returns them as text; otherwise a `CORE_UTF8`
/// failure whose span (in source `source`) covers the first bad byte. Hosts call it on
/// file contents before building a `Request`.
pub fn check_utf8(source: SourceId, bytes: &[u8]) -> Result<&str, Fail> {
    std::str::from_utf8(bytes).map_err(|e| {
        let bad = u32::try_from(e.valid_up_to()).unwrap_or(u32::MAX);
        let span = SourceSpan::new(source, bad, bad.saturating_add(1));
        Fail::from_diagnostic(diagnostic(CORE_UTF8, span, Arg::None, Arg::None))
    })
}

/// Converts a byte offset into a 1-based line and byte column.
pub fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let end = offset.min(text.len());
    let head = &text.as_bytes()[..end];
    let line = 1 + head.iter().filter(|&&c| c == b'\n').count();
    let start = head.iter().rposition(|&c| c == b'\n').map_or(0, |i| i + 1);
    (line, end - start + 1)
}

/// Returns the byte range of the line holding `offset`, without its break.
pub fn line_bounds(text: &str, offset: usize) -> (usize, usize) {
    let bytes = text.as_bytes();
    let o = offset.min(bytes.len());
    let start = bytes[..o].iter().rposition(|&c| c == b'\n').map_or(0, |i| i + 1);
    let end =
        bytes[o..].iter().position(|&c| c == b'\n' || c == b'\r').map_or(bytes.len(), |i| o + i);
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_fold_case_fully() {
        let mut x = String::new();
        let mut y = String::new();
        normalize_label(&mut x, "  Straße \n  ẞ ");
        normalize_label(&mut y, "STRASSE SS");
        assert_eq!(x, y);
        assert_eq!(x, "strasse ss");
    }

    #[test]
    fn character_classes() {
        assert!(is_unicode_punctuation('€'));
        assert!(is_unicode_punctuation('«'));
        assert!(!is_unicode_punctuation('é'));
        assert!(is_unicode_whitespace('\u{3000}'));
        assert!(!is_unicode_whitespace('\u{2028}'));
        assert!(is_letter('ж') && !is_letter('1'));
        assert!(is_decimal_digit('٣'));
        assert_eq!(to_lower('\u{130}'), 'i');
    }

    #[test]
    fn positions() {
        let text = "ab\ncd\r\nef";
        assert_eq!(line_column(text, 4), (2, 2));
        assert_eq!(line_bounds(text, 4), (3, 5));
        assert_eq!(decode_rune("é", 1), ('\u{fffd}', 1));
        assert_eq!(decode_last_rune("aé", 3), ('é', 2));
    }

    #[test]
    fn utf8_check_names_the_bad_byte() {
        let fail = check_utf8(SourceId::MAIN, b"ab\xff").err();
        assert_eq!(fail.map(|f| f.diagnostic.span.start), Some(2));
    }
}
