//! Linkify ("linkify" syntax): bare URLs in text become links: http://, https://,
//! ftp://, and mailto: URLs, www. hosts, and, with `linkify_fuzzy_links`, host names
//! with a known top-level domain and e-mail addresses. Trailing punctuation and
//! unbalanced closing parentheses stay outside the link, as GitHub does.

use guidedoc::{LinkKind, NodeKind};

use super::{Inline, Txt};

const LINKIFY_TLDS: [&str; 19] = [
    "biz", "com", "edu", "gov", "net", "org", "pro", "web", "xxx", "aero", "asia", "coop", "info",
    "museum", "name", "shop", "io", "dev", "app",
];

/// A URL found in text: its byte range, the prefix its destination needs ("http://" for
/// www. and fuzzy hosts, "mailto:" for e-mail), and whether it is e-mail.
#[derive(Debug, Clone, Copy)]
struct Found {
    a: usize,
    z: usize,
    prefix: &'static str,
    email: bool,
}

impl Inline<'_, '_, '_, '_> {
    /// Links the URLs in every text item outside links.
    pub(super) fn linkify_items(&mut self) {
        for id in self.text_items(0, false) {
            self.linkify_text(id);
        }
    }

    /// Splits a text item around each URL it holds.
    fn linkify_text(&mut self, id: usize) {
        let fuzzy = self.h.settings.linkify_fuzzy_links;
        let mut cur = Some(id);
        while let Some(c) = cur {
            let it = self.items[c];
            let Some(f) = find_url(self.str(it.text).as_bytes(), fuzzy) else { return };
            let whole = it.text;
            let span_a = if it.end - it.start == whole.len { it.start + f.a } else { it.start };
            let span_z = span_a + (f.z - f.a);
            let link = self.new_item(NodeKind::Link, span_a, span_z);
            let text = self.new_item(NodeKind::Text, span_a, span_z);
            // E-mail links keep the bare address, as e-mail autolinks do.
            let mut dest = String::new();
            if !f.email {
                dest.push_str(f.prefix);
            }
            dest.push_str(&self.str(whole)[f.a..f.z]);
            let dest = self.push_text(&dest);
            let l = &mut self.items[link];
            l.dest = dest;
            l.link_kind = if f.email { LinkKind::Email } else { LinkKind::External };
            l.autolink = true;
            // The pieces are ranges of the item's text, not copies.
            self.items[text].text = Txt { start: whole.start + f.a, len: f.z - f.a };
            self.append_child(link, text);
            let mut after = None;
            if f.z < whole.len {
                let a = self.new_item(NodeKind::Text, span_z, it.end);
                self.items[a].text = Txt { start: whole.start + f.z, len: whole.len - f.z };
                after = Some(a);
            }
            self.items[c].text = Txt { start: whole.start, len: f.a };
            self.items[c].end = span_a;
            self.insert_after(c, link);
            if let Some(a) = after {
                self.insert_after(link, a);
            }
            cur = after;
        }
    }
}

/// Finds the first URL in `t`.
fn find_url(t: &[u8], fuzzy: bool) -> Option<Found> {
    for i in 0..t.len() {
        if i > 0 && (t[i - 1].is_ascii_alphanumeric() || t[i - 1] == b'@' || t[i - 1] == b'.') {
            continue;
        }
        let rest = &t[i..];
        let n = scheme_prefix(rest);
        if n > 0 {
            let e = url_end(t, i + n);
            if e > i + n {
                return Some(Found { a: i, z: e, prefix: "", email: false });
            }
            continue;
        }
        if rest.len() > 4 && rest[..4].eq_ignore_ascii_case(b"www.") {
            let e = url_end(t, i);
            if host_len(&t[i + 4..]) > 0 && e > i + 4 {
                return Some(Found { a: i, z: e, prefix: "http://", email: false });
            }
            continue;
        }
        if !fuzzy || !t[i].is_ascii_alphanumeric() {
            continue;
        }
        if let Some(e) = fuzzy_email(t, i) {
            return Some(Found { a: i, z: e, prefix: "mailto:", email: true });
        }
        let n = host_len(rest);
        if n > 0 && known_tld(&rest[..n]) {
            let e = url_end(t, i);
            if e >= i + n {
                return Some(Found { a: i, z: e, prefix: "http://", email: false });
            }
        }
    }
    None
}

fn scheme_prefix(s: &[u8]) -> usize {
    for pre in ["http://", "https://", "ftp://", "mailto:"] {
        if s.len() > pre.len() && s[..pre.len()].eq_ignore_ascii_case(pre.as_bytes()) {
            return pre.len();
        }
    }
    0
}

/// The length of a host name at the start of `s` (labels of letters, digits, and
/// hyphens joined by dots, at least two labels), or 0.
fn host_len(s: &[u8]) -> usize {
    let (mut i, mut labels, mut last) = (0, 0, 0);
    while i < s.len() {
        let a = i;
        while i < s.len() && (s[i].is_ascii_alphanumeric() || s[i] == b'-') {
            i += 1;
        }
        if i == a {
            break;
        }
        labels += 1;
        last = i;
        if i + 1 < s.len() && s[i] == b'.' && s[i + 1].is_ascii_alphanumeric() {
            i += 1;
            continue;
        }
        break;
    }
    if labels >= 2 { last } else { 0 }
}

fn known_tld(host: &[u8]) -> bool {
    let k = host.iter().rposition(|&c| c == b'.').map_or(0, |k| k + 1);
    let tld = &host[k..];
    if tld.len() == 2 && tld.iter().all(u8::is_ascii_alphabetic) {
        return true;
    }
    LINKIFY_TLDS.iter().any(|t| tld.eq_ignore_ascii_case(t.as_bytes()))
}

fn fuzzy_email(t: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    while j < t.len() && (t[j].is_ascii_alphanumeric() || b"._-+".contains(&t[j])) {
        j += 1;
    }
    if j == i || j >= t.len() || t[j] != b'@' {
        return None;
    }
    let n = host_len(&t[j + 1..]);
    if n == 0 || !known_tld(&t[j + 1..j + 1 + n]) {
        return None;
    }
    Some(j + 1 + n)
}

/// Where a URL starting at `i` ends: at white space or "<", less trailing punctuation
/// and unbalanced closing parentheses.
fn url_end(t: &[u8], i: usize) -> usize {
    let mut e = i;
    while e < t.len() && !guidedoc::is_ascii_space(t[e]) && t[e] != b'<' {
        e += 1;
    }
    while e > i {
        let c = t[e - 1];
        if b"?!.,:*_~;'\"".contains(&c) {
            e -= 1;
            continue;
        }
        if c == b')' {
            let open = t[i..e].iter().filter(|&&x| x == b'(').count();
            let close = t[i..e].iter().filter(|&&x| x == b')').count();
            if close > open {
                e -= 1;
                continue;
            }
        }
        break;
    }
    e
}
