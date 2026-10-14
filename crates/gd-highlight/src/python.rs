//! Python, Python console sessions (pycon) and tracebacks (pytb), with the token kinds
//! Pygments gives them: docstrings, f-string interpolation, decorators, magic names.

use crate::lexers::Lexer;
use crate::scanner::{Scanner, State, has, in_list, is_ident_char, is_ident_start, is_space};
use crate::scanner::{index_byte_from, trim_spaces};
use crate::token::TokenKind as K;

const PY_KEYWORDS: &[&str] = &[
    "assert",
    "async",
    "await",
    "break",
    "continue",
    "del",
    "elif",
    "else",
    "except",
    "finally",
    "for",
    "global",
    "if",
    "lambda",
    "pass",
    "raise",
    "nonlocal",
    "return",
    "try",
    "while",
    "yield",
    "as",
    "with",
    "print_function",
];

const PY_BUILTINS: &[&str] = &[
    "__import__",
    "abs",
    "aiter",
    "all",
    "any",
    "anext",
    "ascii",
    "bin",
    "bool",
    "breakpoint",
    "bytearray",
    "bytes",
    "callable",
    "chr",
    "classmethod",
    "compile",
    "complex",
    "delattr",
    "dict",
    "dir",
    "divmod",
    "enumerate",
    "eval",
    "exec",
    "filter",
    "float",
    "format",
    "frozenset",
    "getattr",
    "globals",
    "hasattr",
    "hash",
    "hex",
    "id",
    "input",
    "int",
    "isinstance",
    "issubclass",
    "iter",
    "len",
    "list",
    "locals",
    "map",
    "max",
    "memoryview",
    "min",
    "next",
    "object",
    "oct",
    "open",
    "ord",
    "pow",
    "print",
    "property",
    "range",
    "repr",
    "reversed",
    "round",
    "set",
    "setattr",
    "slice",
    "sorted",
    "staticmethod",
    "str",
    "sum",
    "super",
    "tuple",
    "type",
    "vars",
    "zip",
];

const PY_PSEUDO: &[&str] = &["self", "Ellipsis", "NotImplemented", "cls"];

const PY_EXCEPTIONS: &[&str] = &[
    "ArithmeticError",
    "AssertionError",
    "AttributeError",
    "BaseException",
    "BaseExceptionGroup",
    "BlockingIOError",
    "BrokenPipeError",
    "BufferError",
    "BytesWarning",
    "ChildProcessError",
    "ConnectionAbortedError",
    "ConnectionError",
    "ConnectionRefusedError",
    "ConnectionResetError",
    "DeprecationWarning",
    "EOFError",
    "EncodingWarning",
    "EnvironmentError",
    "Exception",
    "ExceptionGroup",
    "FileExistsError",
    "FileNotFoundError",
    "FloatingPointError",
    "FutureWarning",
    "GeneratorExit",
    "IOError",
    "ImportError",
    "ImportWarning",
    "IndentationError",
    "IndexError",
    "InterruptedError",
    "IsADirectoryError",
    "KeyError",
    "KeyboardInterrupt",
    "LookupError",
    "MemoryError",
    "ModuleNotFoundError",
    "NameError",
    "NotADirectoryError",
    "NotImplementedError",
    "OSError",
    "OverflowError",
    "PendingDeprecationWarning",
    "PermissionError",
    "ProcessLookupError",
    "RecursionError",
    "ReferenceError",
    "ResourceWarning",
    "RuntimeError",
    "RuntimeWarning",
    "StopAsyncIteration",
    "StopIteration",
    "SyntaxError",
    "SyntaxWarning",
    "SystemError",
    "SystemExit",
    "TabError",
    "TimeoutError",
    "TypeError",
    "UnboundLocalError",
    "UnicodeDecodeError",
    "UnicodeEncodeError",
    "UnicodeError",
    "UnicodeTranslateError",
    "UnicodeWarning",
    "UserWarning",
    "ValueError",
    "VMSError",
    "Warning",
    "WindowsError",
    "ZeroDivisionError",
];

const PY_MAGIC_VARIABLES: &[&str] = &[
    "__annotations__",
    "__bases__",
    "__class__",
    "__closure__",
    "__code__",
    "__defaults__",
    "__dict__",
    "__doc__",
    "__file__",
    "__func__",
    "__globals__",
    "__kwdefaults__",
    "__module__",
    "__mro__",
    "__name__",
    "__objclass__",
    "__qualname__",
    "__self__",
    "__slots__",
    "__weakref__",
    "__all__",
    "__path__",
    "__spec__",
    "__loader__",
    "__package__",
    "__builtins__",
];

// Modes after a keyword that names what follows.
const PY_FUNCNAME: u8 = 1;
const PY_CLASSNAME: u8 = 2;
const PY_IMPORT: u8 = 3;
const PY_FROM: u8 = 4;

const FRAME_STR: u8 = 1;
const FRAME_FEXPR: u8 = 2;

// String frame data bits.
const STR_DOUBLE: u8 = 1;
const STR_TRIPLE: u8 = 2;
const STR_RAW: u8 = 4;
const STR_FORMAT: u8 = 8;

/// `st.flags`: the whitespace after def, class, import and from is Whitespace, not Text.
const AFTER_KEYWORD: u32 = 1;

pub(crate) fn python_step(s: &mut Scanner<'_>, st: &mut State) {
    if let Some(f) = st.top() {
        if f.mode == FRAME_STR {
            return py_string_step(s, st, f.data);
        }
        if f.mode == FRAME_FEXPR && fexpr_step(s, st) {
            return;
        }
    }
    py_code(s, st);
}

fn py_code(s: &mut Scanner<'_>, st: &mut State) {
    let c = s.peek(0);
    if c == b'\n' || c == b'\r' {
        if st.depth > 0 && !py_triple_open(st) {
            st.depth = 0;
        }
        st.mode = 0;
        s.emit_n(1, K::Whitespace);
        return;
    }
    if st.depth == 0 && s.at_line_start() && py_docstring(s) {
        return;
    }
    if matches!(c, b' ' | b'\t' | 0x0c | 0x0b) {
        let end = s.skip_set(s.pos, " \t\x0c\x0b");
        let in_expr = st.top().is_some_and(|f| f.mode == FRAME_FEXPR);
        let after = st.flags & AFTER_KEYWORD != 0;
        s.emit(end, if after || in_expr { K::Whitespace } else { K::Text });
        return;
    }
    st.flags &= !AFTER_KEYWORD;
    if st.mode != 0 && py_named(s, st, c) {
        return;
    }
    match c {
        b'#' => {
            let bang = s.pos == 0 && s.peek(1) == b'!';
            s.emit_line(if bang { K::CommentHashbang } else { K::CommentSingle });
        }
        b'\\' => s.emit_n(if s.peek(1) == b'\n' { 2 } else { 1 }, K::Text),
        b'0'..=b'9' => py_number(s),
        b'.' if s.peek(1).is_ascii_digit() => py_number(s),
        b'.' => s.emit_n(1, K::Operator),
        b'"' | b'\'' => py_string_open(s, st, s.pos),
        b'@' => py_at(s),
        _ => {
            if py_name_start(s, s.pos) {
                return py_word(s, st);
            }
            if c >= 0x80
                && let Some(ch) = s.char_at(s.pos).filter(|ch| ch.is_whitespace())
            {
                s.emit_n(ch.len_utf8(), K::Text);
                return;
            }
            py_symbol(s, c);
        }
    }
}

fn py_symbol(s: &mut Scanner<'_>, c: u8) {
    if s.looking_at("!=", 0) {
        s.emit_n(2, K::Operator);
    } else if has("-~+/*%=<>&^|:", c) && !(c == b':' && s.peek(1) != b'=') {
        s.emit_n(1, K::Operator);
    } else if has("[]{}:(),;", c) {
        s.emit_n(1, K::Punctuation);
    } else {
        s.emit_rune(K::Error);
    }
}

fn py_triple_open(st: &State) -> bool {
    st.frames().iter().any(|f| f.mode == FRAME_STR && f.data & STR_TRIPLE != 0)
}

/// Lexes a triple-quoted string that starts a line as `String.Doc`, with its indentation
/// as Whitespace, as Pygments does.
fn py_docstring(s: &mut Scanner<'_>) -> bool {
    let i = s.skip_by(s.pos, is_space);
    let mut j = i;
    while j < i + 2 && has("rRuUbB", s.byte_at(j)) {
        j += 1;
    }
    let quote = if s.looking_at("\"\"\"", j - s.pos) {
        "\"\"\""
    } else if s.looking_at("'''", j - s.pos) {
        "'''"
    } else {
        return false;
    };
    let close = s.find_from(j + 3, quote);
    s.emit(i, K::Whitespace);
    s.emit(j, K::StringAffix);
    s.emit(close.map_or(s.limit, |c| c + 3), K::StringDoc);
    true
}

/// Lexes the names after def, class, import and from.
fn py_named(s: &mut Scanner<'_>, st: &mut State, c: u8) -> bool {
    let mode = st.mode;
    if py_name_start(s, s.pos) {
        let end = py_name_end(s, s.pos);
        let w = s.slice(s.pos, end);
        match mode {
            PY_FUNCNAME => {
                s.emit(end, if is_dunder(w) { K::NameFunctionMagic } else { K::NameFunction });
                st.mode = 0;
            }
            PY_CLASSNAME => {
                s.emit(end, K::NameClass);
                st.mode = 0;
            }
            PY_IMPORT => s.emit(end, if w == b"as" { K::Keyword } else { K::NameNamespace }),
            PY_FROM if w == b"import" => {
                s.emit(end, K::KeywordNamespace);
                st.mode = 0;
                st.flags |= AFTER_KEYWORD;
            }
            PY_FROM => {
                s.emit(end, if w == b"None" { K::KeywordConstant } else { K::NameNamespace })
            }
            _ => {}
        }
        return true;
    }
    if c == b'.' && (mode == PY_IMPORT || mode == PY_FROM) {
        s.emit_n(1, K::NameNamespace);
        return true;
    }
    if c == b',' && mode == PY_IMPORT {
        s.emit_n(1, K::Operator);
        return true;
    }
    st.mode = 0;
    false
}

fn is_dunder(w: &[u8]) -> bool {
    w.len() > 4 && w.starts_with(b"__") && w.ends_with(b"__")
}

fn py_at(s: &mut Scanner<'_>) {
    if !py_name_start(s, s.pos + 1) || !s.only_space_before_line() {
        s.emit_n(1, K::Operator);
        return;
    }
    let mut i = py_name_end(s, s.pos + 1);
    while s.byte_at(i) == b'.' && py_name_start(s, i + 1) {
        i = py_name_end(s, i + 1);
    }
    s.emit(i, K::NameDecorator);
}

fn py_word(s: &mut Scanner<'_>, st: &mut State) {
    let end = py_name_end(s, s.pos);
    let w = s.slice(s.pos, end);
    let next = s.byte_at(end);
    if (next == b'"' || next == b'\'') && w.len() <= 2 && w.iter().all(|&c| has("rRuUbBfFtT", c)) {
        py_string_open(s, st, end);
        return;
    }
    let after_dot = s.before() == b'.';
    let import = w == b"import" || w == b"from";
    let kind = if w == b"def" || w == b"class" {
        st.mode = if w == b"def" { PY_FUNCNAME } else { PY_CLASSNAME };
        st.flags |= AFTER_KEYWORD;
        K::Keyword
    } else if import && s.only_space_before_line() {
        st.mode = if w == b"import" { PY_IMPORT } else { PY_FROM };
        st.flags |= AFTER_KEYWORD;
        K::KeywordNamespace
    } else if import {
        if w == b"import" { K::KeywordNamespace } else { K::Keyword }
    } else if in_list(w, PY_KEYWORDS)
        || (in_list(w, &["match", "case", "type"]) && py_soft_keyword(s, end))
    {
        K::Keyword
    } else {
        plain_word_kind(w, after_dot)
    };
    s.emit(end, kind);
}

/// The kind of a word that does not change the lexer's mode.
fn plain_word_kind(w: &[u8], after_dot: bool) -> K {
    if in_list(w, &["True", "False", "None"]) {
        K::KeywordConstant
    } else if in_list(w, &["in", "is", "and", "or", "not"]) {
        K::OperatorWord
    } else if after_dot {
        K::Name
    } else if in_list(w, PY_BUILTINS) {
        K::NameBuiltin
    } else if in_list(w, PY_PSEUDO) {
        K::NameBuiltinPseudo
    } else if in_list(w, PY_EXCEPTIONS) {
        K::NameException
    } else if is_dunder(w) {
        if in_list(w, PY_MAGIC_VARIABLES) { K::NameVariableMagic } else { K::NameFunctionMagic }
    } else {
        K::Name
    }
}

/// match, case and type are keywords when they start a statement.
fn py_soft_keyword(s: &Scanner<'_>, end: usize) -> bool {
    if !s.only_space_before_line() {
        return false;
    }
    let next = s.skip_spaces(end);
    if next == end || next >= s.limit {
        return false;
    }
    !has(":,;=^&|@~)]}.(\n", s.bytes[next])
}

fn py_number(s: &mut Scanner<'_>) {
    let radix = if s.peek(0) == b'0' { s.peek(1).to_ascii_lowercase() } else { 0 };
    let (i, kind) = match radix {
        b'x' => (s.skip_by(s.pos + 2, |c| c.is_ascii_hexdigit() || c == b'_'), K::NumberHex),
        b'o' => (s.skip_set(s.pos + 2, "01234567_"), K::NumberOct),
        b'b' => (s.skip_set(s.pos + 2, "01_"), K::NumberBin),
        _ => py_decimal(s),
    };
    s.emit(i, kind);
}

fn py_decimal(s: &Scanner<'_>) -> (usize, K) {
    const DIGITS: &str = "0123456789_";
    let mut kind = K::NumberInteger;
    let mut i = s.skip_set(s.pos, DIGITS);
    if s.byte_at(i) == b'.' {
        i = s.skip_set(i + 1, DIGITS);
        kind = K::NumberFloat;
    }
    let sign = usize::from(s.byte_at(i + 1) == b'+' || s.byte_at(i + 1) == b'-');
    if s.byte_at(i).eq_ignore_ascii_case(&b'e') && s.byte_at(i + 1 + sign).is_ascii_digit() {
        i = s.skip_set(i + 1 + sign, DIGITS);
        kind = K::NumberFloat;
    }
    if s.byte_at(i).eq_ignore_ascii_case(&b'j') {
        i += 1;
    }
    if s.byte_at(i) == b'L' || s.byte_at(i) == b'l' {
        i += 1;
        kind = K::NumberIntegerLong;
    }
    (i, kind)
}

fn py_string_open(s: &mut Scanner<'_>, st: &mut State, at: usize) {
    let prefix = s.slice(s.pos, at);
    s.emit(at, K::StringAffix);
    let q = s.byte_at(at);
    let mut data = if q == b'"' { STR_DOUBLE } else { 0 };
    let mut width = 1;
    if s.byte_at(at + 1) == q && s.byte_at(at + 2) == q {
        data |= STR_TRIPLE;
        width = 3;
    }
    for c in prefix {
        match c.to_ascii_lowercase() {
            b'r' => data |= STR_RAW,
            b'f' | b't' => data |= STR_FORMAT,
            _ => {}
        }
    }
    s.emit(at + width, py_string_kind(data));
    st.push(FRAME_STR, data, 0);
}

fn py_string_kind(data: u8) -> K {
    if data & STR_DOUBLE != 0 { K::StringDouble } else { K::StringSingle }
}

fn py_string_step(s: &mut Scanner<'_>, st: &mut State, data: u8) {
    let kind = py_string_kind(data);
    let q = if data & STR_DOUBLE != 0 { b'"' } else { b'\'' };
    let triple = data & STR_TRIPLE != 0;
    let c = s.peek(0);
    if c == q && (!triple || (s.peek(1) == q && s.peek(2) == q)) {
        s.emit_n(if triple { 3 } else { 1 }, kind);
        st.pop();
        return;
    }
    if c == b'\n' && !triple {
        st.depth = 0;
        s.emit_n(1, K::Whitespace);
        return;
    }
    if c == b'\\' {
        return py_escape(s, data, kind);
    }
    if data & STR_FORMAT != 0 {
        if s.looking_at("{{", 0) || s.looking_at("}}", 0) {
            s.emit_n(2, K::StringEscape);
            return;
        }
        if c == b'{' {
            s.emit_n(1, K::StringInterpol);
            st.push(FRAME_FEXPR, 0, 0);
            return;
        }
        if c == b'}' {
            s.emit_n(1, K::StringInterpol);
            return;
        }
    } else if c == b'%' || c == b'{' {
        let n = format_field(s, c);
        if n > 0 {
            s.emit_n(n, K::StringInterpol);
            return;
        }
    }
    let i = s.skip_by(s.pos + 1, |b| !has("\\\n{}%", b) && b != q);
    s.emit(i, kind);
}

fn py_escape(s: &mut Scanner<'_>, data: u8, kind: K) {
    let next = s.peek(1);
    if data & STR_RAW != 0 {
        let q = if data & STR_DOUBLE != 0 { b'"' } else { b'\'' };
        if next == b'\\' || next == q || next == b'\n' {
            s.emit_n(2, K::StringEscape);
        } else {
            s.emit_n(1, kind);
        }
        return;
    }
    if next == b'N'
        && s.peek(2) == b'{'
        && let Some(end) = s.find_on_line(s.pos, "}")
    {
        s.emit(end + 1, K::StringEscape);
        return;
    }
    if has("\\abfnrtv\"'\n01234567xuU", next) {
        s.emit_n(s.escape_length(s.pos), K::StringEscape);
        return;
    }
    s.emit_n(1, kind);
}

/// The length of a %-format or str.format field at `pos`, or 0.
fn format_field(s: &Scanner<'_>, c: u8) -> usize {
    let mut i = s.pos + 1;
    if c == b'%' {
        if s.byte_at(i) == b'(' {
            match s.find_on_line(i, ")") {
                Some(close) => i = close + 1,
                None => return 0,
            }
        }
        i = s.skip_set(i, "-#0 +");
        i = s.skip_set(i, "0123456789*");
        if s.byte_at(i) == b'.' {
            i = s.skip_set(i + 1, "0123456789*");
        }
        if has("hlL", s.byte_at(i)) {
            i += 1;
        }
        let conversion = s.byte_at(i);
        return if conversion != 0 && has("diouxXeEfFgGcrsa%", conversion) {
            i + 1 - s.pos
        } else {
            0
        };
    }
    let stop = s.limit.min(s.pos + 64);
    while i < stop {
        let b = s.bytes[i];
        if b == b'}' {
            return i + 1 - s.pos;
        }
        if !is_ident_char(b) && !has(".[]!:<>=^+- #,%", b) {
            return 0;
        }
        i += 1;
    }
    0
}

/// Ends an f-string expression; true when it emitted.
fn fexpr_step(s: &mut Scanner<'_>, st: &mut State) -> bool {
    let c = s.peek(0);
    let Some(f) = st.top() else { return false };
    if f.count == 0 {
        let mut i = s.pos;
        if c == b'=' && s.peek(1) != b'=' {
            i = s.skip_spaces(i + 1);
        }
        if s.byte_at(i) == b'!' && s.byte_at(i + 1) != 0 && has("sraf", s.byte_at(i + 1)) {
            i += 2;
        }
        let walrus = c == b':' && s.peek(1) == b'=';
        if (s.byte_at(i) == b'}' || (s.byte_at(i) == b':' && !walrus))
            && (i > s.pos || c == b'}' || c == b':')
        {
            s.emit(i + 1, K::StringInterpol);
            st.pop();
            return true;
        }
    }
    if let Some(f) = st.top_mut() {
        if has("([{", c) && c != 0 {
            f.count = f.count.saturating_add(1);
        }
        if has(")]}", c) && c != 0 && f.count > 0 {
            f.count -= 1;
        }
    }
    false
}

// Python console sessions: prompts, Python after them, output, and tracebacks.

/// `st.flags`: inside a traceback.
const PYCON_TRACEBACK: u32 = 1;
const TRACEBACK: &[u8] = b"Traceback (most recent call last):";

pub(crate) fn pycon_step(s: &mut Scanner<'_>, st: &mut State) {
    if !s.at_line_start() {
        let kind = if s.peek(0) == b'\n' { K::Whitespace } else { K::GenericOutput };
        s.emit(s.line_after(s.pos), kind);
        return;
    }
    if st.flags & PYCON_TRACEBACK != 0 {
        if !traceback_line(s, st) {
            st.flags &= !PYCON_TRACEBACK;
        }
        return;
    }
    let line = s.slice(s.pos, s.line_end(s.pos));
    for prompt in [b">>>", b"..."] {
        if !line.starts_with(prompt) {
            continue;
        }
        if line.len() == 3 {
            s.emit_n(3, K::GenericPrompt);
            return;
        }
        if line[3] != b' ' {
            continue;
        }
        s.emit_n(4, K::GenericPrompt);
        let fresh = prompt == b">>>" && !inner_continues(s);
        s.start_inner(Lexer::Python, s.line_after(s.pos), fresh);
        return;
    }
    if line.starts_with(TRACEBACK) {
        s.emit(s.line_after(s.pos), K::GenericTraceback);
        st.flags |= PYCON_TRACEBACK;
        st.mode = 1;
        return;
    }
    s.emit(s.line_after(s.pos), K::GenericOutput);
}

/// Whether the Python being typed is inside a multi-line string.
fn inner_continues(s: &Scanner<'_>) -> bool {
    s.inner.lexer == Lexer::Python && py_triple_open(&s.inner)
}

pub(crate) fn pytb_step(s: &mut Scanner<'_>, st: &mut State) {
    if !s.at_line_start() {
        let kind = if s.peek(0) == b'\n' { K::Whitespace } else { K::Other };
        s.emit(s.line_after(s.pos), kind);
        return;
    }
    if st.mode != 0 {
        traceback_line(s, st);
        return;
    }
    let line = s.slice(s.pos, s.line_end(s.pos));
    if line.starts_with(TRACEBACK) || line.starts_with(b"^CTraceback (most recent call last):") {
        s.emit(s.line_after(s.pos), K::GenericTraceback);
        st.mode = 1;
    } else if line.starts_with(b"During handling of the above exception")
        || line.starts_with(b"The above exception was the direct cause")
    {
        s.emit(s.line_after(s.pos), K::GenericTraceback);
    } else if line.starts_with(b"  File \"") {
        st.mode = 1;
        traceback_line(s, st);
    } else {
        s.emit(s.line_after(s.pos), K::Other);
    }
}

/// Lexes one line inside a traceback (`st.mode` 1, or 2 after a source line, when a
/// marker line may follow). It returns false after the exception line.
fn traceback_line(s: &mut Scanner<'_>, st: &mut State) -> bool {
    let line_stop = s.line_end(s.pos);
    let line = s.slice(s.pos, line_stop);
    let after_source = st.mode == 2;
    st.mode = 1;
    if line.starts_with(b"  File \"") {
        traceback_file_line(s, line_stop);
        return true;
    }
    let indent = s.skip_spaces(s.pos) - s.pos;
    if after_source
        && indent >= 4
        && indent < line.len()
        && line[indent..].iter().all(|&c| c == b'~' || c == b'^')
    {
        s.emit_n(indent, K::Whitespace);
        s.emit(line_stop, K::PunctuationMarker);
        return true;
    }
    if indent >= 4 && indent < line.len() {
        s.emit_n(4, K::Whitespace);
        s.start_inner(Lexer::Python, line_stop, true);
        st.mode = 2;
        return true;
    }
    if trim_spaces(line) == b"..." {
        s.emit_n(indent, K::Whitespace);
        s.emit(line_stop, K::Comment);
        return true;
    }
    exception_line(s, st, line, line_stop)
}

fn exception_line(s: &mut Scanner<'_>, st: &mut State, line: &[u8], line_stop: usize) -> bool {
    let colon = index_byte_from(line, 0, b':');
    if let Some(colon) = colon.filter(|&c| c > 0 && c + 2 < line.len() && line[c + 1] == b' ') {
        s.emit(s.pos + colon, K::GenericError);
        s.emit_n(2, K::Text);
        s.emit(line_stop, K::Name);
        st.mode = 0;
        return false;
    }
    if !line.is_empty() && is_ident_start(line[0]) {
        let end = colon.filter(|&c| c > 0).unwrap_or(line.len());
        s.emit(s.pos + end, K::GenericError);
        s.emit(line_stop, K::Whitespace);
        st.mode = 0;
        return false;
    }
    s.emit(line_stop, K::Text);
    true
}

/// Lexes `  File "x.py", line 3, in f`.
fn traceback_file_line(s: &mut Scanner<'_>, line_stop: usize) {
    s.emit_n(7, K::Text);
    let Some(close) = s.find_on_line(s.pos + 1, "\"") else {
        s.emit(line_stop, K::Text);
        return;
    };
    s.emit(close + 1, K::NameBuiltin);
    if s.looking_at(", line ", 0) {
        s.emit_n(7, K::Text);
        s.emit(s.skip_digits(s.pos), K::Number);
    }
    if s.looking_at(", in ", 0) {
        s.emit_n(5, K::Text);
        s.emit(line_stop, K::Name);
    }
    s.emit(line_stop, K::Text);
}

/// Whether a Python name starts at `at`: a letter or an underscore, in ASCII or Unicode.
/// Other characters, such as arrows and box drawing, are not names, so prose in a
/// "default" block has errors and falls back to plain text.
fn py_name_start(s: &Scanner<'_>, at: usize) -> bool {
    let c = s.byte_at(at);
    if c < 0x80 {
        return is_ident_start(c);
    }
    s.char_at(at).is_some_and(char::is_alphabetic)
}

/// The end of the Python name at `from`: letters, digits, underscores and combining marks.
fn py_name_end(s: &Scanner<'_>, from: usize) -> usize {
    let mut i = from;
    while i < s.limit {
        let c = s.bytes[i];
        if c < 0x80 {
            if !is_ident_char(c) {
                break;
            }
            i += 1;
            continue;
        }
        match s.char_at(i) {
            Some(ch) if ch.is_alphanumeric() || is_combining(ch) => i += ch.len_utf8(),
            _ => break,
        }
    }
    i
}

/// Whether a character is a combining mark of the common blocks.
fn is_combining(ch: char) -> bool {
    matches!(ch as u32, 0x0300..=0x036f | 0x1ab0..=0x1aff | 0x1dc0..=0x1dff | 0x20d0..=0x20ff
        | 0xfe20..=0xfe2f)
}
