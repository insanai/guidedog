//! Every lexer on random input, odd characters and huge
//! lines must end, cover every byte exactly once, and give the same tokens every time.
//!
//! Input is a `&str`, so invalid byte sequences arrive as
//! U+FFFD (through `String::from_utf8_lossy`).

use super::*;
use crate::*;

/// A xorshift generator, so runs are reproducible.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Bytes that open, close or escape constructs somewhere, so random input reaches deep
/// states instead of staying in plain text.
const SPICY: &[u8] =
    b"\"'`/*#<>{}[]()$\\@-=:.!%&|\n\t ~^_+?;,rRbfxu0123456789aZ\xff\xc3\xa9\xe2\x82";

fn random_input(r: &mut Random, n: usize, spicy: bool) -> String {
    let bytes: Vec<u8> = (0..n)
        .map(|_| {
            let v = r.next();
            if spicy { SPICY[(v % SPICY.len() as u64) as usize] } else { v as u8 }
        })
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[track_caller]
fn check_input(lexer: Lexer, code: &str) {
    expect_coverage(lexer, code);
    assert_eq!(tokenize(lexer, code), tokenize(lexer, code), "{lexer:?}: runs differ");
}

#[test]
fn random_bytes_never_break_a_lexer() {
    let mut r = Random(0x9E37_79B9_7F4A_7C15);
    for lexer in Lexer::ALL {
        for round in 0..60 {
            let n = (r.next() % 400) as usize;
            let code = random_input(&mut r, n, round % 3 != 0);
            check_input(lexer, &code);
        }
    }
}

/// Openers that make lexers look ahead: repeated for a whole line, they must still cost
/// linear time.
pub(crate) const OPENERS: &[&str] = &[
    "\"", "'", "`", "/", "/*", "*", "#", "<", "<!--", "{", "[", "(", "$", "\\", "@", "-", "=",
    ":", "!", "%", "&", "|", ">", "\"\"\"", "'''", "<<EOF\n", "R\"(", "r#\"", "${", "{{", "f'{",
    "$(", "\\\n", ">>> ", "$ ", "- ", "[x](", "``", "**", ".. ", "#let ", "0x", "1.", "a'", "'a",
    "::",
];

#[test]
fn huge_lines_remain_complete_and_deterministic() {
    for opener in OPENERS {
        let code: String = opener.chars().cycle().take(64 * 1024).collect();
        for lexer in Lexer::ALL {
            check_input(lexer, &code);
        }
    }
}

#[test]
fn odd_characters_and_edges() {
    let inputs = [
        "",
        "\n",
        "\0",
        "\u{fffd}\u{fffd}",
        "\u{fffd}",
        "é",
        "€",
        "a\u{80}b",
        "\r\n\r\n",
        "\"\u{fffd}\"",
        "'é'",
        "#\u{fffd}",
        "<\u{fffd}>",
        "${\u{fffd}}",
        "\\",
        "\\é",
        "`€`",
        "'\u{1F600}'",
        "\\\u{1F600}",
        "&é;",
        "é:",
        "$é",
        "@é",
        "#é",
    ];
    for lexer in Lexer::ALL {
        for code in inputs {
            check_input(lexer, code);
        }
    }
}

#[test]
fn deep_nesting_is_bounded() {
    let patterns = ["f'{", "${", "$(", "{", "[", "(", "#{", "\"${", "\\(", "{{"];
    for p in patterns {
        let code: String = p.chars().cycle().take(8192).collect();
        for lexer in Lexer::ALL {
            check_input(lexer, &code);
        }
    }
}
