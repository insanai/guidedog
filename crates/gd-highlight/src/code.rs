//! The code engine: C-like languages lexed from a [`CodeSpec`] of word tables, comment and
//! string delimiters, operator and punctuation sets, and features for the constructs that
//! set languages apart (C++ raw strings, Rust lifetimes, JavaScript regular expressions).
//! A language is a table, not a function; see `code_tables.rs`.

pub(crate) use crate::code_tables::code_spec;
use crate::scanner::is_space;
use crate::scanner::{LOOKAHEAD, Scanner, State, has, in_list, in_list_fold, is_ident_start};
use crate::token::TokenKind as K;

/// Code features, as bits of [`CodeSpec::features`].
pub(crate) mod feature {
    /// `#` first on a line is `Comment.Preproc`; `#include`'s file is `PreprocFile`.
    pub const PREPROCESSOR: u32 = 1 << 0;
    /// `'x'` is `String.Char`.
    pub const CHAR_LITERALS: u32 = 1 << 1;
    /// Rust: `'a` is `Name.Label`, `'a'` is `String.Char`.
    pub const LIFETIMES: u32 = 1 << 2;
    /// `R"delim(...)delim"` with its prefixes.
    pub const CPP_RAW_STRINGS: u32 = 1 << 3;
    /// `r"..."`, `r#"..."#`, `br"..."`.
    pub const RUST_RAW_STRINGS: u32 = 1 << 4;
    /// JavaScript: `/re/flags` where an operand may start.
    pub const REGEX_LITERALS: u32 = 1 << 5;
    /// `@name` is `Name.Decorator`; Odin's `@(...)` too.
    pub const ANNOTATIONS: u32 = 1 << 6;
    /// `#[...]` and `#![...]` are `Comment.Preproc`.
    pub const RUST_ATTRIBUTES: u32 = 1 << 7;
    /// `name!` is `Name.Function.Magic`.
    pub const MACRO_BANG: u32 = 1 << 8;
    /// Odin: `#name` is `Keyword.Pseudo`.
    pub const HASH_DIRECTIVES: u32 = 1 << 9;
    /// `\\` to the end of the line is `String.Heredoc`.
    pub const ZIG_LINE_STRINGS: u32 = 1 << 10;
    /// `@name` is `Name.Builtin`.
    pub const ZIG_BUILTINS: u32 = 1 << 11;
    /// `$name` and `${name}` are `Name.Variable`.
    pub const DOLLAR_VARIABLES: u32 = 1 << 12;
    /// PowerShell: `-eq` after a space is `Operator.Word`.
    pub const DASH_OPERATORS: u32 = 1 << 13;
    /// Words match in any case (SQL, PowerShell).
    pub const CASE_INSENSITIVE: u32 = 1 << 14;
    /// `type name(` at a declaration level makes name `Name.Function`.
    pub const C_FUNCTIONS: u32 = 1 << 15;
    /// `.name` is `Name.Attribute` (Java).
    pub const MEMBER_ATTRIBUTES: u32 = 1 << 16;
    /// `name :: proc` is `Name.Function`; `name :: struct` is `Name.Class`.
    pub const ODIN_DECLARATIONS: u32 = 1 << 17;
    /// `func (r T) Name`: the name after the receiver is the function.
    pub const GO_RECEIVERS: u32 = 1 << 18;
    /// Block comments nest.
    pub const NESTED_COMMENTS: u32 = 1 << 19;
    /// `0755` is `Number.Oct`.
    pub const C_OCTAL: u32 = 1 << 20;
    /// Graphviz: `name =` is `Name.Attribute`.
    pub const DOT_ATTRIBUTES: u32 = 1 << 21;
    /// Graphviz: `<...>` labels are `String`.
    pub const HTML_LABELS: u32 = 1 << 22;
    /// C++14: `1'000'000`.
    pub const DIGIT_QUOTES: u32 = 1 << 23;
}

use feature as F;

/// Quote flags, as bits of [`Quote::flags`].
pub(crate) mod quote {
    /// Backslash (or the spec's escape character) sequences are `String.Escape`.
    pub const ESCAPES: u8 = 1;
    /// The string may span lines; otherwise it ends at the line's end.
    pub const MULTILINE: u8 = 2;
    /// A doubled closing quote is part of the string: `'it''s'`.
    pub const DOUBLING: u8 = 4;
}

use quote as Q;

/// The interpolation syntax of a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Interp {
    None,
    /// `${expr}`
    DollarBrace,
    /// `$name` and `${expr}`
    DollarName,
    /// `\(expr)`
    BackslashParen,
    /// `{expr}`; `{{` is an escape.
    Brace,
    /// `$name` and `$(expr)`
    DollarParen,
}

impl Interp {
    fn from_u8(v: u8) -> Interp {
        match v {
            1 => Interp::DollarBrace,
            2 => Interp::DollarName,
            3 => Interp::BackslashParen,
            4 => Interp::Brace,
            5 => Interp::DollarParen,
            _ => Interp::None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct WordGroup {
    pub kind: K,
    pub words: &'static [&'static str],
}

/// Gives the name after a word its kind: "class" makes the next name `Name.Class`.
#[derive(Debug)]
pub(crate) struct Lead {
    pub word: &'static str,
    pub kind: K,
}

#[derive(Debug)]
pub(crate) struct Quote {
    pub open: &'static str,
    pub close: &'static str,
    pub kind: K,
    pub flags: u8,
    pub interp: Interp,
}

#[derive(Debug)]
pub(crate) struct CodeSpec {
    pub features: u32,
    pub words: &'static [WordGroup],
    pub leads: &'static [Lead],
    pub line_comments: &'static [&'static str],
    /// Line-comment starts that are `String.Doc`: Rust's `///`.
    pub doc_comments: &'static [&'static str],
    pub block_open: &'static str,
    pub block_close: &'static str,
    pub quotes: &'static [Quote],
    /// Names glued to a quote that are `String.Affix`: `L"x"`.
    pub string_prefixes: &'static [&'static str],
    pub operators: &'static str,
    pub punctuation: &'static str,
    /// Plain names.
    pub name_kind: K,
    /// Whitespace.
    pub space_kind: K,
    /// More identifier bytes, e.g. `$` in JavaScript.
    pub ident_extra: &'static str,
    /// The string escape character.
    pub escape: u8,
    /// `C_FUNCTIONS`: the deepest brace level of definitions.
    pub function_depth: u32,
}

impl CodeSpec {
    pub const BASE: CodeSpec = CodeSpec {
        features: 0,
        words: &[],
        leads: &[],
        line_comments: &[],
        doc_comments: &[],
        block_open: "",
        block_close: "",
        quotes: &[],
        string_prefixes: &[],
        operators: "",
        punctuation: "",
        name_kind: K::Name,
        space_kind: K::Whitespace,
        ident_extra: "",
        escape: b'\\',
        function_depth: 0,
    };

    fn has(&self, feature: u32) -> bool {
        self.features & feature != 0
    }
}

const FRAME_STRING: u8 = 1;
const FRAME_INTERP: u8 = 2;
const FRAME_RAW_CPP: u8 = 3;
const FRAME_RAW_RUST: u8 = 4;

/// `st.flags`: inside a Go receiver.
const IN_RECEIVER: u32 = 1;

pub(crate) fn code_step(s: &mut Scanner<'_>, st: &mut State, spec: &CodeSpec) {
    if let Some(f) = st.top() {
        match f.mode {
            FRAME_STRING => return string_step(s, st, spec, f.data),
            FRAME_RAW_CPP => return raw_cpp_body(s, st),
            FRAME_RAW_RUST => return raw_rust_body(s, st, f.count),
            FRAME_INTERP if interp_step(s, st) => {
                return;
            }
            _ => {}
        }
    }
    code_token(s, st, spec);
}

fn code_token(s: &mut Scanner<'_>, st: &mut State, spec: &CodeSpec) {
    let c = s.peek(0);
    update_lead(s, st, spec, c);
    if is_space(c) {
        s.emit_spaces(spec.space_kind);
        return;
    }
    if comment_token(s, spec) || feature_token(s, spec, c) || quote_token(s, st, spec) {
        return;
    }
    if c.is_ascii_digit() || (c == b'.' && s.peek(1).is_ascii_digit()) {
        return number_token(s, spec);
    }
    if is_ident_start(c) || (c != b'-' && has(spec.ident_extra, c)) {
        return word_token(s, st, spec);
    }
    symbol_token(s, st, spec, c);
}

/// Ends a pending lead at anything but space, a name, or a dotted namespace.
fn update_lead(s: &Scanner<'_>, st: &mut State, spec: &CodeSpec, c: u8) {
    if !st.leading || is_space(c) || is_ident_start(c) || has(spec.ident_extra, c) {
        return;
    }
    if st.lead == K::NameNamespace && c == b'.' {
        return;
    }
    if spec.has(F::GO_RECEIVERS) && st.lead == K::NameFunction {
        let in_receiver = st.flags & IN_RECEIVER != 0;
        if c == b'(' && !in_receiver && s.track().last == K::KeywordDeclaration {
            st.flags |= IN_RECEIVER;
            return;
        }
        if in_receiver {
            if c == b')' {
                st.flags &= !IN_RECEIVER;
            }
            return;
        }
    }
    st.leading = false;
}

fn comment_token(s: &mut Scanner<'_>, spec: &CodeSpec) -> bool {
    if spec.doc_comments.iter().any(|d| s.looking_at(d, 0)) && !s.looking_at("////", 0) {
        s.emit_line(K::StringDoc);
        return true;
    }
    if spec.line_comments.iter().any(|c| s.looking_at(c, 0)) {
        s.emit_line(K::CommentSingle);
        return true;
    }
    if spec.block_open.is_empty() || !s.looking_at(spec.block_open, 0) {
        return false;
    }
    let nested = spec.has(F::NESTED_COMMENTS);
    s.emit(s.block_comment_end(spec.block_open, spec.block_close, nested), K::CommentMultiline);
    true
}

fn feature_token(s: &mut Scanner<'_>, spec: &CodeSpec, c: u8) -> bool {
    match c {
        b'#' => hash_token(s, spec),
        b'@' if spec.has(F::ZIG_BUILTINS) && is_ident_start(s.peek(1)) => {
            s.emit(s.ident_end(s.pos + 1, ""), K::NameBuiltin);
            true
        }
        b'@' if spec.has(F::ANNOTATIONS) => annotation(s),
        b'\\' if spec.has(F::ZIG_LINE_STRINGS) && s.peek(1) == b'\\' => {
            s.emit_line(K::StringHeredoc);
            true
        }
        b'$' if spec.has(F::DOLLAR_VARIABLES) => dollar_variable(s),
        b'-' if spec.has(F::DASH_OPERATORS)
            && s.peek(1).is_ascii_alphabetic()
            && s.pos > 0
            && is_space(s.before()) =>
        {
            s.emit(s.ident_end(s.pos + 1, ""), K::OperatorWord);
            true
        }
        b'<' if spec.has(F::HTML_LABELS)
            && s.track().last == K::Operator
            && s.track().last_byte == b'=' =>
        {
            s.emit(bracket_end(s, s.pos, b'<', b'>'), K::String);
            true
        }
        b'/' if spec.has(F::REGEX_LITERALS) && regex_allowed(s) => regex_literal(s),
        b'\'' if spec.has(F::CHAR_LITERALS) || spec.has(F::LIFETIMES) => char_literal(s, spec),
        _ => false,
    }
}

fn hash_token(s: &mut Scanner<'_>, spec: &CodeSpec) -> bool {
    if spec.has(F::PREPROCESSOR) && only_space_before(s) {
        preprocessor(s);
        return true;
    }
    if spec.has(F::RUST_ATTRIBUTES) && (s.peek(1) == b'[' || s.looking_at("![", 1)) {
        s.emit(bracket_end(s, s.pos + 1, b'[', b']'), K::CommentPreproc);
        return true;
    }
    if spec.has(F::HASH_DIRECTIVES) && (is_ident_start(s.peek(1)) || s.peek(1) == b'+') {
        if s.peek(1) == b'+' {
            s.emit_line(K::CommentPreproc);
        } else {
            s.emit(s.ident_end(s.pos + 1, ""), K::KeywordPseudo);
        }
        return true;
    }
    false
}

fn only_space_before(s: &Scanner<'_>) -> bool {
    for &c in s.bytes[..s.pos].iter().rev() {
        match c {
            b'\n' => return true,
            b' ' | b'\t' => {}
            _ => return false,
        }
    }
    true
}

/// The position after the bracket that closes the one at `from`, on lines continued by
/// nesting, or the end of the line when it does not close.
fn bracket_end(s: &Scanner<'_>, from: usize, open: u8, close: u8) -> usize {
    let mut level = 0i32;
    let stop = s.limit.min(from + 4 * LOOKAHEAD);
    for i in from..stop {
        let c = s.bytes[i];
        if c == open {
            level += 1;
        } else if c == close {
            level -= 1;
            if level <= 0 {
                return i + 1;
            }
        } else if c == b'\n' && level <= 0 {
            return i;
        }
    }
    s.line_end(from)
}

fn preprocessor(s: &mut Scanner<'_>) {
    let word = s.skip_spaces(s.pos + 1);
    let word_stop = s.ident_end(word, "");
    let directive = s.slice(word, word_stop);
    if in_list(directive, &["include", "import", "include_next"]) {
        s.emit(word_stop, K::CommentPreproc);
        s.emit(s.skip_spaces(s.pos), K::Whitespace);
        let open = s.peek(0);
        if open == b'<' || open == b'"' {
            let close = if open == b'<' { b'>' } else { b'"' };
            let mut end = s.pos + 1;
            while end < s.limit && s.bytes[end] != close && s.bytes[end] != b'\n' {
                end += 1;
            }
            s.emit((end + 1).min(s.line_end(s.pos)), K::CommentPreprocFile);
        }
    }
    let mut i = s.pos;
    while i < s.limit {
        let c = s.bytes[i];
        if c == b'\n' {
            if i > s.pos && s.bytes[i - 1] == b'\\' {
                i += 1;
                continue;
            }
            break;
        }
        if c == b'/' && i + 1 < s.limit && (s.bytes[i + 1] == b'/' || s.bytes[i + 1] == b'*') {
            break;
        }
        i += 1;
    }
    s.emit(i, K::CommentPreproc);
}

fn annotation(s: &mut Scanner<'_>) -> bool {
    let next = s.peek(1);
    if next == b'(' {
        s.emit(bracket_end(s, s.pos + 1, b'(', b')'), K::NameDecorator);
        return true;
    }
    if !is_ident_start(next) {
        return false;
    }
    let mut i = s.ident_end(s.pos + 1, "");
    while s.byte_at(i) == b'.' && is_ident_start(s.byte_at(i + 1)) {
        i = s.ident_end(i + 1, "");
    }
    s.emit(i, K::NameDecorator);
    true
}

fn dollar_variable(s: &mut Scanner<'_>) -> bool {
    let next = s.peek(1);
    if next == b'{' {
        let Some(end) = s.find_on_line(s.pos, "}") else { return false };
        s.emit(end + 1, K::NameVariable);
        return true;
    }
    if !is_ident_start(next) && !has("_?$^", next) {
        return false;
    }
    let end = if is_ident_start(next) { s.ident_end(s.pos + 1, ":") } else { s.pos + 2 };
    s.emit(end, K::NameVariable);
    true
}

/// Whether an operand may start here, so a slash begins a regular expression rather than
/// a division.
fn regex_allowed(s: &Scanner<'_>) -> bool {
    let t = s.track();
    if !t.started {
        return true;
    }
    match t.last {
        K::Operator
        | K::OperatorWord
        | K::Keyword
        | K::KeywordDeclaration
        | K::KeywordReserved => true,
        K::Punctuation => !has(")]}", t.last_byte),
        _ => false,
    }
}

fn regex_literal(s: &mut Scanner<'_>) -> bool {
    if s.peek(1) == b'/' || s.peek(1) == b'*' {
        return false;
    }
    let mut in_class = false;
    let stop = s.limit.min(s.pos + LOOKAHEAD);
    let mut i = s.pos + 1;
    while i < stop {
        match s.bytes[i] {
            b'\n' => return false,
            b'\\' => i += 1,
            b'[' => in_class = true,
            b']' => in_class = false,
            b'/' if !in_class => {
                let end = s.skip_by(i + 1, |c| c.is_ascii_alphabetic());
                s.emit(end, K::StringRegex);
                return true;
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// Lexes `'x'`, `'\n'` and `'\u{1F600}'` as `String.Char` and, with lifetimes, `'a` and
/// `'static` as `Name.Label`.
fn char_literal(s: &mut Scanner<'_>, spec: &CodeSpec) -> bool {
    let mut i = s.pos + 1;
    if s.byte_at(i) == b'\\' {
        i += s.escape_length(i);
    } else if s.byte_at(i) != b'\'' && s.byte_at(i) != b'\n' && i < s.limit {
        i += s.rune_size(i);
    }
    if i > s.pos + 1 && s.byte_at(i) == b'\'' {
        s.emit(i + 1, K::StringChar);
        return true;
    }
    if spec.has(F::LIFETIMES) && is_ident_start(s.peek(1)) {
        s.emit(s.ident_end(s.pos + 1, ""), K::NameLabel);
        return true;
    }
    s.emit_n(1, K::Error);
    true
}

fn quote_token(s: &mut Scanner<'_>, st: &mut State, spec: &CodeSpec) -> bool {
    let Some(i) = spec.quotes.iter().position(|q| s.looking_at(q.open, 0)) else {
        return false;
    };
    let q = &spec.quotes[i];
    s.emit_n(q.open.len(), q.kind);
    st.push(FRAME_STRING, i as u8, 0);
    true
}

fn string_step(s: &mut Scanner<'_>, st: &mut State, spec: &CodeSpec, index: u8) {
    let q = &spec.quotes[usize::from(index)];
    let escapes = q.flags & Q::ESCAPES != 0;
    if s.looking_at(q.close, 0) {
        if q.flags & Q::DOUBLING != 0 && s.looking_at(q.close, q.close.len()) {
            s.emit_n(2 * q.close.len(), K::StringEscape);
            return;
        }
        s.emit_n(q.close.len(), q.kind);
        st.pop();
        return;
    }
    let c = s.peek(0);
    if c == b'\n' && q.flags & Q::MULTILINE == 0 {
        st.pop();
        s.emit_n(1, K::Whitespace);
        return;
    }
    if interp_start(s, st, q) {
        return;
    }
    if c == spec.escape && escapes {
        s.emit_n(s.escape_length(s.pos), K::StringEscape);
        return;
    }
    let close0 = q.close.as_bytes()[0];
    let mut i = s.pos;
    while i < s.limit {
        let b = s.bytes[i];
        if b == close0 || b == b'\n' || (b == spec.escape && escapes) {
            break;
        }
        if q.interp != Interp::None && has("${}\\", b) && i > s.pos {
            break;
        }
        i += 1;
    }
    s.emit(i.max(s.pos + 1), q.kind);
}

fn interp_start(s: &mut Scanner<'_>, st: &mut State, q: &Quote) -> bool {
    // `{{` and `}}` are escapes in brace interpolation; `$name` interpolates without a
    // bracket. Everything else opens a bracketed expression of `open` bytes.
    if q.interp == Interp::Brace && (s.looking_at("{{", 0) || s.looking_at("}}", 0)) {
        s.emit_n(2, K::StringEscape);
        return true;
    }
    let dollar_name = matches!(q.interp, Interp::DollarName | Interp::DollarParen);
    let opener = match q.interp {
        Interp::None => return false,
        Interp::DollarBrace | Interp::DollarName => "${",
        Interp::DollarParen => "$(",
        Interp::BackslashParen => "\\(",
        Interp::Brace => "{",
    };
    if s.looking_at(opener, 0) {
        s.emit_n(opener.len(), K::StringInterpol);
        st.push(FRAME_INTERP, q.interp as u8, 0);
        return true;
    }
    if dollar_name && s.peek(0) == b'$' && is_ident_start(s.peek(1)) {
        let kind =
            if q.interp == Interp::DollarName { K::StringInterpol } else { K::NameVariable };
        s.emit(s.ident_end(s.pos + 1, ""), kind);
        return true;
    }
    false
}

/// Ends an interpolation at its closing bracket; true when it emitted.
fn interp_step(s: &mut Scanner<'_>, st: &mut State) -> bool {
    let Some(f) = st.top_mut() else { return false };
    let interp = Interp::from_u8(f.data);
    let paren = interp == Interp::BackslashParen || interp == Interp::DollarParen;
    let (open, close) = if paren { (b'(', b')') } else { (b'{', b'}') };
    let c = s.peek(0);
    if c == close && f.count == 0 {
        s.emit_n(1, K::StringInterpol);
        st.pop();
        return true;
    }
    if c == open {
        f.count = f.count.wrapping_add(1);
    }
    if c == close {
        f.count = f.count.wrapping_sub(1);
    }
    false
}

fn number_token(s: &mut Scanner<'_>, spec: &CodeSpec) {
    let separators = if spec.has(F::DIGIT_QUOTES) { "_'" } else { "_" };
    let radix = if s.peek(0) == b'0' { s.peek(1).to_ascii_lowercase() } else { 0 };
    let (i, kind) = match radix {
        b'x' => {
            let end = s.skip_by(s.pos + 2, |c| c.is_ascii_hexdigit() || has(separators, c));
            (end, K::NumberHex)
        }
        b'b' | b'o' => {
            let kind = if radix == b'b' { K::NumberBin } else { K::NumberOct };
            (s.skip_set(s.pos + 2, "01234567_"), kind)
        }
        _ => decimal(s, spec, separators),
    };
    s.emit(s.ident_end(i, ""), kind);
}

fn decimal(s: &Scanner<'_>, spec: &CodeSpec, separators: &str) -> (usize, K) {
    let mut kind = K::NumberInteger;
    let mut i = skip_digit_run(s, s.pos, separators);
    if s.byte_at(i) == b'.' && s.byte_at(i + 1).is_ascii_digit() {
        i = skip_digit_run(s, i + 1, separators);
        kind = K::NumberFloat;
    } else if s.byte_at(i) == b'.'
        && !is_ident_start(s.byte_at(i + 1))
        && s.byte_at(i + 1) != b'.'
        && s.peek(0) != b'.'
    {
        i += 1;
        kind = K::NumberFloat;
    }
    let sign = usize::from(s.byte_at(i + 1) == b'+' || s.byte_at(i + 1) == b'-');
    if s.byte_at(i).eq_ignore_ascii_case(&b'e') && s.byte_at(i + 1 + sign).is_ascii_digit() {
        i = skip_digit_run(s, i + 1 + sign, separators);
        kind = K::NumberFloat;
    }
    let text = s.slice(s.pos, i);
    if spec.has(F::C_OCTAL) && kind == K::NumberInteger && text.len() > 1 && text[0] == b'0' {
        kind = K::NumberOct;
    }
    (i, kind)
}

fn skip_digit_run(s: &Scanner<'_>, from: usize, separators: &str) -> usize {
    let mut i = from;
    while i < s.limit {
        let c = s.bytes[i];
        if c.is_ascii_digit() || (has(separators, c) && s.byte_at(i + 1).is_ascii_digit()) {
            i += 1;
            continue;
        }
        break;
    }
    i
}

fn word_token(s: &mut Scanner<'_>, st: &mut State, spec: &CodeSpec) {
    let end = s.ident_end(s.pos, spec.ident_extra);
    let w = s.slice(s.pos, end);
    let next = s.byte_at(end);
    if special_word(s, st, spec, w, end, next) {
        return;
    }
    if st.leading && st.flags & IN_RECEIVER == 0 {
        s.emit(end, st.lead);
        if st.lead != K::NameNamespace || s.byte_at(end) != b'.' {
            st.leading = false;
        }
        return;
    }
    if let Some(kind) = word_kind(spec, w) {
        let fold = spec.has(F::CASE_INSENSITIVE);
        for l in spec.leads {
            if l.word.as_bytes() == w || (fold && l.word.as_bytes().eq_ignore_ascii_case(w)) {
                st.lead = l.kind;
                st.leading = true;
            }
        }
        s.emit(end, kind);
        return;
    }
    s.emit(end, name_kind(s, st, spec, end));
}

/// Handles names that start something else: string prefixes, raw strings and macros.
fn special_word(
    s: &mut Scanner<'_>,
    st: &mut State,
    spec: &CodeSpec,
    w: &[u8],
    end: usize,
    next: u8,
) -> bool {
    if spec.has(F::CPP_RAW_STRINGS) && next == b'"' && in_list(w, &["R", "u8R", "uR", "UR", "LR"])
    {
        raw_cpp_open(s, st, end);
        return true;
    }
    if spec.has(F::RUST_RAW_STRINGS)
        && (next == b'"' || next == b'#')
        && in_list(w, &["r", "br", "cr"])
    {
        return raw_rust_open(s, st, end);
    }
    if !spec.string_prefixes.is_empty() && in_list(w, spec.string_prefixes) {
        let quoted = spec.quotes.iter().any(|q| q.open.as_bytes()[0] == next);
        if quoted || (next == b'\'' && spec.has(F::CHAR_LITERALS)) {
            s.emit(end, K::StringAffix);
            return true;
        }
    }
    if spec.has(F::MACRO_BANG) && next == b'!' && s.byte_at(end + 1) != b'=' {
        s.emit(end + 1, K::NameFunctionMagic);
        return true;
    }
    false
}

fn word_kind(spec: &CodeSpec, w: &[u8]) -> Option<K> {
    let fold = spec.has(F::CASE_INSENSITIVE);
    spec.words
        .iter()
        .find(|g| if fold { in_list_fold(w, g.words) } else { in_list(w, g.words) })
        .map(|g| g.kind)
}

/// The kind of a name that is not a word of the language.
fn name_kind(s: &Scanner<'_>, st: &State, spec: &CodeSpec, end: usize) -> K {
    let after = s.skip_spaces(end);
    if spec.has(F::ODIN_DECLARATIONS) && s.looking_at("::", after - s.pos) {
        let decl = s.skip_spaces(after + 2);
        let word = s.slice(decl, s.ident_end(decl, ""));
        if word == b"proc" {
            return K::NameFunction;
        }
        if in_list(word, &["struct", "enum", "union", "bit_set", "distinct", "bit_field"]) {
            return K::NameClass;
        }
    }
    if spec.has(F::C_FUNCTIONS)
        && s.byte_at(after) == b'('
        && st.count <= spec.function_depth
        && declares(s)
    {
        return K::NameFunction;
    }
    if spec.has(F::MEMBER_ATTRIBUTES) && s.pos > 0 && s.before() == b'.' {
        return K::NameAttribute;
    }
    if spec.has(F::DOT_ATTRIBUTES) && s.byte_at(after) == b'=' {
        return K::NameAttribute;
    }
    // PowerShell cmdlets are Verb-Noun.
    if spec.has(F::DASH_OPERATORS) && s.slice(s.pos, end).contains(&b'-') {
        return K::NameBuiltin;
    }
    spec.name_kind
}

/// Whether the token before a name followed by `(` makes it the name of a function being
/// declared: a type or name (`int main(`, `String get(`), or a pointer.
fn declares(s: &Scanner<'_>) -> bool {
    let t = s.track();
    if !t.started {
        return false;
    }
    match t.last {
        K::Name | K::NameOther | K::KeywordType | K::NameClass | K::NameBuiltin => true,
        K::Operator => matches!(t.last_byte, b'*' | b'&' | b'>'),
        _ => false,
    }
}

fn symbol_token(s: &mut Scanner<'_>, st: &mut State, spec: &CodeSpec, c: u8) {
    if has(spec.punctuation, c) {
        if c == b'{' {
            st.count += 1;
        }
        if c == b'}' && st.count > 0 {
            st.count -= 1;
        }
        s.emit_n(1, K::Punctuation);
        return;
    }
    if has(spec.operators, c) {
        s.emit_n(1, K::Operator);
        return;
    }
    if c == b'\\' {
        s.emit_n(if s.peek(1) == b'\n' { 2 } else { 1 }, K::Text);
        return;
    }
    s.emit_rune(K::Error);
}

/// C++ raw strings: `R"delim( ... )delim"`, split as Pygments splits them.
fn raw_cpp_open(s: &mut Scanner<'_>, st: &mut State, prefix_end: usize) {
    let delim = prefix_end + 1;
    let mut paren = delim;
    while paren < s.limit && paren < delim + 17 && !has("()\\ \t\n\"", s.bytes[paren]) {
        paren += 1;
    }
    if s.byte_at(paren) != b'(' {
        s.emit(prefix_end, K::Name);
        return;
    }
    s.emit(prefix_end, K::StringAffix);
    s.emit_n(1, K::String);
    st.mark = [delim, paren];
    s.emit(paren + 1, K::StringDelimiter);
    st.push(FRAME_RAW_CPP, 0, 0);
}

fn raw_cpp_body(s: &mut Scanner<'_>, st: &mut State) {
    let delim = s.slice(st.mark[0], st.mark[1]);
    for i in s.pos..s.limit {
        if s.bytes[i] != b')' {
            continue;
        }
        let close = i + 1 + delim.len();
        if close < s.limit && s.slice(i + 1, close) == delim && s.bytes[close] == b'"' {
            s.emit(i, K::String);
            s.emit(close, K::StringDelimiter);
            s.emit(close + 1, K::String);
            st.pop();
            return;
        }
    }
    s.emit(s.limit, K::String);
}

/// Rust raw strings: `r"..."`, `r#"..."#`; the prefix is an affix and the rest `String`.
fn raw_rust_open(s: &mut Scanner<'_>, st: &mut State, prefix_end: usize) -> bool {
    let mut hashes = 0;
    while s.byte_at(prefix_end + hashes) == b'#' {
        hashes += 1;
    }
    if s.byte_at(prefix_end + hashes) != b'"' || hashes > 255 {
        return false;
    }
    s.emit(prefix_end, K::StringAffix);
    s.emit(prefix_end + hashes + 1, K::String);
    st.push(FRAME_RAW_RUST, 0, hashes as u16);
    true
}

fn raw_rust_body(s: &mut Scanner<'_>, st: &mut State, hashes: u16) {
    let hashes = usize::from(hashes);
    for i in s.pos..s.limit {
        if s.bytes[i] != b'"' {
            continue;
        }
        let mut n = 0;
        while n < hashes && s.byte_at(i + 1 + n) == b'#' {
            n += 1;
        }
        if n == hashes {
            s.emit(i + 1 + hashes, K::String);
            st.pop();
            return;
        }
    }
    s.emit(s.limit, K::String);
}
