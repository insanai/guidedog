//! Sphinx's roles that are not cross-references: generic text roles, keys, GUI labels,
//! samples, abbreviations, manual pages, PEPs, RFCs, CVEs, CWEs, and index entries.

use guidedoc::{
    AttributeKey, Builder, Fail, INVALID_NODE, LinkInfo, LinkKind, NodeKind, SourceSpan,
    is_ascii_space,
};
use guidedoc_rst_inline::RoleCall;

use super::{RoleCx, cook};
use crate::directives::check_index_entry;
use crate::messages::{CWE_INVALID, PEP_INVALID, RFC_INVALID};
use crate::text::{
    all_digits, digits_of, equal_fold, parse_int, split_title, trim_space, trim_suffix,
};

/// Emits Role{text = name} > Text.
pub(crate) fn role_node(
    b: &mut Builder<'_>,
    span: SourceSpan,
    name: &str,
    text: &str,
) -> Result<(), Fail> {
    let id = b.begin_node(NodeKind::Role, span)?;
    let t = b.add_text(name)?;
    b.set_text(id, t);
    if !text.is_empty() {
        b.add_text_leaf(NodeKind::Text, span, text)?;
    }
    b.end_node(id)
}

/// A role name in lower case, without a "std:" or other domain.
pub(crate) fn lower_name(name: &str) -> String {
    name.rsplit(':').next().unwrap_or(name).to_ascii_lowercase()
}

/// Role{name} > the text.
pub(crate) fn generic_role(
    b: &mut Builder<'_>,
    call: &RoleCall<'_>,
    text: &str,
) -> Result<(), Fail> {
    role_node(b, call.span, &lower_name(call.name), text)
}

/// kbd: Role{kbd}; keys joined by "-", "+", "^", or spaces become a compound
/// Role{kbd, class compound} of Role{kbd} keys and separator text, as Sphinx's keyboard
/// transform does; multi-word keys such as "Caps Lock" stay together.
pub(crate) fn kbd_role(b: &mut Builder<'_>, call: &RoleCall<'_>) -> Result<(), Fail> {
    let text = call.text;
    let parts = split_keys(text);
    let n = parts.len();
    if n == 1 || (n == 3 && multiword(&parts)) {
        return role_node(b, call.span, "kbd", text);
    }
    let outer = b.begin_node(NodeKind::Role, call.span)?;
    let t = b.add_text("kbd")?;
    b.set_text(outer, t);
    b.add_attribute_string(outer, AttributeKey::Class, "compound")?;
    let mut k = 0;
    while k < n {
        if multiword(&parts[k..]) {
            let joined = parts[k..k + 3].concat();
            role_node(b, call.span, "kbd", &joined)?;
            k += 3;
        } else {
            role_node(b, call.span, "kbd", parts[k])?;
            k += 1;
        }
        if k < n {
            b.add_text_leaf(NodeKind::Text, call.span, parts[k])?;
            k += 1;
        }
    }
    b.end_node(outer)
}

/// Splits at separators with a character on both sides; parts alternate between keys
/// and separators.
fn split_keys(text: &str) -> Vec<&str> {
    let t = text.as_bytes();
    let mut parts = Vec::new();
    let (mut last, mut i) = (0, 1);
    while i + 1 < t.len() {
        let c = t[i];
        let mut end = i;
        if c == b'-' || c == b'+' || c == b'^' {
            end = i + 1;
        } else if is_ascii_space(c) {
            while end < t.len() && is_ascii_space(t[end]) {
                end += 1;
            }
            if end >= t.len() {
                end = t.len() - 1;
            }
        }
        if end == i || end >= t.len() || !text.is_char_boundary(i) {
            i += 1;
            continue;
        }
        parts.push(&text[last..i]);
        parts.push(&text[i..end]);
        (last, i) = (end, end);
    }
    parts.push(&text[last..]);
    parts
}

fn multiword(parts: &[&str]) -> bool {
    const PAIRS: [(&str, &str); 7] = [
        ("caps", "lock"),
        ("page", "down"),
        ("page", "up"),
        ("scroll", "lock"),
        ("num", "lock"),
        ("sys", "rq"),
        ("back", "space"),
    ];
    if parts.len() < 3 || !trim_space(parts[1]).is_empty() {
        return false;
    }
    PAIRS.iter().any(|(a, z)| equal_fold(parts[0], a) && equal_fold(parts[2], z))
}

/// guilabel and menuselection; "-->" in a menu selection becomes "‣", and "&x" marks x
/// as the accelerator (Role{accelerator}); "&&" is a literal "&".
pub(crate) fn gui_role(b: &mut Builder<'_>, call: &RoleCall<'_>) -> Result<(), Fail> {
    let name = lower_name(call.name);
    let replaced;
    let text = if name == "menuselection" {
        replaced = call.text.replace("-->", "\u{2023}");
        replaced.as_str()
    } else {
        call.text
    };
    let id = b.begin_node(NodeKind::Role, call.span)?;
    let t = b.add_text(&name)?;
    b.set_text(id, t);
    let s = text.as_bytes();
    let mut run = String::new();
    let (mut i, mut first) = (0, true);
    while i < s.len() {
        let prev_amp = i > 0 && s[i - 1] == b'&';
        let amp = s[i] == b'&'
            && !prev_amp
            && i + 1 < s.len()
            && s[i + 1] != b'&'
            && !is_ascii_space(s[i + 1]);
        let at_end = s[i] == b'&' && i + 1 == s.len() && !prev_amp;
        if !amp && !at_end {
            if !first && s[i] == b'&' && i + 1 < s.len() && s[i + 1] == b'&' {
                run.push('&');
                i += 2;
                continue;
            }
            let ch = text[i..].chars().next().unwrap_or('\0');
            run.push(ch);
            i += ch.len_utf8();
            continue;
        }
        flush_text(b, call.span, &mut run)?;
        first = false;
        i += 1;
        if i >= s.len() {
            break;
        }
        let size = text[i..].chars().next().map_or(1, char::len_utf8);
        role_node(b, call.span, "accelerator", &text[i..i + size])?;
        i += size;
    }
    flush_text(b, call.span, &mut run)?;
    b.end_node(id)
}

/// Emits the text gathered in `run`, if any, and empties it.
fn flush_text(b: &mut Builder<'_>, span: SourceSpan, run: &mut String) -> Result<(), Fail> {
    if !run.is_empty() {
        b.add_text_leaf(NodeKind::Text, span, run)?;
        run.clear();
    }
    Ok(())
}

/// samp and file; "{variable}" parts become Emphasis; "\{" and "\}" (a doubled
/// backslash in the source) are literal braces.
pub(crate) fn samp_role(b: &mut Builder<'_>, call: &RoleCall<'_>) -> Result<(), Fail> {
    let text = call.text;
    let t = text.as_bytes();
    let id = b.begin_node(NodeKind::Role, call.span)?;
    let name = b.add_text(&lower_name(call.name))?;
    b.set_text(id, name);
    let mut run = String::new();
    let mut i = 0;
    while i < t.len() {
        let c = t[i];
        if c == b'\\' && i + 1 < t.len() && matches!(t[i + 1], b'{' | b'}' | b'\\') {
            run.push(char::from(t[i + 1]));
            i += 2;
            continue;
        }
        if c == b'{' {
            let mut close = i + 1;
            while close < t.len() && t[close] != b'}' && t[close] != b'{' {
                close += 1;
            }
            if close < t.len() && t[close] == b'}' && close > i + 1 {
                flush_text(b, call.span, &mut run)?;
                let em = b.begin_node(NodeKind::Emphasis, call.span)?;
                b.add_text_leaf(NodeKind::Text, call.span, &text[i + 1..close])?;
                b.end_node(em)?;
                i = close + 1;
                continue;
            }
        }
        let ch = text[i..].chars().next().unwrap_or('\0');
        run.push(ch);
        i += ch.len_utf8();
    }
    flush_text(b, call.span, &mut run)?;
    b.end_node(id)
}

/// abbr: Abbreviation{Title = explanation} > the abbreviation; "LIFO (last-in,
/// first-out)".
pub(crate) fn abbr_role(b: &mut Builder<'_>, call: &RoleCall<'_>) -> Result<(), Fail> {
    let mut text = trim_space(call.text);
    let mut explanation = "";
    if text.ends_with(')')
        && let Some(i) = text.find('(')
    {
        explanation = trim_space(&text[i + 1..text.len() - 1]);
        text = trim_space(&text[..i]);
    }
    let id = b.begin_node(NodeKind::Abbreviation, call.span)?;
    if !explanation.is_empty() {
        b.add_attribute_string(id, AttributeKey::Title, explanation)?;
    }
    b.add_text_leaf(NodeKind::Text, call.span, text)?;
    b.end_node(id)
}

/// manpage: Role{manpage} > the text, linked through manpages_url when it is set.
pub(crate) fn manpage_role(rx: &mut RoleCx<'_, '_>, call: &RoleCall<'_>) -> Result<(), Fail> {
    let (title_m, target_m, _) = split_title(call.marked);
    let title = cook(title_m);
    let path = crate::text::collapse(&cook(target_m)).into_owned();
    let (mut page, mut section) = (path.as_str(), "");
    let pb = path.as_bytes();
    if let Some(i) = (1..pb.len()).rev().find(|&i| pb[i] == b'(' || pb[i] == b'.') {
        let sec = trim_suffix(&path[i + 1..], ")");
        if sec.as_bytes().first().is_some_and(|c| (b'1'..=b'9').contains(c)) {
            (page, section) = (&path[..i], sec);
        }
    }
    let url = &rx.config.manpages_url;
    let b = rx.b();
    let id = b.begin_node(NodeKind::Role, call.span)?;
    let t = b.add_text("manpage")?;
    b.set_text(id, t);
    if url.is_empty() {
        b.add_text_leaf(NodeKind::Text, call.span, &title)?;
        return b.end_node(id);
    }
    let dest = fill_url(url, page, section, &path);
    external_link(b, call.span, &dest, "manpage", |b| {
        b.add_text_leaf(NodeKind::Text, call.span, &title).map(|_| ())
    })?;
    b.end_node(id)
}

/// Substitutes "{page}", "{section}", and "{path}" in a manpages_url, left to right.
fn fill_url(url: &str, page: &str, section: &str, path: &str) -> String {
    let mut out = String::with_capacity(url.len() + path.len());
    let mut i = 0;
    while i < url.len() {
        let rest = &url[i..];
        let (value, len) = if rest.starts_with("{page}") {
            (page, 6)
        } else if rest.starts_with("{section}") {
            (section, 9)
        } else if rest.starts_with("{path}") {
            (path, 6)
        } else {
            let ch = rest.chars().next().unwrap_or('\0');
            out.push(ch);
            i += ch.len_utf8();
            continue;
        };
        out.push_str(value);
        i += len;
    }
    out
}

/// Emits Link{External, class} > what `inner` emits.
fn external_link(
    b: &mut Builder<'_>,
    span: SourceSpan,
    dest: &str,
    class: &str,
    inner: impl FnOnce(&mut Builder<'_>) -> Result<(), Fail>,
) -> Result<(), Fail> {
    let link = b.begin_node(NodeKind::Link, span)?;
    let destination = b.add_text(dest)?;
    let info = LinkInfo {
        destination,
        kind: LinkKind::External,
        resolved: INVALID_NODE,
        ..LinkInfo::UNRESOLVED
    };
    b.set_link(link, info)?;
    b.add_attribute_string(link, AttributeKey::Class, class)?;
    inner(b)?;
    b.end_node(link)
}

/// Emits Index{Id "index-N"; entries}.
fn index_node(rx: &mut RoleCx<'_, '_>, span: SourceSpan, entries: &str) -> Result<(), Fail> {
    let index = rx.st.index_id();
    let b = rx.b();
    let ix = b.begin_node(NodeKind::Index, span)?;
    b.add_attribute_string(ix, AttributeKey::Id, &index)?;
    b.add_custom_attribute(ix, "entries", entries)?;
    b.end_node(ix)
}

/// Emits a strong external link: Link{External, class} > Strong > the text.
fn strong_link(
    b: &mut Builder<'_>,
    span: SourceSpan,
    dest: &str,
    class: &str,
    text: &str,
) -> Result<(), Fail> {
    external_link(b, span, dest, class, |b| {
        let strong = b.begin_node(NodeKind::Strong, span)?;
        b.add_text_leaf(NodeKind::Text, span, text)?;
        b.end_node(strong)
    })
}

/// pep and rfc: an index entry and target, then Link{External, class pep/rfc} > Strong.
pub(crate) fn pep_rfc_role(
    rx: &mut RoleCx<'_, '_>,
    call: &RoleCall<'_>,
    pep: bool,
) -> Result<(), Fail> {
    let (title_m, target_m, explicit) = split_title(call.marked);
    let title = cook(title_m);
    let target = trim_space(&cook(target_m)).to_owned();
    let (number, fragment) = match target.find('#') {
        Some(h) => (&target[..h], Some(&target[h + 1..])),
        None => (target.as_str(), None),
    };
    if !all_digits(number) || number.len() > 9 || (!pep && parse_int(number) < 1) {
        rx.report(if pep { PEP_INVALID } else { RFC_INVALID }, call.span);
        rx.b().add_text_leaf(NodeKind::Text, call.span, call.text)?;
        return Ok(());
    }
    let n = parse_int(number);
    let label =
        if pep { "single: Python Enhancement Proposals; PEP " } else { "single: RFC; RFC " };
    index_node(rx, call.span, &format!("{label}{target}"))?;
    let mut dest = if pep {
        let base = &rx.config.settings.pep_base_url;
        let base = if base.is_empty() { "https://peps.python.org/" } else { base };
        let zeros = "0".repeat(4usize.saturating_sub(digits_of(n)));
        format!("{base}pep-{zeros}{n}/")
    } else {
        let base = &rx.config.settings.rfc_base_url;
        let base = if base.is_empty() { "https://datatracker.ietf.org/doc/html/" } else { base };
        format!("{base}rfc{n}.html")
    };
    if let Some(f) = fragment {
        dest.push('#');
        dest.push_str(f);
    }
    let prefix = if explicit {
        ""
    } else if pep {
        "PEP "
    } else {
        "RFC "
    };
    let shown = format!("{prefix}{title}");
    strong_link(rx.b(), call.span, &dest, if pep { "pep" } else { "rfc" }, &shown)
}

/// Sphinx's :cve: and :cwe: (8.1): an index entry and its target, and a strong link to
/// the CVE record or the CWE definition, "CVE <id>" without a title.
pub(crate) fn cve_cwe_role(
    rx: &mut RoleCx<'_, '_>,
    call: &RoleCall<'_>,
    cve: bool,
) -> Result<(), Fail> {
    let (title_m, target_m, explicit) = split_title(call.marked);
    let title = cook(title_m);
    let target = trim_space(&cook(target_m)).to_owned();
    // A CWE is a number, which Sphinx reads with int(); then nothing but the error.
    let hash = target.find('#');
    let number = hash.map_or(target.as_str(), |h| &target[..h]);
    if !cve && (!all_digits(number) || number.len() > 9) {
        rx.report(CWE_INVALID, call.span);
        rx.b().add_text_leaf(NodeKind::Text, call.span, call.text)?;
        return Ok(());
    }
    let entry = if cve {
        "single: Common Vulnerabilities and Exposures; CVE "
    } else {
        "single: Common Weakness Enumeration; CWE "
    };
    index_node(rx, call.span, &format!("{entry}{target}"))?;
    let dest = if cve {
        format!("https://www.cve.org/CVERecord?id=CVE-{target}")
    } else {
        // Sphinx builds "<base><int(number)>.html", with any "#fragment" after it.
        let fragment = hash.map_or("", |h| &target[h..]);
        format!("https://cwe.mitre.org/data/definitions/{}.html{fragment}", parse_int(number))
    };
    let prefix = if explicit {
        ""
    } else if cve {
        "CVE "
    } else {
        "CWE "
    };
    let shown = format!("{prefix}{title}");
    strong_link(rx.b(), call.span, &dest, if cve { "cve" } else { "cwe" }, &shown)
}

/// index: Index{Id "index-N"; entries} then the text. With an explicit title the target
/// is a full entry as the index directive takes it; otherwise it is a single entry ("!"
/// marks a main entry).
pub(crate) fn index_role(rx: &mut RoleCx<'_, '_>, call: &RoleCall<'_>) -> Result<(), Fail> {
    let (title_m, target_m, explicit) = split_title(call.marked);
    let mut title = cook(title_m);
    let mut target = trim_space(&cook(target_m)).to_owned();
    let main = !explicit && target.starts_with('!');
    if main {
        target.remove(0);
        if !title.is_empty() {
            title.remove(0);
        }
    }
    let entries = if explicit {
        if let Some(m) = check_index_entry(&target) {
            rx.report(m, call.span);
        }
        target
    } else if main {
        format!("!single: {target}")
    } else {
        format!("single: {target}")
    };
    index_node(rx, call.span, &entries)?;
    if !title.is_empty() {
        rx.b().add_text_leaf(NodeKind::Text, call.span, &title)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_split_at_separators_between_characters() {
        assert_eq!(split_keys("Control-x"), ["Control", "-", "x"]);
        assert_eq!(split_keys("-"), ["-"]);
        assert_eq!(split_keys("Caps Lock"), ["Caps", " ", "Lock"]);
        assert!(multiword(&split_keys("Caps Lock")));
    }

    #[test]
    fn manpage_urls_fill_their_fields() {
        assert_eq!(fill_url("x/{section}/{page}.html", "ls", "1", "ls(1)"), "x/1/ls.html");
    }
}
