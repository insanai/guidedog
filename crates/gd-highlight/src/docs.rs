//! Document languages: reStructuredText and Markdown, whose code blocks are lexed as their
//! language.

use crate::lexers::{Lexer, find_lexer, resolve_lexer};
use crate::scanner::{Scanner, State, has, in_list, index_byte_from, is_blank, is_ident_char};
use crate::scanner::{is_space, trim_right, trim_spaces};
use crate::token::TokenKind as K;

// reStructuredText, after Pygments' reST lexer.

/// `st.flags`: "::" ended a paragraph; the next indented block is literal.
const RST_LITERAL: u32 = 1;
/// `st.flags`: a code directive's content follows; `st.count` is its lexer's index.
const RST_CODE: u32 = 2;

pub(crate) fn rst_step(s: &mut Scanner<'_>, st: &mut State) {
    if !s.at_line_start() {
        return rst_inline(s, st);
    }
    let stop = s.line_end(s.pos);
    let line = s.slice(s.pos, stop);
    if is_blank(line) {
        s.emit(s.line_after(s.pos), K::Text);
        return;
    }
    let indent = s.skip_spaces(s.pos) - s.pos;
    if indent > 0 && st.flags & (RST_LITERAL | RST_CODE) != 0 {
        return rst_indented_block(s, st, indent);
    }
    if indent == 0 {
        st.flags &= !(RST_LITERAL | RST_CODE);
    }
    if rst_heading(s, line, stop) || rst_explicit(s, st, line, indent) {
        return;
    }
    if rst_list_marker(s, indent) {
        return;
    }
    let n = field_marker(&line[indent..]);
    if n > 0 {
        s.emit_n(indent, K::Text);
        s.emit_n(n, K::NameClass);
        return;
    }
    rst_inline(s, st);
}

/// Whether a line is a section adornment: one punctuation character repeated at least
/// three times.
fn adornment(line: &[u8]) -> bool {
    let l = trim_right(line);
    if l.len() < 3 || !has("=-`:.'\"~^_*+#<>!$%&(),/;?@[\\]{|}", l[0]) {
        return false;
    }
    l.iter().all(|&c| c == l[0])
}

/// Lexes an adornment line, or a title followed by one, as `Generic.Heading`.
fn rst_heading(s: &mut Scanner<'_>, line: &[u8], stop: usize) -> bool {
    if adornment(line) {
        s.emit(stop, K::GenericHeading);
        s.emit(s.line_after(s.pos), K::Text);
        return true;
    }
    if is_space(line[0]) || stop >= s.limit {
        return false;
    }
    let next_stop = s.line_end(stop + 1);
    if !adornment(s.slice(stop + 1, next_stop)) {
        return false;
    }
    s.emit(stop, K::GenericHeading);
    s.emit_n(1, K::Text);
    s.emit(next_stop, K::GenericHeading);
    s.emit(s.line_after(s.pos), K::Text);
    true
}

/// Lexes explicit markup: directives, targets, footnotes and comments.
fn rst_explicit(s: &mut Scanner<'_>, st: &mut State, line: &[u8], indent: usize) -> bool {
    let rest = &line[indent..];
    if !rest.starts_with(b"..") || (rest.len() > 2 && rest[2] != b' ') {
        return false;
    }
    let body = trim_spaces(&rest[2..]);
    let lead = s.pos + indent + 2;
    let after = s.skip_spaces(lead);
    if body.starts_with(b"_") || body.starts_with(b"[") {
        s.emit(lead, K::Punctuation);
        s.emit(after, K::Text);
        let close = index_byte_from(body, 1, if body[0] == b'_' { b':' } else { b']' });
        s.emit(after + close.map_or(body.len(), |c| c + 1), K::NameTag);
        return true;
    }
    if body.starts_with(b"|")
        && let Some(close) = index_byte_from(body, 1, b'|')
    {
        // A substitution definition: |name| directive:: argument.
        s.emit(lead, K::Punctuation);
        s.emit(after, K::Text);
        s.emit(after + close + 1, K::NameTag);
        s.emit(s.skip_spaces(s.pos), K::Text);
        rst_directive(s, st, None);
        return true;
    }
    if rst_directive(s, st, Some(lead)) {
        return true;
    }
    rst_comment(s);
    true
}

/// Lexes a comment: the line and the lines indented below it.
fn rst_comment(s: &mut Scanner<'_>) {
    let mut end = s.line_after(s.pos);
    while end < s.limit {
        let next = s.line_end(end);
        let next_line = s.slice(end, next);
        if !is_blank(next_line) && !is_space(next_line[0]) {
            break;
        }
        if is_blank(next_line) {
            let follow = s.line_after(next);
            if follow >= s.limit || !is_space(s.bytes[follow]) {
                break;
            }
        }
        end = s.line_after(next);
    }
    s.emit(end, K::CommentPreproc);
}

/// Lexes `.. name:: argument`; `lead` is where the name's search starts, after the `..`
/// it then emits.
fn rst_directive(s: &mut Scanner<'_>, st: &mut State, lead: Option<usize>) -> bool {
    let start = s.skip_spaces(lead.unwrap_or(s.pos));
    let mut name_end = start;
    while name_end < s.limit && (is_ident_char(s.bytes[name_end]) || has(":-", s.bytes[name_end]))
    {
        if s.bytes[name_end] == b':' && s.byte_at(name_end + 1) == b':' {
            break;
        }
        name_end += 1;
    }
    if name_end == start || !s.looking_at("::", name_end - s.pos) {
        return false;
    }
    let name = s.slice(start, name_end);
    if let Some(lead) = lead {
        s.emit(lead, K::Punctuation);
        s.emit(start, K::Text);
    }
    s.emit(name_end, K::OperatorWord);
    s.emit_n(2, K::Punctuation);
    let stop = s.line_end(s.pos);
    let argument = trim_spaces(s.slice(s.pos, stop));
    if in_list(name, &["code", "code-block", "sourcecode", "ipython"]) && !argument.is_empty() {
        let lexer = std::str::from_utf8(argument).ok().and_then(find_lexer);
        if let Some(l) = lexer.filter(|&l| l != Lexer::Text) {
            st.flags |= RST_CODE;
            st.count = l as u32;
        }
    }
    s.emit(s.skip_spaces(s.pos), K::Text);
    true
}

/// Lexes an indented block after "::" or a code directive: option lines, then the content
/// as a literal String or with the directive's language.
fn rst_indented_block(s: &mut Scanner<'_>, st: &mut State, indent: usize) {
    if st.flags & RST_CODE != 0 {
        let n = field_marker(s.slice(s.pos + indent, s.line_end(s.pos)));
        if n > 0 {
            s.emit_n(indent, K::Text);
            s.emit_n(n, K::NameClass);
            return;
        }
    }
    let mut end = s.pos;
    while end < s.limit {
        let stop = s.line_end(end);
        if !is_blank(s.slice(end, stop)) && s.skip_spaces(end) - end < indent {
            break;
        }
        end = s.line_after(stop);
    }
    // Trailing blank lines belong to what follows.
    while end > s.pos && is_blank(s.slice(line_start_before(s, end), end)) {
        end = line_start_before(s, end);
    }
    if st.flags & RST_CODE != 0 {
        let lexer = Lexer::ALL.get(st.count as usize).copied().unwrap_or(Lexer::Text);
        st.flags &= !RST_CODE;
        let lexer = resolve_lexer(lexer, &s.code[s.pos..end]);
        s.start_inner(lexer, end, true);
        return;
    }
    st.flags &= !RST_LITERAL;
    s.emit(end, K::String);
}

/// The start of the line that ends just before position `at`.
fn line_start_before(s: &Scanner<'_>, at: usize) -> usize {
    let mut i = at - 1;
    while i > s.pos && s.bytes[i - 1] != b'\n' {
        i -= 1;
    }
    i.max(s.pos)
}

/// The length of a `:field:` at the start of `text`, or 0.
fn field_marker(text: &[u8]) -> usize {
    if text.len() < 3 || text[0] != b':' || text[1] == b' ' {
        return 0;
    }
    for i in 1..text.len() {
        if text[i] == b':' && (i + 1 == text.len() || is_space(text[i + 1])) {
            return i + 1;
        }
        if text[i] == b'`' {
            return 0;
        }
    }
    0
}

fn rst_list_marker(s: &mut Scanner<'_>, indent: usize) -> bool {
    let at = s.pos + indent;
    let c = s.byte_at(at);
    let mut n = 0;
    if c != 0 && has("-*+", c) && is_space_or_stop(s, at + 1) {
        n = 1;
    } else if c.is_ascii_digit() || (c == b'#' && s.byte_at(at + 1) == b'.') {
        let d = if c != b'#' { s.skip_digits(at) } else { at + 1 };
        let mark = s.byte_at(d);
        if (mark == b'.' || mark == b')') && is_space_or_stop(s, d + 1) {
            n = d + 1 - at;
        }
    }
    if n == 0 {
        return false;
    }
    s.emit(at, K::Text);
    s.emit_n(n, K::Number);
    true
}

fn is_space_or_stop(s: &Scanner<'_>, at: usize) -> bool {
    at >= s.limit || is_space(s.bytes[at])
}

/// Lexes inline markup up to the end of the line.
fn rst_inline(s: &mut Scanner<'_>, st: &mut State) {
    let c = s.peek(0);
    if c == b'\n' {
        s.emit_n(1, K::Text);
        return;
    }
    if s.looking_at("::", 0) && trim_right(s.slice(s.pos, s.line_end(s.pos))) == b"::" {
        s.emit_n(2, K::StringEscape);
        st.flags |= RST_LITERAL;
        return;
    }
    if rst_inline_markup(s, c) {
        return;
    }
    let i = s.skip_by(s.pos + 1, |b| !has("\\\n[*`:<", b));
    s.emit(i, K::Text);
}

/// Lexes escapes, literals, emphasis, roles, footnote references and URLs; false when
/// none starts at `pos`.
fn rst_inline_markup(s: &mut Scanner<'_>, c: u8) -> bool {
    match c {
        b'\\' => {
            s.emit(s.pos + 1 + s.rune_size(s.pos + 1), K::Text);
            true
        }
        b'`' => rst_backquoted(s),
        b'*' => {
            let strong = s.peek(1) == b'*';
            let width = if strong { 2 } else { 1 };
            let close = s.find_on_line(s.pos + width + 1, if strong { "**" } else { "*" });
            let Some(close) = close else { return false };
            s.emit(close + width, if strong { K::GenericStrong } else { K::GenericEmph });
            true
        }
        b':' => {
            let n = role_length(s, true);
            if n == 0 {
                return false;
            }
            s.emit_n(n, K::NameAttribute);
            if s.peek(0) == b'`' {
                rst_backquoted(s);
            }
            true
        }
        b'[' | b'<' => {
            let footnote = c == b'[';
            let Some(close) = s.find_on_line(s.pos + 1, if footnote { "]_" } else { ">" }) else {
                return false;
            };
            let (width, kind) = if footnote { (2, K::String) } else { (1, K::NameTag) };
            s.emit(close + width, kind);
            true
        }
        _ => false,
    }
}

/// The length of a role (`:name:`) at `pos`, or 0; `before_text` requires a backquote
/// after it.
fn role_length(s: &Scanner<'_>, before_text: bool) -> usize {
    let stop = s.limit.min(s.pos + 64);
    for i in s.pos + 1..stop {
        let c = s.bytes[i];
        if c == b':' {
            let ok = i > s.pos + 1 && (!before_text || s.byte_at(i + 1) == b'`');
            return if ok { i + 1 - s.pos } else { 0 };
        }
        if !is_ident_char(c) && c != b'-' {
            return 0;
        }
    }
    0
}

/// Lexes ``` ``literal`` ```, `` `interpreted` ``, `` `reference`_ `` and
/// `` `text <url>`_ ``.
fn rst_backquoted(s: &mut Scanner<'_>) -> bool {
    if s.peek(1) == b'`' {
        let Some(close) = s.find_on_line(s.pos + 2, "``") else { return false };
        s.emit(close + 2, K::String);
        return true;
    }
    let Some(close) = s.find_on_line(s.pos + 1, "`") else { return false };
    if s.byte_at(close + 1) == b'_' {
        let end = close + if s.byte_at(close + 2) == b'_' { 3 } else { 2 };
        let open_url = s.find_on_line(s.pos, "<");
        if let Some(open_url) = open_url.filter(|&o| o > 0 && o < close)
            && s.bytes[close - 1] == b'>'
        {
            s.emit(open_url, K::String);
            s.emit(close, K::StringInterpol);
        }
        s.emit(end, K::String);
        return true;
    }
    s.emit(close + 1, K::NameVariable);
    if s.peek(0) == b':' {
        let n = role_length(s, false);
        if n > 0 {
            s.emit_n(n, K::NameAttribute);
        }
    }
    true
}

// Markdown, after Pygments' Markdown lexer; fenced code is lexed as its language.

/// `st.mode`: inside a fenced block whose code an inner lexer lexes.
const MD_FENCE: u8 = 1;

pub(crate) fn markdown_step(s: &mut Scanner<'_>, st: &mut State) {
    if s.at_line_start() && st.mode == MD_FENCE {
        // The closing fence of a block whose code an inner lexer has lexed.
        st.mode = 0;
        s.emit_line(K::StringBacktick);
        return;
    }
    if s.at_line_start() && md_line(s, st) {
        return;
    }
    md_inline(s);
}

/// Lexes what a line's start decides; false to lex the line's inline content.
fn md_line(s: &mut Scanner<'_>, st: &mut State) -> bool {
    let stop = s.line_end(s.pos);
    let line = s.slice(s.pos, stop);
    let indent = s.skip_spaces(s.pos) - s.pos;
    let rest = &line[indent..];
    if rest.starts_with(b"```") || rest.starts_with(b"~~~") {
        md_fence(s, st, line, indent);
        return true;
    }
    if md_heading(s, line, stop) {
        return true;
    }
    md_list_marker(s, s.pos + indent, stop)
}

/// Lexes an ATX heading (`# Title`) or a setext heading (a title underlined with `=` or
/// `-`).
fn md_heading(s: &mut Scanner<'_>, line: &[u8], stop: usize) -> bool {
    if line.len() > 1 && line[0] == b'#' {
        let level = s.skip_set(s.pos, "#") - s.pos;
        if level <= 6 && level < line.len() && line[level] != b'#' {
            s.emit(stop, if level == 1 { K::GenericHeading } else { K::GenericSubheading });
            return true;
        }
    }
    if is_blank(line) || stop >= s.limit {
        return false;
    }
    let next = s.slice(stop + 1, s.line_end(stop + 1));
    let underline = trim_right(next);
    if underline.is_empty() || !(all_bytes(underline, b'=') || all_bytes(underline, b'-')) {
        return false;
    }
    let kind = if underline[0] == b'=' { K::GenericHeading } else { K::GenericSubheading };
    s.emit(stop, kind);
    s.emit_n(1, K::Text);
    s.emit(stop + 1 + next.len(), kind);
    true
}

/// Lexes a list item's or a block quote's marker at `marker`.
fn md_list_marker(s: &mut Scanner<'_>, marker: usize, stop: usize) -> bool {
    let c = s.byte_at(marker);
    if (c == b'*' || c == b'-' || c == b'+') && s.byte_at(marker + 1) == b' ' {
        s.emit(marker, K::Whitespace);
        s.emit_n(1, K::Keyword);
        s.emit_n(1, K::Whitespace);
        if s.looking_at("[ ]", 0) || s.looking_at("[x]", 0) || s.looking_at("[X]", 0) {
            s.emit_n(3, K::Keyword);
        }
        return true;
    }
    if c.is_ascii_digit() {
        let d = s.skip_digits(marker);
        if s.byte_at(d) != b'.' || s.byte_at(d + 1) != b' ' {
            return false;
        }
        s.emit(marker, K::Whitespace);
        s.emit(d + 1, K::Keyword);
        return true;
    }
    if c == b'>' && s.byte_at(marker + 1) == b' ' {
        s.emit(marker + 2, K::Keyword);
        s.emit(stop, K::GenericEmph);
        return true;
    }
    false
}

fn all_bytes(s: &[u8], c: u8) -> bool {
    s.iter().all(|&b| b == c)
}

/// Lexes a fenced code block: the fences are `String.Backtick` and the code is lexed as
/// the fence's language, or `String.Backtick` when there is none.
fn md_fence(s: &mut Scanner<'_>, st: &mut State, line: &[u8], indent: usize) {
    let fence = &line[indent..indent + 3];
    let mut info = trim_spaces(&line[indent + 3..]);
    if let Some(i) = info.iter().position(|&c| is_space(c) || c == b'{' || c == b'}') {
        info = &info[..i];
    }
    let body = s.line_after(s.pos);
    let mut end = body;
    while end < s.limit {
        let stop = s.line_end(end);
        if trim_spaces(s.slice(end, stop)).starts_with(fence) {
            break;
        }
        end = s.line_after(stop);
    }
    let lexer = std::str::from_utf8(info).ok().and_then(find_lexer);
    match lexer {
        Some(lexer) if !info.is_empty() => {
            let fence_end = s.pos + indent + 3;
            s.emit(fence_end, K::StringBacktick);
            s.emit(fence_end + info.len(), K::NameLabel);
            s.emit(body, K::Text);
            let lexer = resolve_lexer(lexer, &s.code[body..end]);
            s.start_inner(lexer, end, true);
            st.mode = MD_FENCE;
        }
        _ => s.emit(s.line_end(end), K::StringBacktick),
    }
}

fn md_inline(s: &mut Scanner<'_>) {
    let c = s.peek(0);
    if md_inline_markup(s, c) {
        return;
    }
    let i = s.skip_by(s.pos + 1, |b| !has("\\`*_~@#[!\n", b));
    s.emit(i, K::Text);
}

/// Lexes escapes, code spans, emphasis, mentions and links; false when none starts at
/// `pos`.
fn md_inline_markup(s: &mut Scanner<'_>, c: u8) -> bool {
    match c {
        b'\n' => {
            s.emit_n(1, K::Text);
            true
        }
        b'\\' => {
            s.emit(s.pos + 1 + s.rune_size(s.pos + 1), K::Text);
            true
        }
        b'`' => match s.find_on_line(s.pos + 1, "`") {
            Some(close) if close > s.pos + 1 => {
                s.emit(close + 1, K::StringBacktick);
                true
            }
            _ => false,
        },
        b'*' | b'_' | b'~' => md_emphasis(s, c),
        b'@' | b'#' => {
            let end = s.skip_by(s.pos + 1, |b| is_ident_char(b) || b == b'/' || b == b':');
            if end > s.pos + 1 && (s.pos == 0 || is_space(s.before())) {
                s.emit(end, K::NameEntity);
                return true;
            }
            false
        }
        b'[' | b'!' => md_link(s),
        _ => false,
    }
}

fn md_emphasis(s: &mut Scanner<'_>, c: u8) -> bool {
    let double = s.peek(1) == c;
    if c == b'~' && !double {
        return false;
    }
    let width = if double { 2 } else { 1 };
    let first = s.peek(width);
    if first == b' ' || first == b'\n' || first == c || first == 0 {
        return false;
    }
    let marker = match (c, double) {
        (b'*', false) => "*",
        (b'*', true) => "**",
        (b'_', false) => "_",
        (b'_', true) => "__",
        _ => "~~",
    };
    let Some(close) = s.find_on_line(s.pos + width + 1, marker) else { return false };
    let kind = match (double, c) {
        (true, b'~') => K::GenericDeleted,
        (true, _) => K::GenericStrong,
        (false, _) => K::GenericEmph,
    };
    s.emit(close + width, kind);
    true
}

/// Lexes `[text](url)`, `![alt](src)`, `[text][label]` and `[label]: url`.
fn md_link(s: &mut Scanner<'_>) -> bool {
    let open = s.pos + usize::from(s.peek(0) == b'!');
    if s.byte_at(open) != b'[' {
        return false;
    }
    let Some(close) = s.find_on_line(open + 1, "]").filter(|&c| c != open + 1) else {
        return false;
    };
    let after = s.byte_at(close + 1);
    if after != b'(' && after != b'[' {
        if after == b':' && s.at_line_start() {
            s.emit(open + 1, K::Text);
            s.emit(close, K::NameLabel);
            s.emit(s.skip_spaces(close + 2), K::Text);
            s.emit_line(K::NameAttribute);
            return true;
        }
        return false;
    }
    let Some(end) = s.find_on_line(close + 2, if after == b'(' { ")" } else { "]" }) else {
        return false;
    };
    s.emit(open + 1, K::Text);
    s.emit(close, K::NameTag);
    s.emit(close + 2, K::Text);
    s.emit(end, if after == b'(' { K::NameAttribute } else { K::NameLabel });
    s.emit(end + 1, K::Text);
    true
}
