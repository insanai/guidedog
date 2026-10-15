//! The unified diff literalinclude's `:diff:` shows: Python's difflib.unified_diff with
//! three lines of context. The matching blocks of SequenceMatcher (longest matches
//! first, popular lines of long files not starting a match), grouped hunks, and
//! "--- old" / "+++ new" headers.

use std::collections::HashMap;
use std::fmt::Write as _;

use guidedoc::{Builder, Fail};

#[derive(Debug, Clone, Copy)]
struct Match {
    i: usize,
    j: usize,
    k: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tag {
    Equal,
    Replace,
    Delete,
    Insert,
}

#[derive(Debug, Clone, Copy)]
struct Opcode {
    tag: Tag,
    i1: usize,
    i2: usize,
    j1: usize,
    j2: usize,
}

/// A range to search, or a match to emit.
#[derive(Debug, Clone, Copy)]
enum Task {
    Search { alo: usize, ahi: usize, blo: usize, bhi: usize },
    Emit(Match),
}

struct Differ<'d> {
    a: &'d [&'d str],
    b: &'d [&'d str],
    popular: Vec<bool>,
    prev: Vec<usize>,
    cur: Vec<usize>,
}

/// Returns the diff from `a` to `b` (lines with their line breaks) as text, charging
/// the work to `bld`.
pub(crate) fn unified_diff(
    bld: &mut Builder<'_>,
    a: &[&str],
    b: &[&str],
    from: &str,
    to: &str,
) -> Result<String, Fail> {
    let mut df = Differ {
        a,
        b,
        popular: popular_lines(b),
        prev: vec![0; b.len() + 1],
        cur: vec![0; b.len() + 1],
    };
    bld.spend(b.len() as u64)?;
    let blocks = matching_blocks(bld, &mut df)?;
    let codes = opcodes(&blocks);
    let mut out = String::new();
    write_diff(&mut out, a, b, &codes, from, to);
    Ok(out)
}

/// Marks lines of `b` that occur more than len(b)/100 + 1 times, for len(b) >= 200
/// (SequenceMatcher's autojunk heuristic).
fn popular_lines(b: &[&str]) -> Vec<bool> {
    let n = b.len();
    if n < 200 {
        return vec![false; n];
    }
    let mut counts: HashMap<&str, usize> = HashMap::with_capacity(n);
    for l in b {
        *counts.entry(l).or_default() += 1;
    }
    let limit = n / 100 + 1;
    b.iter().map(|l| counts[l] > limit).collect()
}

fn longest_match(
    bld: &mut Builder<'_>,
    df: &mut Differ<'_>,
    alo: usize,
    ahi: usize,
    blo: usize,
    bhi: usize,
) -> Result<Match, Fail> {
    let mut best = Match { i: alo, j: blo, k: 0 };
    df.prev[blo..=bhi].fill(0);
    for i in alo..ahi {
        bld.spend(((bhi - blo) / 16 + 1) as u64)?;
        df.cur[blo] = 0;
        for j in blo..bhi {
            let k = if !df.popular[j] && df.a[i] == df.b[j] { df.prev[j] + 1 } else { 0 };
            df.cur[j + 1] = k;
            if k > best.k {
                best = Match { i: i + 1 - k, j: j + 1 - k, k };
            }
        }
        std::mem::swap(&mut df.prev, &mut df.cur);
    }
    while best.i > alo && best.j > blo && df.a[best.i - 1] == df.b[best.j - 1] {
        best = Match { i: best.i - 1, j: best.j - 1, k: best.k + 1 };
    }
    while best.i + best.k < ahi
        && best.j + best.k < bhi
        && df.a[best.i + best.k] == df.b[best.j + best.k]
    {
        best.k += 1;
    }
    Ok(best)
}

/// Searches ranges depth first, left before right, so the blocks come out sorted;
/// adjacent blocks merge and a (len a, len b, 0) sentinel ends the list.
fn matching_blocks(bld: &mut Builder<'_>, df: &mut Differ<'_>) -> Result<Vec<Match>, Fail> {
    let (la, lb) = (df.a.len(), df.b.len());
    let mut blocks: Vec<Match> = Vec::new();
    let mut stack = vec![Task::Search { alo: 0, ahi: la, blo: 0, bhi: lb }];
    while let Some(t) = stack.pop() {
        let (alo, ahi, blo, bhi) = match t {
            Task::Emit(m) => {
                match blocks.last_mut() {
                    Some(last) if last.i + last.k == m.i && last.j + last.k == m.j => {
                        last.k += m.k
                    }
                    _ => blocks.push(m),
                }
                continue;
            }
            Task::Search { alo, ahi, blo, bhi } => (alo, ahi, blo, bhi),
        };
        let m = longest_match(bld, df, alo, ahi, blo, bhi)?;
        if m.k == 0 {
            continue;
        }
        if m.i + m.k < ahi && m.j + m.k < bhi {
            stack.push(Task::Search { alo: m.i + m.k, ahi, blo: m.j + m.k, bhi });
        }
        stack.push(Task::Emit(m));
        if alo < m.i && blo < m.j {
            stack.push(Task::Search { alo, ahi: m.i, blo, bhi: m.j });
        }
    }
    blocks.push(Match { i: la, j: lb, k: 0 });
    Ok(blocks)
}

fn opcodes(blocks: &[Match]) -> Vec<Opcode> {
    let mut codes = Vec::with_capacity(2 * blocks.len() + 1);
    let (mut i, mut j) = (0, 0);
    for m in blocks {
        let tag = match (i < m.i, j < m.j) {
            (true, true) => Some(Tag::Replace),
            (true, false) => Some(Tag::Delete),
            (false, true) => Some(Tag::Insert),
            (false, false) => None,
        };
        if let Some(tag) = tag {
            codes.push(Opcode { tag, i1: i, i2: m.i, j1: j, j2: m.j });
        }
        (i, j) = (m.i + m.k, m.j + m.k);
        if m.k > 0 {
            codes.push(Opcode { tag: Tag::Equal, i1: m.i, i2: i, j1: m.j, j2: j });
        }
    }
    if codes.is_empty() {
        codes.push(Opcode { tag: Tag::Equal, i1: 0, i2: 1, j1: 0, j2: 1 });
    }
    codes
}

/// Writes a range as difflib does: "start,length", or "start" for one line.
fn put_range(out: &mut String, start: usize, stop: usize) {
    let (mut beginning, length) = (start + 1, stop - start);
    if length == 1 {
        let _ = write!(out, "{beginning}");
        return;
    }
    if length == 0 {
        beginning -= 1;
    }
    let _ = write!(out, "{beginning},{length}");
}

fn write_code(out: &mut String, a: &[&str], b: &[&str], c: &Opcode) {
    if c.tag == Tag::Equal {
        a[c.i1..c.i2].iter().for_each(|l| {
            out.push(' ');
            out.push_str(l);
        });
        return;
    }
    if matches!(c.tag, Tag::Replace | Tag::Delete) {
        a[c.i1..c.i2].iter().for_each(|l| {
            out.push('-');
            out.push_str(l);
        });
    }
    if matches!(c.tag, Tag::Replace | Tag::Insert) {
        b[c.j1..c.j2].iter().for_each(|l| {
            out.push('+');
            out.push_str(l);
        });
    }
}

/// Groups the opcodes into hunks with three lines of context, as
/// SequenceMatcher.get_grouped_opcodes does, and writes them.
fn write_diff(out: &mut String, a: &[&str], b: &[&str], codes: &[Opcode], from: &str, to: &str) {
    const CONTEXT: usize = 3;
    let n = codes.len();
    let mut group: Vec<Opcode> = Vec::with_capacity(n + 1);
    let mut started = false;
    for (k, &code) in codes.iter().enumerate() {
        let mut c = code;
        if k == 0 && c.tag == Tag::Equal {
            c.i1 = c.i1.max(c.i2.saturating_sub(CONTEXT));
            c.j1 = c.j1.max(c.j2.saturating_sub(CONTEXT));
        }
        if k == n - 1 && c.tag == Tag::Equal {
            c.i2 = c.i2.min(c.i1 + CONTEXT);
            c.j2 = c.j2.min(c.j1 + CONTEXT);
        }
        if c.tag == Tag::Equal && c.i2 - c.i1 > 2 * CONTEXT {
            group.push(Opcode {
                tag: Tag::Equal,
                i1: c.i1,
                i2: c.i2.min(c.i1 + CONTEXT),
                j1: c.j1,
                j2: c.j2.min(c.j1 + CONTEXT),
            });
            write_group(out, a, b, &group, &mut started, from, to);
            group.clear();
            c.i1 = c.i1.max(c.i2.saturating_sub(CONTEXT));
            c.j1 = c.j1.max(c.j2.saturating_sub(CONTEXT));
        }
        group.push(c);
    }
    if !group.is_empty() && !(group.len() == 1 && group[0].tag == Tag::Equal) {
        write_group(out, a, b, &group, &mut started, from, to);
    }
}

fn write_group(
    out: &mut String,
    a: &[&str],
    b: &[&str],
    group: &[Opcode],
    started: &mut bool,
    from: &str,
    to: &str,
) {
    if !*started {
        *started = true;
        let _ = write!(out, "--- {from}\n+++ {to}\n");
    }
    let (head, last) = (group[0], group[group.len() - 1]);
    out.push_str("@@ -");
    put_range(out, head.i1, last.i2);
    out.push_str(" +");
    put_range(out, head.j1, last.j2);
    out.push_str(" @@\n");
    for c in group {
        write_code(out, a, b, c);
    }
}
