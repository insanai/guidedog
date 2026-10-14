//! Data languages: JSON, YAML, TOML and INI.

use crate::scanner::{Scanner, State, has, is_ident_char, is_space};
use crate::token::TokenKind as K;

/// JSON, as Pygments' JSON lexer tokenizes it: object keys are `Name.Tag`, and anything
/// that is not JSON (a bare word, a stray character, an unterminated string) is `Error`.
pub(crate) fn json_step(s: &mut Scanner<'_>, _st: &mut State) {
    let c = s.peek(0);
    match c {
        b' ' | b'\t' | b'\n' | b'\r' => s.emit(s.skip_set(s.pos, " \t\n\r"), K::Whitespace),
        b'{' | b'}' | b'[' | b']' | b',' | b':' => s.emit_n(1, K::Punctuation),
        b'"' => json_string(s),
        b'-' | b'0'..=b'9' => {
            let end = s.skip_set(s.pos, "-+0123456789.eE");
            let float = s.slice(s.pos, end).iter().any(|&b| has(".eE", b));
            s.emit(end, if float { K::NumberFloat } else { K::NumberInteger });
        }
        b'/' if s.peek(1) == b'/' => s.emit_line(K::CommentSingle),
        b'/' if s.peek(1) == b'*' => {
            s.emit(s.block_comment_end("/*", "*/", false), K::CommentMultiline)
        }
        b'/' => s.emit_n(1, K::Error),
        _ if c.is_ascii_alphabetic() => {
            let end = s.skip_by(s.pos, |b| b.is_ascii_alphabetic());
            let word = s.slice(s.pos, end);
            let constant = word == b"true" || word == b"false" || word == b"null";
            s.emit(end, if constant { K::KeywordConstant } else { K::Error });
        }
        _ => s.emit_rune(K::Error),
    }
}

fn json_string(s: &mut Scanner<'_>) {
    let mut i = s.pos + 1;
    while i < s.limit && s.bytes[i] != b'"' {
        i += if s.bytes[i] == b'\\' { 2 } else { 1 };
    }
    if i >= s.limit {
        s.emit(s.limit, K::Error);
        return;
    }
    let after = s.skip_set(i + 1, " \t\n\r");
    s.emit(i + 1, if s.byte_at(after) == b':' { K::NameTag } else { K::StringDouble });
}

// YAML, after Pygments' YAML lexer: keys are Name.Tag, plain scalars Literal.Scalar.Plain,
// indicators Punctuation.Indicator.

/// `st.mode`: the lines of a `|` or `>` block; `st.count` is its indent.
const YAML_BLOCK_SCALAR: u8 = 1;

pub(crate) fn yaml_step(s: &mut Scanner<'_>, st: &mut State) {
    let c = s.peek(0);
    if s.at_line_start() {
        // The line's indent, which a block scalar starting on it measures against.
        st.mark[0] = s.skip_set(s.pos, " ") - s.pos;
    }
    if st.mode == YAML_BLOCK_SCALAR && s.at_line_start() && yaml_block_line(s, st) {
        return;
    }
    if c == b'\n' || c == b'\r' {
        s.emit_n(1, K::Whitespace);
        return;
    }
    if c == b' ' || c == b'\t' {
        s.emit(s.skip_spaces(s.pos), K::Whitespace);
        return;
    }
    if c == b'#' && (s.pos == 0 || is_space(s.before())) {
        s.emit_line(K::CommentSingle);
        return;
    }
    if s.at_line_start() {
        if (s.looking_at("---", 0) || s.looking_at("...", 0)) && is_space_or_end(s, s.pos + 3) {
            s.emit_n(3, K::NameNamespace);
            return;
        }
        if c == b'%' {
            s.emit_line(K::NameTag);
            return;
        }
    }
    yaml_node(s, st, c);
}

fn is_space_or_end(s: &Scanner<'_>, at: usize) -> bool {
    at >= s.limit || is_space(s.bytes[at])
}

fn yaml_node(s: &mut Scanner<'_>, st: &mut State, c: u8) {
    let flow = st.depth > 0;
    match c {
        b'-' | b'?' | b':' if is_space_or_end(s, s.pos + 1) => {
            s.emit_n(1, if c == b':' { K::Punctuation } else { K::PunctuationIndicator });
        }
        b'[' | b'{' => {
            st.push(1, 0, 0);
            s.emit_n(1, K::PunctuationIndicator);
        }
        b']' | b'}' => {
            st.pop();
            s.emit_n(1, K::PunctuationIndicator);
        }
        b',' if flow => s.emit_n(1, K::PunctuationIndicator),
        b'&' | b'*' => {
            let end = s.skip_by(s.pos + 1, |b| !is_space(b) && !has(",[]{}", b));
            s.emit(end, if c == b'&' { K::NameLabel } else { K::NameVariable });
        }
        b'!' => s.emit(s.skip_by(s.pos + 1, |b| !is_space(b)), K::KeywordType),
        b'|' | b'>' if !flow => yaml_block_start(s, st),
        b'"' | b'\'' => yaml_quoted(s, c),
        _ => yaml_plain(s, flow),
    }
}

fn yaml_quoted(s: &mut Scanner<'_>, q: u8) {
    let mut i = s.pos + 1;
    while i < s.limit {
        if s.bytes[i] == b'\\' && q == b'"' {
            i += 2;
            continue;
        }
        if s.bytes[i] == q {
            if q == b'\'' && s.byte_at(i + 1) == b'\'' {
                i += 2;
                continue;
            }
            break;
        }
        i += 1;
    }
    let end = (i + 1).min(s.limit);
    let after = s.skip_spaces(end);
    let key = s.byte_at(after) == b':' && is_space_or_end(s, after + 1);
    s.emit(end, if key { K::NameTag } else { K::String });
}

/// Lexes a key (`name:`) or a plain scalar up to a comment or the line's end.
fn yaml_plain(s: &mut Scanner<'_>, flow: bool) {
    let stop = s.line_end(s.pos);
    let mut i = s.pos;
    while i < stop {
        let b = s.bytes[i];
        let key_end =
            i + 1 >= stop || is_space(s.bytes[i + 1]) || (flow && has(",[]{}", s.bytes[i + 1]));
        if b == b':' && key_end {
            if i > s.pos {
                s.emit(i, K::NameTag);
                return;
            }
            break;
        }
        if b == b'#' && i > s.pos && is_space(s.bytes[i - 1]) {
            break;
        }
        if flow && has(",[]{}", b) {
            break;
        }
        i += 1;
    }
    let mut end = i;
    while end > s.pos && is_space(s.bytes[end - 1]) {
        end -= 1;
    }
    if end == s.pos {
        s.emit_rune(K::LiteralScalarPlain);
        return;
    }
    s.emit(end, K::LiteralScalarPlain);
}

fn yaml_block_start(s: &mut Scanner<'_>, st: &mut State) {
    let end = s.skip_set(s.pos + 1, "+-0123456789");
    s.emit(end, K::PunctuationIndicator);
    st.mode = YAML_BLOCK_SCALAR;
    st.count = st.mark[0] as u32;
}

/// Lexes a line of a block scalar; false when the block has ended.
fn yaml_block_line(s: &mut Scanner<'_>, st: &mut State) -> bool {
    let indent = s.skip_set(s.pos, " ");
    let stop = s.line_end(s.pos);
    let blank = indent >= stop || (indent == stop - 1 && s.bytes[indent] == b'\r');
    if !blank && indent - s.pos <= st.count as usize {
        st.mode = 0;
        return false;
    }
    s.emit(indent, K::Whitespace);
    s.emit(stop, K::NameConstant);
    s.emit(s.line_after(s.pos), K::Whitespace);
    true
}

// TOML: tables are Keyword, keys Name, values strings, numbers, dates and constants.

/// `st.mode`: after `=`, until the line (or inline-table entry) ends.
const TOML_VALUE: u8 = 1;
const FRAME_ARRAY: u8 = 1;
const FRAME_TABLE: u8 = 2;
/// A `"""` string.
const FRAME_BASIC: u8 = 3;
/// A `'''` string.
const FRAME_LITERAL: u8 = 4;

pub(crate) fn toml_step(s: &mut Scanner<'_>, st: &mut State) {
    if let Some(f) = st.top().filter(|f| f.mode == FRAME_BASIC || f.mode == FRAME_LITERAL) {
        return toml_multiline(s, st, f.mode == FRAME_BASIC);
    }
    let c = s.peek(0);
    if c == b'\n' {
        if st.depth == 0 {
            st.mode = 0;
        }
        s.emit_n(1, K::Whitespace);
    } else if c == b' ' || c == b'\t' || c == b'\r' {
        s.emit(s.skip_set(s.pos, " \t\r"), K::Whitespace);
    } else if c == b'#' {
        s.emit_line(K::CommentSingle);
    } else if c == b'[' && st.mode == 0 && st.depth == 0 {
        let close = s.find_on_line(s.pos, "]");
        let mut end = close.map_or_else(|| s.line_end(s.pos), |c| c + 1);
        if s.byte_at(end) == b']' {
            end += 1;
        }
        s.emit(end, K::Keyword);
    } else if st.mode == 0 {
        toml_key(s, st, c);
    } else {
        toml_value(s, st, c);
    }
}

fn toml_key(s: &mut Scanner<'_>, st: &mut State, c: u8) {
    if c == b'=' {
        s.emit_n(1, K::Operator);
        st.mode = TOML_VALUE;
    } else if c == b'.' {
        s.emit_n(1, K::Punctuation);
    } else if c == b'"' || c == b'\'' {
        let close = s.find_on_line(s.pos + 1, if c == b'"' { "\"" } else { "'" });
        s.emit(close.map_or_else(|| s.line_end(s.pos), |c| c + 1), K::Name);
    } else if c == b'}' {
        st.pop();
        s.emit_n(1, K::Punctuation);
        st.mode = TOML_VALUE;
    } else if is_ident_char(c) || c == b'-' {
        s.emit(s.ident_end(s.pos, "-"), K::Name);
    } else {
        s.emit_rune(K::Error);
    }
}

fn toml_value(s: &mut Scanner<'_>, st: &mut State, c: u8) {
    if c == b'[' || c == b'{' {
        st.push(if c == b'[' { FRAME_ARRAY } else { FRAME_TABLE }, 0, 0);
        s.emit_n(1, K::Punctuation);
        if c == b'{' {
            st.mode = 0;
        }
    } else if c == b']' || c == b'}' {
        st.pop();
        s.emit_n(1, K::Punctuation);
    } else if c == b',' {
        s.emit_n(1, K::Punctuation);
        if st.top().is_some_and(|f| f.mode == FRAME_TABLE) {
            st.mode = 0;
        }
    } else if s.looking_at("\"\"\"", 0) || s.looking_at("'''", 0) {
        let basic = c == b'"';
        s.emit_n(3, if basic { K::StringDouble } else { K::StringSingle });
        st.push(if basic { FRAME_BASIC } else { FRAME_LITERAL }, 0, 0);
    } else if c == b'"' {
        toml_basic(s);
    } else if c == b'\'' {
        let close = s.find_on_line(s.pos + 1, "'");
        s.emit(close.map_or_else(|| s.line_end(s.pos), |c| c + 1), K::StringSingle);
    } else if c.is_ascii_digit() || c == b'+' || c == b'-' {
        toml_number(s);
    } else if c.is_ascii_alphabetic() {
        let end = s.ident_end(s.pos, "");
        let kind = match s.slice(s.pos, end) {
            b"true" | b"false" => K::KeywordConstant,
            b"inf" | b"nan" => K::NumberFloat,
            _ => K::Error,
        };
        s.emit(end, kind);
    } else {
        s.emit_rune(K::Error);
    }
}

fn toml_basic(s: &mut Scanner<'_>) {
    s.emit_n(1, K::StringDouble);
    while s.pos < s.limit {
        let c = s.peek(0);
        if c == b'"' {
            s.emit_n(1, K::StringDouble);
            return;
        }
        if c == b'\n' {
            return;
        }
        if c == b'\\' {
            s.emit_n(s.escape_length(s.pos), K::StringEscape);
            return;
        }
        let i = s.skip_by(s.pos, |b| !has("\"\\\n", b));
        s.emit(i, K::StringDouble);
        if s.byte_at(i) == b'\\' {
            return;
        }
    }
}

fn toml_multiline(s: &mut Scanner<'_>, st: &mut State, basic: bool) {
    let kind = if basic { K::StringDouble } else { K::StringSingle };
    let close = if basic { "\"\"\"" } else { "'''" };
    if s.looking_at(close, 0) {
        s.emit_n(3, kind);
        st.pop();
        return;
    }
    if basic && s.peek(0) == b'\\' {
        s.emit_n(s.escape_length(s.pos), K::StringEscape);
        return;
    }
    let quote = close.as_bytes()[0];
    let i = s.skip_by(s.pos + 1, |b| b != quote && !(basic && b == b'\\'));
    s.emit(i, kind);
}

fn toml_number(s: &mut Scanner<'_>) {
    let number_byte = |b: u8| is_ident_char(b) || has("+-.:", b);
    let mut end = s.skip_by(s.pos, number_byte);
    // A space between a date and a time: 1979-05-27 07:32:00.
    if end + 3 <= s.limit
        && s.bytes[end] == b' '
        && s.bytes[end + 1].is_ascii_digit()
        && end - s.pos == 10
        && s.bytes[s.pos + 4] == b'-'
    {
        end = s.skip_by(end + 1, number_byte);
    }
    s.emit(end, toml_number_kind(s.slice(s.pos, end)));
}

fn toml_number_kind(text: &[u8]) -> K {
    if text.len() >= 8 && (text[2] == b':' || (text.len() > 4 && text[4] == b'-')) {
        K::LiteralDate
    } else if text.starts_with(b"0x") {
        K::NumberHex
    } else if text.starts_with(b"0o") {
        K::NumberOct
    } else if text.starts_with(b"0b") {
        K::NumberBin
    } else if text.iter().any(|&b| has(".eE", b))
        || text.windows(3).any(|w| w == b"inf" || w == b"nan")
    {
        K::NumberFloat
    } else {
        K::NumberInteger
    }
}

/// INI and similar configuration files.
pub(crate) fn ini_step(s: &mut Scanner<'_>, _st: &mut State) {
    let c = s.peek(0);
    if is_space(c) {
        s.emit_spaces(K::Whitespace);
        return;
    }
    let stop = s.line_end(s.pos);
    if c == b';' || c == b'#' {
        s.emit(stop, K::CommentSingle);
        return;
    }
    if c == b'[' {
        s.emit(stop, K::Keyword);
        return;
    }
    if !s.at_line_start() && s.before() != b' ' && s.before() != b'\t' {
        s.emit(stop, K::String);
        return;
    }
    let line = s.slice(s.pos, stop);
    let Some(op) = line.iter().position(|&b| b == b'=' || b == b':').map(|i| s.pos + i) else {
        let kind = if s.only_space_before_line() { K::NameAttribute } else { K::String };
        s.emit(stop, kind);
        return;
    };
    let mut key_end = op;
    while key_end > s.pos && (s.bytes[key_end - 1] == b' ' || s.bytes[key_end - 1] == b'\t') {
        key_end -= 1;
    }
    s.emit(key_end, K::NameAttribute);
    s.emit(op, K::Whitespace);
    s.emit_n(1, K::Operator);
    s.emit(s.skip_spaces(s.pos), K::Whitespace);
    let rest = s.slice(s.pos, stop);
    let value = rest.iter().position(|&b| b == b';' || b == b'#').map_or(stop, |i| s.pos + i);
    s.emit(value, K::String);
}
