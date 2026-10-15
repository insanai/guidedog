//! Python's string algorithms: splitting, case mapping, stripping and wrapping.

/// Whether c is whitespace as Python's str.isspace() sees it.
pub(crate) fn is_space(c: char) -> bool {
    matches!(c,
        ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c' | '\x1c'..='\x1f' | '\u{85}' | '\u{a0}'
        | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}'
        | '\u{205f}' | '\u{3000}')
}

/// Whether c belongs to a word (`\w`).
pub(crate) fn is_word_char(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

/// Splits on runs of whitespace like str.split() without arguments.
pub(crate) fn split_whitespace(s: &str) -> impl Iterator<Item = &str> {
    s.split(is_space).filter(|p| !p.is_empty())
}

/// str.splitlines(keepends).
pub(crate) fn splitlines(s: &str, keepends: bool) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let is_break = matches!(
            c,
            '\n' | '\r'
                | '\x0b'
                | '\x0c'
                | '\x1c'
                | '\x1d'
                | '\x1e'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        );
        if !is_break {
            continue;
        }
        let mut end = i + c.len_utf8();
        if c == '\r' && chars.peek().is_some_and(|&(_, n)| n == '\n') {
            chars.next();
            end += 1;
        }
        out.push(if keepends { &s[start..end] } else { &s[start..i] });
        start = end;
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// str.split(sep, maxsplit), or rsplit when from_right; sep is not empty.
pub(crate) fn split_sep<'a>(
    s: &'a str,
    sep: &str,
    maxsplit: i64,
    from_right: bool,
) -> Vec<&'a str> {
    let n = if maxsplit < 0 { usize::MAX } else { maxsplit as usize + 1 };
    if from_right {
        let mut out: Vec<&str> = s.rsplitn(n, sep).collect();
        out.reverse();
        out
    } else {
        s.splitn(n, sep).collect()
    }
}

/// str.split() / rsplit() on whitespace with maxsplit.
pub(crate) fn split_ws(s: &str, maxsplit: i64, from_right: bool) -> Vec<&str> {
    if maxsplit < 0 {
        return split_whitespace(s).collect();
    }
    let mut out = Vec::new();
    if !from_right {
        let mut rest = s.trim_start_matches(is_space);
        while !rest.is_empty() && (out.len() as i64) < maxsplit {
            let end = rest.find(is_space).unwrap_or(rest.len());
            out.push(&rest[..end]);
            rest = rest[end..].trim_start_matches(is_space);
        }
        if !rest.is_empty() {
            out.push(rest);
        }
        return out;
    }
    let mut rest = s.trim_end_matches(is_space);
    while !rest.is_empty() && (out.len() as i64) < maxsplit {
        let start = rest
            .rfind(is_space)
            .map_or(0, |i| i + rest[i..].chars().next().map_or(1, char::len_utf8));
        out.push(&rest[start..]);
        rest = rest[..start].trim_end_matches(is_space);
    }
    if !rest.is_empty() {
        out.push(rest);
    }
    out.reverse();
    out
}

/// Unicode's simple upper-case mapping: a character that maps to several stays.
pub(crate) fn upper_char(c: char) -> char {
    let mut u = c.to_uppercase();
    match (u.next(), u.next()) {
        (Some(x), None) => x,
        _ => c,
    }
}

/// Unicode's simple lower-case mapping.
pub(crate) fn lower_char(c: char) -> char {
    let mut l = c.to_lowercase();
    match (l.next(), l.next()) {
        (Some(x), None) => x,
        _ => c,
    }
}

/// s in upper case (simple mapping).
pub(crate) fn to_upper(s: &str) -> String {
    s.chars().map(upper_char).collect()
}

/// s in lower case (simple mapping).
pub(crate) fn to_lower(s: &str) -> String {
    s.chars().map(lower_char).collect()
}

/// str.capitalize(): the first character upper case, the rest lower case.
pub(crate) fn capitalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, c) in s.chars().enumerate() {
        out.push(if i == 0 { upper_char(c) } else { lower_char(c) });
    }
    out
}

/// str.title(): a cased letter after a cased letter is lowercased.
pub(crate) fn python_title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_cased = false;
    for c in s.chars() {
        let cased = c.is_alphabetic();
        if cased {
            out.push(if prev_cased { lower_char(c) } else { upper_char(c) });
        } else {
            out.push(c);
        }
        prev_cased = cased;
    }
    out
}

/// Jinja's title filter: words start after spaces, "-", "(", "{", "[", "<".
pub(crate) fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut at_start = true;
    for c in s.chars() {
        if is_space(c) || matches!(c, '-' | '(' | '{' | '[' | '<') {
            out.push(c);
            at_start = true;
            continue;
        }
        out.push(if at_start { upper_char(c) } else { lower_char(c) });
        at_start = false;
    }
    out
}

/// Strips the characters of chars (whitespace when None) from the chosen sides.
pub(crate) fn strip<'a>(s: &'a str, chars: Option<&str>, left: bool, right: bool) -> &'a str {
    let test = |c: char| chars.map_or_else(|| is_space(c), |set| set.contains(c));
    let mut out = s;
    if left {
        out = out.trim_start_matches(test);
    }
    if right {
        out = out.trim_end_matches(test);
    }
    out
}

/// str.replace(old, new, count); count < 0 replaces all.
pub(crate) fn replace(s: &str, old: &str, new: &str, count: i64) -> String {
    let limit = if count < 0 { usize::MAX } else { count as usize };
    if old.is_empty() {
        let mut out = String::with_capacity(s.len());
        let mut n = 0;
        for c in s.chars() {
            if n < limit {
                out.push_str(new);
                n += 1;
            }
            out.push(c);
        }
        if n < limit {
            out.push_str(new);
        }
        return out;
    }
    s.replacen(old, new, limit)
}

/// str.expandtabs(size).
pub(crate) fn expand_tabs(s: &str, size: i64) -> String {
    let mut out = String::with_capacity(s.len());
    let mut col: i64 = 0;
    for c in s.chars() {
        match c {
            '\t' => {
                if size > 0 {
                    let spaces = size - col % size;
                    out.extend(std::iter::repeat_n(' ', spaces as usize));
                    col += spaces;
                }
            }
            '\n' | '\r' => {
                out.push(c);
                col = 0;
            }
            _ => {
                out.push(c);
                col += 1;
            }
        }
    }
    out
}

/// How [`wrap`] breaks lines (textwrap.wrap with expand_tabs and replace_whitespace off).
#[derive(Debug, Clone, Copy)]
pub(crate) struct WrapOptions {
    pub width: usize,
    pub break_long_words: bool,
    pub break_on_hyphens: bool,
}

/// Splits text like textwrap: runs of whitespace, and words (split after hyphens between
/// letters when break_on_hyphens is set).
fn wrap_chunks(text: &str, break_on_hyphens: bool) -> Vec<&str> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let at = |i: usize| chars.get(i).map_or(text.len(), |&(b, _)| b);
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let mut j = i;
        if is_space(chars[i].1) {
            while j < chars.len() && is_space(chars[j].1) {
                j += 1;
            }
        } else {
            while j < chars.len() && !is_space(chars[j].1) {
                j += 1;
                if break_on_hyphens
                    && chars[j - 1].1 == '-'
                    && j < chars.len()
                    && j >= 2
                    && chars[j - 2].1.is_alphabetic()
                    && chars[j].1.is_alphabetic()
                {
                    break;
                }
            }
        }
        out.push(&text[at(i)..at(j)]);
        i = j;
    }
    out
}

/// textwrap.wrap: the lines of text, each at most width characters where it can be.
pub(crate) fn wrap(text: &str, opts: &WrapOptions) -> Vec<String> {
    let mut chunks = wrap_chunks(text, opts.break_on_hyphens);
    let mut lines = Vec::new();
    let mut line: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < chunks.len() {
        line.clear();
        let mut used = 0;
        if chunks[i].trim_matches(is_space).is_empty() && !lines.is_empty() {
            i += 1;
        }
        while i < chunks.len() {
            let n = chunks[i].chars().count();
            if used + n > opts.width {
                break;
            }
            line.push(chunks[i]);
            used += n;
            i += 1;
        }
        if i < chunks.len() && chunks[i].chars().count() > opts.width {
            if opts.break_long_words {
                let head = long_word_head(chunks[i], opts.width - used, opts.break_on_hyphens);
                line.push(head);
                chunks[i] = &chunks[i][head.len()..];
            } else if line.is_empty() {
                line.push(chunks[i]);
                i += 1;
            }
        }
        while line.last().is_some_and(|l| l.trim_matches(is_space).is_empty()) {
            line.pop();
        }
        if !line.is_empty() {
            lines.push(line.concat());
        }
    }
    lines
}

/// The part of a long word that fits in space characters, cut after a hyphen if it can.
fn long_word_head(word: &str, space: usize, break_on_hyphens: bool) -> &str {
    let end = word.char_indices().nth(space).map_or(word.len(), |(b, _)| b);
    let head = &word[..end];
    if break_on_hyphens
        && let Some(cut) = head.rfind('-')
        && cut > 0
        && !head[..cut].trim_start_matches('-').is_empty()
    {
        return &head[..=cut];
    }
    head
}
