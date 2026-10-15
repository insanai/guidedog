//! Text transforms of the optional syntax, run on a leaf's inline tree before it is
//! emitted: replacements ((c), +-, ..., --), smartquotes, and linkify. Code, math, raw
//! HTML, and (for linkify and replacements) the text of links keep their characters.

use guidedoc::is_unicode_whitespace;
use guidedoc::{Fail, decode_last_rune, decode_rune, is_unicode_punctuation};

use super::{Inline, Txt};
use crate::settings::SyntaxExtension;

/// An opening quotation mark waiting for its close.
#[derive(Debug, Clone, Copy)]
struct Quote {
    item: usize,
    at: usize,
    single: bool,
}

/// A quotation mark to replace: byte `at` of an item's text.
#[derive(Debug, Clone, Copy)]
struct Fix {
    item: usize,
    at: usize,
    mark: &'static str,
}

/// The most quotation marks waiting for their close, as markdown-it bounds them.
const MAX_QUOTES: usize = 64;

impl Inline<'_, '_, '_, '_> {
    pub(super) fn transform_text(&mut self) -> Result<(), Fail> {
        if self.h.has(SyntaxExtension::Linkify) {
            self.linkify_items();
        }
        if self.h.has(SyntaxExtension::Replacements) {
            let found = self.text_items(0, false);
            for &id in &found {
                self.replace_text(id);
            }
            self.found = found;
        }
        if self.h.has(SyntaxExtension::Smartquotes) {
            self.smart_quote_items();
        }
        Ok(())
    }

    /// Applies markdown-it's typographic replacements to one text item.
    fn replace_text(&mut self, id: usize) {
        if !self.str(self.items[id].text).bytes().any(|c| b"(+.?!,-".contains(&c)) {
            return;
        }
        let mut t = std::mem::take(&mut self.spare);
        t.clear();
        t.push_str(self.str(self.items[id].text));
        let b = t.as_bytes();
        let start = self.text.len();
        let mut i = 0;
        while i < b.len() {
            let (out, used) = replacement(&t, i);
            match out {
                Some(s) => self.text.push_str(s),
                None => self.text.push_str(&t[i..i + used]),
            }
            i += used;
        }
        self.items[id].text = Txt { start, len: self.text.len() - start };
        self.spare = t;
    }

    /// Turns straight quotation marks into typographic ones, pairing them as
    /// markdown-it's smartquotes does.
    fn smart_quote_items(&mut self) {
        let texts = self.text_items(0, true);
        let mut fixes = Vec::new();
        let mut stack: Vec<Quote> = Vec::new();
        for k in 0..texts.len() {
            self.quote_item(&texts, k, &mut stack, &mut fixes);
        }
        self.found = texts;
        if fixes.is_empty() {
            return;
        }
        fixes.sort_by_key(|f: &Fix| (f.item, f.at));
        let mut k = 0;
        while k < fixes.len() {
            let id = fixes[k].item;
            let t = self.str(self.items[id].text).to_owned();
            let start = self.text.len();
            let mut run = 0;
            while k < fixes.len() && fixes[k].item == id {
                let f = fixes[k];
                self.text.push_str(&t[run..f.at]);
                self.text.push_str(f.mark);
                run = f.at + 1;
                k += 1;
            }
            self.text.push_str(&t[run..]);
            self.items[id].text = Txt { start, len: self.text.len() - start };
        }
    }

    /// The character before (`dir` < 0) or after byte `at` of text item `texts[k]`,
    /// looking into the neighbouring text items; a space at the ends.
    fn neighbour(&self, texts: &[usize], k: usize, at: usize, before: bool) -> char {
        let t = self.str(self.items[texts[k]].text);
        if before && at > 0 {
            return decode_last_rune(t, at).0;
        }
        if !before && at + 1 < t.len() {
            return decode_rune(t, at + 1).0;
        }
        let j = if before { k.checked_sub(1) } else { Some(k + 1) };
        let Some(u) = j.and_then(|j| texts.get(j)).map(|&j| self.str(self.items[j].text)) else {
            return ' ';
        };
        if u.is_empty() {
            return ' ';
        }
        if before { decode_last_rune(u, u.len()).0 } else { decode_rune(u, 0).0 }
    }

    fn quote_item(&self, texts: &[usize], k: usize, stack: &mut Vec<Quote>, fixes: &mut Vec<Fix>) {
        let item = texts[k];
        let t = self.str(self.items[item].text).as_bytes();
        for (at, &c) in t.iter().enumerate() {
            if c != b'"' && c != b'\'' {
                continue;
            }
            let single = c == b'\'';
            let (last, next) =
                (self.neighbour(texts, k, at, true), self.neighbour(texts, k, at, false));
            let (last_ws, next_ws) = (is_unicode_whitespace(last), is_unicode_whitespace(next));
            let (last_p, next_p) = (is_quote_punct(last), is_quote_punct(next));
            let mut can_open = !next_ws && !(next_p && !last_ws && !last_p);
            let mut can_close = !last_ws && !(last_p && !next_ws && !next_p);
            if can_open && can_close {
                (can_open, can_close) = (last_p, next_p);
            }
            if !can_open && !can_close {
                if single {
                    fixes.push(Fix { item, at, mark: "\u{2019}" });
                }
                continue;
            }
            if can_close && close_quote(stack, fixes, item, at, single) {
                continue;
            }
            if can_open && stack.len() < MAX_QUOTES {
                stack.push(Quote { item, at, single });
            } else if single {
                fixes.push(Fix { item, at, mark: "\u{2019}" });
            }
        }
    }
}

fn is_quote_punct(r: char) -> bool {
    r.is_ascii_punctuation() || is_unicode_punctuation(r)
}

/// Pairs a closing mark with the nearest opener of its kind.
fn close_quote(
    stack: &mut Vec<Quote>,
    fixes: &mut Vec<Fix>,
    item: usize,
    at: usize,
    single: bool,
) -> bool {
    let Some(j) = stack.iter().rposition(|q| q.single == single) else { return false };
    let o = stack[j];
    let (open, close) = if single { ("\u{2018}", "\u{2019}") } else { ("\u{201c}", "\u{201d}") };
    fixes.push(Fix { item: o.item, at: o.at, mark: open });
    fixes.push(Fix { item, at, mark: close });
    stack.truncate(j);
    true
}

fn run_length(t: &[u8], i: usize, c: u8) -> usize {
    t[i..].iter().take_while(|&&x| x == c).count()
}

/// The replacement at byte `i` of `t`, and the bytes it covers (one character when
/// nothing is replaced).
fn replacement(t: &str, i: usize) -> (Option<&'static str>, usize) {
    let b = t.as_bytes();
    let rest = &b[i..];
    let c = rest[0];
    match c {
        b'(' if rest.len() >= 3 && rest[2] == b')' && b"cCrRpP".contains(&rest[1]) => {
            let s = match rest[1] | 0x20 {
                b'c' => "\u{a9}",
                b'r' => "\u{ae}",
                _ => "\u{a7}",
            };
            (Some(s), 3)
        }
        b'(' if rest.len() >= 4 && rest[3] == b')' && rest[1..3].eq_ignore_ascii_case(b"tm") => {
            (Some("\u{2122}"), 4)
        }
        b'+' if rest.get(1) == Some(&b'-') => (Some("\u{b1}"), 2),
        b'.' if rest.get(1) == Some(&b'.') => {
            let prev = if i > 0 { b[i - 1] } else { 0 };
            let s = if prev == b'?' || prev == b'!' { ".." } else { "\u{2026}" };
            (Some(s), run_length(b, i, b'.'))
        }
        b'?' | b'!' if run_length(b, i, c) >= 4 => {
            (Some(if c == b'?' { "???" } else { "!!!" }), run_length(b, i, c))
        }
        b',' if run_length(b, i, b',') >= 2 => (Some(","), run_length(b, i, b',')),
        b'-' => match run_length(b, i, b'-') {
            3 => (Some("\u{2014}"), 3),
            2 if dash_context(b, i) => (Some("\u{2013}"), 2),
            n => (None, n),
        },
        _ => (None, t[i..].chars().next().map_or(1, char::len_utf8)),
    }
}

/// "--" becomes an en dash between white space, or between characters that are
/// neither white space nor dashes.
fn dash_context(t: &[u8], i: usize) -> bool {
    let before = if i > 0 { t[i - 1] } else { b' ' };
    let after = t.get(i + 2).copied().unwrap_or(b' ');
    let space = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r');
    if space(before) && space(after) {
        return true;
    }
    !space(before) && !space(after) && before != b'-' && after != b'-'
}
