//! Terminal display widths of characters.
//!
//! The wide table is derived from Unicode 15.1.0 `EastAsianWidth.txt` (Fullwidth and Wide).
//! The version is pinned, so a report's carets line up the same on every build.

use std::borrow::Cow;

/// Inclusive ranges of East Asian Fullwidth (F) and Wide (W) characters, sorted.
#[rustfmt::skip]
const WIDE: &[(u32, u32)] = &[
    (0x1100, 0x115F), (0x231A, 0x231B), (0x2329, 0x232A), (0x23E9, 0x23EC), (0x23F0, 0x23F0),
    (0x23F3, 0x23F3), (0x25FD, 0x25FE), (0x2614, 0x2615), (0x2648, 0x2653), (0x267F, 0x267F),
    (0x2693, 0x2693), (0x26A1, 0x26A1), (0x26AA, 0x26AB), (0x26BD, 0x26BE), (0x26C4, 0x26C5),
    (0x26CE, 0x26CE), (0x26D4, 0x26D4), (0x26EA, 0x26EA), (0x26F2, 0x26F3), (0x26F5, 0x26F5),
    (0x26FA, 0x26FA), (0x26FD, 0x26FD), (0x2705, 0x2705), (0x270A, 0x270B), (0x2728, 0x2728),
    (0x274C, 0x274C), (0x274E, 0x274E), (0x2753, 0x2755), (0x2757, 0x2757), (0x2795, 0x2797),
    (0x27B0, 0x27B0), (0x27BF, 0x27BF), (0x2B1B, 0x2B1C), (0x2B50, 0x2B50), (0x2B55, 0x2B55),
    (0x2E80, 0x303E), (0x3041, 0x3247), (0x3250, 0x4DBF), (0x4E00, 0xA4C6), (0xA960, 0xA97C),
    (0xAC00, 0xD7A3), (0xF900, 0xFAFF), (0xFE10, 0xFE19), (0xFE30, 0xFE6B), (0xFF01, 0xFF60),
    (0xFFE0, 0xFFE6), (0x16FE0, 0x1B2FB), (0x1F004, 0x1F004), (0x1F0CF, 0x1F0CF), (0x1F18E,
    0x1F18E), (0x1F191, 0x1F19A), (0x1F200, 0x1F320), (0x1F32D, 0x1F335), (0x1F337, 0x1F37C),
    (0x1F37E, 0x1F393), (0x1F3A0, 0x1F3CA), (0x1F3CF, 0x1F3D3), (0x1F3E0, 0x1F3F0), (0x1F3F4,
    0x1F3F4), (0x1F3F8, 0x1F43E), (0x1F440, 0x1F440), (0x1F442, 0x1F4FC), (0x1F4FF, 0x1F53D),
    (0x1F54B, 0x1F54E), (0x1F550, 0x1F567), (0x1F57A, 0x1F57A), (0x1F595, 0x1F596), (0x1F5A4,
    0x1F5A4), (0x1F5FB, 0x1F64F), (0x1F680, 0x1F6C5), (0x1F6CC, 0x1F6CC), (0x1F6D0, 0x1F6D2),
    (0x1F6D5, 0x1F6DF), (0x1F6EB, 0x1F6EC), (0x1F6F4, 0x1F6FC), (0x1F7E0, 0x1F7F0), (0x1F90C,
    0x1F93A), (0x1F93C, 0x1F945), (0x1F947, 0x1F9FF), (0x1FA70, 0x1FAF8), (0x20000, 0x3FFFD),
];

/// Inclusive ranges of characters that take no cell: combining marks, zero-width spaces
/// and joiners, variation selectors and the byte order mark. Sorted.
#[rustfmt::skip]
const ZERO: &[(u32, u32)] = &[
    (0x0300, 0x036F), (0x0483, 0x0489), (0x0591, 0x05BD), (0x0610, 0x061A),
    (0x064B, 0x065F), (0x0E31, 0x0E31), (0x0E34, 0x0E3A), (0x0E47, 0x0E4E),
    (0x1AB0, 0x1AFF), (0x1DC0, 0x1DFF), (0x200B, 0x200F), (0x202A, 0x202E),
    (0x2060, 0x2064), (0x20D0, 0x20FF), (0x302A, 0x302D), (0x3099, 0x309A),
    (0xFE00, 0xFE0F), (0xFE20, 0xFE2F), (0xFEFF, 0xFEFF), (0xE0100, 0xE01EF),
];

fn in_table(table: &[(u32, u32)], c: u32) -> bool {
    table
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

/// Reports whether `c` is a C0 or C1 control character or DEL.
pub fn is_control(c: char) -> bool {
    let c = c as u32;
    c < 0x20 || c == 0x7F || (0x80..=0x9F).contains(&c)
}

/// Reports whether `c` is a combining mark or another character drawn in no cell.
pub fn is_zero_width(c: char) -> bool {
    in_table(ZERO, c as u32)
}

/// Reports whether `c` is East Asian Wide or Fullwidth, taking two terminal cells.
pub fn is_wide(c: char) -> bool {
    let c = c as u32;
    c >= 0x1100 && in_table(WIDE, c)
}

/// The number of terminal cells `c` takes: 0 for controls and combining or zero-width
/// characters, 2 for East Asian Wide and Fullwidth ones, 1 for the rest.
pub fn char_width(c: char) -> usize {
    if is_control(c) || is_zero_width(c) {
        0
    } else if is_wide(c) {
        2
    } else {
        1
    }
}

/// The number of terminal cells `text` takes, the sum of [`char_width`] over its
/// characters. Grapheme clusters such as emoji sequences are not joined.
pub fn display_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

/// The width of `c` once [`escape_char`] has made it safe: a tab is one space and a
/// control character is six cells (`\u001b`).
pub(crate) fn escaped_width(c: char) -> usize {
    if c == '\t' {
        1
    } else if is_control(c) {
        6
    } else {
        char_width(c)
    }
}

/// Writes `c` so it cannot affect a terminal: a tab becomes a space and other control
/// characters `\uXXXX`.
pub(crate) fn escape_char(out: &mut String, c: char) {
    use std::fmt::Write;
    if c == '\t' {
        out.push(' ');
    } else if is_control(c) {
        let _ = write!(out, "\\u{:04x}", c as u32);
    } else {
        out.push(c);
    }
}

/// Returns `text` with every control character except the line break escaped as
/// `\uXXXX` (tabs included), so file names and sources cannot send escape sequences to a
/// terminal. Borrows `text` when it has nothing to escape and allocates otherwise.
pub fn escape_terminal(text: &str) -> Cow<'_, str> {
    use std::fmt::Write;
    if !text.chars().any(|c| c != '\n' && is_control(c)) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        if c != '\n' && is_control(c) {
            let _ = write!(out, "\\u{:04x}", c as u32);
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_follow_east_asian_width_and_combining_marks() {
        assert_eq!(char_width('a'), 1);
        assert_eq!(char_width('漢'), 2);
        assert_eq!(char_width('Ａ'), 2);
        assert_eq!(char_width('\u{301}'), 0);
        assert_eq!(char_width('\u{200B}'), 0);
        assert_eq!(char_width('\u{1b}'), 0);
        assert_eq!(char_width('😀'), 2);
        assert_eq!(display_width("e\u{301}漢"), 3);
    }

    #[test]
    fn escaping_keeps_line_breaks_only() {
        assert_eq!(escape_terminal("a\u{1b}[31m\tb\nc"), "a\\u001b[31m\\u0009b\nc");
        assert!(matches!(escape_terminal("plain\n"), Cow::Borrowed(_)));
        let mut s = String::new();
        escape_char(&mut s, '\t');
        escape_char(&mut s, '\u{9b}');
        assert_eq!(s, " \\u009b");
    }
}
