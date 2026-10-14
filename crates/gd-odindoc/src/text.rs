//! Source text made into what a signature or doc comment shows.

/// Writes `src` as one line, as a signature shows it: comments are removed, every run
/// of white space becomes one space, none follows an opening bracket or precedes a
/// closing one or a comma, and a comma before a closing bracket goes. Strings, runes,
/// and raw strings are kept as written.
pub(crate) fn normalize(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    normalize_into(src, &mut out);
    out
}

/// `normalize`, appending to `out`.
pub(crate) fn normalize_into(src: &str, out: &mut String) {
    let base = out.len();
    let bytes = src.as_bytes();
    let mut space = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'/' && bytes.get(i + 1) == Some(&b'/') {
            i = src[i..].find('\n').map_or(bytes.len(), |k| i + k);
            space = true;
            continue;
        }
        if c == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i = skip_block_comment(bytes, i);
            space = true;
            continue;
        }
        if matches!(c, b' ' | b'\t' | b'\n' | b'\r') {
            space = true;
            i += 1;
            continue;
        }
        if space {
            write_space(out, base, c);
        }
        space = false;
        if matches!(c, b'"' | b'\'' | b'`') {
            let end = string_end(bytes, i);
            out.push_str(&src[i..end]);
            i = end;
            continue;
        }
        if matches!(c, b')' | b']' | b'}') {
            drop_trailing_comma(out, base);
        }
        // Copy the whole character: `c` may start a multi-byte one.
        let width = src[i..].chars().next().map_or(1, char::len_utf8);
        out.push_str(&src[i..i + width]);
        i += width;
    }
}

/// The space a run of white space stands for, unless it follows an opening bracket,
/// precedes a closing bracket or comma, or begins the text.
fn write_space(out: &mut String, base: usize, next: u8) {
    if out.len() == base || matches!(next, b')' | b']' | b'}' | b',') {
        return;
    }
    if out.ends_with(['(', '[', '{']) {
        return;
    }
    out.push(' ');
}

fn drop_trailing_comma(out: &mut String, base: usize) {
    while out.len() > base && out.ends_with([',', ' ']) {
        out.pop();
    }
}

/// The index after the `/* */` comment at `i`, which may nest.
pub(crate) fn skip_block_comment(src: &[u8], i: usize) -> usize {
    let mut depth = 0;
    let mut k = i;
    while k + 1 < src.len() {
        if src[k] == b'/' && src[k + 1] == b'*' {
            depth += 1;
            k += 2;
            continue;
        }
        if src[k] == b'*' && src[k + 1] == b'/' {
            depth -= 1;
            k += 2;
            if depth == 0 {
                return k;
            }
            continue;
        }
        k += 1;
    }
    src.len()
}

/// The index after the string, rune, or raw string starting at `i`.
pub(crate) fn string_end(src: &[u8], i: usize) -> usize {
    let quote = src[i];
    if src[i..].starts_with(b"\"\"\"") {
        return triple_quoted_end(src, i);
    }
    let mut k = i + 1;
    while k < src.len() {
        let c = src[k];
        if c == b'\\' && quote != b'`' {
            k += 2;
            continue;
        }
        if c == quote {
            return k + 1;
        }
        if c == b'\n' && quote != b'`' {
            return k;
        }
        k += 1;
    }
    src.len()
}

/// The index after the `"""` string starting at `i`.
fn triple_quoted_end(src: &[u8], i: usize) -> usize {
    let mut k = i + 3;
    while k < src.len() {
        if src[k] == b'\\' {
            k += 2;
            continue;
        }
        if src[k..].starts_with(b"\"\"\"") {
            return k + 3;
        }
        k += 1;
    }
    src.len()
}

/// A comment group's text without its markers: `//` lines lose `//`, a `/* */` block
/// its delimiters (and the leading ` * ` of each line, when every line has one), and the
/// lines their common indentation. Leading and trailing blank lines go.
pub(crate) fn comment_text<'s>(comments: impl Iterator<Item = &'s str>) -> String {
    let mut lines: Vec<&str> = Vec::new();
    for text in comments {
        if let Some(line) = text.strip_prefix("//") {
            lines.push(line.trim_end());
            continue;
        }
        let body = text.strip_prefix("/*").unwrap_or(text);
        let body = body.strip_suffix("*/").unwrap_or(body);
        lines.extend(split_lines(body).map(str::trim_end));
    }
    strip_stars(&mut lines);
    dedent(&lines).join("\n")
}

/// Lines as Odin's `split_lines` makes them: split at "\n", without a "\r" before it.
pub(crate) fn split_lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line))
}

/// Removes a ` * ` margin that every non-blank line of a block has.
fn strip_stars(lines: &mut [&str]) {
    let mut count = 0;
    for line in lines.iter() {
        let t = line.trim_start_matches([' ', '\t']);
        if t.is_empty() {
            continue;
        }
        if t != "*" && !t.starts_with("* ") {
            return;
        }
        count += 1;
    }
    if count < 2 {
        return;
    }
    for line in lines.iter_mut() {
        let t = line.trim_start_matches([' ', '\t']);
        if t.is_empty() {
            continue;
        }
        let t = t.strip_prefix('*').unwrap_or(t);
        *line = t.strip_prefix(' ').unwrap_or(t);
    }
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// The lines without the indentation every non-blank line shares, and without blank
/// lines at either end; blank lines inside become empty.
pub(crate) fn dedent<'s>(lines: &[&'s str]) -> Vec<&'s str> {
    let first = lines.iter().position(|l| !is_blank(l)).unwrap_or(lines.len());
    let last = lines.iter().rposition(|l| !is_blank(l)).map_or(first, |k| k + 1);
    let body = &lines[first..last];
    let prefix = body.iter().find(|l| !is_blank(l)).copied().unwrap_or("").as_bytes();
    let mut common: Option<usize> = None;
    for line in body.iter().filter(|l| !is_blank(l)) {
        let b = line.as_bytes();
        let n = b.iter().take_while(|&&c| c == b' ' || c == b'\t').count();
        common = Some(match common {
            None => n,
            Some(c) => {
                let shared = (0..c.min(n)).take_while(|&k| b[k] == prefix[k]).count();
                c.min(shared)
            }
        });
    }
    let cut = common.unwrap_or(0);
    body.iter().map(|l| if is_blank(l) { "" } else { &l[cut..] }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_makes_one_line() {
        assert_eq!(
            normalize("proc(\n\ta: int, // x\n\tb: f32,\n) -> int"),
            "proc(a: int, b: f32) -> int"
        );
        assert_eq!(normalize("{ a,  b }"), "{a, b}");
        assert_eq!(normalize("\"a  b\"  /* c */ x"), "\"a  b\" x");
    }

    #[test]
    fn comments_lose_markers_and_margins() {
        let text = comment_text(["/*\n * One.\n * Two.\n */"].into_iter());
        assert_eq!(text, "One.\nTwo.");
        assert_eq!(comment_text(["// a", "//   b"].into_iter()), "a\n  b");
    }
}
