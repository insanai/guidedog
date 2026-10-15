//! The token patterns of Sphinx's C and C++ parsers (`sphinx/util/cfamily.py` and
//! `sphinx/domains/cpp/_ids.py`), matched by hand as their regular expressions match.
//! Each matcher takes the text and a byte offset and returns the end of the match.

/// Tells whether a byte belongs to a word, as `\w` does for the UTF-8 encoding: ASCII
/// letters, digits, `_`, and every byte of a non-ASCII character.
pub(crate) fn is_word_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80
}

/// Tells whether a byte is an ASCII letter, digit, or `_`.
pub(crate) fn is_ascii_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Tells whether a byte is white space as Python's `\s` sees it in ASCII.
pub(crate) fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0c | 0x0b)
}

/// Returns `s` without leading and trailing white space.
pub(crate) fn trim_space(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_ascii() && is_space(c as u8))
}

/// `\b` after a word character: the end of the text, or a byte that is not a word's.
fn boundary(s: &[u8], k: usize) -> bool {
    k >= s.len() || !is_word_byte(s[k])
}

fn byte_at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

fn starts_at(s: &[u8], i: usize, prefix: &str) -> bool {
    s.get(i..).is_some_and(|rest| rest.starts_with(prefix.as_bytes()))
}

/// Matches `D+('D+)*` for decimal or hexadecimal digits.
fn digits(s: &[u8], i: usize, hex: bool) -> Option<usize> {
    let is = |c: u8| if hex { c.is_ascii_hexdigit() } else { c.is_ascii_digit() };
    let run = |mut k: usize| {
        while k < s.len() && is(s[k]) {
            k += 1;
        }
        k
    };
    if i >= s.len() || !is(s[i]) {
        return None;
    }
    let mut k = run(i);
    while k + 1 < s.len() && s[k] == b'\'' && is(s[k + 1]) {
        k = run(k + 1);
    }
    Some(k)
}

/// Matches `[eE][+-]?D+('D+)*`, or with `[pP]` for hexadecimal floats.
fn exponent(s: &[u8], i: usize, hex: bool) -> Option<usize> {
    let c = *s.get(i)?;
    let marks = if hex { *b"pP" } else { *b"eE" };
    if !marks.contains(&c) {
        return None;
    }
    let mut k = i + 1;
    if matches!(byte_at(s, k), b'+' | b'-') {
        k += 1;
    }
    digits(s, k, hex)
}

/// Matches `float_literal_re`, whose first matching alternative wins.
pub(crate) fn float_literal(s: &[u8], at: usize) -> Option<usize> {
    let mut i = at;
    if matches!(byte_at(s, i), b'+' | b'-') {
        i += 1;
    }
    if let Some(e) = float_forms(s, i, false) {
        return Some(e);
    }
    if byte_at(s, i) != b'0' || !matches!(byte_at(s, i + 1), b'x' | b'X') {
        return None;
    }
    float_forms(s, i + 2, true)
}

/// Tries the three forms of a decimal (or hexadecimal) float: digits and an exponent;
/// optional digits, a point, digits, and an optional exponent; digits, a point, and an
/// optional exponent.
fn float_forms(s: &[u8], i: usize, hex: bool) -> Option<usize> {
    if let Some(x) = digits(s, i, hex).and_then(|e| exponent(s, e, hex)) {
        return Some(x);
    }
    let k = digits(s, i, hex).unwrap_or(i);
    if byte_at(s, k) == b'.'
        && let Some(f) = digits(s, k + 1, hex)
    {
        return Some(exponent(s, f, hex).unwrap_or(f));
    }
    let e = digits(s, i, hex)?;
    if byte_at(s, e) == b'.' {
        return Some(exponent(s, e + 1, hex).unwrap_or(e + 1));
    }
    None
}

/// Matches digits of one class with `'` separators after the first, from `i`.
fn class_run(s: &[u8], i: usize, is: impl Fn(u8) -> bool) -> usize {
    let run = |mut k: usize| {
        while k < s.len() && is(s[k]) {
            k += 1;
        }
        k
    };
    let mut k = run(i);
    while k + 1 < s.len() && s[k] == b'\'' && is(s[k + 1]) {
        k = run(k + 1);
    }
    k
}

/// Tries `binary_literal_re`, `hex_literal_re`, `integer_literal_re`, and
/// `octal_literal_re` in that order.
pub(crate) fn integer_literal(s: &[u8], i: usize) -> Option<usize> {
    let binary = |c: u8| c == b'0' || c == b'1';
    if byte_at(s, i) == b'0'
        && matches!(byte_at(s, i + 1), b'b' | b'B')
        && binary(byte_at(s, i + 2))
    {
        return Some(class_run(s, i + 2, binary));
    }
    if byte_at(s, i) == b'0'
        && matches!(byte_at(s, i + 1), b'x' | b'X')
        && let Some(e) = digits(s, i + 2, true)
    {
        return Some(e);
    }
    match byte_at(s, i) {
        b'1'..=b'9' => digits(s, i, false),
        b'0' => Some(class_run(s, i + 1, |c| (b'0'..=b'7').contains(&c))),
        _ => None,
    }
}

/// The ends after `l`/`L`, `ll`, and `LL` at `k`, where they match.
fn longs(s: &[u8], k: usize) -> [Option<usize>; 3] {
    [
        matches!(byte_at(s, k), b'l' | b'L').then_some(k + 1),
        starts_at(s, k, "ll").then_some(k + 2),
        starts_at(s, k, "LL").then_some(k + 2),
    ]
}

/// Matches `integers_literal_suffix_re`: `u` or `U` with `l`, `ll`, or `LL`, or those
/// with `u` or `U` after, ending a word.
pub(crate) fn integer_suffix(s: &[u8], i: usize) -> Option<usize> {
    if matches!(byte_at(s, i), b'u' | b'U') {
        if let Some(e) = longs(s, i + 1).into_iter().flatten().find(|&e| boundary(s, e)) {
            return Some(e);
        }
        if boundary(s, i + 1) {
            return Some(i + 1);
        }
    }
    for e in longs(s, i).into_iter().flatten() {
        if matches!(byte_at(s, e), b'u' | b'U') && boundary(s, e + 1) {
            return Some(e + 1);
        }
        if boundary(s, e) {
            return Some(e);
        }
    }
    None
}

/// Matches `[fFlL]\b`.
pub(crate) fn float_suffix(s: &[u8], i: usize) -> Option<usize> {
    (matches!(byte_at(s, i), b'f' | b'F' | b'l' | b'L') && boundary(s, i + 1)).then_some(i + 1)
}

/// Matches `[a-zA-Z_][a-zA-Z0-9_]*\b`, a user-defined literal's suffix.
pub(crate) fn udl_identifier(s: &[u8], i: usize) -> Option<usize> {
    let c = *s.get(i)?;
    if !(c.is_ascii_alphabetic() || c == b'_') {
        return None;
    }
    let mut k = i + 1;
    while k < s.len() && is_ascii_word(s[k]) {
        k += 1;
    }
    boundary(s, k).then_some(k)
}

/// A character literal's parts: where it ends, its prefix, and the data between quotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CharLiteral<'a> {
    pub end: usize,
    pub prefix: &'a str,
    pub data: &'a str,
}

/// Matches `char_literal_re`: an optional prefix (`u8`, `u`, `U`, `L`), then one
/// character or escape between single quotes.
pub(crate) fn char_literal(text: &str, i: usize) -> Option<CharLiteral<'_>> {
    let s = text.as_bytes();
    for prefix in ["u8", "u", "U", "L", ""] {
        let k = i + prefix.len();
        if !starts_at(s, i, prefix) || byte_at(s, k) != b'\'' {
            continue;
        }
        let Some(e) = char_body(s, k + 1) else { continue };
        if byte_at(s, e) != b'\'' {
            continue;
        }
        let data = text.get(k + 1..e)?;
        return Some(CharLiteral { end: e + 1, prefix, data });
    }
    None
}

fn char_body(s: &[u8], i: usize) -> Option<usize> {
    let c = *s.get(i)?;
    if c != b'\\' {
        return (c != b'\'').then(|| i + utf8_width(c));
    }
    let k = i + 1;
    let c = *s.get(k)?;
    match c {
        b'\'' | b'"' | b'?' | b'\\' | b'a' | b'b' | b'f' | b'n' | b'r' | b't' | b'v' => {
            Some(k + 1)
        }
        b'0'..=b'7' => {
            let mut e = k;
            while e < s.len() && e < k + 3 && (b'0'..=b'7').contains(&s[e]) {
                e += 1;
            }
            Some(e)
        }
        b'x' => hex_run(s, k + 1, 2).then_some(k + 3),
        b'u' => hex_run(s, k + 1, 4).then_some(k + 5),
        b'U' => hex_run(s, k + 1, 8).then_some(k + 9),
        _ => None,
    }
}

/// The bytes of the UTF-8 character that starts with `c`.
fn utf8_width(c: u8) -> usize {
    match c {
        0xF0..=0xFF => 4,
        0xE0..=0xEF => 3,
        0xC0..=0xDF => 2,
        _ => 1,
    }
}

fn hex_run(s: &[u8], i: usize, n: usize) -> bool {
    s.get(i..i + n).is_some_and(|run| run.iter().all(u8::is_ascii_hexdigit))
}

/// Decodes a character literal's data as Python's `unicode-escape` codec does; `None`
/// when it gives other than one character (Sphinx refuses those).
pub(crate) fn char_value(data: &str) -> Option<u32> {
    let bytes = data.as_bytes();
    let (&first, rest) = bytes.split_first()?;
    if first != b'\\' {
        // UTF-8 bytes decode to several characters.
        return (first < 0x80 && rest.is_empty()).then_some(u32::from(first));
    }
    let (&kind, tail) = rest.split_first()?;
    let single = |v: u32| tail.is_empty().then_some(v);
    match kind {
        b'\'' => single(39),
        b'"' => single(34),
        b'\\' => single(92),
        b'a' => single(7),
        b'b' => single(8),
        b'f' => single(12),
        b'n' => single(10),
        b'r' => single(13),
        b't' => single(9),
        b'v' => single(11),
        b'x' | b'u' | b'U' => Some(
            tail.iter()
                .fold(0u32, |v, &c| v.wrapping_mul(16).wrapping_add(u32::from(hex_value(c)))),
        ),
        b'0'..=b'7' => {
            Some(rest.iter().fold(0u32, |v, &c| {
                v.wrapping_mul(8).wrapping_add(u32::from(c.wrapping_sub(b'0')))
            }))
        }
        _ => None, // "\?" stays two characters in Python.
    }
}

fn hex_value(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

/// Matches `_operator_re`; the first alternative that matches wins.
pub(crate) fn operator(s: &[u8], i: usize) -> Option<usize> {
    let c = *s.get(i)?;
    for (open, close) in [(b'[', b']'), (b'(', b')')] {
        if c != open {
            continue;
        }
        let mut k = i + 1;
        while k < s.len() && matches!(s[k], b' ' | b'\t' | b'\n' | b'\r') {
            k += 1;
        }
        if byte_at(s, k) == close {
            return Some(k + 1);
        }
    }
    for op in ["++", "--", "->*", "->", ",", "<<=", ">>=", "<<", ">>", "&&", "||", "<=>"] {
        if starts_at(s, i, op) {
            return Some(i + op.len());
        }
    }
    if b"!<>=/*%+|&^~-".contains(&c) {
        return Some(if byte_at(s, i + 1) == b'=' { i + 2 } else { i + 1 });
    }
    let words = [
        "and_eq", "and", "bitand", "bitor", "compl", "not_eq", "not", "or_eq", "or", "xor_eq",
        "xor",
    ];
    let after_word = i > 0 && is_word_byte(s[i - 1]);
    words
        .into_iter()
        .find(|w| !after_word && starts_at(s, i, w) && boundary(s, i + w.len()))
        .map(|w| i + w.len())
}

/// Matches `_fold_operator_re`.
pub(crate) fn fold_operator(s: &[u8], i: usize) -> Option<usize> {
    let c = *s.get(i)?;
    for op in ["->*", ".*", ",", "<<=", ">>=", "<<", ">>", "&&", "||", "!="] {
        if starts_at(s, i, op) {
            return Some(i + op.len());
        }
    }
    if b"<>=/*%+|&^~-".contains(&c) {
        return Some(if byte_at(s, i + 1) == b'=' { i + 2 } else { i + 1 });
    }
    None
}

/// Matches a string literal from `"`, with escapes, as `_parse_string` scans it.
pub(crate) fn string_literal(s: &[u8], i: usize) -> Option<usize> {
    if byte_at(s, i) != b'"' {
        return None;
    }
    let mut escape = false;
    for (k, &c) in s.iter().enumerate().skip(i + 1) {
        if c == b'"' && !escape {
            return Some(k + 1);
        }
        escape = c == b'\\'; // as _parse_string: a backslash escapes, even after one.
    }
    None
}

/// Matches `_string_re` for the fallback scanner: an optional `L`, `u`, `U`, or `8`,
/// then a quoted string.
pub(crate) fn fallback_string(s: &[u8], i: usize) -> Option<usize> {
    let mut k = i;
    if matches!(byte_at(s, k), b'L' | b'u' | b'U' | b'8') {
        k += 1;
    }
    if !matches!(byte_at(s, k), b'\'' | b'"') {
        if k == i {
            return None;
        }
        k = i;
        if !matches!(byte_at(s, k), b'\'' | b'"') {
            return None;
        }
    }
    let quote = s[k];
    k += 1;
    while k < s.len() {
        if s[k] == b'\\' {
            k += 2;
            continue;
        }
        if s[k] == quote {
            return Some(k + 1);
        }
        k += 1;
    }
    None
}

/// The C++ keywords no identifier may be.
const KEYWORDS: [&str; 89] = [
    "alignas",
    "alignof",
    "and",
    "and_eq",
    "asm",
    "auto",
    "bitand",
    "bitor",
    "bool",
    "break",
    "case",
    "catch",
    "class",
    "compl",
    "concept",
    "continue",
    "char",
    "char8_t",
    "char16_t",
    "char32_t",
    "const",
    "const_cast",
    "consteval",
    "constexpr",
    "constinit",
    "decltype",
    "default",
    "delete",
    "do",
    "double",
    "dynamic_cast",
    "else",
    "enum",
    "explicit",
    "export",
    "extern",
    "false",
    "float",
    "for",
    "friend",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "not",
    "not_eq",
    "nullptr",
    "operator",
    "or",
    "or_eq",
    "private",
    "protected",
    "public",
    "register",
    "reinterpret_cast",
    "requires",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "static_assert",
    "static_cast",
    "struct",
    "switch",
    "template",
    "this",
    "thread_local",
    "throw",
    "true",
    "try",
    "typedef",
    "typeid",
    "typename",
    "union",
    "unsigned",
    "using",
    "virtual",
    "void",
    "volatile",
    "wchar_t",
    "while",
    "xor",
    "xor_eq",
];

/// Tells whether a word is a C++ keyword.
pub(crate) fn is_keyword(word: &str) -> bool {
    KEYWORDS.contains(&word)
}

/// The words of `_simple_type_specifiers_re`.
pub(crate) const SIMPLE_TYPES: [&str; 23] = [
    "auto",
    "void",
    "bool",
    "signed",
    "unsigned",
    "short",
    "long",
    "char",
    "wchar_t",
    "char8_t",
    "char16_t",
    "char32_t",
    "int",
    "__int64",
    "__int128",
    "float",
    "double",
    "__float80",
    "_Float64x",
    "__float128",
    "_Float128",
    "_Complex",
    "_Imaginary",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_match_as_sphinx_patterns() {
        assert_eq!(float_literal(b"1.5e3f", 0), Some(5));
        assert_eq!(float_literal(b"1.", 0), Some(2));
        assert_eq!(float_literal(b"0x1p4", 0), Some(5));
        assert_eq!(float_literal(b"12", 0), None);
        assert_eq!(integer_literal(b"0b101", 0), Some(5));
        assert_eq!(integer_literal(b"1'000", 0), Some(5));
        assert_eq!(integer_literal(b"017", 0), Some(3));
        assert_eq!(integer_suffix(b"ull", 0), Some(3));
        assert_eq!(integer_suffix(b"LLu", 0), Some(3));
        assert_eq!(integer_suffix(b"ux", 0), None);
        assert_eq!(string_literal(br#""a\"b" x"#, 0), Some(6));
    }

    #[test]
    fn character_literals_decode_one_character() {
        let lit = char_literal("u8'\\n'", 0).map(|c| (c.end, c.prefix, c.data));
        assert_eq!(lit, Some((6, "u8", "\\n")));
        assert_eq!(char_value("\\n"), Some(10));
        assert_eq!(char_value("\\x41"), Some(65));
        assert_eq!(char_value("\\101"), Some(65));
        assert_eq!(char_value("\\?"), None);
        assert_eq!(char_value("ab"), None);
    }

    #[test]
    fn operators_match_the_first_alternative() {
        assert_eq!(operator(b"( )", 0), Some(3));
        assert_eq!(operator(b"->*", 0), Some(3));
        assert_eq!(operator(b"<=>", 0), Some(3));
        assert_eq!(operator(b"+=", 0), Some(2));
        assert_eq!(operator(b"and_eq", 0), Some(6));
        assert_eq!(operator(b"andx", 0), None);
        assert_eq!(fold_operator(b"!=", 0), Some(2));
        assert!(is_keyword("class") && !is_keyword("foo"));
    }
}
