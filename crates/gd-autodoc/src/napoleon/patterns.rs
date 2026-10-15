//! The regular expressions of Sphinx's napoleon, written as small matchers. Each one
//! matches exactly what its Python pattern matches (named in its comment), so the
//! converter needs no regular-expression engine. Lines never hold a line break.

/// Python's `\w` for one character.
pub(crate) fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `_directive_regex.match`: `.. \S+::` at the start of the line.
pub(crate) fn is_directive(line: &str) -> bool {
    let Some(rest) = line.strip_prefix(".. ") else { return false };
    let run = &rest[..rest.find(char::is_whitespace).unwrap_or(rest.len())];
    run.char_indices().skip(1).any(|(i, _)| run[i..].starts_with("::"))
}

/// `_google_section_regex.match`: `^(\s|\w)+:\s*$`.
pub(crate) fn is_google_section(line: &str) -> bool {
    let Some(body) = line.trim_end().strip_suffix(':') else { return false };
    !body.is_empty() && body.chars().all(|c| c.is_whitespace() || is_word(c))
}

/// `_google_typed_arg_regex.match`: `(.+?)\(\s*(.*[^\s]+)\s*\)` at the start; the
/// name (unstripped), the type, and the end of the match.
pub(crate) fn google_typed_arg(s: &str) -> Option<(&str, &str, usize)> {
    let opens = s.char_indices().filter(|&(i, c)| c == '(' && i > 0);
    for (p, _) in opens {
        let after = &s[p + 1..];
        let rest = after.trim_start();
        let base = p + 1 + (after.len() - rest.len());
        let closes = rest.char_indices().rev().filter(|&(_, c)| c == ')');
        for (q, _) in closes {
            let ty = rest[..q].trim_end();
            if !ty.is_empty() {
                return Some((&s[..p], ty, base + q + 1));
            }
        }
    }
    None
}

/// `_numpy_section_regex.match`: `^[=\-`:'"~^_*+#<>]{2,}\s*$`.
pub(crate) fn is_numpy_underline(line: &str) -> bool {
    let t = line.trim_end();
    t.chars().count() >= 2 && t.chars().all(|c| "=-`:'\"~^_*+#<>".contains(c))
}

/// `_single_colon_regex.search`: the first `:` that has no `:` beside it.
pub(crate) fn single_colon(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    (0..b.len())
        .find(|&i| b[i] == b':' && (i == 0 || b[i - 1] != b':') && b.get(i + 1) != Some(&b':'))
}

/// `_xref_or_code_regex.split`: the text between the cross-references and code
/// spans, and the spans themselves, alternately (text first and last).
pub(crate) fn split_xref_or_code(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut last, mut i) = (0, 0);
    while i < s.len() {
        if let Some(end) = xref_or_code_at(s, i) {
            out.push(&s[last..i]);
            out.push(&s[i..end]);
            (last, i) = (end, end);
            continue;
        }
        i += s[i..].chars().next().map_or(1, char::len_utf8);
    }
    out.push(&s[last..]);
    out
}

/// The end of the `_xref_or_code_regex` match that starts at `i`: a role with its
/// target, a ``` ``literal`` ```, a `:meta` field, or a `` `text <target>` `` link.
fn xref_or_code_at(s: &str, i: usize) -> Option<usize> {
    let at = &s[i..];
    if at.starts_with(':')
        && let Some(end) = role_and_target(at)
    {
        return Some(i + end);
    }
    if let Some(body) = at.strip_prefix("``") {
        let close = body.char_indices().skip(1).find(|&(k, _)| body[k..].starts_with("``"));
        if let Some((k, _)) = close {
            return Some(i + 2 + k + 2);
        }
    }
    if let Some(rest) = at.strip_prefix(":meta ")
        && rest.char_indices().skip(1).any(|(_, c)| c == ':')
    {
        return Some(s.len());
    }
    if let Some(body) = at.strip_prefix('`') {
        let open = body.char_indices().skip(1).find(|&(_, c)| c == '<')?;
        let close = body[open.0 + 1..].find(">`")?;
        return Some(i + 1 + open.0 + 1 + close + 2);
    }
    None
}

/// A role name, `(?:[a-zA-Z0-9]+[\-_+:.])*[a-zA-Z0-9]+`, after the `:` that starts
/// `s`, ended by ``:` ``: the offset of that backtick.
fn role_name_end(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut j = 1;
    loop {
        let start = j;
        while j < b.len() && b[j].is_ascii_alphanumeric() {
            j += 1;
        }
        if j == start {
            return None;
        }
        match (b.get(j), b.get(j + 1)) {
            (Some(b':'), Some(b'`')) => return Some(j + 1),
            (Some(b'-' | b'_' | b'+' | b':' | b'.'), _) => j += 1,
            _ => return None,
        }
    }
}

/// `:role:` then `` `.+?` ``: the end of the match at the start of `s`.
fn role_and_target(s: &str) -> Option<usize> {
    let tick = role_name_end(s)?;
    backtick_span(&s[tick..]).map(|end| tick + end)
}

/// `` `.+?` `` at the start of `s`: the end of the span.
fn backtick_span(s: &str) -> Option<usize> {
    let body = s.strip_prefix('`')?;
    let (k, _) = body.char_indices().skip(1).find(|&(_, c)| c == '`')?;
    Some(1 + k + 1)
}

/// `_xref_regex.match`: an optional role, then `` `.+?` ``, at the start.
pub(crate) fn is_xref(s: &str) -> bool {
    (s.starts_with(':') && role_and_target(s).is_some()) || backtick_span(s).is_some()
}

/// `(\s+\S|\s*$)`, the end of a list marker.
fn list_tail(rest: &str) -> bool {
    let t = rest.trim_start();
    t.is_empty() || t.len() < rest.len()
}

/// `_bullet_list_regex.match`.
pub(crate) fn is_bullet(line: &str) -> bool {
    line.strip_prefix(['*', '+', '-']).is_some_and(list_tail)
}

/// `_enumerated_list_regex.match`: `1.`, `#.`, `iv.`, `a.`, `(1)`, ...
pub(crate) fn is_enumerated(line: &str) -> bool {
    let (rest, close) = match line.strip_prefix('(') {
        Some(rest) => (rest, ')'),
        None => (line, '.'),
    };
    let run = |pred: fn(char) -> bool| rest.find(|c: char| !pred(c)).unwrap_or(rest.len());
    let single = rest.chars().next().filter(char::is_ascii_alphabetic).map(char::len_utf8);
    let lengths = [
        Some(run(|c| c.is_ascii_digit())),
        rest.starts_with('#').then_some(1),
        Some(run(|c| "ivxlcdm".contains(c))),
        Some(run(|c| "IVXLCDM".contains(c))),
        single,
    ];
    lengths
        .into_iter()
        .flatten()
        .filter(|&n| n > 0)
        .any(|n| rest[n..].strip_prefix(close).is_some_and(list_tail))
}

/// What `GoogleDocstring._name_rgx` matched: `` :role:`name` ``, `` `name` `` or a
/// bare name, with the whitespace around it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NameMatch<'a> {
    /// The role, when written.
    pub role: Option<&'a str>,
    /// The name.
    pub name: &'a str,
    /// The name was between backticks (`_name_rgx`'s `name` group, not `name2`).
    pub ticked: bool,
    /// The end of the match, trailing whitespace included.
    pub end: usize,
}

/// `~?[a-zA-Z0-9_.-]+` at the start of `s`: its length.
fn name_chars(s: &str) -> Option<usize> {
    let tilde = usize::from(s.starts_with('~'));
    let n = s[tilde..]
        .find(|c: char| !(c.is_ascii_alphanumeric() || "_.-".contains(c)))
        .unwrap_or(s.len() - tilde);
    (n > 0).then_some(tilde + n)
}

/// `` `name` `` at the start of `s`: the name and the end.
fn ticked_name(s: &str) -> Option<(&str, usize)> {
    let body = s.strip_prefix('`')?;
    let n = name_chars(body)?;
    body[n..].starts_with('`').then(|| (&body[..n], n + 2))
}

/// `_name_rgx.match`.
pub(crate) fn match_name(s: &str) -> Option<NameMatch<'_>> {
    let p = s.len() - s.trim_start().len();
    let at = &s[p..];
    let ticked = role_name(at).or_else(|| ticked_name(at).map(|(name, len)| (None, name, len)));
    let (role, name, len) = match ticked {
        Some(found) => found,
        None => name_chars(at).map(|n| (None, &at[..n], n))?,
    };
    let after = &s[p + len..];
    let end = p + len + (after.len() - after.trim_start().len());
    Some(NameMatch { role, name, ticked: ticked.is_some(), end })
}

/// `` :role:`name` `` at the start of `s`, the role being as long as it can be.
fn role_name(s: &str) -> Option<(Option<&str>, &str, usize)> {
    let body = s.strip_prefix(':')?;
    let run = &body[..body.find(char::is_whitespace).unwrap_or(body.len())];
    let ends = run.char_indices().rev().filter(|&(k, _)| k > 0 && run[k..].starts_with(":`"));
    for (k, _) in ends {
        if let Some((name, len)) = ticked_name(&body[k + 1..]) {
            return Some((Some(&body[..k]), name, 1 + k + 1 + len));
        }
    }
    None
}

/// `_default_regex.match`: `default` and one more character, not a word character.
pub(crate) fn starts_default(s: &str) -> bool {
    s.strip_prefix("default")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| !(c.is_ascii_alphanumeric() || c == '_'))
}

/// `_token_regex.split`: the parts of a type specification and the delimiters between
/// them, alternately (text first and last).
pub(crate) fn split_type_tokens(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut last, mut i) = (0, 0);
    while i < s.len() {
        if let Some(len) = token_delimiter(&s[i..]) {
            out.push(&s[last..i]);
            out.push(&s[i..i + len]);
            (last, i) = (i + len, i + len);
            continue;
        }
        i += s[i..].chars().next().map_or(1, char::len_utf8);
    }
    out.push(&s[last..]);
    out
}

/// One whitespace character at the start of `s`: its length.
fn space(s: &str) -> Option<usize> {
    s.chars().next().filter(|c| c.is_whitespace()).map(char::len_utf8)
}

/// `word` between single whitespace characters at the start of `s`.
fn spaced_word(s: &str, word: &str) -> Option<usize> {
    let a = space(s)?;
    let rest = s[a..].strip_prefix(word)?;
    Some(a + word.len() + space(rest)?)
}

/// The length of the `_token_regex` delimiter at the start of `s`.
fn token_delimiter(s: &str) -> Option<usize> {
    let comma = |word: &str| s.strip_prefix(',').and_then(|r| spaced_word(r, word)).map(|n| n + 1);
    let colon = || s.strip_prefix(':').and_then(space).map(|n| n + 1);
    let comma_space = || s.strip_prefix(',').and_then(space).map(|n| n + 1);
    comma("or")
        .or_else(|| spaced_word(s, "or"))
        .or_else(|| spaced_word(s, "of"))
        .or_else(colon)
        .or_else(|| spaced_word(s, "to"))
        .or_else(|| comma("and"))
        .or_else(|| spaced_word(s, "and"))
        .or_else(comma_space)
        .or_else(|| (s.starts_with('{') || s.starts_with('}')).then_some(1))
        .or_else(|| quoted(s, '"'))
        .or_else(|| quoted(s, '\''))
}

/// `"(?:\\"|[^"])*"` (or with `'`) at the start of `s`: its length. Python's
/// backtracking ends the string at the first unescaped quote, or, when there is none,
/// at the last escaped one.
fn quoted(s: &str, q: char) -> Option<usize> {
    let body = s.strip_prefix(q)?;
    let mut last_escaped = None;
    let mut chars = body.char_indices();
    while let Some((k, c)) = chars.next() {
        if c == '\\' && body[k + 1..].starts_with(q) {
            chars.next();
            last_escaped = Some(k + 1);
        } else if c == q {
            return Some(1 + k + 1);
        }
    }
    last_escaped.map(|k| 1 + k + 1)
}

/// Whether Python's `complex(token)` reads `token` as a number: `1`, `-4.6`, `2j`,
/// `1e3`, `1+2j`, `inf`, ...
pub(crate) fn is_numeric(token: &str) -> bool {
    let t = token.trim();
    let t = t.strip_prefix('(').and_then(|r| r.strip_suffix(')')).unwrap_or(t).trim();
    if let Some(imag) = t.strip_suffix(['j', 'J']) {
        let split = imag
            .char_indices()
            .rev()
            .find(|&(k, c)| k > 0 && (c == '+' || c == '-') && !imag[..k].ends_with(['e', 'E']));
        return match split {
            Some((k, _)) => is_real(&imag[..k]) && is_imag_part(&imag[k..]),
            None => is_imag_part(imag),
        };
    }
    is_real(t)
}

/// The coefficient of an imaginary part: a real number, or only a sign.
fn is_imag_part(s: &str) -> bool {
    matches!(s, "" | "+" | "-") || is_real(s)
}

/// A real number as Python's `float` reads it, underscores between digits allowed.
fn is_real(s: &str) -> bool {
    let b = s.as_bytes();
    let underscore_ok = b.iter().enumerate().all(|(i, &c)| {
        c != b'_'
            || (i > 0 && b[i - 1].is_ascii_digit() && b.get(i + 1).is_some_and(u8::is_ascii_digit))
    });
    let plain: String = s.chars().filter(|&c| c != '_').collect();
    underscore_ok && !plain.is_empty() && plain.parse::<f64>().is_ok()
}
