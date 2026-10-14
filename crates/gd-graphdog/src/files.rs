//! File confinement. Graphs can make Graphviz open files: a node's image or shapefile, an
//! `<IMG SRC>` in an HTML-like label, and the folders imagepath and fontpath name. After
//! the graph is parsed and before it is laid out, graphdog shows the caller each file name
//! the graph gives and sets the path it returns instead. So Graphviz opens only what the
//! caller allows, by the path the caller chose, which is also what the output shows (an
//! SVG `<image>` names its file).

use std::borrow::Cow;
use std::ffi::{CStr, c_int, c_void};
use std::ptr;

use crate::errors::{Error, ErrorKind};
use crate::{FILE_ATTRIBUTES, FileAccess as Access, FileDecision, c_string, ffi};

/// Attributes that may hold HTML-like labels, whose `<IMG SRC="...">` Graphviz reads.
const LABEL_ATTRIBUTES: [&str; 4] = ["label", "xlabel", "headlabel", "taillabel"];

/// One graph's check: the root graph, the caller's decision, and the first refusal.
struct Walk<'w, 'a> {
    root: *mut ffi::Agraph,
    access: &'w mut Access<'a>,
    /// The attribute and file name refused first.
    refused: Option<(String, String)>,
}

/// Shows `access` every file name in `g` and sets what it returns, before `g` is laid
/// out. Call with the Graphviz lock held. Fails with `FileRefused` for the first name
/// `access` refuses.
pub(crate) fn check(g: *mut ffi::Agraph, access: &mut Access<'_>) -> Result<(), Error> {
    let mut w = Walk { root: g, access, refused: None };
    check_graph(&mut w, g);
    // SAFETY: the lock is held and `g` is a live root graph; the traversal functions
    // return its own nodes and edges, or null at the end.
    unsafe {
        let mut n = ffi::agfstnode(g);
        while !n.is_null() && w.refused.is_none() {
            check_object(&mut w, g, n.cast(), ffi::AG_NODE);
            let mut e = ffi::agfstout(g, n);
            while !e.is_null() && w.refused.is_none() {
                check_object(&mut w, g, e.cast(), ffi::AG_EDGE);
                e = ffi::agnxtout(g, e);
            }
            n = ffi::agnxtnode(g, n);
        }
    }
    let Some((attribute, name)) = w.refused else { return Ok(()) };
    Err(Error::with(
        ErrorKind::FileRefused,
        format!(
            "The graph names the file {name:?} in its {attribute} attribute, which may not \
             be read."
        ),
    ))
}

/// Checks a graph's own attributes and its subgraphs', which may differ.
fn check_graph(w: &mut Walk<'_, '_>, g: *mut ffi::Agraph) {
    check_object(w, g, g.cast(), ffi::AG_GRAPH);
    // SAFETY: the lock is held and `g` is a live graph; agfstsubg/agnxtsubg return its
    // subgraphs, or null at the end.
    let mut sub = unsafe { ffi::agfstsubg(g) };
    while !sub.is_null() && w.refused.is_none() {
        check_graph(w, sub);
        // SAFETY: as above.
        sub = unsafe { ffi::agnxtsubg(sub) };
    }
}

fn check_object(w: &mut Walk<'_, '_>, g: *mut ffi::Agraph, obj: *mut c_void, kind: c_int) {
    // SAFETY: the lock is held and `g` is live; agnxtattr walks its attribute dictionary.
    let mut sym = unsafe { ffi::agnxtattr(g, kind, ptr::null_mut()) };
    while !sym.is_null() && w.refused.is_none() {
        check_symbol(w, obj, sym);
        // SAFETY: as above; `sym` came from the same dictionary.
        sym = unsafe { ffi::agnxtattr(g, kind, sym) };
    }
}

fn check_symbol(w: &mut Walk<'_, '_>, obj: *mut c_void, sym: *mut ffi::Agsym) {
    // SAFETY: `obj` belongs to the graph whose dictionary holds `sym`; agxget returns a
    // NUL-terminated string cgraph owns (or null), and `sym.name` is NUL-terminated.
    let (raw, name) = unsafe {
        let raw = ffi::agxget(obj, sym);
        if raw.is_null() || *raw == 0 {
            return;
        }
        (raw, CStr::from_ptr((*sym).name).to_string_lossy().into_owned())
    };
    // SAFETY: as above; copied before the attribute is set again.
    let value = unsafe { CStr::from_ptr(raw) }.to_string_lossy().into_owned();
    if FILE_ATTRIBUTES.contains(&name.as_str()) {
        let path = match (w.access)(&name, &value) {
            FileDecision::Open(path) => path,
            FileDecision::Omit => String::new(),
            FileDecision::Refuse => {
                w.refused = Some((name, value));
                return;
            }
        };
        let path = c_string(&path);
        // SAFETY: the lock is held; cgraph copies the value.
        unsafe { ffi::agxset(obj, sym, path.as_ptr()) };
        return;
    }
    // SAFETY: `raw` is a cgraph string, which is what aghtmlstr inspects.
    if LABEL_ATTRIBUTES.contains(&name.as_str()) && unsafe { ffi::aghtmlstr(raw) } != 0 {
        check_label(w, obj, sym, &value);
    }
}

/// Rewrites the SRC of every `<IMG>` in an HTML-like label.
fn check_label(w: &mut Walk<'_, '_>, obj: *mut c_void, sym: *mut ffi::Agsym, label: &str) {
    let mut out = String::with_capacity(label.len());
    let mut rest = label;
    let mut changed = false;
    while let Some((start, end)) = next_img_src(rest) {
        let name = xml_unescape(&rest[start..end]);
        let path = match (w.access)("src", &name) {
            FileDecision::Open(path) => path,
            FileDecision::Omit => String::new(),
            FileDecision::Refuse => {
                w.refused = Some(("IMG SRC".to_string(), name.into_owned()));
                return;
            }
        };
        out.push_str(&rest[..start]);
        xml_escape(&mut out, &path);
        rest = &rest[end..];
        changed = true;
    }
    if !changed {
        return;
    }
    out.push_str(rest);
    let text = c_string(&out);
    // SAFETY: the lock is held and `w.root` is the live root graph; agstrdup_html returns
    // a reference-counted cgraph string that keeps its HTML kind, and agxset copies it.
    unsafe {
        let html = ffi::agstrdup_html(w.root, text.as_ptr());
        if !html.is_null() {
            ffi::agxset(obj, sym, html);
        }
    }
}

/// The value of the next `<IMG SRC="...">` in an HTML-like label: its start and end in
/// text, without the quotes. Tags and attributes match in any case, as Graphviz's lexer
/// matches them.
pub(crate) fn next_img_src(text: &str) -> Option<(usize, usize)> {
    let lower = text.to_ascii_lowercase();
    let mut from = 0;
    loop {
        let tag = from + lower[from..].find("<img")? + "<img".len();
        let close = lower[tag..].find('>')?;
        if let Some((s, e)) = src_value(&lower[tag..tag + close]) {
            return Some((tag + s, tag + e));
        }
        from = tag + close;
    }
}

/// Finds `src="..."` or `src='...'` among a tag's attributes.
fn src_value(attrs: &str) -> Option<(usize, usize)> {
    let b = attrs.as_bytes();
    let skip = |mut j: usize| {
        while j < b.len() && is_space(b[j]) {
            j += 1;
        }
        j
    };
    for i in 0..b.len().saturating_sub(2) {
        if &b[i..i + 3] != b"src" || (i > 0 && !is_space(b[i - 1])) {
            continue;
        }
        let j = skip(i + 3);
        if j >= b.len() || b[j] != b'=' {
            continue;
        }
        let j = skip(j + 1);
        if j >= b.len() || (b[j] != b'"' && b[j] != b'\'') {
            continue;
        }
        if let Some(length) = b[j + 1..].iter().position(|&c| c == b[j]) {
            return Some((j + 1, j + 1 + length));
        }
    }
    None
}

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

/// Decodes the five XML entities; borrows `s` when it holds none.
fn xml_unescape(s: &str) -> Cow<'_, str> {
    if !s.contains('&') {
        return Cow::Borrowed(s);
    }
    let pairs = [("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&apos;", "'"), ("&amp;", "&")];
    Cow::Owned(pairs.iter().fold(s.to_string(), |out, (from, to)| out.replace(from, to)))
}

fn xml_escape(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
}
