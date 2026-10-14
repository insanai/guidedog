//! Bash, shell sessions (console), Makefiles and Dockerfiles. Commands inside the latter
//! three are lexed by the Bash lexer as an inner lexer.

use crate::lexers::Lexer;
use crate::scanner::{Scanner, State, has, in_list, in_list_fold, index_byte_from};
use crate::scanner::{is_ident_char, is_ident_start, is_space};
use crate::token::TokenKind as K;

const BASH_KEYWORDS: &[&str] = &[
    "if", "fi", "else", "while", "in", "do", "done", "for", "then", "return", "function", "case",
    "select", "break", "continue", "until", "esac", "elif",
];

const BASH_BUILTINS: &[&str] = &[
    "alias", "bg", "bind", "break", "builtin", "caller", "cd", "command", "compgen", "complete",
    "declare", "dirs", "disown", "echo", "enable", "eval", "exec", "exit", "export", "false",
    "fc", "fg", "getopts", "hash", "help", "history", "jobs", "kill", "let", "local", "logout",
    "popd", "printf", "pushd", "pwd", "read", "readonly", "set", "shift", "shopt", "source",
    "suspend", "test", "time", "times", "trap", "true", "type", "typeset", "ulimit", "umask",
    "unalias", "unset", "wait",
];

const FRAME_DQ: u8 = 1; // "..."
const FRAME_PAREN: u8 = 2; // $( ... )
const FRAME_MATH: u8 = 3; // $(( ... ))
const FRAME_BACKTICK: u8 = 4;
const FRAME_CURLY: u8 = 5; // ${ ... }

/// `st.mode`: lines up to the heredoc's delimiter.
const BASH_HEREDOC_BODY: u8 = 1;

/// `st.flags`: a heredoc starts after this line.
const HEREDOC_PENDING: u32 = 1;
/// `st.flags`: `<<-`, so the delimiter line may be indented with tabs.
const HEREDOC_TABS: u32 = 2;

/// Bytes that end a plain shell word.
const WORD_STOP: &str = "=[]{}()$\"'`\\<&|; \t\n\r";

pub(crate) fn bash_step(s: &mut Scanner<'_>, st: &mut State) {
    if st.mode == BASH_HEREDOC_BODY {
        return heredoc_line(s, st);
    }
    if let Some(f) = st.top() {
        let c = s.peek(0);
        match f.mode {
            FRAME_DQ => return double_quoted(s, st),
            FRAME_CURLY => {
                if curly(s, st) {
                    return;
                }
            }
            FRAME_PAREN => {
                if c == b')' && f.count == 0 {
                    s.emit_n(1, K::Keyword);
                    st.pop();
                    return;
                }
                if let Some(top) = st.top_mut() {
                    if c == b'(' {
                        top.count = top.count.wrapping_add(1);
                    }
                    if c == b')' {
                        top.count = top.count.wrapping_sub(1);
                    }
                }
            }
            FRAME_MATH if s.looking_at("))", 0) => {
                s.emit_n(2, K::Keyword);
                st.pop();
                return;
            }
            FRAME_BACKTICK if c == b'`' => {
                s.emit_n(1, K::StringBacktick);
                st.pop();
                return;
            }
            _ => {}
        }
    }
    bash_token(s, st);
}

fn bash_token(s: &mut Scanner<'_>, st: &mut State) {
    let c = s.peek(0);
    match c {
        b'\n' => {
            s.emit_n(1, K::Whitespace);
            if st.flags & HEREDOC_PENDING != 0 {
                st.flags &= !HEREDOC_PENDING;
                st.mode = BASH_HEREDOC_BODY;
            }
        }
        b' ' | b'\t' | b'\r' => s.emit(s.skip_set(s.pos, " \t\r"), K::Whitespace),
        b'#' if s.pos == 0 || has(" \t\n;&|(`", s.before()) => {
            let bang = s.pos == 0 && s.peek(1) == b'!';
            s.emit_line(if bang { K::CommentHashbang } else { K::CommentSingle });
        }
        b'\\' => s.emit(s.pos + 1 + s.rune_size(s.pos + 1), K::StringEscape),
        b'$' => dollar(s, st),
        b'"' => {
            s.emit_n(1, K::StringDouble);
            st.push(FRAME_DQ, 0, 0);
        }
        b'\'' => {
            let close = s.find_from(s.pos + 1, "'");
            s.emit(close.map_or(s.limit, |c| c + 1), K::StringSingle);
        }
        b'`' => {
            s.emit_n(1, K::StringBacktick);
            st.push(FRAME_BACKTICK, 0, 0);
        }
        b'<' if s.looking_at("<<<", 0) => s.emit_n(3, K::Operator),
        b'<' if s.looking_at("<<", 0) => heredoc_start(s, st),
        b'<' => s.emit_n(1, K::Text),
        b'&' | b'|' if s.peek(1) == c => s.emit_n(2, K::Operator),
        b'&' | b'|' | b';' => s.emit_n(1, K::Punctuation),
        b'[' | b']' | b'{' | b'}' | b'(' | b')' | b'=' => s.emit_n(1, K::Operator),
        _ => bash_word(s),
    }
}

fn bash_word(s: &mut Scanner<'_>) {
    let end = s.skip_by(s.pos + 1, |c| !has(WORD_STOP, c));
    let w = s.slice(s.pos, end);
    let next = s.byte_at(end);
    let all_digits = w.iter().all(u8::is_ascii_digit);
    let identifier = is_ident_start(w[0]) && w.iter().all(|&c| is_ident_char(c));
    let assigns = next == b'=' || (next == b'+' && s.byte_at(end + 1) == b'=');
    let ends_command = end >= s.limit || has(" \t\r\n)`;", next);
    let kind = if all_digits && !is_ident_char(next) {
        K::Number
    } else if identifier && assigns {
        K::NameVariable
    } else if identifier && in_list(w, BASH_KEYWORDS) {
        K::Keyword
    } else if identifier && in_list(w, BASH_BUILTINS) && ends_command {
        K::NameBuiltin
    } else {
        K::Text
    };
    s.emit(end, kind);
    if kind == K::NameVariable && next == b'+' {
        s.emit_n(2, K::Operator);
    }
}

/// Lexes `$var`, `${...}`, `$(...)`, `$((...))`, `$'...'` and `$"..."`.
fn dollar(s: &mut Scanner<'_>, st: &mut State) {
    let next = s.peek(1);
    if s.looking_at("$((", 0) {
        s.emit_n(3, K::Keyword);
        st.push(FRAME_MATH, 0, 0);
    } else if next == b'(' {
        s.emit_n(2, K::Keyword);
        st.push(FRAME_PAREN, 0, 0);
    } else if next == b'{' {
        s.emit(s.pos + if s.peek(2) == b'#' { 3 } else { 2 }, K::StringInterpol);
        st.push(FRAME_CURLY, 0, 0);
    } else if is_ident_start(next) && next < 0x80 {
        s.emit(s.ident_end(s.pos + 1, ""), K::NameVariable);
    } else if next.is_ascii_digit() {
        s.emit(s.skip_digits(s.pos + 1), K::NameVariable);
    } else if next != 0 && has("#$?!_*@-", next) {
        s.emit_n(2, K::NameVariable);
    } else if next == b'\'' {
        let mut i = s.pos + 2;
        while i < s.limit && s.bytes[i] != b'\'' {
            i += if s.bytes[i] == b'\\' { 2 } else { 1 };
        }
        s.emit((i + 1).min(s.limit), K::StringSingle);
    } else if next == b'"' {
        s.emit_n(2, K::StringDouble);
        st.push(FRAME_DQ, 0, 0);
    } else {
        s.emit_n(1, K::Text);
    }
}

fn double_quoted(s: &mut Scanner<'_>, st: &mut State) {
    let c = s.peek(0);
    if c == b'"' {
        s.emit_n(1, K::StringDouble);
        st.pop();
        return;
    }
    let next = s.peek(1);
    let expands = next == b'('
        || next == b'{'
        || is_ident_start(next)
        || (next != 0 && has("#$?!_*@-0123456789", next));
    if c == b'$' && expands {
        return dollar(s, st);
    }
    if c == b'`' {
        s.emit_n(1, K::StringBacktick);
        st.push(FRAME_BACKTICK, 0, 0);
        return;
    }
    let mut i = s.pos;
    while i < s.limit && s.bytes[i] != b'"' && s.bytes[i] != b'`' {
        if s.bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if s.bytes[i] == b'$' && i > s.pos {
            break;
        }
        i += 1;
    }
    s.emit(i.min(s.limit).max(s.pos + 1), K::StringDouble);
}

/// Lexes inside `${...}`; true when it emitted.
fn curly(s: &mut Scanner<'_>, st: &mut State) -> bool {
    let c = s.peek(0);
    if c == b'}' {
        s.emit_n(1, K::StringInterpol);
        st.pop();
    } else if s.looking_at(":-", 0) {
        s.emit_n(2, K::Keyword);
    } else if is_ident_char(c) && c < 0x80 {
        s.emit(s.ident_end(s.pos, ""), K::NameVariable);
    } else if c == b':' {
        s.emit_n(1, K::Punctuation);
    } else if has("\"'`$\\\n", c) {
        return false;
    } else {
        let i = s.skip_by(s.pos, |b| !has("}:\"'`$\\\n", b) && !is_ident_char(b));
        s.emit(i.max(s.pos + 1), K::Punctuation);
    }
    true
}

/// Lexes `<<WORD`, `<<-WORD`, `<<'WORD'` and `<<"WORD"`; the body follows the line.
fn heredoc_start(s: &mut Scanner<'_>, st: &mut State) {
    let mut i = s.pos + 2;
    let dash = s.byte_at(i) == b'-';
    if dash {
        i += 1;
    }
    i = s.skip_spaces(i);
    let mut quote = s.byte_at(i);
    if quote == b'\'' || quote == b'"' {
        i += 1;
    } else {
        quote = 0;
    }
    if s.byte_at(i) == b'\\' {
        i += 1;
    }
    let word = i;
    i = s.skip_by(i, |c| is_ident_char(c) || c == b'-' || c == b'.');
    if i == word {
        s.emit_n(2, K::Operator);
        return;
    }
    st.mark = [word, i];
    if quote != 0 && s.byte_at(i) == quote {
        i += 1;
    }
    st.flags |= HEREDOC_PENDING;
    if dash {
        st.flags |= HEREDOC_TABS;
    } else {
        st.flags &= !HEREDOC_TABS;
    }
    s.emit(i, K::StringHeredoc);
}

fn heredoc_line(s: &mut Scanner<'_>, st: &mut State) {
    let stop = s.line_end(s.pos);
    let mut line = s.slice(s.pos, stop);
    if st.flags & HEREDOC_TABS != 0 {
        while let [b'\t', rest @ ..] = line {
            line = rest;
        }
    }
    if let [rest @ .., b'\r'] = line {
        line = rest;
    }
    if line == s.slice(st.mark[0], st.mark[1]) {
        st.mode = 0;
    }
    s.emit(stop, K::StringHeredoc);
    s.emit(s.line_after(s.pos), K::Whitespace);
}

/// Whether a Bash state is inside a construct that spans lines.
fn bash_continues(st: &State) -> bool {
    st.lexer == Lexer::Bash
        && (st.depth > 0 || st.mode == BASH_HEREDOC_BODY || st.flags & HEREDOC_PENDING != 0)
}

// Shell sessions: a prompt, then a command lexed as Bash; other lines are output.

/// `st.flags`: the last command line ended with a backslash.
const CONSOLE_CONTINUED: u32 = 1;

pub(crate) fn console_step(s: &mut Scanner<'_>, st: &mut State) {
    let stop = s.line_end(s.pos);
    let after = s.line_after(s.pos);
    if !s.at_line_start() {
        s.emit(after, K::GenericOutput);
        return;
    }
    let line = s.slice(s.pos, stop);
    let continued = st.flags & CONSOLE_CONTINUED != 0 || bash_continues(&s.inner);
    st.flags &= !CONSOLE_CONTINUED;
    if continued {
        if line.starts_with(b"> ") {
            s.emit_n(2, K::GenericPrompt);
        }
        mark_continuation(st, line);
        s.start_inner(Lexer::Bash, after, false);
        return;
    }
    let n = prompt_length(line);
    if n > 0 {
        s.emit_n(n, K::GenericPrompt);
        mark_continuation(st, line);
        s.start_inner(Lexer::Bash, after, true);
        return;
    }
    s.emit(after, K::GenericOutput);
}

fn mark_continuation(st: &mut State, line: &[u8]) {
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    if line.ends_with(b"\\") {
        st.flags |= CONSOLE_CONTINUED;
    }
}

/// The length of a shell prompt that starts `line`, with the spaces after it, or 0.
///
/// Prompts end in `$`, `#` or `%` followed by a space or the line's end, after an optional
/// `(venv)`, and a `user@host:path`, `[user@host dir]` or `sh-5.2` style prefix.
pub(crate) fn prompt_length(line: &[u8]) -> usize {
    let mut i = 0;
    if line.first() == Some(&b'(')
        && let Some(close) = index_byte_from(line, 0, b')').filter(|&c| c > 0)
        && !line[..close].contains(&b' ')
    {
        i = close + 1;
        while i < line.len() && line[i] == b' ' {
            i += 1;
        }
    }
    if line.get(i) == Some(&b'[')
        && let Some(close) = index_byte_from(line, i, b']').filter(|&c| c > 0)
    {
        return prompt_symbol(line, close + 1);
    }
    let n = prompt_symbol(line, i);
    if n > 0 {
        return n;
    }
    prefixed_prompt_length(line, i)
}

/// The length of a prompt whose symbol follows a `user@host:path` or `sh-5.2` token.
fn prefixed_prompt_length(line: &[u8], start: usize) -> usize {
    let mut token_end = start;
    while token_end < line.len() && line[token_end] != b' ' && line[token_end] != b'\t' {
        token_end += 1;
    }
    let token = &line[start..token_end];
    if token.len() < 2 {
        return 0;
    }
    let prefixed = token.starts_with(b"sh")
        || index_byte_from(token, 0, b'@').is_some_and(|i| i > 0)
        || index_byte_from(token, 0, b':').is_some_and(|i| i > 0);
    if !prefixed {
        return 0;
    }
    let last = token[token.len() - 1];
    if has("$#%", last) && prompt_symbol(line, token_end - 1) > 0 {
        return prompt_symbol(line, token_end - 1);
    }
    let n = prompt_symbol(line, token_end);
    if n > 0 {
        return n;
    }
    // user@host dir$
    let mut j = token_end;
    while j < line.len() && line[j] == b' ' {
        j += 1;
    }
    let mut k = j;
    while k < line.len() && line[k] != b' ' {
        k += 1;
    }
    if k > j + 1 && has("$#%", line[k - 1]) {
        return prompt_symbol(line, k - 1);
    }
    0
}

/// The prompt's length when spaces then `$`, `#` or `%` start at `from`.
fn prompt_symbol(line: &[u8], from: usize) -> usize {
    let mut i = from;
    while i < line.len() && (line[i] == b' ' || line[i] == b'\t') {
        i += 1;
    }
    if i >= line.len() || !has("$#%", line[i]) {
        return 0;
    }
    i += 1;
    if i < line.len() && !has(" \t\r", line[i]) {
        return 0;
    }
    while i < line.len() && (line[i] == b' ' || line[i] == b'\t') {
        i += 1;
    }
    i
}

// Makefiles.

const MAKE_DIRECTIVES: &[&str] = &[
    "include", "-include", "sinclude", "ifeq", "ifneq", "ifdef", "ifndef", "else", "endif",
    "define", "endef", "export", "unexport", "override", "vpath", "undefine", "private",
];

/// `st.mode`: after a target's colon.
const MAKE_HEADER: u8 = 1;
const MAKE_TEXT: u8 = 2;
const FRAME_EXPANSION: u8 = 1;

const WORD_LETTERS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_";

pub(crate) fn makefile_step(s: &mut Scanner<'_>, st: &mut State) {
    if st.depth > 0 {
        return expansion(s, st);
    }
    let c = s.peek(0);
    if c == b'\n' {
        s.emit_n(1, K::Whitespace);
        st.mode = 0;
        return;
    }
    if s.at_line_start() && st.mode == 0 && make_line_start(s, st, c) {
        return;
    }
    if (c == b'$' && s.peek(1) == b'(') || s.looking_at("${", 0) {
        s.emit_n(2, K::Keyword);
        st.push(FRAME_EXPANSION, 0, 0);
    } else if c == b'$' && s.peek(1) != 0 && has("<@$+%?|*^", s.peek(1)) {
        s.emit_n(2, K::Keyword);
    } else if c == b'#' {
        s.emit_line(K::Comment);
    } else if c == b' ' || c == b'\t' {
        s.emit(s.skip_spaces(s.pos), K::Whitespace);
    } else if st.mode == MAKE_HEADER {
        make_header(s, c);
    } else {
        make_text(s, c);
    }
}

/// Lexes the prerequisites after a target's colon.
fn make_header(s: &mut Scanner<'_>, c: u8) {
    if c == b',' || c == b'|' {
        s.emit_n(1, K::Punctuation);
    } else if c.is_ascii_alphabetic() || c == b'_' {
        s.emit(s.skip_set(s.pos, WORD_LETTERS), K::Name);
    } else if c == b'\\' {
        s.emit_n(if s.peek(1) == b'\n' { 2 } else { 1 }, K::Text);
    } else {
        s.emit_rune(K::Text);
    }
}

fn make_text(s: &mut Scanner<'_>, c: u8) {
    if c == b'"' || c == b'\'' {
        let quote = if c == b'"' { "\"" } else { "'" };
        if let Some(close) = s.find_on_line(s.pos + 1, quote) {
            let kind = if c == b'"' { K::StringDouble } else { K::StringSingle };
            s.emit(close + 1, kind);
            return;
        }
    }
    s.emit_rune(K::Text);
}

/// Lexes what a line's start decides: recipes, comments, directives, assignments and
/// rules. It returns false to lex the line as text.
fn make_line_start(s: &mut Scanner<'_>, st: &mut State, c: u8) -> bool {
    if c == b'\t' || c == b' ' {
        s.start_inner(Lexer::Bash, s.logical_line_end(s.pos), false);
        return true;
    }
    if c == b'#' {
        s.emit_line(K::Comment);
        return true;
    }
    let word_stop = s.skip_by(s.pos, |c| !is_space(c));
    if in_list(s.slice(s.pos, word_stop), MAKE_DIRECTIVES) {
        s.emit(word_stop, K::Keyword);
        st.mode = MAKE_TEXT;
        return true;
    }
    if make_assignment(s) {
        return true;
    }
    let stop = s.line_end(s.pos);
    let colon = index_byte_from(s.slice(s.pos, stop), 0, b':').map_or(stop, |i| s.pos + i);
    if colon < stop && colon > s.pos {
        s.emit(colon, K::NameFunction);
        s.emit(s.skip_set(s.pos, ":"), K::Operator);
        st.mode = MAKE_HEADER;
        return true;
    }
    st.mode = MAKE_TEXT;
    false
}

/// Lexes `NAME = value` and its `:=`, `::=`, `?=`, `+=` and `!=` forms.
fn make_assignment(s: &mut Scanner<'_>) -> bool {
    let name = s.skip_by(s.pos, |c| c.is_ascii_alphanumeric() || has("_${}().-/", c));
    let op = s.skip_spaces(name);
    let mut op_end = op + usize::from(s.byte_at(op) != 0 && has("!?:+", s.byte_at(op)));
    if s.byte_at(op) == b':' && s.byte_at(op + 1) == b':' {
        op_end += 1;
    }
    if name == s.pos || s.byte_at(op_end) != b'=' {
        return false;
    }
    s.emit(name, K::NameVariable);
    s.emit(op, K::Whitespace);
    s.emit(op_end + 1, K::Operator);
    s.emit(s.skip_spaces(s.pos), K::Whitespace);
    s.start_inner(Lexer::Bash, s.logical_line_end(s.pos), false);
    true
}

fn expansion(s: &mut Scanner<'_>, st: &mut State) {
    let c = s.peek(0);
    if c == b')' || c == b'}' {
        s.emit_n(1, K::Keyword);
        st.pop();
    } else if c == b'(' || c == b'{' {
        s.emit_n(1, K::Keyword);
        st.push(FRAME_EXPANSION, 0, 0);
    } else if c == b'$' {
        if s.peek(1) == b'(' || s.peek(1) == b'{' {
            s.emit_n(2, K::Keyword);
            st.push(FRAME_EXPANSION, 0, 0);
        } else {
            s.emit_n(1, K::Keyword);
        }
    } else if is_ident_char(c) || c == b'.' || c == b'-' {
        s.emit(s.ident_end(s.pos, ".-"), K::NameVariable);
    } else if c == b'\n' {
        st.depth = 0;
        s.emit_n(1, K::Whitespace);
    } else {
        s.emit_rune(K::Text);
    }
}

// Dockerfiles: an instruction, then its arguments; shell-form commands are Bash.

const DOCKER_STRING_INSTRUCTIONS: &[&str] =
    &["MAINTAINER", "EXPOSE", "WORKDIR", "USER", "STOPSIGNAL"];
const DOCKER_BASH_INSTRUCTIONS: &[&str] = &[
    "RUN",
    "CMD",
    "ENTRYPOINT",
    "ENV",
    "ARG",
    "LABEL",
    "ADD",
    "COPY",
    "ONBUILD",
    "HEALTHCHECK",
    "SHELL",
    "VOLUME",
];

pub(crate) fn dockerfile_step(s: &mut Scanner<'_>, _st: &mut State) {
    let c = s.peek(0);
    if !s.at_line_start() || is_space(c) {
        if is_space(c) {
            s.emit_spaces(K::Whitespace);
        } else {
            s.start_inner(Lexer::Bash, s.logical_line_end(s.pos), false);
        }
        return;
    }
    if c == b'#' {
        s.emit_line(K::Comment);
        return;
    }
    let word_stop = s.ident_end(s.pos, "");
    let word = s.slice(s.pos, word_stop);
    let after = s.skip_spaces(word_stop);
    if word.eq_ignore_ascii_case(b"FROM") {
        s.emit(word_stop, K::Keyword);
        docker_from(s);
    } else if in_list_fold(word, DOCKER_STRING_INSTRUCTIONS)
        || (word.eq_ignore_ascii_case(b"VOLUME") && s.byte_at(after) != b'[')
    {
        s.emit(word_stop, K::Keyword);
        s.emit(after, K::Whitespace);
        s.emit_line(K::String);
    } else if in_list_fold(word, DOCKER_BASH_INSTRUCTIONS) {
        s.emit(word_stop, K::Keyword);
        s.emit(after, K::Whitespace);
        if s.byte_at(s.pos) == b'[' {
            s.start_inner(Lexer::Json, s.line_end(s.pos), true);
        } else {
            s.start_inner(Lexer::Bash, s.logical_line_end(s.pos), false);
        }
    } else {
        s.start_inner(Lexer::Bash, s.logical_line_end(s.pos), false);
    }
}

/// Lexes `FROM image AS name` after the instruction.
fn docker_from(s: &mut Scanner<'_>) {
    s.emit(s.skip_spaces(s.pos), K::Whitespace);
    let image = s.skip_by(s.pos, |c| !is_space(c));
    s.emit(image, K::String);
    s.emit(s.skip_spaces(s.pos), K::Whitespace);
    if s.looking_at_fold("AS", 0) && is_space(s.peek(2)) {
        s.emit_n(2, K::Keyword);
        s.emit(s.skip_spaces(s.pos), K::Whitespace);
        s.emit_line(K::String);
    }
}
