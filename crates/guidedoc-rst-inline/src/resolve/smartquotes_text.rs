//! Rewriting one Text node with typographic quotes, dashes, and ellipses.

use guidedoc::{Fail, NodeId, decode_last_rune, decode_rune, is_decimal_digit, is_letter};

use super::Resolver;
use super::smartquotes::QuoteContext;
use crate::chars::is_space;

const APOSTROPHE: &str = "’";

impl Resolver<'_, '_> {
    /// Rewrites one Text node. `next` is the character after the node.
    pub fn educate_text(
        &mut self,
        id: NodeId,
        qc: &mut QuoteContext<'_>,
        next: char,
    ) -> Result<(), Fail> {
        let text = self.text_of(id);
        if !text.bytes().any(|c| matches!(c, b'"' | b'\'' | b'-' | b'.')) {
            if let Some(last) = text.chars().next_back() {
                qc.prev = last;
            }
            return Ok(());
        }
        let mut text = std::mem::take(&mut self.plain);
        text.clear();
        text.push_str(self.text_of(id));
        let mut escaped = std::mem::take(&mut self.escaped);
        self.mark_escapes(id, &text, &mut escaped);
        let mut out = std::mem::take(&mut self.key);
        out.clear();
        let mut i = 0;
        while i < text.len() {
            let (_, w) = decode_rune(&text, i);
            let mut n = educate_at(&mut out, &text, i, &escaped, qc, next);
            if n == 0 {
                out.push_str(&text[i..i + w]);
                n = w;
            }
            qc.prev = decode_last_rune(&text, i + n).0;
            i += n;
        }
        let t = self.ctx.b.add_text(&out);
        (self.plain, self.escaped, self.key) = (text, escaped, out);
        self.ctx.b.set_text(id, t?);
        Ok(())
    }

    /// Finds the characters of a Text node that were escaped in the source: the node's
    /// source span and its text are walked together.
    fn mark_escapes(&self, id: NodeId, text: &str, escaped: &mut Vec<bool>) {
        escaped.clear();
        escaped.resize(text.len(), false);
        let sp = self.node_span(id);
        let source = self.ctx.source_of(sp.source);
        let Some(src) = source.as_bytes().get(sp.start as usize..sp.end as usize) else {
            return;
        };
        let t = text.as_bytes();
        let (mut s, mut k) = (0, 0);
        while s < src.len() && k < t.len() {
            if src[s] == b'\\' && s + 1 < src.len() {
                if matches!(src[s + 1], b' ' | b'\n') {
                    s += 2;
                    continue;
                }
                if src[s + 1] != t[k] {
                    return;
                }
                escaped[k] = true;
                s += 2;
                k += 1;
                continue;
            }
            if src[s] != t[k] {
                return;
            }
            s += 1;
            k += 1;
        }
    }
}

/// Writes the replacement for the markup at `i` and returns the bytes it consumed, or
/// 0 to keep the character.
fn educate_at(
    out: &mut String,
    text: &str,
    i: usize,
    escaped: &[bool],
    qc: &QuoteContext<'_>,
    next: char,
) -> usize {
    if escaped[i] {
        return 0;
    }
    let before = if i > 0 { decode_last_rune(text, i).0 } else { qc.prev };
    let after = rune_after(text, i + 1, next);
    let rest = &text[i..];
    match text.as_bytes()[i] {
        b'-' if rest.starts_with("---") => replace(out, "—", 3),
        b'-' if rest.starts_with("--") => replace(out, "–", 2),
        b'.' if rest.starts_with("...") => replace(out, "…", 3),
        b'.' if rest.starts_with(". . .") => replace(out, "…", 5),
        b'"' => {
            let pair = after == '\'' && is_letter_or_digit(third(text, i, next));
            let open = opens(before) || pair;
            replace(out, qc.quotes[if open { 0 } else { 1 }], 1)
        }
        b'\'' => {
            let apostrophe = (is_letter_or_digit(before) && is_letter_or_digit(after))
                || (qc.english && opens(before) && decade(text, i + 1));
            let mark = if apostrophe {
                APOSTROPHE
            } else if opens(before) || (before == '"' && is_letter_or_digit(after)) {
                qc.quotes[2]
            } else {
                qc.quotes[3]
            };
            replace(out, mark, 1)
        }
        _ => 0,
    }
}

fn replace(out: &mut String, with: &str, consumed: usize) -> usize {
    out.push_str(with);
    consumed
}

fn rune_after(text: &str, i: usize, next: char) -> char {
    if i >= text.len() { next } else { decode_rune(text, i).0 }
}

fn third(text: &str, i: usize, next: char) -> char {
    match (i + 2).cmp(&text.len()) {
        std::cmp::Ordering::Less => decode_rune(text, i + 2).0,
        std::cmp::Ordering::Equal => next,
        std::cmp::Ordering::Greater => '\0',
    }
}

/// A quotation mark after this character opens a quotation.
fn opens(before: char) -> bool {
    matches!(before, '\0' | ' ' | '\t' | '\n' | '(' | '[' | '{' | '<' | '-' | '–' | '—' | '\u{a0}')
        || is_space(before)
        || matches!(before, '“' | '‘' | '„' | '‚' | '«' | '‹' | '「' | '『')
}

fn is_letter_or_digit(c: char) -> bool {
    c != '\0' && (is_letter(c) || is_decimal_digit(c))
}

/// Two digits and "s" follow ('80s).
fn decade(s: &str, i: usize) -> bool {
    let t = s.as_bytes();
    i + 3 <= t.len() && t[i].is_ascii_digit() && t[i + 1].is_ascii_digit() && t[i + 2] == b's'
}
