//! A small Rust tokenizer that finds function bodies and counts their logical lines.
//!
//! A logical line is a source line holding at least one token other than a bracket,
//! brace, comma or semicolon. Comments, string contents spanning lines, and the
//! signature do not count.

/// One function found in a source file.
#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub line: usize,
    pub logic_lines: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Ident(String),
    Punct(char),
    Literal,
}

#[derive(Debug)]
struct Token {
    kind: Kind,
    line: usize,
}

/// Returns every `fn` with a body in `source`, with its logical line count.
pub fn functions(source: &str) -> Vec<Function> {
    let tokens = tokenize(source);
    let mut found = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        if t.kind != Kind::Ident("fn".into()) {
            continue;
        }
        let Some(Token { kind: Kind::Ident(name), .. }) = tokens.get(i + 1) else { continue };
        if let Some((first, last)) = body(&tokens, i + 2) {
            let logic_lines = logic_lines(&tokens[first..last]);
            found.push(Function { name: name.clone(), line: t.line, logic_lines });
        }
    }
    found
}

/// Finds the token range inside the body braces, or `None` for a declaration.
fn body(tokens: &[Token], start: usize) -> Option<(usize, usize)> {
    let (mut parens, mut brackets, mut angles) = (0i32, 0i32, 0i32);
    for i in start..tokens.len() {
        match tokens[i].kind {
            Kind::Punct('(') => parens += 1,
            Kind::Punct(')') => parens -= 1,
            Kind::Punct('[') => brackets += 1,
            Kind::Punct(']') => brackets -= 1,
            Kind::Punct('<') => angles += 1,
            Kind::Punct('>') => angles -= 1,
            Kind::Punct(';') if parens == 0 && brackets == 0 => return None,
            Kind::Punct('{') if parens == 0 && brackets == 0 && angles <= 0 => {
                return matching_brace(tokens, i).map(|end| (i + 1, end));
            }
            _ => {}
        }
    }
    None
}

fn matching_brace(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0;
    for (j, t) in tokens.iter().enumerate().skip(open) {
        match t.kind {
            Kind::Punct('{') => depth += 1,
            Kind::Punct('}') => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

fn logic_lines(tokens: &[Token]) -> usize {
    let mut last_line = 0;
    let mut count = 0;
    for t in tokens {
        if matches!(t.kind, Kind::Punct('{' | '}' | '(' | ')' | '[' | ']' | ',' | ';')) {
            continue;
        }
        if t.line != last_line {
            count += 1;
            last_line = t.line;
        }
    }
    count
}

fn tokenize(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let (mut i, mut line) = (0usize, 1usize);
    while i < bytes.len() {
        let c = bytes[i];
        let start_line = line;
        let next = bytes.get(i + 1).copied().unwrap_or(0);
        if c == b'\n' {
            line += 1;
            i += 1;
        } else if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'/' && next == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && next == b'*' {
            i = skip_block_comment(bytes, i, &mut line);
        } else if c == b'"' {
            i = skip_string(bytes, i + 1, &mut line);
            tokens.push(Token { kind: Kind::Literal, line: start_line });
        } else if let Some(end) = raw_string_end(bytes, i, &mut line) {
            i = end;
            tokens.push(Token { kind: Kind::Literal, line: start_line });
        } else if c == b'\'' {
            i = skip_char_or_lifetime(bytes, i);
            tokens.push(Token { kind: Kind::Literal, line: start_line });
        } else if c == b'_' || c.is_ascii_alphanumeric() || c >= 0x80 {
            let start = i;
            while i < bytes.len()
                && (bytes[i] == b'_' || bytes[i].is_ascii_alphanumeric() || bytes[i] >= 0x80)
            {
                i += 1;
            }
            let word = String::from_utf8_lossy(&bytes[start..i]).into_owned();
            tokens.push(Token { kind: Kind::Ident(word), line: start_line });
        } else {
            tokens.push(Token { kind: Kind::Punct(c as char), line: start_line });
            i += 1;
        }
    }
    tokens
}

fn skip_block_comment(bytes: &[u8], mut i: usize, line: &mut usize) -> usize {
    let mut depth = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            depth += 1;
            i += 2;
        } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return i;
            }
        } else {
            if bytes[i] == b'\n' {
                *line += 1;
            }
            i += 1;
        }
    }
    i
}

fn skip_string(bytes: &[u8], mut i: usize, line: &mut usize) -> usize {
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            b'\n' => {
                *line += 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    i
}

/// Recognizes `r"..."`, `r#"..."#`, `br"..."` and `b"..."`; returns the end offset.
fn raw_string_end(bytes: &[u8], start: usize, line: &mut usize) -> Option<usize> {
    let mut i = start;
    if bytes[i] == b'b' || bytes[i] == b'c' {
        i += 1;
        if bytes.get(i) == Some(&b'"') {
            return Some(skip_string(bytes, i + 1, line));
        }
    }
    if bytes.get(i) != Some(&b'r') {
        return None;
    }
    if start > 0 && (bytes[start - 1] == b'_' || bytes[start - 1].is_ascii_alphanumeric()) {
        return None;
    }
    i += 1;
    let mut hashes = 0;
    while bytes.get(i) == Some(&b'#') {
        hashes += 1;
        i += 1;
    }
    if bytes.get(i) != Some(&b'"') {
        return None;
    }
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            *line += 1;
        }
        if bytes[i] == b'"'
            && bytes[i + 1..].iter().take(hashes).filter(|&&b| b == b'#').count() == hashes
            && bytes.len() >= i + 1 + hashes
        {
            return Some(i + 1 + hashes);
        }
        i += 1;
    }
    Some(i)
}

fn skip_char_or_lifetime(bytes: &[u8], i: usize) -> usize {
    // 'a' or '\n' or '\u{..}' is a char; 'a (no closing quote soon) is a lifetime.
    if bytes.get(i + 1) == Some(&b'\\') {
        let mut j = i + 2;
        while j < bytes.len() && bytes[j] != b'\'' && j < i + 12 {
            j += 1;
        }
        return j + 1;
    }
    let mut j = i + 1;
    // Skip one UTF-8 character.
    if j < bytes.len() {
        j += 1;
        while j < bytes.len() && (bytes[j] & 0xC0) == 0x80 {
            j += 1;
        }
    }
    if bytes.get(j) == Some(&b'\'') {
        return j + 1;
    }
    // A lifetime or label: consume the identifier.
    let mut k = i + 1;
    while k < bytes.len() && (bytes[k] == b'_' || bytes[k].is_ascii_alphanumeric()) {
        k += 1;
    }
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_logic_lines_but_not_brackets_or_comments() {
        let src = "fn a(x: u8) -> u8 {\n    // note\n    let y = x;\n    if y > 1 {\n        \
                   return 2;\n    }\n    y\n}\n";
        let f = functions(src);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].name, "a");
        assert_eq!(f[0].logic_lines, 4);
    }

    #[test]
    fn declarations_without_bodies_are_skipped() {
        let src = "trait T { fn a(&self); fn b<'x>(&'x self) -> &'x str { \"}\" } }";
        let f = functions(src);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].name, "b");
    }
}
