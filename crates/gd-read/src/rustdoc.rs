//! The rust domain's API pages (new in the Rust port): each crate of
//! `rust_autoapi_crates` is read by gd-rustdoc (tree-sitter, nothing compiled) into a
//! page per documented module, `<rust_autoapi_root>/<crate>/<module>`, and an index,
//! `<rust_autoapi_root>/index`. The pages are generated documents whose dependencies
//! are the crate's files, so editing a crate reads its pages again; gd-resolve adds the
//! index to the root document's first toctree (`rust_autoapi_add_toctree_entry`).

use std::path::{Path, PathBuf};

use gd_conf::{Config, Digest};
use gd_env::{Dependency, SourceFile};
use gd_report::{Excerpt, Report, Severity};
use gd_rustdoc::{Crate, Diagnostic, MemberOrder, Options};

use crate::api::{generated_source, title_block};
use crate::rustcache::{CratePages, RustCache, crate_folder, crate_key, listing_digest};

/// The pages of `rust_autoapi_crates`, sorted by crate, index first; what loading and
/// documenting the crates found is added to `reports` (`rust.autodoc.*`), every build. A
/// crate whose name an earlier crate has is left out with `rust.autoapi.clash`. A crate
/// none of whose files changed since the last build keeps the pages it had then
/// (`RustCache`), so it is not parsed again.
pub(crate) fn rust_api_sources(c: &Config, reports: &mut Vec<Report>) -> Vec<SourceFile> {
    let r = &c.rust_api;
    if !r.generate_api_docs || r.crates.is_empty() {
        return Vec::new();
    }
    let options = rust_options(c);
    let mut cache = RustCache::load(c);
    let jobs = r.crates.iter().map(|folder| {
        let path = gd_conf::source_path(c, folder);
        let key = crate_key(&path, &options);
        let cached = cache.take(&key);
        CrateJob { path, key, cached }
    });
    let mut crates: Vec<(Digest, CratePages)> = Vec::new();
    let mut reused = 0;
    for done in document_crates(jobs.collect(), &options) {
        reused += usize::from(done.reused);
        let pages = match done.pages {
            Ok(pages) => pages,
            Err(report) => {
                reports.push(*report);
                continue;
            }
        };
        reports.extend(pages.reports.iter().cloned());
        if crates.iter().any(|(_, known)| known.name == pages.name) {
            reports.push(clash(&done.path, &pages.name));
            continue;
        }
        crates.push((done.key, pages));
    }
    cache.save(&crates, reused);
    let crates: Vec<CratePages> = crates.into_iter().map(|(_, pages)| pages).collect();
    crate_sources(c, crates)
}

/// The generated documents of `crates`: the index, then each crate's module pages.
fn crate_sources(c: &Config, crates: Vec<CratePages>) -> Vec<SourceFile> {
    if crates.is_empty() {
        return Vec::new();
    }
    let root = &c.rust_api.root;
    let index = format!("{root}/index");
    let deps: Vec<Dependency> = crates.iter().flat_map(crate_deps).collect();
    let mut out = vec![generated_source(c, &index, index_page(&crates), deps)];
    for k in crates {
        let deps = crate_deps(&k);
        for (path, text) in k.modules {
            let docname = format!("{root}/{}", path.trim_end_matches(".rst"));
            out.push(generated_source(c, &docname, text, deps.clone()));
        }
    }
    out
}

/// A crate to document: its path, its key, and the pages its record kept, if any.
struct CrateJob {
    path: PathBuf,
    key: Digest,
    cached: Option<CratePages>,
}

/// A crate documented: its pages (or why it has none) and whether they are the kept ones.
struct CrateDone {
    path: PathBuf,
    key: Digest,
    pages: Result<CratePages, Box<Report>>,
    reused: bool,
}

/// Documents each crate of `jobs`, in order, on a thread per processor: checking a
/// record hashes every file of its crate, and parsing a crate takes longer still.
fn document_crates(jobs: Vec<CrateJob>, options: &Options) -> Vec<CrateDone> {
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    let threads = threads.min(jobs.len()).max(1);
    gd_host::map_ordered(threads, jobs, || (), |_, job| document_crate(job, options))
}

/// The pages of one crate: its record's when they still hold, else made now.
fn document_crate(job: CrateJob, options: &Options) -> CrateDone {
    let CrateJob { path, key, cached } = job;
    let listing = listing_digest(&crate_folder(&path));
    if let Some(pages) = cached.filter(|p| p.still_holds(&listing)) {
        return CrateDone { path, key, pages: Ok(pages), reused: true };
    }
    let pages = crate_pages(&path, options, listing);
    CrateDone { path, key, pages, reused: false }
}

/// `rust_autoapi_options` and `rust_autoapi_member_order` as gd-rustdoc's options.
fn rust_options(c: &Config) -> Options {
    let r = &c.rust_api;
    let has = |name: &str| r.options.iter().any(|o| o == name);
    Options {
        include_private: has("private-members"),
        document_hidden: has("hidden-members"),
        noindex: has("noindex"),
        member_order: match r.member_order.as_str() {
            "alphabetical" => MemberOrder::Alphabetical,
            _ => MemberOrder::Source,
        },
        ..Options::default()
    }
}

/// Loads and documents the crate at `path`, whose folder's listing is `listing`; the
/// error as a report when its root cannot be read.
fn crate_pages(
    path: &Path,
    options: &Options,
    listing: Digest,
) -> Result<CratePages, Box<Report>> {
    let krate = gd_rustdoc::load_crate(path, options)
        .map_err(|e| Box::new(diagnostic_report(&e.diagnostic)))?;
    let mut reports: Vec<Report> = krate.warnings.iter().map(diagnostic_report).collect();
    let mut pages = gd_rustdoc::document(&krate, options).into_iter();
    let index = pages.next().map(|p| p.text).unwrap_or_default();
    let mut modules = Vec::new();
    for page in pages {
        reports.extend(page.warnings.iter().map(diagnostic_report));
        modules.push((page.path, page.text));
    }
    let inputs = crate_inputs(&krate);
    Ok(CratePages { name: krate.name, index, modules, reports, inputs, listing })
}

/// The files the crate was made from with the digests of their bytes, as
/// `gd_env::dependencies_changed` checks them. A module file's digest is that of the
/// text parsed, so a file edited while the crate loaded is seen as changed next time.
fn crate_inputs(krate: &Crate) -> Vec<(std::path::PathBuf, Option<Digest>)> {
    krate
        .inputs
        .iter()
        .map(|path| {
            let parsed = krate.files.iter().find(|f| f.path == *path);
            let digest = match parsed {
                Some(f) => Some(Digest::of_parts(&[f.text.as_bytes()])),
                None => gd_env::file_digest(path),
            };
            (path.clone(), digest)
        })
        .collect()
}

/// The dependencies of a crate's pages: its inputs that could be read. One that could
/// not is still watched through the crate's listing and the record of its pages.
fn crate_deps(k: &CratePages) -> Vec<Dependency> {
    let readable = k.inputs.iter().filter_map(|(path, digest)| Some((path, (*digest)?)));
    readable
        .map(|(path, digest)| Dependency { path: path.clone(), digest, included: false })
        .collect()
}

/// The index: one crate's own, or, for several, every crate's under one title.
fn index_page(crates: &[CratePages]) -> String {
    if let [only] = crates {
        return only.index.clone();
    }
    let mut page = title_block("Rust API reference");
    for k in crates {
        // Each crate's index without its title: its toctree and its list of modules.
        let body = k.index.split_once("\n\n").map_or("", |(_, body)| body);
        page.push_str(body.trim_end());
        page.push_str("\n\n");
    }
    page
}

fn clash(path: &Path, name: &str) -> Report {
    let message = format!(
        "rust_autoapi_crates names {}, whose crate {name} an earlier entry already \
         documents; it is left out.",
        path.display()
    );
    Report {
        path: path.display().to_string(),
        ..Report::warning("rust.autoapi.clash", "CRATE DOCUMENTED TWICE", &message)
            .hint("List each crate once in rust_autoapi_crates.")
    }
}

/// A gd-rustdoc diagnostic as a report, with its place and excerpt.
fn diagnostic_report(d: &Diagnostic) -> Report {
    let severity = match d.severity {
        gd_rustdoc::Severity::Warning => Severity::Warning,
        gd_rustdoc::Severity::Error => Severity::Error,
    };
    let mut report = Report {
        code: d.code.into(),
        severity,
        title: d.title.into(),
        message: d.message.clone(),
        hints: vec![d.hint.clone()],
        path: d.file.clone().unwrap_or_default(),
        ..Report::default()
    };
    if let Some(p) = &d.position {
        report.line = p.line;
        report.column = p.column;
        let start = p.excerpt.char_indices().nth(p.column.saturating_sub(1)).map(|(i, _)| i);
        let start = start.unwrap_or(p.excerpt.len());
        let end = p.excerpt[start..].chars().next().map_or(start, |ch| start + ch.len_utf8());
        report.excerpt =
            Some(Excerpt { text: p.excerpt.clone(), start, end, before: String::new() });
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crate_with(name: &str, index: &str) -> CratePages {
        CratePages {
            name: name.into(),
            index: index.into(),
            modules: Vec::new(),
            reports: Vec::new(),
            inputs: Vec::new(),
            listing: Digest::default(),
        }
    }

    #[test]
    fn several_crates_share_one_index() {
        let a = crate_with("a", "a API reference\n===============\n\n.. toctree::\n   a\n");
        let b = crate_with("b", "b API reference\n===============\n\n- :rust:mod:`b`\n");
        assert_eq!(index_page(std::slice::from_ref(&a)), a.index);
        assert_eq!(
            index_page(&[a, b]),
            "Rust API reference\n==================\n\n.. toctree::\n   a\n\n- :rust:mod:`b`\n\n"
        );
    }

    #[test]
    fn diagnostics_keep_their_place() {
        let d = Diagnostic::warning(
            "rust.autodoc.parse",
            "RUST SOURCE DOES NOT PARSE",
            "m".into(),
            "h",
        )
        .at("src/lib.rs", "fn a() {}\nfn é(\n", 13);
        let r = diagnostic_report(&d);
        assert_eq!((r.line, r.column, r.path.as_str()), (2, 4, "src/lib.rs"));
        let excerpt = r.excerpt.unwrap();
        assert_eq!(&excerpt.text[excerpt.start..excerpt.end], "é");
    }
}
