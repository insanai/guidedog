//! HTML recognition: the seven kinds of HTML block starts and ends, and inline raw
//! HTML (open and closing tags, comments, processing instructions, declarations, CDATA).

use crate::blocks::is_space_or_tab;

const BLOCK_TAGS: [&str; 62] = [
    "address",
    "article",
    "aside",
    "base",
    "basefont",
    "blockquote",
    "body",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "iframe",
    "legend",
    "li",
    "link",
    "main",
    "menu",
    "menuitem",
    "nav",
    "noframes",
    "ol",
    "optgroup",
    "option",
    "p",
    "param",
    "search",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
];

const RAW_TAGS: [&str; 4] = ["pre", "script", "style", "textarea"];

fn has_prefix_fold(s: &[u8], lowered: &str) -> bool {
    s.len() >= lowered.len() && s[..lowered.len()].eq_ignore_ascii_case(lowered.as_bytes())
}

fn find(s: &[u8], sub: &[u8], from: usize) -> Option<usize> {
    if from > s.len() || sub.len() > s.len() {
        return None;
    }
    (from..=s.len() - sub.len()).find(|&i| &s[i..i + sub.len()] == sub)
}

pub(crate) fn is_html_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0c | 0x0b)
}

fn skip_html_space(s: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < s.len() && is_html_space(s[j]) {
        j += 1;
    }
    j
}

fn scan_tag_name(s: &[u8], i: usize) -> usize {
    if i >= s.len() || !s[i].is_ascii_alphabetic() {
        return i;
    }
    let mut j = i + 1;
    while j < s.len() && (s[j].is_ascii_alphanumeric() || s[j] == b'-') {
        j += 1;
    }
    j
}

/// Tells whether a line holding `s` (from its first non-space character, a '<')
/// starts an HTML block of type `t`.
pub(crate) fn html_block_start(t: u8, s: &str) -> bool {
    let s = s.as_bytes();
    match t {
        1 => RAW_TAGS.iter().any(|name| {
            let n = 1 + name.len();
            has_prefix_fold(&s[1..], name)
                && (n >= s.len() || is_space_or_tab(s[n]) || s[n] == b'>')
        }),
        2 => s.starts_with(b"<!--"),
        3 => s.starts_with(b"<?"),
        4 => s.len() > 2 && s[1] == b'!' && s[2].is_ascii_alphabetic(),
        5 => has_prefix_fold(s, "<![cdata["),
        6 => block_tag_start(s),
        7 => complete_tag_line(s),
        _ => false,
    }
}

fn block_tag_start(s: &[u8]) -> bool {
    let i = if s.get(1) == Some(&b'/') { 2 } else { 1 };
    let e = scan_tag_name(s, i);
    if e == i || !BLOCK_TAGS.iter().any(|name| s[i..e].eq_ignore_ascii_case(name.as_bytes())) {
        return false;
    }
    e >= s.len()
        || is_space_or_tab(s[e])
        || s[e] == b'>'
        || (s[e] == b'/' && s.get(e + 1) == Some(&b'>'))
}

/// Type 7: a complete open or closing tag, other than a raw tag, alone on its line.
fn complete_tag_line(s: &[u8]) -> bool {
    let mut n = scan_close_tag(s);
    if n == 0 {
        let mut memo = HtmlMemo::default();
        n = scan_open_tag(s, 0, &mut memo);
        if n == 0 {
            return false;
        }
        let name = &s[1..scan_tag_name(s, 1)];
        if RAW_TAGS.iter().any(|raw| name.eq_ignore_ascii_case(raw.as_bytes())) {
            return false;
        }
    }
    s[n..].iter().all(|&c| is_space_or_tab(c))
}

/// Tells whether a line holding `s` ends an HTML block of type `t` (1 to 5).
pub(crate) fn html_block_end(t: u8, s: &str) -> bool {
    let s = s.as_bytes();
    match t {
        1 => RAW_TAGS.iter().any(|name| {
            (0..s.len()).any(|i| {
                let r = &s[i..];
                r.len() >= name.len() + 3
                    && r[0] == b'<'
                    && r[1] == b'/'
                    && r[2..2 + name.len()].eq_ignore_ascii_case(name.as_bytes())
                    && r[2 + name.len()] == b'>'
            })
        }),
        2 => find(s, b"-->", 0).is_some(),
        3 => find(s, b"?>", 0).is_some(),
        4 => s.contains(&b'>'),
        5 => find(s, b"]]>", 0).is_some(),
        _ => false,
    }
}

/// Remembers the earliest position from which a search for a terminator failed, so
/// repeated openers cannot make inline HTML scanning quadratic.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HtmlMemo {
    comment: usize,
    pi: usize,
    declaration: usize,
    cdata: usize,
    double_quote: usize,
    single_quote: usize,
}

impl Default for HtmlMemo {
    fn default() -> Self {
        HtmlMemo {
            comment: usize::MAX,
            pi: usize::MAX,
            declaration: usize::MAX,
            cdata: usize::MAX,
            double_quote: usize::MAX,
            single_quote: usize::MAX,
        }
    }
}

/// Returns the length of an open tag starting `s`, or 0. `s` begins at subject offset
/// `base`; `memo` records quotes known to be unterminated after an offset.
pub(crate) fn scan_open_tag(s: &[u8], base: usize, memo: &mut HtmlMemo) -> usize {
    if s.len() < 3 || s[0] != b'<' {
        return 0;
    }
    let mut i = scan_tag_name(s, 1);
    if i == 1 {
        return 0;
    }
    loop {
        let j = skip_html_space(s, i);
        if j == i || j >= s.len() {
            break;
        }
        let c = s[j];
        if !c.is_ascii_alphabetic() && c != b'_' && c != b':' {
            break;
        }
        let mut k = j + 1;
        while k < s.len() && (s[k].is_ascii_alphanumeric() || b":._-".contains(&s[k])) {
            k += 1;
        }
        i = k;
        let v = skip_html_space(s, k);
        if v < s.len() && s[v] == b'=' {
            let v = skip_html_space(s, v + 1);
            let e = scan_attribute_value(s, v, base, memo);
            if e > v {
                i = e;
            }
        }
    }
    i = skip_html_space(s, i);
    if s.get(i) == Some(&b'/') {
        i += 1;
    }
    if s.get(i) == Some(&b'>') { i + 1 } else { 0 }
}

fn scan_attribute_value(s: &[u8], i: usize, base: usize, memo: &mut HtmlMemo) -> usize {
    let Some(&q) = s.get(i) else { return i };
    if q == b'"' || q == b'\'' {
        let known = if q == b'"' { &mut memo.double_quote } else { &mut memo.single_quote };
        if base + i + 1 >= *known {
            return i;
        }
        if let Some(j) = s[i + 1..].iter().position(|&c| c == q) {
            return i + 1 + j + 1;
        }
        *known = base + i + 1;
        return i;
    }
    let mut j = i;
    while j < s.len() && !(s[j] <= b' ' || b"\"'=<>`".contains(&s[j])) {
        j += 1;
    }
    j
}

fn scan_close_tag(s: &[u8]) -> usize {
    if s.len() < 4 || s[0] != b'<' || s[1] != b'/' {
        return 0;
    }
    let i = scan_tag_name(s, 2);
    if i == 2 {
        return 0;
    }
    let i = skip_html_space(s, i);
    if s.get(i) == Some(&b'>') { i + 1 } else { 0 }
}

fn search_from(s: &[u8], sub: &[u8], from: usize, memo: &mut usize) -> Option<usize> {
    if from >= *memo {
        return None;
    }
    let at = find(s, sub, from);
    if at.is_none() {
        *memo = from;
    }
    at
}

/// Returns the length of raw HTML starting at `subject[pos]`, or 0.
pub(crate) fn scan_inline_html(subject: &[u8], pos: usize, memo: &mut HtmlMemo) -> usize {
    let s = &subject[pos..];
    if s.len() < 3 || s[0] != b'<' {
        return 0;
    }
    match s[1] {
        b'/' => scan_close_tag(s),
        b'?' => search_from(subject, b"?>", pos + 2, &mut memo.pi).map_or(0, |at| at + 2 - pos),
        b'!' => scan_bang(subject, pos, memo),
        _ => scan_open_tag(s, pos, memo),
    }
}

/// Comments, CDATA, and declarations.
fn scan_bang(subject: &[u8], pos: usize, memo: &mut HtmlMemo) -> usize {
    let s = &subject[pos..];
    if s.len() >= 4 && s[2] == b'-' && s[3] == b'-' {
        if s.get(4) == Some(&b'>') {
            return 5;
        }
        if s.get(4) == Some(&b'-') && s.get(5) == Some(&b'>') {
            return 6;
        }
        return search_from(subject, b"-->", pos + 4, &mut memo.comment)
            .map_or(0, |at| at + 3 - pos);
    }
    if s.starts_with(b"<![CDATA[") {
        return search_from(subject, b"]]>", pos + 9, &mut memo.cdata)
            .map_or(0, |at| at + 3 - pos);
    }
    if s[2].is_ascii_alphabetic() {
        return search_from(subject, b">", pos + 3, &mut memo.declaration)
            .map_or(0, |at| at + 1 - pos);
    }
    0
}
