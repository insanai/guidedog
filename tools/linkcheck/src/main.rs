//! Checks the internal links of a built HTML site: every href that stays in the site must
//! name a file that exists, and a #fragment must name an id (or a name) on that page.
//! Links to other sites, `mailto:`, `javascript:`, and `data:` are left out.
//!
//! ```sh
//! cargo run -p linkcheck -- HTML_DIR [-v]
//! ```
//!
//! The last line is "pages N, internal links M, missing files X, missing anchors Y"; with
//! `-v`, up to 50 of each kind of broken link are listed first. It exits 1 when a link is
//! broken and 2 for a usage problem.

#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::ExitCode;

/// How many broken links of each kind `-v` lists.
const LISTED: usize = 50;

/// Folders whose pages are resources, source copies, or download attachments: their
/// contents are not generated pages, so their links are not checked.
const SKIPPED: [&str; 3] = ["_static", "_sources", "_downloads"];

/// What a check found. Broken links read "page -> href".
#[derive(Debug, Default)]
struct Outcome {
    pages: usize,
    links: usize,
    missing_files: Vec<String>,
    missing_anchors: Vec<String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dir, verbose) = match args.as_slice() {
        [dir] => (dir, false),
        [dir, flag] if flag == "-v" => (dir, true),
        _ => {
            eprintln!("usage: linkcheck HTML_DIR [-v]");
            eprintln!("Hint: give the folder of a built site, such as build/site.");
            return ExitCode::from(2);
        }
    };
    if !Path::new(dir).is_dir() {
        eprintln!("{dir} is not a folder.");
        eprintln!("Hint: build the site first, then pass its output folder.");
        return ExitCode::from(2);
    }
    let outcome = check_site(Path::new(dir));
    if verbose {
        list("missing files", &outcome.missing_files);
        list("missing anchors", &outcome.missing_anchors);
    }
    println!(
        "pages {}, internal links {}, missing files {}, missing anchors {}",
        outcome.pages,
        outcome.links,
        outcome.missing_files.len(),
        outcome.missing_anchors.len()
    );
    if outcome.missing_files.is_empty() && outcome.missing_anchors.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn list(what: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    println!("{what} ({}):", items.len());
    for item in items.iter().take(LISTED) {
        println!("  {item}");
    }
}

/// Checks every page below `dir`; the ids of each linked page are read once.
fn check_site(dir: &Path) -> Outcome {
    let mut outcome = Outcome::default();
    let mut pages = Vec::new();
    walk(dir, "", &mut pages);
    pages.sort();
    let mut ids: HashMap<String, HashSet<String>> = HashMap::new();
    for page in &pages {
        outcome.pages += 1;
        check_page(dir, page, &mut ids, &mut outcome);
    }
    outcome
}

/// Checks the links of one page, named from the site's root.
fn check_page(
    dir: &Path,
    page: &str,
    ids: &mut HashMap<String, HashSet<String>>,
    outcome: &mut Outcome,
) {
    let html = read(dir, page);
    // A <base> names the site's root as browsers see it (a 404 page has one, since the
    // server shows it at any depth): the page's links start at the site's root.
    let base = base_href(&html);
    let from = match base {
        Some(_) => page.rsplit('/').next().unwrap_or(page),
        None => page,
    };
    for href in values(&html, "href") {
        if base == Some(href) {
            continue;
        }
        let Some((mut target, fragment)) = resolve_href(from, href) else {
            continue;
        };
        outcome.links += 1;
        if target.is_empty() {
            target = page.to_owned();
        }
        if dir.join(&target).is_dir() {
            target.push_str("/index.html");
        }
        let broken = format!("{page} -> {href}");
        if !dir.join(&target).exists() {
            outcome.missing_files.push(broken);
            continue;
        }
        if fragment.is_empty() || !target.ends_with(".html") {
            continue;
        }
        let known = ids.entry(target).or_insert_with_key(|t| page_ids(&read(dir, t)));
        if !known.contains(&fragment) {
            outcome.missing_anchors.push(broken);
        }
    }
}

/// The href of the page's `<base>` element, when it has one.
fn base_href(html: &str) -> Option<&str> {
    const OPEN: &str = "<base href=\"";
    let rest = &html[html.find(OPEN)? + OPEN.len()..];
    rest.find('"').map(|end| &rest[..end])
}

/// Turns an href on `page` into a path from the site's root and a fragment, or `None`
/// for a link that leaves the site.
fn resolve_href(page: &str, href: &str) -> Option<(String, String)> {
    let unescaped = href.trim().replace("&amp;", "&");
    let href = unescaped.as_str();
    let leaves = ["mailto:", "javascript:", "data:", "//"];
    if href.is_empty() || href.contains("://") || leaves.iter().any(|p| href.starts_with(p)) {
        return None;
    }
    let (rest, fragment) = href.split_once('#').unwrap_or((href, ""));
    let rest = rest.split_once('?').map_or(rest, |(path, _)| path);
    if rest.is_empty() {
        return Some((String::new(), fragment.to_owned()));
    }
    let mut parts: Vec<&str> = Vec::new();
    let absolute = rest.starts_with('/');
    if !absolute && let Some((folder, _)) = page.rsplit_once('/') {
        parts.extend(folder.split('/'));
    }
    for part in rest.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    Some((parts.join("/"), fragment.to_owned()))
}

/// The ids and names a page defines.
fn page_ids(html: &str) -> HashSet<String> {
    values(html, "id").chain(values(html, "name")).map(str::to_owned).collect()
}

/// The values of every `name="..."` attribute in a piece of HTML, in order.
fn values<'h>(html: &'h str, name: &str) -> impl Iterator<Item = &'h str> {
    let needle = format!(" {name}=\"");
    let mut rest = html;
    std::iter::from_fn(move || {
        let start = rest.find(&needle)? + needle.len();
        let end = rest[start..].find('"')?;
        let value = &rest[start..start + end];
        rest = &rest[start + end..];
        Some(value)
    })
}

/// The text of a page, or "" when it cannot be read (it then has no links or ids).
fn read(dir: &Path, page: &str) -> String {
    fs::read_to_string(dir.join(page)).unwrap_or_default()
}

/// Lists the HTML pages below `dir`, named from the site's root (`prefix` is `dir`'s own
/// path there), leaving out resources, source copies, download attachments, and hidden
/// files.
fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || SKIPPED.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            walk(&path, &format!("{prefix}{name}/"), out);
        } else if name.ends_with(".html") {
            out.push(format!("{prefix}{name}"));
        }
    }
}

#[cfg(test)]
mod tests;
