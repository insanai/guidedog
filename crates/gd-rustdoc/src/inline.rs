//! Markdown inline text to reStructuredText inline text: code spans, emphasis, links,
//! intra-doc links (as `rust` domain roles), autolinks, and escaping of everything else
//! reST would read as markup.
//!
//! reST inline markup has to stand apart from its neighbours (``x``s is not markup); an
//! escaped space (`\ `) is written wherever Markdown put markup against a letter.

use std::collections::HashMap;

/// What an intra-doc link destination names, as the emitter resolves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Link {
    /// A documented item: the role to link with (`fn`, `struct`, ...) and its path.
    Found { role: &'static str, path: String },
    /// An item of this crate that is not documented (private or hidden): shown as code,
    /// without a link, as rustdoc does.
    Plain,
    /// Not a documented item of this crate; the path with any disambiguator removed,
    /// left to the reader's `:rust:any:` resolution (another crate, the standard library).
    Unresolved { path: String },
}

/// Resolves intra-doc link destinations (`Foo`, `crate::a::b`, `struct@Foo`, `foo()`).
pub(crate) trait LinkResolver {
    /// Resolves `destination`; `explicit` is true when the author certainly meant a link
    /// (`[text](path)`, a reference, or backquoted `[`Foo`]`), so a failure is worth a
    /// warning.
    fn resolve(&mut self, destination: &str, explicit: bool) -> Option<Link>;
}

/// Converts Markdown inline text. Link reference definitions are looked up in `refs`
/// (keys lower-cased).
pub(crate) struct Inline<'r> {
    pub links: &'r mut dyn LinkResolver,
    pub refs: &'r HashMap<String, String>,
}

/// Characters reST accepts right before inline markup starts.
fn may_precede_markup(c: char) -> bool {
    c.is_whitespace() || "'\"([{<-/:‘“’«¡¿‐‑‒–— ".contains(c)
}

/// Characters reST accepts right after inline markup ends.
fn may_follow_markup(c: char) -> bool {
    c.is_whitespace() || "'\")]}>-/:.,;!?\\’”»‐‑‒–— ".contains(c)
}

/// The output of one conversion, which knows how to keep markup apart.
struct Out {
    text: String,
    /// The last thing written was markup; a letter next needs `\ ` first.
    after_markup: bool,
}

impl Out {
    fn begin_markup(&mut self) {
        if let Some(last) = self.text.chars().last()
            && !may_precede_markup(last)
        {
            self.text.push_str("\\ ");
        }
    }

    fn end_markup(&mut self) {
        self.after_markup = true;
    }

    /// Writes text that is already reST (markup or escaped text).
    fn raw(&mut self, s: &str) {
        if self.after_markup {
            self.after_markup = false;
            if s.chars().next().is_some_and(|c| !may_follow_markup(c)) {
                self.text.push_str("\\ ");
            }
        }
        self.text.push_str(s);
    }

    /// Writes one character of plain text, escaped so reST shows it as written.
    /// `prev` and `next` are its neighbours in the source, which decide whether an
    /// underscore could end a reference name.
    fn plain(&mut self, c: char, prev: Option<char>, next: Option<char>) {
        let escape = match c {
            '\\' | '*' | '`' | '|' => true,
            '_' => {
                !prev.is_some_and(char::is_alphanumeric)
                    || !next.is_some_and(char::is_alphanumeric)
            }
            _ => false,
        };
        let mut buffer = [0u8; 4];
        if escape {
            self.raw("\\");
            self.text.push(c);
        } else {
            self.raw(c.encode_utf8(&mut buffer));
        }
    }
}

/// Escapes `text` as plain reST text with no markup.
pub(crate) fn escape_plain(text: &str) -> String {
    let mut out = Out { text: String::with_capacity(text.len()), after_markup: false };
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).map(|p| chars[p]);
        out.plain(c, prev, chars.get(i + 1).copied());
    }
    out.text
}

/// The visible text of Markdown inline text, without markup: for synopses and the
/// insides of reST markup that cannot nest.
pub(crate) fn strip_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '`' | '*' => {}
            '_' if !is_intraword(&chars, i) => {}
            '\\' if chars.get(i + 1).is_some_and(|n| n.is_ascii_punctuation()) => {
                i += 1;
                out.push(chars[i]);
            }
            '!' if chars.get(i + 1) == Some(&'[') => {}
            '[' | ']' => {}
            '(' if i > 0 && chars[i - 1] == ']' => {
                // Skip a link destination.
                let close = matching(&chars, i, '(', ')').unwrap_or(chars.len() - 1);
                i = close;
            }
            _ => out.push(c),
        }
        i += 1;
    }
    out
}

fn is_intraword(chars: &[char], i: usize) -> bool {
    let before = i > 0 && chars[i - 1].is_alphanumeric();
    let after = chars.get(i + 1).is_some_and(|c| c.is_alphanumeric());
    before && after
}

/// The index of the bracket closing the one at `open`, skipping backslash escapes and
/// code spans; `None` when it is not closed.
fn matching(chars: &[char], open: usize, left: char, right: char) -> Option<usize> {
    let mut depth = 0;
    let mut i = open;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            i += 2;
            continue;
        }
        if c == '`'
            && let Some(end) = code_span_end(chars, i)
        {
            i = end;
            continue;
        }
        if c == left {
            depth += 1;
        } else if c == right {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// The length of the run of `c` starting at `i`.
fn run_length(chars: &[char], i: usize, c: char) -> usize {
    chars[i..].iter().take_while(|&&x| x == c).count()
}

/// The index after the code span whose backtick run starts at `i`, or `None` when the
/// run is not closed by one of the same length.
fn code_span_end(chars: &[char], i: usize) -> Option<usize> {
    let n = run_length(chars, i, '`');
    let mut j = i + n;
    while j < chars.len() {
        if chars[j] == '`' {
            let m = run_length(chars, j, '`');
            if m == n {
                return Some(j + m);
            }
            j += m;
        } else {
            j += 1;
        }
    }
    None
}

/// Whether a link destination is a URL (or a file or anchor) rather than a Rust path.
fn is_url(destination: &str) -> bool {
    destination.contains("://")
        || destination.starts_with("mailto:")
        || destination.starts_with(['#', '/', '.'])
        || destination.ends_with(".html")
        || destination.ends_with(".md")
        || destination.contains(".html#")
}

/// Whether text could be a Rust path an intra-doc link names: `Foo`, `a::b`,
/// `struct@Foo`, `foo()`, `foo!`, `Vec<T>`.
fn is_path_like(text: &str) -> bool {
    let text = text.trim();
    !text.is_empty()
        && text.chars().all(|c| c.is_alphanumeric() || "_:@!()<>, ".contains(c))
        && text.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == ':')
        && !text.contains("  ")
        && (!text.contains(' ') || text.contains('<'))
}

/// A parsed `[text](destination)`, `[text][label]` or `[text]`.
struct LinkSyntax {
    text: String,
    destination: Option<String>,
    /// Whether the destination was written (inline or by reference) rather than taken
    /// from the text.
    explicit: bool,
    end: usize,
}

impl Inline<'_> {
    /// Converts one paragraph's inline text.
    pub fn convert(&mut self, text: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        let mut out = Out { text: String::with_capacity(text.len() + 16), after_markup: false };
        let mut i = 0;
        while i < chars.len() {
            i = self.step(&chars, i, &mut out);
        }
        out.text
    }

    /// Converts the construct starting at `i`; returns the index after it.
    fn step(&mut self, chars: &[char], i: usize, out: &mut Out) -> usize {
        let c = chars[i];
        let prev = i.checked_sub(1).map(|p| chars[p]);
        let next = chars.get(i + 1).copied();
        match c {
            '\\' if next.is_some_and(|n| n.is_ascii_punctuation()) => {
                out.plain(chars[i + 1], prev, chars.get(i + 2).copied());
                i + 2
            }
            '`' => code_span(chars, i, out),
            '[' => self.link(chars, i, out),
            '!' if next == Some('[') => image(chars, i, out),
            '<' => autolink(chars, i, out),
            '*' | '_' => emphasis(chars, i, out),
            '~' if next == Some('~') => strikethrough(chars, i, out),
            _ => {
                out.plain(c, prev, next);
                i + 1
            }
        }
    }

    fn link(&mut self, chars: &[char], i: usize, out: &mut Out) -> usize {
        let Some(link) = self.parse_link(chars, i) else {
            out.plain('[', None, None);
            return i + 1;
        };
        let destination = link.destination.clone().unwrap_or_else(|| link.text.clone());
        let destination = destination.trim().trim_matches('`').to_string();
        if link.destination.is_some() && is_url(&destination) {
            write_hyperlink(&strip_markup(&link.text), &destination, out);
            return link.end;
        }
        let backquoted = link.text.trim().starts_with('`') && link.text.trim().ends_with('`');
        let explicit = link.explicit || backquoted;
        let resolved = if is_path_like(&destination) {
            self.links.resolve(&destination, explicit)
        } else {
            None
        };
        let shown = strip_markup(&link.text);
        match resolved {
            Some(Link::Found { role, path }) => write_role(role, &shown, &path, out),
            Some(Link::Unresolved { path }) if explicit => write_role("any", &shown, &path, out),
            Some(Link::Plain) => write_literal(&shown, out),
            _ => {
                // Not a link after all: brackets in prose, such as `[0, 1)`.
                out.plain('[', None, None);
                return i + 1;
            }
        }
        link.end
    }

    fn parse_link(&self, chars: &[char], i: usize) -> Option<LinkSyntax> {
        let close = matching(chars, i, '[', ']')?;
        let text: String = chars[i + 1..close].iter().collect();
        let after = close + 1;
        if chars.get(after) == Some(&'(') {
            let end = matching(chars, after, '(', ')')?;
            let inside: String = chars[after + 1..end].iter().collect();
            let destination = inside
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(['<', '>'])
                .to_string();
            return Some(LinkSyntax {
                text,
                destination: Some(destination),
                explicit: true,
                end: end + 1,
            });
        }
        if chars.get(after) == Some(&'[')
            && let Some(end) = matching(chars, after, '[', ']')
        {
            let label: String = chars[after + 1..end].iter().collect();
            let label = if label.trim().is_empty() { text.clone() } else { label };
            let destination = self.refs.get(&label.trim().to_lowercase()).cloned();
            let destination = destination.or(Some(label));
            return Some(LinkSyntax { text, destination, explicit: true, end: end + 1 });
        }
        let destination = self.refs.get(&text.trim().to_lowercase()).cloned();
        let explicit = destination.is_some();
        Some(LinkSyntax { text, destination, explicit, end: after })
    }
}

/// Writes `` `text <url>`__ `` (an anonymous hyperlink, so equal texts never clash).
fn write_hyperlink(text: &str, url: &str, out: &mut Out) {
    let shown = if text.trim().is_empty() { url } else { text.trim() };
    out.begin_markup();
    let shown = shown.replace('`', "").replace('<', "\\<");
    out.raw(&format!("`{shown} <{url}>`__"));
    out.end_markup();
}

/// Writes `:rust:role:`text <path>`` (or `:rust:role:`path`` when they agree).
fn write_role(role: &str, shown: &str, path: &str, out: &mut Out) {
    let shown = shown.trim().replace('`', "");
    out.begin_markup();
    if shown.is_empty() || shown == path {
        out.raw(&format!(":rust:{role}:`{path}`"));
    } else {
        let shown = shown.replace('<', "\\<");
        out.raw(&format!(":rust:{role}:`{shown} <{path}>`"));
    }
    out.end_markup();
}

/// Writes text as an inline literal.
fn write_literal(text: &str, out: &mut Out) {
    let text = text.trim().replace('`', "");
    if text.is_empty() {
        return;
    }
    out.begin_markup();
    out.raw(&format!("``{text}``"));
    out.end_markup();
}

fn code_span(chars: &[char], i: usize, out: &mut Out) -> usize {
    let n = run_length(chars, i, '`');
    let Some(end) = code_span_end(chars, i) else {
        for _ in 0..n {
            out.plain('`', None, None);
        }
        return i + n;
    };
    let content: String = chars[i + n..end - n].iter().collect();
    let content = content.replace('\n', " ");
    write_literal(&content, out);
    end
}

/// `![alt](src)` shows its alternative text; images are not carried into the API page.
fn image(chars: &[char], i: usize, out: &mut Out) -> usize {
    let Some(close) = matching(chars, i + 1, '[', ']') else {
        out.plain('!', None, None);
        return i + 1;
    };
    let alt: String = chars[i + 2..close].iter().collect();
    out.raw(&escape_plain(&strip_markup(&alt)));
    if chars.get(close + 1) == Some(&'(')
        && let Some(end) = matching(chars, close + 1, '(', ')')
    {
        return end + 1;
    }
    close + 1
}

/// `<https://example.com>` and `<someone@example.com>`.
fn autolink(chars: &[char], i: usize, out: &mut Out) -> usize {
    let end = chars[i + 1..].iter().position(|&c| c == '>' || c.is_whitespace()).map(|e| e + 1);
    let Some(end) = end.map(|e| i + e).filter(|&e| chars[e] == '>') else {
        out.plain('<', None, None);
        return i + 1;
    };
    let inside: String = chars[i + 1..end].iter().collect();
    if inside.contains("://") {
        write_hyperlink(&inside, &inside, out);
        return end + 1;
    }
    if inside.contains('@') && !inside.contains(':') {
        write_hyperlink(&inside, &format!("mailto:{inside}"), out);
        return end + 1;
    }
    out.plain('<', None, None);
    i + 1
}

/// `*em*`, `_em_`, `**strong**`, `__strong__`. reST markup cannot nest, so the inside is
/// written as plain text.
fn emphasis(chars: &[char], i: usize, out: &mut Out) -> usize {
    let c = chars[i];
    let n = run_length(chars, i, c);
    let prev = i.checked_sub(1).map(|p| chars[p]);
    let opens = chars.get(i + n).is_some_and(|x| !x.is_whitespace())
        && !(c == '_' && prev.is_some_and(char::is_alphanumeric));
    let close = if opens { find_closing(chars, i + n, c, n) } else { None };
    let Some(close) = close else {
        for k in 0..n {
            let at = i + k;
            out.plain(c, at.checked_sub(1).map(|p| chars[p]), chars.get(at + 1).copied());
        }
        return i + n;
    };
    let inner: String = chars[i + n..close].iter().collect();
    let inner = escape_plain(&strip_markup(&inner));
    let marker = if n >= 2 { "**" } else { "*" };
    out.begin_markup();
    out.raw(&format!("{marker}{inner}{marker}"));
    out.end_markup();
    close + n
}

/// The start of a closing run of `n` `c`s after `from`: preceded by a non-space and,
/// for `_`, not followed by a letter.
fn find_closing(chars: &[char], from: usize, c: char, n: usize) -> Option<usize> {
    let mut j = from;
    while j < chars.len() {
        if chars[j] == '`'
            && let Some(end) = code_span_end(chars, j)
        {
            j = end;
            continue;
        }
        if chars[j] == c {
            let m = run_length(chars, j, c);
            let before_ok = j > from && !chars[j - 1].is_whitespace();
            let after_ok = c != '_' || !chars.get(j + m).is_some_and(|x| x.is_alphanumeric());
            if m == n && before_ok && after_ok {
                return Some(j);
            }
            j += m;
            continue;
        }
        j += 1;
    }
    None
}

/// `~~struck~~` has no reST form; its text is kept.
fn strikethrough(chars: &[char], i: usize, out: &mut Out) -> usize {
    let close =
        (i + 2..chars.len().saturating_sub(1)).find(|&j| chars[j] == '~' && chars[j + 1] == '~');
    let Some(close) = close else {
        out.plain('~', None, None);
        return i + 1;
    };
    let inner: String = chars[i + 2..close].iter().collect();
    out.raw(&escape_plain(&strip_markup(&inner)));
    close + 2
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;

    impl LinkResolver for Fixed {
        fn resolve(&mut self, destination: &str, _explicit: bool) -> Option<Link> {
            match destination {
                "Foo" => Some(Link::Found { role: "struct", path: "k::Foo".into() }),
                "Foo::new" => Some(Link::Found { role: "fn", path: "k::Foo::new".into() }),
                other => Some(Link::Unresolved { path: other.to_string() }),
            }
        }
    }

    fn convert(text: &str) -> String {
        let refs = HashMap::from([("bar".to_string(), "Foo::new".to_string())]);
        Inline { links: &mut Fixed, refs: &refs }.convert(text)
    }

    #[test]
    fn code_emphasis_and_escapes() {
        assert_eq!(convert("Use `x`s *now* and __here__."), "Use ``x``\\ s *now* and **here**.");
        assert_eq!(convert("a_b _c_ 2*3 |x|"), "a_b *c* 2\\*3 \\|x\\|");
    }

    #[test]
    fn intra_doc_links_become_roles() {
        assert_eq!(convert("See [`Foo`]."), "See :rust:struct:`Foo <k::Foo>`.");
        assert_eq!(convert("[make one](Foo::new)"), ":rust:fn:`make one <k::Foo::new>`");
        assert_eq!(convert("[the ctor][bar]"), ":rust:fn:`the ctor <k::Foo::new>`");
        assert_eq!(convert("[`std::vec::Vec`]"), ":rust:any:`std::vec::Vec`");
        assert_eq!(convert("[0, 1)"), "[0, 1)");
    }

    #[test]
    fn urls_become_anonymous_links() {
        assert_eq!(convert("[site](https://x.org)"), "`site <https://x.org>`__");
        assert_eq!(convert("<https://x.org>"), "`https://x.org <https://x.org>`__");
    }
}
