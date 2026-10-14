//! Typesetting languages: LaTeX, after Pygments' TeX lexer, and Typst, this library's own.

use crate::lexers::find_lexer;
use crate::scanner::{Scanner, State, has, in_list, is_ident_start, is_space};
use crate::token::TokenKind as K;

// LaTeX: commands are Keyword, math is String.

const TEX_INLINE_MATH: u8 = 1;
const TEX_DISPLAY_MATH: u8 = 2;

pub(crate) fn latex_step(s: &mut Scanner<'_>, st: &mut State) {
    let c = s.peek(0);
    let math = st.mode != 0;
    if math && tex_math_end(s, st, c) {
        return;
    }
    if !math && tex_math_start(s, st, c) {
        return;
    }
    if c == b'%' {
        s.emit(s.line_after(s.pos), K::Comment);
    } else if c == b'\\' {
        tex_command(s, math);
    } else if has("{}&_^", c) {
        s.emit_n(1, K::NameBuiltin);
    } else if math && c.is_ascii_digit() {
        s.emit(s.skip_digits(s.pos), K::Number);
    } else if math && has("-=!+*/()[]", c) {
        s.emit_n(1, K::Operator);
    } else if math {
        let i = s.skip_by(s.pos + 1, |b| !has("=!+*/()[]\\$%&_^{}0123456789-", b));
        s.emit(i, K::NameBuiltin);
    } else {
        let i = s.skip_by(s.pos + 1, |b| !has("\\$%&_^{}", b));
        s.emit(i, K::Text);
    }
}

/// Ends math at `\)`, `\]`, `$` or `$$`; true when it did.
fn tex_math_end(s: &mut Scanner<'_>, st: &mut State, c: u8) -> bool {
    let inline = st.mode == TEX_INLINE_MATH;
    let close = if inline { "$" } else { "$$" };
    if !(s.looking_at("\\)", 0) || s.looking_at("\\]", 0) || s.looking_at(close, 0)) {
        return false;
    }
    let width = if c == b'\\' { 2 } else { close.len() };
    s.emit_n(width, if inline { K::String } else { K::StringBacktick });
    st.mode = 0;
    true
}

/// Starts math at `\[`, `$$`, `\(` or `$`; true when it did.
fn tex_math_start(s: &mut Scanner<'_>, st: &mut State, c: u8) -> bool {
    if s.looking_at("\\[", 0) || s.looking_at("$$", 0) {
        s.emit_n(2, K::StringBacktick);
        st.mode = TEX_DISPLAY_MATH;
        return true;
    }
    if s.looking_at("\\(", 0) || c == b'$' {
        s.emit_n(if c == b'\\' { 2 } else { 1 }, K::String);
        st.mode = TEX_INLINE_MATH;
        return true;
    }
    false
}

fn tex_command(s: &mut Scanner<'_>, math: bool) {
    let mut i = s.skip_by(s.pos + 1, |c| c.is_ascii_alphabetic() || (!math && has("@_:", c)));
    if i == s.pos + 1 && i < s.limit && !is_space(s.bytes[i]) {
        i += s.rune_size(i);
    }
    s.emit(i, if math { K::NameVariable } else { K::Keyword });
    if math {
        return;
    }
    if s.peek(0) == b'*' {
        s.emit_n(1, K::Keyword);
    }
    if s.peek(0) == b'['
        && let Some(close) = s.find_on_line(s.pos, "]")
    {
        s.emit(close + 1, K::NameAttribute);
    }
}

// Typst: markup with headings, strong and emphasis, raw text and math, and embedded code
// after "#" with its keywords, calls, strings and numbers.

/// `[ markup ]` inside code.
const FRAME_CONTENT: u8 = 1;
/// `{ code }` or `#{ code }`.
const FRAME_CODE: u8 = 2;
/// `( code )`.
const FRAME_ARGS: u8 = 3;
/// `$ math $`.
const FRAME_MATH: u8 = 4;

/// `st.flags`: a #expression may continue with `(`, `[` or `.name`.
const TYPST_EXPRESSION: u32 = 1;

const TYPST_KEYWORDS: &[&str] = &[
    "if", "else", "for", "in", "while", "break", "continue", "return", "and", "or", "not", "as",
    "context",
];

pub(crate) fn typst_step(s: &mut Scanner<'_>, st: &mut State) {
    let mode = st.top().map_or(FRAME_CONTENT, |f| f.mode);
    let after_colon = s.pos > 0 && s.before() == b':';
    if s.looking_at("//", 0) && (mode != FRAME_CONTENT || !after_colon) {
        s.emit_line(K::CommentSingle);
        return;
    }
    if s.looking_at("/*", 0) {
        s.emit(s.block_comment_end("/*", "*/", true), K::CommentMultiline);
        return;
    }
    match mode {
        FRAME_CODE | FRAME_ARGS => typst_code(s, st, mode),
        FRAME_MATH => typst_math(s, st),
        _ => {
            if st.flags & TYPST_EXPRESSION != 0 && typst_continue(s, st) {
                return;
            }
            st.flags &= !TYPST_EXPRESSION;
            typst_markup(s, st);
        }
    }
}

/// Continues a #expression with arguments, content or a field.
fn typst_continue(s: &mut Scanner<'_>, st: &mut State) -> bool {
    let c = s.peek(0);
    if c == b'(' {
        s.emit_n(1, K::Punctuation);
        st.push(FRAME_ARGS, 0, 0);
    } else if c == b'[' {
        s.emit_n(1, K::Punctuation);
        st.push(FRAME_CONTENT, 0, 0);
    } else if c == b'.' && is_ident_start(s.peek(1)) {
        s.emit_n(1, K::Punctuation);
        let end = s.ident_end(s.pos, "-");
        s.emit(end, if s.byte_at(end) == b'(' { K::NameFunction } else { K::Name });
    } else {
        return false;
    }
    true
}

fn typst_markup(s: &mut Scanner<'_>, st: &mut State) {
    let c = s.peek(0);
    if (s.at_line_start() || s.only_space_before_line()) && typst_line_marker(s, c) {
        return;
    }
    if typst_markup_construct(s, st, c) {
        return;
    }
    let mut i = s.skip_by(s.pos + 1, |b| !has("#$`]\\*_<@/\n", b));
    if i < s.limit && s.bytes[i] == b'\n' {
        i += 1;
    }
    s.emit(i, K::Text);
}

/// Lexes what starts a line: headings and list markers.
fn typst_line_marker(s: &mut Scanner<'_>, c: u8) -> bool {
    if c == b'=' {
        s.emit_line(K::GenericHeading);
        return true;
    }
    let next = s.peek(1);
    let list = (c == b'-' || c == b'+') && (next == b' ' || next == b'\t');
    if list || (c == b'/' && next == b' ') {
        s.emit_n(1, K::Punctuation);
        return true;
    }
    false
}

/// Lexes code, math, raw text, escapes, strong and emphasis, labels and references; false
/// when none starts at `pos`.
fn typst_markup_construct(s: &mut Scanner<'_>, st: &mut State, c: u8) -> bool {
    match c {
        b'#' => typst_hash(s, st),
        b'$' => {
            s.emit_n(1, K::Punctuation);
            st.push(FRAME_MATH, 0, 0);
        }
        b'`' => typst_raw(s),
        b']' if st.depth > 0 => {
            s.emit_n(1, K::Punctuation);
            st.pop();
            st.flags |= TYPST_EXPRESSION;
        }
        b'\\' => s.emit(s.pos + 1 + s.rune_size(s.pos + 1), K::StringEscape),
        b'*' | b'_' => {
            let marker = if c == b'*' { "*" } else { "_" };
            match s.find_on_line(s.pos + 1, marker) {
                Some(close) if close > s.pos + 1 => {
                    let kind = if c == b'*' { K::GenericStrong } else { K::GenericEmph };
                    s.emit(close + 1, kind);
                }
                _ => return false,
            }
        }
        b'<' => match s.find_on_line(s.pos + 1, ">") {
            Some(close) if close > s.pos + 1 && s.ident_end(s.pos + 1, "-.:") == close => {
                s.emit(close + 1, K::NameLabel);
            }
            _ => return false,
        },
        b'@' if is_ident_start(s.peek(1)) => {
            s.emit(s.ident_end(s.pos + 1, "-.:"), K::NameLabel);
        }
        _ => return false,
    }
    true
}

fn typst_raw(s: &mut Scanner<'_>) {
    if !s.looking_at("```", 0) {
        let close = s.find_from(s.pos + 1, "`");
        s.emit(close.map_or(s.limit, |c| c + 1), K::StringBacktick);
        return;
    }
    let info_end = s.ident_end(s.pos + 3, "-");
    let close = s.find_from(info_end, "```");
    let info = std::str::from_utf8(s.slice(s.pos + 3, info_end)).ok();
    let lexer = info.and_then(find_lexer);
    match (lexer, close) {
        (Some(lexer), Some(close)) if info_end > s.pos + 3 => {
            s.emit(s.pos + 3, K::StringBacktick);
            s.emit(info_end, K::NameLabel);
            s.start_inner(lexer, close, true);
        }
        _ => s.emit(close.map_or(s.limit, |c| c + 3), K::StringBacktick),
    }
}

fn typst_hash(s: &mut Scanner<'_>, st: &mut State) {
    let next = s.peek(1);
    if next == b'{' || next == b'(' {
        s.emit_n(2, K::Punctuation);
        st.push(if next == b'{' { FRAME_CODE } else { FRAME_ARGS }, 0, 0);
    } else if is_ident_start(next) {
        let end = s.ident_end(s.pos + 1, "-");
        let word = s.slice(s.pos + 1, end);
        let kind = typst_word_kind(s, word, end);
        s.emit(end, kind);
        let statement = kind == K::KeywordDeclaration
            || kind == K::KeywordNamespace
            || (kind == K::Keyword && word != b"none" && word != b"auto");
        if statement {
            // #let x = ..., #set text(...), #if ...: code to the end of the line.
            st.push(FRAME_CODE, 1, 0);
        } else {
            st.flags |= TYPST_EXPRESSION;
        }
    } else {
        s.emit_n(1, K::Text);
    }
}

fn typst_word_kind(s: &Scanner<'_>, word: &[u8], end: usize) -> K {
    match word {
        b"let" => return K::KeywordDeclaration,
        b"set" | b"show" => return K::Keyword,
        b"import" | b"include" => return K::KeywordNamespace,
        b"none" | b"auto" | b"true" | b"false" => return K::KeywordConstant,
        _ => {}
    }
    if in_list(word, TYPST_KEYWORDS) {
        return K::Keyword;
    }
    let next = s.byte_at(end);
    if next == b'(' || next == b'[' { K::NameFunction } else { K::NameVariable }
}

/// Lexes code; a frame with data 1 ends at the end of its line.
fn typst_code(s: &mut Scanner<'_>, st: &mut State, mode: u8) {
    let c = s.peek(0);
    let Some(f) = st.top() else {
        // typst_step passes a code mode only with its frame open.
        s.emit_n(1, K::Text);
        return;
    };
    let closes =
        (c == b'}' && mode == FRAME_CODE && f.data == 0) || (c == b')' && mode == FRAME_ARGS);
    if c == b'\n' && f.data == 1 {
        st.pop();
        s.emit_n(1, K::Text);
    } else if is_space(c) {
        let spaces = if f.data == 0 { " \t\r\n" } else { " \t\r" };
        s.emit(s.skip_set(s.pos, spaces), K::Whitespace);
    } else if closes {
        s.emit_n(1, K::Punctuation);
        st.pop();
        st.flags |= TYPST_EXPRESSION;
    } else if c == b'[' || c == b'{' || c == b'(' {
        s.emit_n(1, K::Punctuation);
        let frame = match c {
            b'[' => FRAME_CONTENT,
            b'{' => FRAME_CODE,
            _ => FRAME_ARGS,
        };
        st.push(frame, 0, 0);
    } else {
        typst_code_token(s, st, c);
    }
}

/// Lexes a code token that does not open or close a frame.
fn typst_code_token(s: &mut Scanner<'_>, st: &mut State, c: u8) {
    if c == b'"' {
        typst_string(s);
    } else if c.is_ascii_digit() || (c == b'.' && s.peek(1).is_ascii_digit()) {
        typst_number(s);
    } else if is_ident_start(c) {
        let end = s.ident_end(s.pos, "-");
        let kind = match typst_word_kind(s, s.slice(s.pos, end), end) {
            K::NameVariable => K::Name,
            kind => kind,
        };
        s.emit(end, kind);
    } else if c == b'$' {
        s.emit_n(1, K::Punctuation);
        st.push(FRAME_MATH, 0, 0);
    } else if has("+-*/=<>!.:", c) {
        s.emit_n(1, K::Operator);
    } else if has(",;)]}#", c) {
        s.emit_n(1, K::Punctuation);
    } else {
        s.emit_rune(K::Error);
    }
}

fn typst_string(s: &mut Scanner<'_>) {
    let mut i = s.pos + 1;
    while i < s.limit && s.bytes[i] != b'"' && s.bytes[i] != b'\n' {
        if s.bytes[i] == b'\\' {
            s.emit(i, K::StringDouble);
            s.emit(i + s.escape_length(i), K::StringEscape);
            i = s.pos;
            continue;
        }
        i += 1;
    }
    let end = if s.byte_at(i) == b'"' { (i + 1).min(s.limit) } else { i };
    s.emit(end, K::StringDouble);
}

fn typst_number(s: &mut Scanner<'_>) {
    let mut i = s.skip_digits(s.pos);
    let mut kind = K::NumberInteger;
    if s.byte_at(i) == b'.' && s.byte_at(i + 1).is_ascii_digit() {
        i = s.skip_digits(i + 1);
        kind = K::NumberFloat;
    }
    if s.byte_at(i).eq_ignore_ascii_case(&b'e') && s.byte_at(i + 1).is_ascii_digit() {
        i = s.skip_digits(i + 1);
        kind = K::NumberFloat;
    }
    s.emit(i, kind);
    let unit = s.ident_end(s.pos, "");
    let units = ["pt", "mm", "cm", "in", "em", "fr", "deg", "rad"];
    if unit > s.pos && in_list(s.slice(s.pos, unit), &units) {
        s.emit(unit, K::KeywordType);
    } else if s.peek(0) == b'%' {
        s.emit_n(1, K::KeywordType);
    }
}

fn typst_math(s: &mut Scanner<'_>, st: &mut State) {
    let c = s.peek(0);
    if c == b'$' {
        s.emit_n(1, K::Punctuation);
        st.pop();
    } else if c == b'\\' {
        s.emit(s.pos + 1 + s.rune_size(s.pos + 1), K::StringEscape);
    } else if c == b'"' {
        typst_string(s);
    } else if c == b'#' {
        typst_hash(s, st);
    } else if c.is_ascii_digit() {
        s.emit(s.skip_set(s.pos, "0123456789."), K::Number);
    } else if is_ident_start(c) {
        s.emit(s.ident_end(s.pos, "."), K::NameVariable);
    } else if is_space(c) {
        s.emit_spaces(K::Whitespace);
    } else if has("+-*/=<>!^_&|~,;:()[]{}", c) {
        s.emit_n(1, K::Operator);
    } else {
        s.emit_rune(K::Text);
    }
}
