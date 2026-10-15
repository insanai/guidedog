//! literalinclude's `:pyobject:`: a class or function of Python source found by its
//! dotted path ("Class.method") from indentation, as Sphinx's source analyzer tags it.

use crate::text::is_identifier_byte;

/// Finds `path` in `lines`: the range starts at the first decorator and ends at the last
/// code line of the body; trailing blank and comment-only lines are left out. Returns
/// 0-based lines [a, b).
pub(crate) fn find_pyobject<S: AsRef<str>>(lines: &[S], path: &str) -> Option<(usize, usize)> {
    if path.is_empty() {
        return None;
    }
    let (mut from, mut to) = (0, lines.len());
    let mut found = (0, 0);
    for name in path.split('.') {
        let (start, header, end) = find_definition(lines, from, to, name)?;
        found = (start, end);
        (from, to) = (header + 1, end);
    }
    Some(found)
}

/// A line's indentation (tabs to multiples of 8) and whether it holds code rather than
/// white space or a comment.
fn py_line(l: &str) -> (usize, bool) {
    let mut indent = 0;
    for c in l.bytes() {
        match c {
            b' ' => indent += 1,
            b'\t' => indent = (indent / 8 + 1) * 8,
            b'\x0c' => indent = 0,
            _ => return (indent, c != b'#' && c != b'\n' && c != b'\r'),
        }
    }
    (indent, false)
}

/// The name a "class", "def", or "async def" line defines.
fn definition_name(l: &str) -> &str {
    fn keyword<'t>(t: &'t str, k: &str) -> Option<&'t str> {
        let rest = t.strip_prefix(k)?;
        if !rest.starts_with([' ', '\t']) {
            return None;
        }
        Some(rest.trim_start_matches([' ', '\t']))
    }
    let mut t = l.trim_start_matches([' ', '\t']);
    if let Some(r) = keyword(t, "async") {
        t = r;
    }
    let Some(r) = keyword(t, "def").or_else(|| keyword(t, "class")) else { return "" };
    let n = r.bytes().position(|c| !is_identifier_byte(c)).unwrap_or(r.len());
    &r[..n]
}

/// Searches lines [from, to) at the block's own indentation: (start, header end, end).
fn find_definition<S: AsRef<str>>(
    lines: &[S],
    from: usize,
    to: usize,
    name: &str,
) -> Option<(usize, usize, usize)> {
    let mut base = None;
    for i in from..to {
        let (indent, code) = py_line(lines[i].as_ref());
        if !code {
            continue;
        }
        let base = *base.get_or_insert(indent);
        if indent != base || definition_name(lines[i].as_ref()) != name {
            continue;
        }
        let mut start = i;
        for j in (from..i).rev() {
            let (ind, c) = py_line(lines[j].as_ref());
            if !c || ind != base || !is_decorator(lines[j].as_ref()) {
                break;
            }
            start = j;
        }
        let header = header_end(lines, i, to);
        let mut last = header;
        for (j, l) in lines.iter().enumerate().take(to).skip(header + 1) {
            let (ind, c) = py_line(l.as_ref());
            if !c {
                continue;
            }
            if ind <= base {
                break;
            }
            last = j;
        }
        return Some((start, header, last + 1));
    }
    None
}

fn is_decorator(l: &str) -> bool {
    l.trim_start_matches([' ', '\t']).starts_with('@')
}

/// Returns the last line of a definition's header: where its brackets close.
fn header_end<S: AsRef<str>>(lines: &[S], i: usize, to: usize) -> usize {
    let mut depth = 0i32;
    for (k, l) in lines.iter().enumerate().take(to).skip(i) {
        let l = l.as_ref().as_bytes();
        let mut quote = 0u8;
        let mut j = 0;
        while j < l.len() {
            let c = l[j];
            if quote != 0 {
                if c == b'\\' {
                    j += 1;
                } else if c == quote {
                    quote = 0;
                }
                j += 1;
                continue;
            }
            match c {
                b'#' => j = l.len(),
                b'\'' | b'"' => quote = c,
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                _ => {}
            }
            j += 1;
        }
        if depth <= 0 {
            return k;
        }
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &[&str] = &[
        "import os\n",
        "\n",
        "@decorator\n",
        "def hello(name):\n",
        "    print(name)\n",
        "\n",
        "    # done\n",
        "\n",
        "class Greeter:\n",
        "    def greet(self):\n",
        "        return 1\n",
        "\n",
        "    def other(self):\n",
        "        pass\n",
    ];

    #[test]
    fn objects_are_found_with_decorators_and_without_trailing_comments() {
        assert_eq!(find_pyobject(SOURCE, "hello"), Some((2, 5)));
        assert_eq!(find_pyobject(SOURCE, "Greeter.greet"), Some((9, 11)));
        assert_eq!(find_pyobject(SOURCE, "Greeter.missing"), None);
    }
}
