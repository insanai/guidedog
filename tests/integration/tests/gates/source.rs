//! A small Rust tokenizer for the source gates: enough to tell code from comments and
//! string literals, so a gate that looks for `unsafe` or a `hint:` field is not fooled by
//! a comment or a string that merely mentions one.

use std::fs;
use std::path::{Path, PathBuf};

/// One token of Rust source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// An identifier or keyword.
    Ident(String),
    /// A string literal's contents, escapes as written (raw and byte strings included).
    Str(String),
    /// A line or block comment's text, without its markers.
    Comment(String),
    /// Any other character that is not white space (numbers come as single characters).
    Punct(char),
}

/// A token and the 1-based line it starts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    /// The token.
    pub token: Token,
    /// Its line.
    pub line: usize,
}

/// The tokens of `source`, comments included.
pub fn tokenize(source: &str) -> Vec<Located> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    while i < chars.len() {
        let c = chars[i];
        let start_line = line;
        let prefixed =
            if matches!(c, 'r' | 'b' | 'c') { raw_or_prefixed_string(&chars, i) } else { None };
        let (token, next) = match c {
            '\n' => {
                line += 1;
                i += 1;
                continue;
            }
            c if c.is_whitespace() => {
                i += 1;
                continue;
            }
            '/' if chars.get(i + 1) == Some(&'/') => line_comment(&chars, i),
            '/' if chars.get(i + 1) == Some(&'*') => block_comment(&chars, i),
            '"' => quoted(&chars, i + 1),
            _ if prefixed.is_some() => prefixed.unwrap_or((Token::Punct(c), i + 1)),
            '\'' => char_or_lifetime(&chars, i),
            c if c.is_alphabetic() || c == '_' => identifier(&chars, i),
            c => (Token::Punct(c), i + 1),
        };
        line += chars[i..next].iter().filter(|&&c| c == '\n').count();
        out.push(Located { token, line: start_line });
        i = next;
    }
    out
}

fn line_comment(chars: &[char], start: usize) -> (Token, usize) {
    let end = chars[start..].iter().position(|&c| c == '\n').map_or(chars.len(), |p| start + p);
    (Token::Comment(chars[start + 2..end].iter().collect()), end)
}

fn block_comment(chars: &[char], start: usize) -> (Token, usize) {
    let (mut depth, mut i) = (0usize, start);
    while i < chars.len() {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            depth += 1;
            i += 2;
        } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
            depth -= 1;
            i += 2;
            if depth == 0 {
                break;
            }
        } else {
            i += 1;
        }
    }
    let text_end = i.saturating_sub(2).max(start + 2);
    (Token::Comment(chars[start + 2..text_end].iter().collect()), i)
}

/// A string whose opening quote is just before `start`: its contents and the index after
/// the closing quote.
fn quoted(chars: &[char], start: usize) -> (Token, usize) {
    let mut i = start;
    while i < chars.len() && chars[i] != '"' {
        i += if chars[i] == '\\' { 2 } else { 1 };
    }
    let end = i.min(chars.len());
    (Token::Str(chars[start..end].iter().collect()), (end + 1).min(chars.len()))
}

/// `r"..."`, `r#"..."#`, `b"..."`, `br"..."`, `c"..."` at `start`, if one starts there
/// (and the letter does not end an identifier).
fn raw_or_prefixed_string(chars: &[char], start: usize) -> Option<(Token, usize)> {
    if start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        return None;
    }
    let mut i = start;
    while i < chars.len() && matches!(chars[i], 'r' | 'b' | 'c') && i - start < 2 {
        i += 1;
    }
    let raw = chars[start..i].contains(&'r');
    let hashes = chars[i..].iter().take_while(|&&c| c == '#').count();
    if !raw && hashes > 0 {
        return None;
    }
    if chars.get(i + hashes) != Some(&'"') {
        return None;
    }
    if !raw {
        return Some(quoted(chars, i + 1));
    }
    let body = i + hashes + 1;
    let mut j = body;
    while j < chars.len() {
        if chars[j] == '"'
            && chars[j + 1..].iter().take(hashes).filter(|&&c| c == '#').count() == hashes
        {
            return Some((Token::Str(chars[body..j].iter().collect()), j + 1 + hashes));
        }
        j += 1;
    }
    Some((Token::Str(chars[body..].iter().collect()), chars.len()))
}

/// A character literal (`'a'`, `'\n'`, `'\u{1F600}'`) or a lifetime (`'a`), as punctuation.
fn char_or_lifetime(chars: &[char], start: usize) -> (Token, usize) {
    if chars.get(start + 1) == Some(&'\\') {
        let close = chars[start + 2..].iter().position(|&c| c == '\'');
        return (Token::Punct('\''), close.map_or(start + 1, |p| start + 3 + p));
    }
    if chars.get(start + 2) == Some(&'\'') {
        return (Token::Punct('\''), start + 3);
    }
    // A lifetime or label: skip the quote; the name follows as an identifier.
    (Token::Punct('\''), start + 1)
}

fn identifier(chars: &[char], start: usize) -> (Token, usize) {
    let end = chars[start..]
        .iter()
        .position(|&c| !(c.is_alphanumeric() || c == '_'))
        .map_or(chars.len(), |p| start + p);
    (Token::Ident(chars[start..end].iter().collect()), end)
}

/// The Rust files below `dir`, skipping build output and fixture data.
pub fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect(dir, &mut out);
    out.sort();
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !matches!(name.as_ref(), "target" | "data" | ".git" | "_build" | "native") {
                collect(&path, out);
            }
        } else if name.ends_with(".rs") {
            out.push(path);
        }
    }
}

/// The workspace's own Rust sources: `crates`, `cmd`, `tools`, `examples`, `tests`, with
/// paths relative to the repository root and `/` separators.
pub fn workspace_sources() -> Vec<(String, String)> {
    let root = guidedog_tests::repository_root();
    let mut out = Vec::new();
    for top in ["crates", "cmd", "tools", "examples", "tests"] {
        for path in rust_files(&root.join(top)) {
            let rel = path.strip_prefix(&root).unwrap_or(&path).to_string_lossy();
            let text = fs::read_to_string(&path).unwrap_or_default();
            out.push((rel.replace('\\', "/"), text));
        }
    }
    out
}

#[test]
fn comments_strings_and_lifetimes_are_not_code() {
    let source = "// unsafe here\nfn f<'a>(x: &'a str) -> char {\n    let s = \"unsafe\";\n    \
        let r = r#\"un\"safe\"#;\n    /* unsafe /* nested */ */\n    'x'\n}\nunsafe { }\n";
    let tokens = tokenize(source);
    let unsafe_lines: Vec<usize> = tokens
        .iter()
        .filter(|t| t.token == Token::Ident("unsafe".into()))
        .map(|t| t.line)
        .collect();
    assert_eq!(unsafe_lines, [8]);
    let strings: Vec<&Token> =
        tokens.iter().map(|t| &t.token).filter(|t| matches!(t, Token::Str(_))).collect();
    assert_eq!(strings, [&Token::Str("unsafe".into()), &Token::Str("un\"safe".into())]);
    assert!(tokens.iter().any(|t| t.token == Token::Ident("str".into())));
}
