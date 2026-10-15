//! Escaping: what a reStructuredText reader would take as markup in plain text, the
//! inline markup recognition rules around start- and end-strings, and the line starts
//! that would begin another construct.

use guidedoc::{is_unicode_punctuation, is_unicode_whitespace};

/// The URI schemes a standalone hyperlink may start with (Docutils' list), lowercase.
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

/// Tells whether `c` may stand right before an inline markup start-string.
pub(crate) fn may_precede_markup(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_whitespace() || "-:/'\"<([{".contains(c);
    }
    is_unicode_whitespace(c) || (is_unicode_punctuation(c) && OPENERS.contains(c))
}

/// Non-ASCII punctuation that may stand before a start-string: dashes, opening
/// brackets, and quotation marks.
const OPENERS: &str = "‐‑‒–—―([{«‹‘‚‛“„‟〈《「『【〔〖〘〚";

/// Non-ASCII punctuation that may stand after an end-string.
const CLOSERS: &str = "‐‑‒–—―)]}»›’‘‛”“‟〉》」』】〕〗〙〛…、。，．：；！？";

/// Tells whether `c` may stand right after an inline markup end-string.
pub(crate) fn may_follow_markup(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_whitespace() || "-.,:;!?\\/'\")]}>".contains(c);
    }
    is_unicode_whitespace(c) || (is_unicode_punctuation(c) && CLOSERS.contains(c))
}

fn is_email_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "._%+-".contains(c)
}

/// Tells whether the colon at byte `at` of `s` would end the scheme of a standalone
/// hyperlink.
fn ends_scheme(s: &str, at: usize) -> bool {
    let head = &s[..at];
    let start = head
        .char_indices()
        .rev()
        .find(|&(_, c)| !(c.is_ascii_alphanumeric() || "+-.".contains(c)))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let scheme = &head[start..];
    let follows = s[at + 1..].chars().next().is_some_and(|c| !c.is_whitespace());
    follows && URI_SCHEMES.iter().any(|k| k.eq_ignore_ascii_case(scheme))
}

/// Appends plain text as reStructuredText paragraph text: backslashes and the
/// characters that start or end inline markup are escaped, and so are the colon of a
/// URI scheme and the at sign of an address, which would make a link. `next` is the
/// character that will follow the text, if known.
pub(crate) fn push_text(out: &mut String, s: &str, next: Option<char>) {
    let mut chars = s.char_indices().peekable();
    let mut prev: Option<char> = out.chars().next_back();
    while let Some((i, c)) = chars.next() {
        let after = chars.peek().map(|&(_, n)| n).or(next);
        let escape = match c {
            '\\' | '*' | '`' | '|' => true,
            '_' => !after.is_some_and(|n| n.is_alphanumeric()),
            ':' => ends_scheme(s, i),
            '@' => {
                prev.is_some_and(is_email_char) && after.is_some_and(|n| n.is_ascii_alphanumeric())
            }
            '[' => after == Some('#') || after == Some('*'),
            _ => false,
        };
        if escape {
            out.push('\\');
        }
        out.push(c);
        prev = Some(c);
    }
}

/// Appends text inside interpreted text, a phrase reference, or emphasis: backslashes,
/// backquotes, and asterisks are escaped, and `<` when `angle` (a phrase whose end
/// would look like an embedded target).
pub(crate) fn push_markup_text(out: &mut String, s: &str, angle: bool) {
    for c in s.chars() {
        if c == '\\' || c == '`' || c == '*' || (angle && c == '<') {
            out.push('\\');
        }
        out.push(c);
    }
}

/// Tells whether a line of paragraph text, written first in its paragraph, would
/// start another construct (a list item, a field, an option, explicit markup, a
/// doctest block, a transition, or a literal block marker).
pub(crate) fn starts_construct(line: &str) -> bool {
    let first = line.chars().next();
    let Some(first) = first else { return false };
    let rest = &line[first.len_utf8()..];
    let spaced = |r: &str| r.is_empty() || r.starts_with(' ');
    if "-+*•‣⁃".contains(first) && spaced(rest) {
        return true;
    }
    if is_adornment(line) || line.starts_with("..") || line.starts_with(">>>") {
        return true;
    }
    if line.starts_with("__ ") || line == "__" || line.starts_with("::") {
        return true;
    }
    if first == ':' && field_marker(line) {
        return true;
    }
    if (first == '-' || first == '/') && rest.starts_with(|c: char| c.is_alphanumeric()) {
        return true;
    }
    if line.starts_with("--") && rest[1..].starts_with(|c: char| c.is_alphanumeric()) {
        return true;
    }
    if first == '|' && spaced(rest) {
        return true;
    }
    enumerator(line)
}

/// Tells whether a line starts with a field marker `:name:` followed by a space or
/// the end of the line.
fn field_marker(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b':' if i > 1 && bytes[i - 1] != b' ' => {
                if i + 1 == bytes.len() || bytes[i + 1] == b' ' {
                    return true;
                }
                if bytes[i + 1] == b'`' {
                    return false;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    false
}

/// Tells whether a line starts with an enumerator: `1.`, `a)`, `(iv)`, `#.`, followed
/// by a space or the end of the line.
fn enumerator(line: &str) -> bool {
    let (open, body) = match line.strip_prefix('(') {
        Some(b) => (true, b),
        None => (false, line),
    };
    let end = body.find(|c: char| !c.is_ascii_alphanumeric() && c != '#').unwrap_or(body.len());
    let token = &body[..end];
    let tail = &body[end..];
    let delim = if open { ')' } else { '.' };
    let closes = tail.starts_with(delim) || (!open && tail.starts_with(')'));
    if token.is_empty() || !closes {
        return false;
    }
    let after = &tail[1..];
    if !(after.is_empty() || after.starts_with(' ')) {
        return false;
    }
    let numeric = token.bytes().all(|b| b.is_ascii_digit());
    let alpha = token.len() == 1 && token.bytes().all(|b| b.is_ascii_alphabetic());
    let roman = token.bytes().all(|b| b"ivxlcdmIVXLCDM".contains(&b));
    numeric || alpha || roman || token == "#"
}

/// Tells whether a line is a run of one repeated punctuation character, which would be
/// a section adornment or a transition.
pub(crate) fn is_adornment(line: &str) -> bool {
    let mut chars = line.chars();
    let Some(first) = chars.next() else { return false };
    first.is_ascii_punctuation() && line.len() >= 2 && chars.all(|c| c == first)
}

/// Tells whether `name` can be written as a bare reference name (`name_`): word
/// characters, with single internal hyphens, underscores, periods, colons, or plus
/// signs.
pub(crate) fn is_simple_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut prev_punct = true;
    for c in name.chars() {
        if c.is_alphanumeric() {
            prev_punct = false;
        } else if "-_.:+".contains(c) && !prev_punct {
            prev_punct = true;
        } else {
            return false;
        }
    }
    !prev_punct
}

/// The display width of `s` in columns: East Asian wide and full-width characters
/// take two, as Docutils counts them for title underlines and table borders.
pub(crate) fn width(s: &str) -> usize {
    s.chars()
        .map(|c| {
            let u = c as u32;
            let wide = (0x1100..=0x115F).contains(&u)
                || (0x2E80..=0xA4CF).contains(&u)
                || (0xAC00..=0xD7A3).contains(&u)
                || (0xF900..=0xFAFF).contains(&u)
                || (0xFE30..=0xFE4F).contains(&u)
                || (0xFF00..=0xFF60).contains(&u)
                || (0xFFE0..=0xFFE6).contains(&u)
                || (0x1F300..=0x1F64F).contains(&u)
                || (0x20000..=0x3FFFD).contains(&u);
            let combining = (0x300..=0x36F).contains(&u) || u == 0x200B;
            if combining {
                0
            } else if wide {
                2
            } else {
                1
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> String {
        let mut out = String::new();
        push_text(&mut out, s, None);
        out
    }

    #[test]
    fn markup_characters_are_escaped() {
        assert_eq!(text("a*b `c` |d| e\\f"), "a\\*b \\`c\\` \\|d\\| e\\\\f");
        assert_eq!(text("snake_case name_ x__"), "snake_case name\\_ x\\_\\_");
        assert_eq!(text("see http://x and a@b.c"), "see http\\://x and a\\@b.c");
        assert_eq!(text("std::vector key: value"), "std::vector key: value");
    }

    #[test]
    fn line_starts_that_begin_constructs() {
        for line in [
            "- item",
            "* x",
            "1. one",
            "a) b",
            "(iv) x",
            "#. y",
            ":field: v",
            "..",
            "-v",
            ":a:, :b:",
            "--all",
            ">>> x",
            "----",
            "::",
            "| x",
            "__ http://x",
        ] {
            assert!(starts_construct(line), "{line}");
        }
        for line in [
            "-- dash",
            "word",
            "1.5 times",
            "ab. c",
            ":role:`x`",
            ":doc:`a` b: c",
            "*em*",
            "(see)",
        ] {
            assert!(!starts_construct(line), "{line}");
        }
    }

    #[test]
    fn simple_names_and_widths() {
        assert!(is_simple_name("a-b.c"));
        assert!(!is_simple_name("a b"));
        assert!(!is_simple_name("a--b"));
        assert!(!is_simple_name("a_"));
        assert_eq!(width("日本"), 4);
    }
}
