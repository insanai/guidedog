//! XML, HTML and CSS. HTML hands `<script>` and `<style>` contents to the JavaScript and
//! CSS lexers. XML and HTML differ as Pygments' lexers do: XML's tag tokens include the
//! brackets and the attribute's `=`, HTML's brackets are Punctuation.

use crate::lexers::Lexer;
use crate::scanner::is_space;
use crate::scanner::{Scanner, State, has, in_list, in_list_fold, is_ident_char, is_ident_start};
use crate::token::TokenKind as K;

/// `st.mode`: inside a start tag.
const TAG: u8 = 1;
/// `st.mode`: after an attribute's `=`.
const ATTR: u8 = 2;

/// `st.flags`: the start tag being lexed is `<script>`.
const SCRIPT_TAG: u32 = 1;
/// `st.flags`: the start tag being lexed is `<style>`.
const STYLE_TAG: u32 = 2;

pub(crate) fn xml_step(s: &mut Scanner<'_>, st: &mut State) {
    markup_step(s, st, false);
}

pub(crate) fn html_step(s: &mut Scanner<'_>, st: &mut State) {
    markup_step(s, st, true);
}

fn markup_step(s: &mut Scanner<'_>, st: &mut State, html: bool) {
    match st.mode {
        TAG => tag_token(s, st, html),
        ATTR => attribute_value(s, st),
        _ => markup_text(s, st, html),
    }
}

fn markup_text(s: &mut Scanner<'_>, st: &mut State, html: bool) {
    let c = s.peek(0);
    if c == b'&' {
        let stop = s.limit.min(s.pos + 32);
        let mut i = s.pos + 1;
        while i < stop && !is_space(s.bytes[i]) {
            if s.bytes[i] == b';' {
                s.emit(i + 1, K::NameEntity);
                return;
            }
            i += 1;
        }
        s.emit_n(1, K::Error);
        return;
    }
    if c != b'<' {
        if html {
            s.emit(s.skip_by(s.pos, |b| b != b'<' && b != b'&'), K::Text);
        } else if is_space(c) {
            s.emit_spaces(K::Whitespace);
        } else {
            s.emit(s.skip_by(s.pos, |b| !has("<&", b) && !is_space(b)), K::Text);
        }
        return;
    }
    if markup_declaration(s) {
        return;
    }
    markup_tag_open(s, st, html);
}

/// Lexes comments, CDATA sections, processing instructions and declarations such as
/// `<!DOCTYPE html>`.
fn markup_declaration(s: &mut Scanner<'_>) -> bool {
    let (open, close, kind) = if s.looking_at("<![CDATA[", 0) {
        ("<![CDATA[", "]]>", K::CommentPreproc)
    } else if s.looking_at("<!--", 0) {
        ("<!--", "-->", K::CommentMultiline)
    } else if s.looking_at("<?", 0) {
        ("<?", "?>", K::CommentPreproc)
    } else if s.looking_at("<!", 0) {
        ("<!", ">", K::CommentPreproc)
    } else {
        return false;
    };
    let end = s.find_from(s.pos + open.len(), close);
    s.emit(end.map_or(s.limit, |e| e + close.len()), kind);
    true
}

fn is_name_byte(c: u8) -> bool {
    is_ident_char(c) || has(":.-", c)
}

fn skip_space_all(s: &Scanner<'_>, from: usize) -> usize {
    s.skip_by(from, is_space)
}

fn markup_tag_open(s: &mut Scanner<'_>, st: &mut State, html: bool) {
    let mut i = skip_space_all(s, s.pos + 1);
    let closing = s.byte_at(i) == b'/';
    if closing {
        i = skip_space_all(s, i + 1);
    }
    let name = i;
    i = s.skip_by(i, is_name_byte);
    if i == name {
        s.emit_n(1, K::Error);
        return;
    }
    if closing {
        return closing_tag(s, html, name, i);
    }
    let tag = s.slice(name, i);
    st.mode = TAG;
    st.flags = 0;
    if html && tag.eq_ignore_ascii_case(b"script") {
        st.flags = SCRIPT_TAG;
    }
    if html && tag.eq_ignore_ascii_case(b"style") {
        st.flags = STYLE_TAG;
    }
    if !html {
        s.emit(i, K::NameTag);
        return;
    }
    s.emit_n(1, K::Punctuation);
    s.emit(name, K::Text);
    s.emit(i, K::NameTag);
}

/// Lexes `</name>`, whose name runs from `name` to `name_end`.
fn closing_tag(s: &mut Scanner<'_>, html: bool, name: usize, name_end: usize) {
    let mut end = skip_space_all(s, name_end);
    if s.byte_at(end) == b'>' {
        end += 1;
    }
    if !html {
        s.emit(end, K::NameTag);
        return;
    }
    s.emit_n(1, K::Punctuation);
    s.emit(skip_space_all(s, s.pos), K::Text);
    s.emit_n(1, K::Punctuation);
    s.emit(name, K::Text);
    s.emit(name_end, K::NameTag);
    s.emit(skip_space_all(s, name_end), K::Text);
    if s.byte_at(s.pos) == b'>' {
        s.emit_n(1, K::Punctuation);
    }
}

fn tag_token(s: &mut Scanner<'_>, st: &mut State, html: bool) {
    let c = s.peek(0);
    if is_space(c) {
        s.emit(skip_space_all(s, s.pos), if html { K::Text } else { K::Whitespace });
        return;
    }
    if c == b'>' || (c == b'/' && s.byte_at(skip_space_all(s, s.pos + 1)) == b'>') {
        let end = if c == b'/' { skip_space_all(s, s.pos + 1) + 1 } else { s.pos + 1 };
        if html {
            if c == b'/' {
                s.emit_n(1, K::Punctuation);
                s.emit(end - 1, K::Text);
            }
            s.emit(end, K::Punctuation);
        } else {
            s.emit(end, K::NameTag);
        }
        st.mode = 0;
        if c == b'>' && st.flags != 0 {
            embedded_content(s, st);
        }
        return;
    }
    if is_name_byte(c) {
        attribute_name(s, st, html);
        return;
    }
    s.emit_rune(K::Error);
}

fn attribute_name(s: &mut Scanner<'_>, st: &mut State, html: bool) {
    let end = s.skip_by(s.pos, is_name_byte);
    let eq = skip_space_all(s, end);
    if s.byte_at(eq) != b'=' {
        s.emit(end, if html { K::NameAttribute } else { K::Error });
        return;
    }
    st.mode = ATTR;
    if !html {
        s.emit(eq + 1, K::NameAttribute);
        return;
    }
    s.emit(eq, K::NameAttribute);
    s.emit_n(1, K::Operator);
    s.emit(skip_space_all(s, s.pos), K::Text);
}

fn attribute_value(s: &mut Scanner<'_>, st: &mut State) {
    let c = s.peek(0);
    if is_space(c) {
        s.emit_spaces(K::Whitespace);
        return;
    }
    st.mode = TAG;
    if c == b'"' || c == b'\'' {
        let close = s.find_from(s.pos + 1, if c == b'"' { "\"" } else { "'" });
        s.emit(close.map_or(s.limit, |e| e + 1), K::String);
        return;
    }
    let end = s.skip_by(s.pos, |b| !is_space(b) && b != b'>');
    s.emit(end.max(s.pos + 1), K::String);
}

/// Hands the contents of `<script>` or `<style>` to their lexer.
fn embedded_content(s: &mut Scanner<'_>, st: &mut State) {
    let (name, lexer) =
        if st.flags == SCRIPT_TAG { ("script", Lexer::Javascript) } else { ("style", Lexer::Css) };
    st.flags = 0;
    for i in s.pos..s.limit {
        if s.bytes[i] != b'<' {
            continue;
        }
        let mut j = skip_space_all(s, i + 1);
        if s.byte_at(j) != b'/' {
            continue;
        }
        j = skip_space_all(s, j + 1);
        if j + name.len() <= s.limit
            && s.slice(j, j + name.len()).eq_ignore_ascii_case(name.as_bytes())
        {
            s.start_inner(lexer, i, true);
            return;
        }
    }
    s.start_inner(lexer, s.limit, true);
}

// CSS: selectors, declaration blocks, and values with their units, colours and functions,
// after Pygments' CSS lexer.

const CSS_PROPERTIES: &[&str] = &[
    "align-content",
    "align-items",
    "align-self",
    "all",
    "animation",
    "animation-delay",
    "animation-direction",
    "animation-duration",
    "animation-fill-mode",
    "animation-iteration-count",
    "animation-name",
    "animation-timing-function",
    "appearance",
    "aspect-ratio",
    "backdrop-filter",
    "background",
    "background-attachment",
    "background-clip",
    "background-color",
    "background-image",
    "background-position",
    "background-repeat",
    "background-size",
    "border",
    "border-bottom",
    "border-collapse",
    "border-color",
    "border-left",
    "border-radius",
    "border-right",
    "border-spacing",
    "border-style",
    "border-top",
    "border-width",
    "bottom",
    "box-shadow",
    "box-sizing",
    "caption-side",
    "clear",
    "clip",
    "clip-path",
    "color",
    "column-count",
    "column-gap",
    "columns",
    "content",
    "counter-increment",
    "counter-reset",
    "cursor",
    "direction",
    "display",
    "empty-cells",
    "fill",
    "filter",
    "flex",
    "flex-basis",
    "flex-direction",
    "flex-flow",
    "flex-grow",
    "flex-shrink",
    "flex-wrap",
    "float",
    "font",
    "font-family",
    "font-feature-settings",
    "font-size",
    "font-style",
    "font-variant",
    "font-weight",
    "gap",
    "grid",
    "grid-area",
    "grid-column",
    "grid-row",
    "grid-template",
    "grid-template-areas",
    "grid-template-columns",
    "grid-template-rows",
    "height",
    "hyphens",
    "inset",
    "isolation",
    "justify-content",
    "justify-items",
    "justify-self",
    "left",
    "letter-spacing",
    "line-height",
    "list-style",
    "list-style-type",
    "margin",
    "margin-block",
    "margin-bottom",
    "margin-inline",
    "margin-left",
    "margin-right",
    "margin-top",
    "mask",
    "max-height",
    "max-width",
    "min-height",
    "min-width",
    "mix-blend-mode",
    "object-fit",
    "object-position",
    "opacity",
    "order",
    "outline",
    "outline-offset",
    "overflow",
    "overflow-wrap",
    "overflow-x",
    "overflow-y",
    "padding",
    "padding-block",
    "padding-bottom",
    "padding-inline",
    "padding-left",
    "padding-right",
    "padding-top",
    "page-break-after",
    "page-break-before",
    "place-items",
    "pointer-events",
    "position",
    "quotes",
    "resize",
    "right",
    "row-gap",
    "scroll-behavior",
    "stroke",
    "tab-size",
    "table-layout",
    "text-align",
    "text-decoration",
    "text-indent",
    "text-overflow",
    "text-shadow",
    "text-transform",
    "top",
    "transform",
    "transform-origin",
    "transition",
    "transition-delay",
    "transition-duration",
    "transition-property",
    "transition-timing-function",
    "unicode-bidi",
    "user-select",
    "vertical-align",
    "visibility",
    "white-space",
    "width",
    "will-change",
    "word-break",
    "word-spacing",
    "word-wrap",
    "writing-mode",
    "z-index",
];

const CSS_VALUES: &[&str] = &[
    "absolute",
    "auto",
    "baseline",
    "block",
    "bold",
    "bolder",
    "both",
    "bottom",
    "center",
    "collapse",
    "column",
    "contain",
    "cover",
    "currentColor",
    "dashed",
    "default",
    "dotted",
    "double",
    "ease",
    "ease-in",
    "ease-in-out",
    "ease-out",
    "fixed",
    "flex",
    "flex-end",
    "flex-start",
    "grid",
    "hidden",
    "important",
    "inherit",
    "initial",
    "inline",
    "inline-block",
    "inline-flex",
    "italic",
    "large",
    "left",
    "lighter",
    "linear",
    "list-item",
    "medium",
    "middle",
    "monospace",
    "no-repeat",
    "none",
    "normal",
    "nowrap",
    "pointer",
    "pre",
    "pre-wrap",
    "relative",
    "repeat",
    "revert",
    "right",
    "row",
    "sans-serif",
    "scroll",
    "serif",
    "small",
    "smaller",
    "solid",
    "space-around",
    "space-between",
    "static",
    "sticky",
    "stretch",
    "table",
    "table-cell",
    "text",
    "top",
    "transparent",
    "underline",
    "unset",
    "uppercase",
    "lowercase",
    "visible",
    "wrap",
    "x-large",
    "x-small",
    "xx-large",
    "black",
    "white",
    "red",
    "green",
    "blue",
    "yellow",
    "orange",
    "purple",
    "gray",
    "grey",
    "silver",
    "maroon",
    "navy",
    "teal",
    "olive",
    "aqua",
    "fuchsia",
    "lime",
];

const CSS_FUNCTIONS: &[&str] = &[
    "rgb",
    "rgba",
    "hsl",
    "hsla",
    "hwb",
    "lab",
    "lch",
    "oklab",
    "oklch",
    "color",
    "calc",
    "var",
    "min",
    "max",
    "clamp",
    "attr",
    "counter",
    "counters",
    "env",
    "url",
    "linear-gradient",
    "radial-gradient",
    "conic-gradient",
    "repeating-linear-gradient",
    "translate",
    "translateX",
    "translateY",
    "rotate",
    "scale",
    "matrix",
    "skew",
    "cubic-bezier",
    "steps",
    "minmax",
    "repeat",
    "fit-content",
    "format",
    "local",
];

const CSS_UNITS: &[&str] = &[
    "px", "em", "rem", "ex", "ch", "vw", "vh", "vmin", "vmax", "cm", "mm", "in", "pt", "pc", "q",
    "deg", "rad", "grad", "turn", "s", "ms", "hz", "khz", "dpi", "dpcm", "dppx", "fr", "x", "lh",
    "rlh", "svh", "lvh", "dvh", "cqw", "cqh",
];

/// A declaration block.
const FRAME_RULE: u8 = 1;
/// An at-rule's block of rules.
const FRAME_AT: u8 = 2;
/// `st.mode`: after a property's colon.
const CSS_VALUE: u8 = 1;
/// `st.flags`: an at-rule whose block holds rules.
const AT_PENDING: u32 = 1;

pub(crate) fn css_step(s: &mut Scanner<'_>, st: &mut State) {
    let c = s.peek(0);
    if is_space(c) {
        s.emit_spaces(K::Whitespace);
        return;
    }
    if s.looking_at("/*", 0) {
        s.emit(s.block_comment_end("/*", "*/", false), K::Comment);
        return;
    }
    if c == b'"' || c == b'\'' {
        return css_string(s, c);
    }
    if st.top().is_some_and(|f| f.mode == FRAME_RULE) {
        if st.mode == CSS_VALUE {
            css_value(s, st, c);
        } else {
            css_declaration(s, st, c);
        }
        return;
    }
    css_selector(s, st, c);
}

fn css_string(s: &mut Scanner<'_>, q: u8) {
    let mut i = s.pos + 1;
    while i < s.limit && s.bytes[i] != q && s.bytes[i] != b'\n' {
        i += if s.bytes[i] == b'\\' { 2 } else { 1 };
    }
    s.emit((i + 1).min(s.limit), if q == b'"' { K::StringDouble } else { K::StringSingle });
}

fn css_name_end(s: &Scanner<'_>, from: usize) -> usize {
    s.ident_end(from, "-")
}

fn css_selector(s: &mut Scanner<'_>, st: &mut State, c: u8) {
    match c {
        b'{' => {
            let mode = if st.flags & AT_PENDING != 0 { FRAME_AT } else { FRAME_RULE };
            st.push(mode, 0, 0);
            st.flags &= !AT_PENDING;
            s.emit_n(1, K::Punctuation);
        }
        b'}' => {
            st.pop();
            s.emit_n(1, K::Punctuation);
        }
        b';' => {
            st.flags &= !AT_PENDING;
            s.emit_n(1, K::Punctuation);
        }
        b':' | b'.' | b'#' | b'@' => css_prefixed_name(s, st, c),
        _ if is_ident_char(c) || c == b'-' => s.emit(css_name_end(s, s.pos), K::NameTag),
        _ if has("~^*!%&$[]()<>|+=,./?-", c) => s.emit_n(1, K::Operator),
        _ => s.emit_rune(K::Error),
    }
}

/// Lexes `:hover`, `::before`, `.class`, `#id` and `@media`.
fn css_prefixed_name(s: &mut Scanner<'_>, st: &mut State, c: u8) {
    let start = s.pos + if c == b':' && s.peek(1) == b':' { 2 } else { 1 };
    let end = css_name_end(s, start);
    if end == start {
        s.emit_n(1, K::Operator);
        return;
    }
    s.emit(start, K::Punctuation);
    let kind = match c {
        b':' => K::NameDecorator,
        b'.' => K::NameClass,
        b'#' => K::NameNamespace,
        _ => {
            st.flags |= AT_PENDING;
            K::Keyword
        }
    };
    s.emit(end, kind);
}

fn css_declaration(s: &mut Scanner<'_>, st: &mut State, c: u8) {
    if c == b'}' {
        st.pop();
        s.emit_n(1, K::Punctuation);
    } else if c == b';' {
        s.emit_n(1, K::Punctuation);
    } else if is_ident_start(c) || c == b'-' {
        let end = css_name_end(s, s.pos);
        let colon = skip_space_all(s, end);
        if s.byte_at(colon) != b':' {
            s.emit(end.max(s.pos + 1), K::NameTag);
            return;
        }
        let name = s.slice(s.pos, end);
        let kind = if in_list(name, CSS_PROPERTIES) {
            K::Keyword
        } else if name.starts_with(b"--") {
            K::NameVariable
        } else if c == b'-' {
            K::KeywordPseudo
        } else {
            K::Name
        };
        s.emit(end, kind);
        s.emit(colon, K::Whitespace);
        s.emit_n(1, K::Punctuation);
        st.mode = CSS_VALUE;
    } else {
        css_selector(s, st, c);
    }
}

fn css_value(s: &mut Scanner<'_>, st: &mut State, c: u8) {
    let next = s.peek(1);
    let number = c.is_ascii_digit()
        || (c == b'.' && next.is_ascii_digit())
        || ((c == b'-' || c == b'+') && (next.is_ascii_digit() || next == b'.'));
    if c == b';' {
        st.mode = 0;
        s.emit_n(1, K::Punctuation);
    } else if c == b'}' {
        st.mode = 0;
        st.pop();
        s.emit_n(1, K::Punctuation);
    } else if s.looking_at("!important", 0) {
        s.emit_n(10, K::CommentPreproc);
    } else if c == b'#' && is_ident_char(next) {
        s.emit(s.ident_end(s.pos + 1, ""), K::NumberHex);
    } else if number {
        css_number(s);
    } else if is_ident_start(c) || (c == b'-' && is_ident_start(next)) {
        css_value_word(s);
    } else if has("[](),", c) {
        s.emit_n(1, K::Punctuation);
    } else if has("~^*!%&<>|+=@:./?-", c) {
        s.emit_n(1, K::Operator);
    } else {
        s.emit_rune(K::Error);
    }
}

fn css_number(s: &mut Scanner<'_>) {
    let mut i = s.pos;
    if s.byte_at(i) == b'-' || s.byte_at(i) == b'+' {
        i += 1;
    }
    i = s.skip_digits(i);
    let mut kind = K::NumberInteger;
    if s.byte_at(i) == b'.' && s.byte_at(i + 1).is_ascii_digit() {
        i = s.skip_digits(i + 1);
        kind = K::NumberFloat;
    }
    s.emit(i, kind);
    let unit = s.ident_end(s.pos, "");
    if unit > s.pos && in_list_fold(s.slice(s.pos, unit), CSS_UNITS) {
        s.emit(unit, K::KeywordType);
    } else if s.peek(0) == b'%' {
        s.emit_n(1, K::KeywordType);
    }
}

fn css_value_word(s: &mut Scanner<'_>) {
    let end = css_name_end(s, s.pos);
    let word = s.slice(s.pos, end);
    if s.byte_at(end) == b'(' {
        if word == b"url" && css_url(s, end) {
            return;
        }
        let kind = if in_list(word, CSS_FUNCTIONS) { K::NameBuiltin } else { K::NameFunction };
        s.emit(end, kind);
        s.emit_n(1, K::Punctuation);
        return;
    }
    let kind = if in_list(word, CSS_VALUES) {
        K::KeywordConstant
    } else if in_list(word, CSS_PROPERTIES) {
        K::Keyword
    } else if word.starts_with(b"-") {
        K::NameBuiltinPseudo
    } else {
        K::Name
    };
    s.emit(end, kind);
}

/// Lexes `url(...)` whose name ends at `end`; false when it does not close on its line.
fn css_url(s: &mut Scanner<'_>, end: usize) -> bool {
    let Some(close) = s.find_on_line(end, ")") else { return false };
    s.emit(end, K::NameBuiltin);
    s.emit_n(1, K::Punctuation);
    let kind = match s.peek(0) {
        b'"' => K::StringDouble,
        b'\'' => K::StringSingle,
        _ => K::StringOther,
    };
    s.emit(close, kind);
    s.emit_n(1, K::Punctuation);
    true
}
