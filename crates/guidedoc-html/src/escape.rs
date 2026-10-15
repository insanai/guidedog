//! Escaping of text, attribute values, and URLs, and the URL policy.

use guidedoc::{ContentPolicy, LinkInfo, LinkKind, NodeId, arg_span};

use crate::state::{Html, Writer};
use crate::{UNRESOLVED, URL_OMITTED, URL_UNSAFE};

impl Writer<'_, '_> {
    /// Writes text or an attribute value with &, <, >, and " escaped, and, outside the
    /// reference profile, @ as &#64;, as Docutils' HTML writer encodes text and
    /// attributes.
    pub fn esc(&mut self, s: &str) {
        self.escape(s, !self.reference);
    }

    /// Escapes as Pygments does, which leaves @ as it is: highlighted code and the
    /// permalinks Sphinx writes without encoding.
    pub fn esc_code(&mut self, s: &str) {
        self.escape(s, false);
    }

    fn escape(&mut self, s: &str, at: bool) {
        let mut run = 0;
        for (i, c) in s.bytes().enumerate() {
            let replacement = match c {
                b'&' => "&amp;",
                b'<' => "&lt;",
                b'>' => "&gt;",
                b'"' => "&quot;",
                b'@' if at => "&#64;",
                _ => continue,
            };
            self.put(&s[run..i]);
            self.put(replacement);
            run = i + 1;
        }
        self.put(&s[run..]);
    }

    /// Percent-encodes bytes outside the URL-safe set, keeping existing %XX escapes,
    /// and escapes the result for an attribute value.
    pub fn write_url(&mut self, s: &str) {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i];
            let escaped = c == b'%'
                && i + 2 < bytes.len()
                && bytes[i + 1].is_ascii_hexdigit()
                && bytes[i + 2].is_ascii_hexdigit();
            if escaped {
                self.put(&s[i..i + 3]);
                i += 3;
                continue;
            }
            if c == b'&' {
                self.put("&amp;");
            } else if c == b'@' && !self.reference {
                self.put("&#64;"); // Docutils encodes attribute values as text.
            } else if url_safe_byte(c) {
                self.put_byte(c);
            } else {
                self.put_byte(b'%');
                self.put_byte(HEX[usize::from(c >> 4)]);
                self.put_byte(HEX[usize::from(c & 15)]);
            }
            i += 1;
        }
    }
}

fn url_safe_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || b";/?:@&=+$,-_.!~*'()#".contains(&c)
}

/// The data: URLs of common raster images, which cannot run code.
const SAFE_IMAGES: [&str; 4] = ["image/png", "image/gif", "image/jpeg", "image/webp"];

/// Tells whether `dest` is a javascript:, vbscript:, or file: URL, or a data: URL other
/// than a common raster image. Browsers ignore leading controls, and tabs or newlines
/// in a scheme.
pub(crate) fn url_unsafe(dest: &str) -> bool {
    let bytes = dest.as_bytes();
    let mut scheme = [0u8; 16];
    let mut n = 0;
    let mut i = bytes.iter().position(|&c| c > b' ').unwrap_or(bytes.len());
    while i < bytes.len() {
        let c = bytes[i];
        if c == b':' {
            break;
        }
        i += 1;
        if matches!(c, b'\t' | b'\n' | b'\r') {
            continue;
        }
        if !c.is_ascii_alphanumeric() && !matches!(c, b'+' | b'-' | b'.') {
            return false;
        }
        if n >= scheme.len() {
            return false;
        }
        scheme[n] = c.to_ascii_lowercase();
        n += 1;
    }
    if i >= bytes.len() {
        return false;
    }
    match &scheme[..n] {
        b"javascript" | b"vbscript" | b"file" => true,
        b"data" => {
            let rest = &bytes[i + 1..];
            !SAFE_IMAGES.iter().any(|kind| {
                rest.len() >= kind.len()
                    && rest[..kind.len()].eq_ignore_ascii_case(kind.as_bytes())
            })
        }
        _ => false,
    }
}

impl Html<'_, '_> {
    /// Writes ` attr="url"` for a link-like node, applying resolution and the URL
    /// policy. It returns false when the attribute was left out.
    pub fn write_target(&mut self, attr: &str, id: NodeId) -> bool {
        let l = self.doc.link_info(id).unwrap_or(LinkInfo::UNRESOLVED);
        let dest = self.doc.text(l.destination);
        let span = self.doc.node(id).span;
        match l.kind {
            LinkKind::Internal => {
                let Some(a) = self.anchors.anchor_of(l.resolved) else {
                    return false;
                };
                self.w.put_byte(b' ');
                self.w.put(attr);
                self.w.put("=\"#");
                self.w.esc(a);
                self.w.put_byte(b'"');
                return true;
            }
            LinkKind::Unresolved => {
                self.w.fail(UNRESOLVED, span, arg_span(span), guidedoc::Arg::None);
                return false;
            }
            LinkKind::External | LinkKind::Project | LinkKind::Email => {}
        }
        if l.kind != LinkKind::Email && url_unsafe(dest) {
            match self.policy.unsafe_urls {
                ContentPolicy::Refuse => {
                    self.w.fail(URL_UNSAFE, span, arg_span(span), guidedoc::Arg::None);
                    return false;
                }
                ContentPolicy::Omit => {
                    self.w.fail(URL_OMITTED, span, arg_span(span), guidedoc::Arg::None);
                    return false;
                }
                ContentPolicy::Allow => {}
            }
        }
        self.w.put_byte(b' ');
        self.w.put(attr);
        self.w.put("=\"");
        if l.kind == LinkKind::Email {
            self.w.put("mailto:");
        }
        self.w.write_url(dest);
        self.w.put_byte(b'"');
        true
    }
}
