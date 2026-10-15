//! Standalone hyperlinks: an absolute URI with a known scheme, or an email address, in
//! text outside other inline markup. The rules follow the specification's "Standalone
//! Hyperlinks" (RFC 2396 URI characters; the last character may not be punctuation
//! unless a ">" follows).

use guidedoc::{Fail, LinkInfo, LinkKind, NodeKind};

use crate::emit::BreakMode;
use crate::scan::{MARK_BLOCKED, MARK_BOUNDARY, Scanner};

/// Lets the gap scan skip work whose outcome is already known: every start inside one
/// scheme run shares the text after the colon, every colon inside one run of URI
/// characters shares the longest valid end, and every start inside one local part
/// shares the text after the "@".
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct UriMemo {
    /// Starts before this share `colon` and `tail_end`.
    scheme_until: usize,
    colon: usize,
    /// End of the absolute URI after `colon`.
    tail_end: Option<usize>,
    /// Starts before this cannot begin an email address.
    email_until: usize,
    /// A hierarchical part starting in `[run_from, run_end)` within `run_bound` has
    /// the longest valid end `run_best`.
    run_from: usize,
    run_end: usize,
    run_bound: usize,
    run_best: Option<usize>,
}

fn is_uric(c: u8) -> bool {
    c.is_ascii_alphanumeric()
        || c == 0
        || matches!(
            c,
            b'-' | b'_'
                | b'.'
                | b'!'
                | b'~'
                | b'*'
                | b'\''
                | b'('
                | b')'
                | b'['
                | b']'
                | b';'
                | b'/'
                | b':'
                | b'@'
                | b'&'
                | b'='
                | b'+'
                | b'$'
                | b','
                | b'%'
        )
}

fn is_urilast(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'_' | b'~' | b'*' | b'/' | b'=' | b'+')
}

fn is_emailc(c: u8) -> bool {
    c.is_ascii_alphanumeric()
        || c == 0
        || matches!(
            c,
            b'-' | b'_'
                | b'!'
                | b'~'
                | b'*'
                | b'\''
                | b'{'
                | b'|'
                | b'}'
                | b'/'
                | b'#'
                | b'?'
                | b'^'
                | b'`'
                | b'&'
                | b'='
                | b'+'
                | b'$'
                | b'%'
        )
}

/// The registered URI schemes, lowercase and sorted.
#[rustfmt::skip]
const URI_SCHEMES: &[&str] = &[
    "about", "acap", "addbook", "afp", "afs", "aim", "callto", "castanet", "chttp", "cid",
    "crid", "data", "dav", "dict", "dns", "eid", "fax", "feed", "file", "finger", "freenet",
    "ftp", "geo", "git", "go", "gopher", "gsm-sms", "h323", "h324", "hdl", "hnews", "http",
    "https", "hydra", "iioploc", "ilu", "im", "imap", "info", "ior", "ipp", "irc", "irc6",
    "ircs", "iris.beep", "iseek", "jar", "javascript", "jdbc", "ldap", "lifn", "livescript",
    "lrq", "magnet", "mailbox", "mailserver", "mailto", "md5", "mid", "mocha", "modem",
    "mtqp", "mupdate", "news", "nfs", "nntp", "opaquelocktoken", "phone", "pop", "pop3",
    "pres", "printer", "prospero", "rdar", "res", "rsync", "rtsp", "rvp", "rwhois", "rx",
    "s3", "sdp", "service", "sftp", "shttp", "sip", "sips", "smb", "sms", "snmp",
    "soap.beep", "soap.beeps", "ssh", "svn", "t120", "tag", "tcp", "tel", "telephone",
    "telnet", "tftp", "tip", "tn3270", "tv", "urn", "uuid", "vemmi", "videotex",
    "view-source", "wais", "whodp", "whois++", "ws", "wss", "x-man-page", "xmlrpc.beep",
    "xmlrpc.beeps", "xmpp", "z39.50r", "z39.50s",
];

/// Compares case-insensitively with the registered URI schemes.
fn known_scheme(s: &[u8]) -> bool {
    s.len() <= 32 && URI_SCHEMES.iter().any(|k| k.as_bytes().eq_ignore_ascii_case(s))
}

impl Scanner<'_, '_> {
    /// The character before `e` may end a URI.
    fn uri_last_ok(&self, e: usize, bound: usize) -> bool {
        let c = self.bytes()[e - 1];
        is_urilast(c) || (is_uric(c) && e < bound && self.bytes()[e] == b'>')
    }

    fn uric_run(&self, p: usize, bound: usize) -> usize {
        let mut i = p;
        while i < bound && is_uric(self.bytes()[i]) {
            i += 1;
        }
        i
    }

    /// Matches the hierarchical part, optional query, and optional fragment after the
    /// colon at `c`; returns the end of the longest match followed by an end context.
    fn absolute_tail(&mut self, c: usize, bound: usize) -> Result<Option<usize>, Fail> {
        let h0 = c + 1;
        let memo = self.uri;
        if memo.run_bound == bound && memo.run_from <= h0 && h0 < memo.run_end {
            return Ok(memo.run_best.filter(|&b| b > h0));
        }
        let t = self.bytes();
        let h = self.uric_run(h0, bound);
        let mut q = h;
        if h < bound && t[h] == b'?' {
            q = self.uric_run(h + 1, bound);
        }
        let mut f = q;
        if q < bound && t[q] == b'#' {
            f = self.uric_run(q + 1, bound);
        }
        let hier_done = h > h0 && is_urilast(t[h - 1]);
        let query_done = q == h || (q >= h + 2 && is_urilast(t[q - 1]));
        let best = (h0 + 1..=f).rev().find(|&e| {
            self.uri_last_ok(e, bound)
                && self.end_ok(e, bound)
                && (e <= h || hier_done)
                && (e <= q || query_done)
        });
        self.ctx.b.spend((f - h0) as u64 / 8 + 1)?;
        self.uri.run_from = h0;
        self.uri.run_end = h;
        self.uri.run_bound = bound;
        self.uri.run_best = best;
        Ok(best)
    }

    /// Matches scheme ":" and a tail at `p`; returns the end and the colon position.
    fn absolute_end(&mut self, p: usize, bound: usize) -> Result<(Option<usize>, usize), Fail> {
        let t = self.bytes();
        if p >= bound || !t[p].is_ascii_alphabetic() {
            return Ok((None, p));
        }
        let mut c = p + 1;
        while c < bound && (t[c].is_ascii_alphanumeric() || matches!(t[c], b'.' | b'+' | b'-')) {
            c += 1;
        }
        if c >= bound || t[c] != b':' {
            return Ok((None, c));
        }
        self.ctx.b.spend((c - p) as u64 / 16 + 1)?;
        Ok((self.absolute_tail(c, bound)?, c))
    }

    /// Matches an email address at `p`: its end, the end of its local part, and the
    /// position after the last ".." in the local part (0 for none), which no address
    /// may contain.
    pub fn email_end(&self, p: usize, bound: usize) -> (Option<usize>, usize, usize) {
        let t = self.bytes();
        if p >= bound || !is_emailc(t[p]) {
            return (None, p + 1, 0);
        }
        let mut dd = 0;
        let mut r = p;
        while r < bound && (is_emailc(t[r]) || t[r] == b'.') {
            if t[r] == b'.' && r > p && t[r - 1] == b'.' {
                dd = r + 1;
            }
            r += 1;
        }
        if dd > 0 || r >= bound || t[r] != b'@' || t[r - 1] == b'.' || t[r - 1] == 0 {
            return (None, r, dd);
        }
        if r + 1 >= bound || !is_emailc(t[r + 1]) {
            return (None, r, 0);
        }
        let mut h = r + 1;
        while h < bound && (is_emailc(t[h]) || t[h] == b'.') {
            h += 1;
        }
        let end = (r + 2..=h).rev().find(|&e| self.uri_last_ok(e, bound) && self.end_ok(e, bound));
        (end, r, 0)
    }

    /// The text `[a, b)` starts with something shaped like a URI or an email address;
    /// an embedded "<name_>" that does is taken as a URI.
    pub fn uri_matches_at(&mut self, a: usize, b: usize) -> bool {
        // A failure here (spent work) poisons the builder; the next call reports it.
        if matches!(self.absolute_end(a, b), Ok((Some(_), _))) {
            return true;
        }
        self.email_end(a, b).0.is_some()
    }

    /// Matches a standalone hyperlink at `p` within the text piece `[base, bound)`.
    fn match_link(
        &mut self,
        p: usize,
        base: usize,
        bound: usize,
    ) -> Result<Option<(usize, LinkKind)>, Fail> {
        let c = self.bytes()[p];
        if !is_emailc(c) || c == 0 || !self.start_ok(p, base) {
            return Ok(None);
        }
        if c.is_ascii_alphabetic() {
            if p >= self.uri.scheme_until {
                let (tail, colon) = self.absolute_end(p, bound)?;
                self.uri.tail_end = tail;
                self.uri.colon = colon;
                self.uri.scheme_until = colon;
            }
            let colon = self.uri.colon;
            if colon < bound
                && self.bytes()[colon] == b':'
                && let Some(end) = self.uri.tail_end
            {
                // The absolute URI alternative matches; an unknown scheme is no link.
                if !known_scheme(&self.bytes()[p..colon]) {
                    return Ok(None);
                }
                return Ok(Some((end, LinkKind::External)));
            }
        }
        if p < self.uri.email_until {
            return Ok(None);
        }
        let (end, local_end, dd) = self.email_end(p, bound);
        if let Some(end) = end {
            return Ok(Some((end, LinkKind::Email)));
        }
        self.uri.email_until = if dd > 0 { dd } else { local_end };
        Ok(None)
    }

    /// Emits the pending text before a construct that starts at `p`.
    pub fn flush(&mut self, p: usize) -> Result<(), Fail> {
        if self.pos < p {
            self.emit_gap(self.pos, p)?;
        }
        self.pos = p;
        Ok(())
    }

    /// Emits text `[a, b)` between constructs, recognizing standalone hyperlinks in
    /// each piece that a problematic start-string did not split off.
    fn emit_gap(&mut self, a: usize, b: usize) -> Result<(), Fail> {
        let mut run = a;
        let mut piece = a;
        while piece < b {
            let mut end = piece + 1;
            while end < b && self.marks[end] & MARK_BOUNDARY == 0 {
                end += 1;
            }
            if self.marks[piece] & MARK_BLOCKED == 0 {
                run = self.scan_links(piece, end, run)?;
            }
            piece = end;
        }
        self.emit_cooked(run, b, BreakMode::Breaks)
    }

    fn scan_links(&mut self, a: usize, b: usize, run: usize) -> Result<usize, Fail> {
        let mut run = run;
        let mut base = a;
        let mut p = a;
        while p < b {
            self.ctx.b.spend(1)?;
            if let Some((ie, n, pep)) = self.match_implicit(p, b) {
                self.emit_cooked(run, p, BreakMode::Breaks)?;
                self.emit_implicit(p, ie, n, pep)?;
                (run, p, base) = (ie, ie, ie);
                continue;
            }
            let Some((e, kind)) = self.match_link(p, base, b)? else {
                p += self.rune_at(p).1.max(1);
                continue;
            };
            self.emit_cooked(run, p, BreakMode::Breaks)?;
            let start = self.ctx.b.text_open();
            self.push_cooked(p, e)?;
            let t = self.ctx.b.text_close(start);
            let span = self.span(p, e);
            let id = self.ctx.b.begin_node(NodeKind::Link, span)?;
            self.ctx.b.set_link(id, LinkInfo { destination: t, kind, ..LinkInfo::UNRESOLVED })?;
            self.ctx.b.add_leaf(NodeKind::Text, span, t)?;
            self.ctx.b.end_node(id)?;
            (run, p, base) = (e, e, e);
        }
        Ok(run)
    }
}
