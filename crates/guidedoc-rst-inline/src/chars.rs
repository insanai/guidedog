//! Character classes of the inline markup recognition rules. Escaping backslashes are
//! replaced by NUL bytes before scanning (see scan.rs), so an escaped character is
//! always preceded by NUL, which belongs to no class below.

use crate::punctuation_table::{PUNCTUATION, SPACES};

/// Tells whether `r` is Python's `str.isspace`, which Docutils' patterns use as `\s`.
/// Zero-width and directional marks (U+200B, U+200E, U+FEFF) are not white space.
pub(crate) fn is_space(r: char) -> bool {
    let r = r as u32;
    if r < 0x1C {
        return (0x9..=0xD).contains(&r);
    }
    for &(lo, hi) in SPACES {
        if r < lo {
            return false;
        }
        if r <= hi {
            return true;
        }
    }
    false
}

/// Bracket pairs of general categories Ps and Pe that inline markup context knows.
#[rustfmt::skip]
const BRACKETS: &[(u32, u32)] = &[
    (0x28, 0x29), (0x5B, 0x5D), (0x7B, 0x7D), (0x3C, 0x3E),
    (0x0F3A, 0x0F3B), (0x0F3C, 0x0F3D), (0x169B, 0x169C), (0x2045, 0x2046),
    (0x207D, 0x207E), (0x208D, 0x208E), (0x2308, 0x2309), (0x230A, 0x230B),
    (0x2329, 0x232A), (0x2768, 0x2769), (0x276A, 0x276B), (0x276C, 0x276D),
    (0x276E, 0x276F), (0x2770, 0x2771), (0x2772, 0x2773), (0x2774, 0x2775),
    (0x27C5, 0x27C6), (0x27E6, 0x27E7), (0x27E8, 0x27E9), (0x27EA, 0x27EB),
    (0x27EC, 0x27ED), (0x27EE, 0x27EF), (0x2983, 0x2984), (0x2985, 0x2986),
    (0x2987, 0x2988), (0x2989, 0x298A), (0x298B, 0x298C), (0x298D, 0x2990),
    (0x298F, 0x298E), (0x2991, 0x2992), (0x2993, 0x2994), (0x2995, 0x2996),
    (0x2997, 0x2998), (0x29D8, 0x29D9), (0x29DA, 0x29DB), (0x29FC, 0x29FD),
    (0x2E22, 0x2E23), (0x2E24, 0x2E25), (0x2E26, 0x2E27), (0x2E28, 0x2E29),
    (0x3008, 0x3009), (0x300A, 0x300B), (0x300C, 0x300D), (0x300E, 0x300F),
    (0x3010, 0x3011), (0x3014, 0x3015), (0x3016, 0x3017), (0x3018, 0x3019),
    (0x301A, 0x301B), (0x301D, 0x301E), (0xFD3F, 0xFD3E), (0xFE17, 0xFE18),
    (0xFE35, 0xFE36), (0xFE37, 0xFE38), (0xFE39, 0xFE3A), (0xFE3B, 0xFE3C),
    (0xFE3D, 0xFE3E), (0xFE3F, 0xFE40), (0xFE41, 0xFE42), (0xFE43, 0xFE44),
    (0xFE47, 0xFE48), (0xFE59, 0xFE5A), (0xFE5B, 0xFE5C), (0xFE5D, 0xFE5E),
    (0xFF08, 0xFF09), (0xFF3B, 0xFF3D), (0xFF5B, 0xFF5D), (0xFF5F, 0xFF60),
    (0xFF62, 0xFF63),
];

/// Quotation marks (categories Pi, Pf, and the low Ps quotes) and the marks that may
/// close them in some language.
#[rustfmt::skip]
const QUOTES: &[(u32, &[u32])] = &[
    (0x22, &[0x22]), (0x27, &[0x27]),
    (0x00AB, &[0x00BB, 0x00AB]), (0x00BB, &[0x00AB, 0x00BB]),
    (0x2018, &[0x2019, 0x201A]), (0x2019, &[0x2018, 0x2019]),
    (0x201A, &[0x2018, 0x2019]), (0x201B, &[0x2019]),
    (0x201C, &[0x201D, 0x201E]), (0x201D, &[0x201C, 0x201D]),
    (0x201E, &[0x201C, 0x201D]), (0x201F, &[0x201D]),
    (0x2039, &[0x203A, 0x2039]), (0x203A, &[0x2039, 0x203A]),
    (0x2E02, &[0x2E03]), (0x2E04, &[0x2E05]), (0x2E09, &[0x2E0A]),
    (0x2E0C, &[0x2E0D]), (0x2E1C, &[0x2E1D]), (0x2E20, &[0x2E21]),
];

/// A Unicode general category of punctuation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneralPunctuation {
    /// Open.
    Ps,
    /// Close.
    Pe,
    /// Initial quote.
    Pi,
    /// Final quote.
    Pf,
    /// Dash.
    Pd,
    /// Other.
    Po,
    /// Connector.
    Pc,
}

/// The role a punctuation character plays around markup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PunctClass {
    None,
    /// Ps.
    Open,
    /// Pe.
    Close,
    /// Pi or Pf: opens and closes.
    Quote,
    /// Pd or Po.
    Delimiter,
}

/// Looks a character up in the generated punctuation table.
pub(crate) fn general_punctuation(r: char) -> Option<GeneralPunctuation> {
    let r = r as u32;
    let k = PUNCTUATION.partition_point(|&(_, hi, _)| hi < r);
    match PUNCTUATION.get(k) {
        Some(&(lo, _, category)) if lo <= r => Some(category),
        _ => None,
    }
}

pub(crate) fn punct_class(r: char) -> PunctClass {
    if r.is_ascii() {
        return match r {
            '"' | '\'' => PunctClass::Quote,
            '(' | '[' | '{' | '<' => PunctClass::Open,
            ')' | ']' | '}' | '>' => PunctClass::Close,
            '-' | '/' | ':' | '\\' | '.' | ',' | ';' | '!' | '?' => PunctClass::Delimiter,
            _ => PunctClass::None,
        };
    }
    match general_punctuation(r) {
        Some(GeneralPunctuation::Ps) => PunctClass::Open,
        Some(GeneralPunctuation::Pe) => PunctClass::Close,
        Some(GeneralPunctuation::Pi | GeneralPunctuation::Pf) => PunctClass::Quote,
        Some(GeneralPunctuation::Pd | GeneralPunctuation::Po) => PunctClass::Delimiter,
        _ => PunctClass::None,
    }
}

/// A start-string may follow white space, `' " ( [ { < - / :`, or a non-ASCII
/// character of category Ps, Pi, Pf, Pd, or Po.
pub(crate) fn start_context(r: char) -> bool {
    if is_space(r) {
        return true;
    }
    if r.is_ascii() {
        return matches!(r, '\'' | '"' | '(' | '[' | '{' | '<' | '-' | '/' | ':' | '\\');
    }
    matches!(punct_class(r), PunctClass::Open | PunctClass::Quote | PunctClass::Delimiter)
}

/// An end-string may precede white space, `- . , : ; ! ? \ / ' " ) ] } >`, or a
/// non-ASCII character of category Pe, Pi, Pf, Pd, or Po.
pub(crate) fn end_context(r: char) -> bool {
    if is_space(r) || r == '\0' {
        return true;
    }
    if r.is_ascii() {
        return matches!(
            r,
            '-' | '.'
                | ','
                | ':'
                | ';'
                | '!'
                | '?'
                | '\\'
                | '/'
                | '\''
                | '"'
                | ')'
                | ']'
                | '}'
                | '>'
        );
    }
    matches!(punct_class(r), PunctClass::Close | PunctClass::Quote | PunctClass::Delimiter)
}

/// Tells whether a start-string between these two characters is quoted, not markup.
pub(crate) fn matching_pair(before: char, after: char) -> bool {
    let (b, a) = (before as u32, after as u32);
    if let Some(&(_, close)) = BRACKETS.iter().find(|p| p.0 == b) {
        return close == a;
    }
    if let Some(&(_, closes)) = QUOTES.iter().find(|q| q.0 == b) {
        return closes.contains(&a);
    }
    false
}

/// Python's `\w` without the underscore, as the simplename rule uses it.
pub(crate) fn is_word(r: char) -> bool {
    if r.is_ascii() {
        return r.is_ascii_alphanumeric();
    }
    guidedoc::is_letter(r) || r.is_numeric()
}

pub(crate) fn is_name_separator(c: u8) -> bool {
    matches!(c, b'-' | b'.' | b'_' | b'+' | b':')
}
