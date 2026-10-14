//! Unicode Normalization Form D, as Python's `unicodedata.normalize("NFD", s)` gives it:
//! every character is replaced by its full canonical decomposition, Hangul syllables are
//! split into their jamo, and runs of combining marks are put in canonical order. Hosts
//! use it where Sphinx does, such as sorting and grouping the general index.

use std::borrow::Cow;

use crate::tables::nfd_classes::NFD_CLASSES;
use crate::tables::nfd_decompositions::NFD_DECOMPOSITIONS;
use crate::tables::nfd_runes::NFD_RUNES;

const HANGUL_BASE: u32 = 0xAC00;
const HANGUL_COUNT: u32 = 11172;
const JAMO_L: u32 = 0x1100;
const JAMO_V: u32 = 0x1161;
const JAMO_T: u32 = 0x11A7;
const T_COUNT: u32 = 28;
/// Vowels (21) times trailing consonants (28).
const N_COUNT: u32 = 588;

/// A character's decomposition: up to three characters held inline (a Hangul syllable,
/// or a character without one), or a slice of the table.
enum Parts {
    Own([char; 3], usize),
    Table(&'static [u32]),
}

impl Parts {
    fn len(&self) -> usize {
        match self {
            Parts::Own(_, n) => *n,
            Parts::Table(t) => t.len(),
        }
    }

    fn get(&self, i: usize) -> char {
        match self {
            Parts::Own(chars, _) => chars[i],
            Parts::Table(t) => char::from_u32(t[i]).unwrap_or(char::REPLACEMENT_CHARACTER),
        }
    }
}

/// Returns the full canonical decomposition of `r`.
fn decompose(r: char) -> Parts {
    let c = r as u32;
    if (HANGUL_BASE..HANGUL_BASE + HANGUL_COUNT).contains(&c) {
        let index = c - HANGUL_BASE;
        let jamo = |v: u32| char::from_u32(v).unwrap_or(char::REPLACEMENT_CHARACTER);
        let parts = [
            jamo(JAMO_L + index / N_COUNT),
            jamo(JAMO_V + (index % N_COUNT) / T_COUNT),
            jamo(JAMO_T + index % T_COUNT),
        ];
        // A syllable without a trailing consonant decomposes into two jamo.
        return Parts::Own(parts, if index.is_multiple_of(T_COUNT) { 2 } else { 3 });
    }
    match NFD_DECOMPOSITIONS.binary_search_by_key(&c, |row| row.0) {
        Ok(i) => {
            let (_, start, length) = NFD_DECOMPOSITIONS[i];
            let start = usize::from(start);
            Parts::Table(&NFD_RUNES[start..start + usize::from(length)])
        }
        Err(_) => Parts::Own([r, '\0', '\0'], 1),
    }
}

/// Returns a character's canonical combining class; 0 for most characters.
fn combining_class(r: char) -> u8 {
    let c = r as u32;
    if c < 0x300 {
        return 0;
    }
    match NFD_CLASSES.binary_search_by_key(&c, |row| row.0) {
        Ok(i) => NFD_CLASSES[i].1,
        Err(_) => 0,
    }
}

/// Appends `s` in Normalization Form D to `out`, reusing `out`'s storage. Combining
/// marks of a run are collected in a small buffer and reordered there.
pub fn normalize_nfd_into(out: &mut String, s: &str) {
    let mut marks: Vec<(u8, char)> = Vec::new();
    for r in s.chars() {
        let parts = decompose(r);
        for i in 0..parts.len() {
            let c = parts.get(i);
            let class = combining_class(c);
            if class == 0 {
                flush_marks(out, &mut marks);
                out.push(c);
            } else {
                marks.push((class, c));
            }
        }
    }
    flush_marks(out, &mut marks);
}

/// Writes a run of combining marks in canonical order: by class, keeping the order of
/// marks of one class (a stable sort).
fn flush_marks(out: &mut String, marks: &mut Vec<(u8, char)>) {
    marks.sort_by_key(|&(class, _)| class);
    out.extend(marks.iter().map(|&(_, c)| c));
    marks.clear();
}

/// Returns `s` in Normalization Form D, borrowing it when it is already in that form
/// (all ASCII, the common case for index keys).
pub fn normalize_nfd(s: &str) -> Cow<'_, str> {
    if s.is_ascii() {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + s.len() / 2);
    normalize_nfd_into(&mut out, s);
    if out == s { Cow::Borrowed(s) } else { Cow::Owned(out) }
}

/// Returns the first character of `s` in NFD, as Sphinx takes an index entry's letter:
/// `unicodedata.normalize("NFD", s[0])[0]`. An empty string gives U+FFFD.
pub fn nfd_first(s: &str) -> char {
    match s.chars().next() {
        Some(r) => decompose(r).get(0),
        None => char::REPLACEMENT_CHARACTER,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected forms are Python's `unicodedata.normalize("NFD", s)`.
    #[test]
    fn nfd_matches_python() {
        let cases = [
            ("plain ascii", "plain ascii"),
            ("\u{e9}lan", "e\u{301}lan"),
            ("\u{c5}ngstr\u{f6}m", "A\u{30a}ngstro\u{308}m"),
            ("\u{1ec7}", "e\u{323}\u{302}"),
            ("e\u{302}\u{323}", "e\u{323}\u{302}"),
            ("\u{d55c}", "\u{1112}\u{1161}\u{11ab}"),
            ("\u{ac00}", "\u{1100}\u{1161}"),
            ("\u{1e9b}\u{323}", "\u{17f}\u{323}\u{307}"),
            ("\u{fb01}", "\u{fb01}"),
        ];
        for (input, want) in cases {
            let mut out = String::from("x");
            normalize_nfd_into(&mut out, input);
            assert_eq!(&out[1..], want, "{input:?}");
            assert_eq!(normalize_nfd(input), want);
        }
        assert!(matches!(normalize_nfd("abc"), Cow::Borrowed(_)));
        assert_eq!(nfd_first("\u{c9}lan"), 'E');
        assert_eq!(nfd_first("\u{d55c}"), '\u{1112}');
        assert_eq!(nfd_first("_x"), '_');
    }
}
