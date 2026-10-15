//! `cargo xtask build-site`: builds the Guidedog website with Guidedog and puts it
//! together in one static folder:
//!
//! ```text
//! /                       the landing page (docs/site); its editions in zh-cn/ ja/ ...
//! /manual/                the manual (docs/manual); its editions in manual/zh-cn/ ...
//! /manual/books/EDITION/  each edition's PDF book, guidedog-manual-LANGUAGE.pdf
//! /discussions/           the discussions (docs/gds), with every record's PDF in pdf/
//! /404.html, /.nojekyll   for GitHub Pages
//! ```
//!
//! Every link is relative, so the folder works under any base path, such as /guidedog/.
//! Only 404.html names the base path (html_context.site_base in docs/site/conf.toml),
//! because GitHub Pages shows it at any depth.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::args::{self, Spec};
use crate::fsutil::{self, absolute, copy_tree, create_dir, remove, rename, shown};
use crate::{Result, fail, process, repository_root};

const HELP: &str = "\
Builds the Guidedog website with Guidedog into one static folder: the landing page, the
manual and its PDF books in every language, and the discussions with their PDFs.

Usage: cargo xtask build-site [--guidedog PATH] [--out DIR] [--no-pdf]

  --guidedog PATH   the guidedog to build with (or set GUIDEDOG). Without one, the task
                    builds target/release/guidedog with Cargo, as the README describes.
  --out DIR         where the site goes (default: build/site in the repository).
  --no-pdf          leave out the PDF books, for a quick look; their links then fail.

The CJK PDF books need the fonts `cargo xtask fetch-fonts` fetches. A VERSION file at the
repository root, when there is one, sets the version every page shows. The site is put
together next to DIR and replaces it only when every build succeeds, so a failed run
leaves the previous site in place; the intermediate folders are removed either way.
";

/// The languages of the site, English first.
const LANGUAGES: [&str; 7] = ["en", "zh_CN", "ja", "ko", "hi", "es", "de"];

/// What `build-site` builds, and where.
#[derive(Debug, Clone, Default)]
pub(crate) struct Options {
    /// The guidedog to build with; `None` builds it with Cargo.
    pub guidedog: Option<PathBuf>,
    /// Where the site goes; `None` is build/site in the repository.
    pub out: Option<PathBuf>,
    /// Whether to build the PDF books.
    pub pdf: bool,
}

/// Runs `cargo xtask build-site`.
pub(crate) fn main(args: &[String]) -> Result<()> {
    let spec = Spec {
        command: "build-site",
        flags: &["--no-pdf"],
        options: &["--guidedog", "--out"],
        help: HELP,
    };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(0, &spec)?;
    let guidedog = parsed.option("--guidedog").map(String::from);
    let guidedog = guidedog.or_else(|| std::env::var("GUIDEDOG").ok()).filter(|g| !g.is_empty());
    let options = Options {
        guidedog: guidedog.map(|g| program_path(&g)),
        out: parsed.option("--out").map(|out| absolute(Path::new(out))),
        pdf: !parsed.has("--no-pdf"),
    };
    build_site(&options)
}

/// A program named on the command line: a path when it has a separator, else found on
/// PATH (or left as it is, to fail later with a clear message).
pub(crate) fn program_path(name: &str) -> PathBuf {
    if name.contains(std::path::MAIN_SEPARATOR) || name.contains('/') {
        return absolute(Path::new(name));
    }
    let found = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path).map(|dir| dir.join(name)).find(|p| fsutil::is_executable(p))
    });
    found.unwrap_or_else(|| PathBuf::from(name))
}

/// The folders a build works in, removed when it ends however it ends.
struct Folders {
    out: PathBuf,
    work: PathBuf,
    next: PathBuf,
    old: PathBuf,
}

impl Folders {
    fn new(out: PathBuf) -> Folders {
        let sibling = |suffix: &str| PathBuf::from(format!("{}.{suffix}", out.display()));
        Folders { work: sibling("work"), next: sibling("next"), old: sibling("old"), out }
    }

    fn clean(&self) -> Result<()> {
        remove(&self.work)?;
        remove(&self.next)?;
        remove(&self.old)
    }
}

impl Drop for Folders {
    fn drop(&mut self) {
        let _ = self.clean();
    }
}

/// Runs `guidedog build -q -W` with the release's version settings after the arguments.
struct Builder {
    guidedog: PathBuf,
    root: PathBuf,
    defines: Vec<String>,
    started: Instant,
}

impl Builder {
    fn build(&self, args: &[&str]) -> Result<()> {
        let mut command = Command::new(&self.guidedog);
        command.current_dir(&self.root).args(["build", "-q", "-W"]).args(args);
        command.args(&self.defines);
        process::run(&mut command)
    }

    fn step(&self, what: &str) {
        println!("== {what} ({} s)", self.started.elapsed().as_secs());
    }
}

/// Builds the site as [`Options`] says.
pub(crate) fn build_site(options: &Options) -> Result<()> {
    let root = repository_root();
    let started = Instant::now();
    let folders = Folders::new(options.out.clone().unwrap_or_else(|| root.join("build/site")));
    folders.clean()?;
    let guidedog = match &options.guidedog {
        Some(guidedog) => guidedog.clone(),
        None => build_guidedog(&root, started)?,
    };
    if !fsutil::is_executable(&guidedog) {
        return Err(fail(
            "site-guidedog",
            "NO GUIDEDOG",
            &format!("{} is not an executable file.", guidedog.display()),
            "Build it with `cargo build --release -p guidedog-cmd`, or pass --guidedog PATH.",
        ));
    }
    let builder = Builder { guidedog, defines: version_defines(&root)?, root, started };
    create_dir(&folders.work)?;
    create_dir(&folders.next)?;
    for language in LANGUAGES {
        build_landing(&builder, &folders, language)?;
        build_manual(&builder, &folders, language, options.pdf)?;
    }
    build_discussions(&builder, &folders, options.pdf)?;
    publish(&folders)?;
    builder.step(&format!("done: {}", shown(&folders.out, &builder.root)));
    print_sizes(&folders.out, &builder.root)
}

/// Builds target/release/guidedog with Cargo (CARGO_TARGET_DIR is honoured) and returns
/// its path; fails when Cargo fails.
pub(crate) fn build_guidedog(root: &Path, started: Instant) -> Result<PathBuf> {
    println!("== building guidedog ({} s)", started.elapsed().as_secs());
    let mut cargo = Command::new("cargo");
    cargo.current_dir(root).args(["build", "--release", "-p", "guidedog-cmd"]);
    process::run(&mut cargo)?;
    let target =
        std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from);
    let target = if target.is_absolute() { target } else { root.join(target) };
    Ok(target.join("release").join(format!("guidedog{}", std::env::consts::EXE_SUFFIX)))
}

/// `-D release=0.7.0 -D version=0.7` from the VERSION file, when there is one.
fn version_defines(root: &Path) -> Result<Vec<String>> {
    let path = root.join("VERSION");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let release: String = fsutil::read_text(&path)?.split_whitespace().collect();
    let version = release.rsplit_once('.').map_or(release.as_str(), |(head, _)| head);
    Ok(vec!["-D".into(), format!("release={release}"), "-D".into(), format!("version={version}")])
}

/// A language's edition folder: "" for English, else "zh-cn/".
fn edition(language: &str) -> String {
    if language == "en" {
        return String::new();
    }
    format!("{}/", language.replace('_', "-").to_lowercase())
}

/// The links the landing page's text names, relative to its edition's folder.
fn landing_links(language: &str) -> String {
    let ed = edition(language);
    let up = if ed.is_empty() { "" } else { "../" };
    let book = if ed.is_empty() { "en/" } else { ed.as_str() };
    format!(
        ".. _manual: {up}manual/{ed}index.html\n\
         .. _manual-pdf: {up}manual/books/{book}guidedog-manual-{language}.pdf\n\
         .. _manual-tutorial: {up}manual/{ed}tutorial/first-project.html\n\
         .. _discussions: {up}discussions/index.html\n\
         .. _manual-discussions: {up}manual/{ed}howto/discussions.html"
    )
}

fn build_landing(builder: &Builder, folders: &Folders, language: &str) -> Result<()> {
    builder.step(&format!("landing page ({language})"));
    let out = folders.work.join(format!("site-{language}"));
    let out_arg = out.to_string_lossy().into_owned();
    let language_arg = format!("language={language}");
    let epilog = format!("rst_epilog={}", landing_links(language));
    builder.build(&["-b", "html", "docs/site", &out_arg, "-D", &language_arg, "-D", &epilog])?;
    // The landing page's search box searches the manual, so its own search page is
    // unused, and its pages use the web fonts (woff2) of the folder it copies, not the
    // print ones.
    remove(&out.join("search.html"))?;
    remove(&out.join("_static/searchindex.js"))?;
    let fonts = fsutil::sorted_entries(&out.join("_static")).unwrap_or_default();
    for font in fonts.iter().filter(|p| p.extension().is_some_and(|e| e == "otf")) {
        remove(font)?;
    }
    if language == "en" {
        return copy_tree(&out, &folders.next);
    }
    // GitHub Pages serves only the root's 404.html and .nojekyll.
    remove(&out.join("404.html"))?;
    remove(&out.join(".nojekyll"))?;
    rename(&out, &folders.next.join(edition(language).trim_end_matches('/')))
}

fn build_manual(builder: &Builder, folders: &Folders, language: &str, pdf: bool) -> Result<()> {
    builder.step(&format!("manual, website ({language})"));
    let out = folders.work.join(format!("manual-{language}"));
    let out_arg = out.to_string_lossy().into_owned();
    let language_arg = format!("language={language}");
    builder.build(&[
        "-b",
        "html",
        "docs/manual",
        &out_arg,
        "-j",
        "auto",
        "-D",
        &language_arg,
        "-D",
        "html_extra_path=[]",
        "-D",
        "html_context.site_home=../",
    ])?;
    let manual = folders.next.join("manual");
    create_dir(&manual)?;
    let ed = edition(language);
    if ed.is_empty() {
        copy_tree(&out, &manual)?;
    } else {
        rename(&out, &manual.join(ed.trim_end_matches('/')))?;
    }
    if !pdf {
        return Ok(());
    }
    builder.step(&format!("manual, PDF book ({language})"));
    let book = folders.work.join(format!("book-{language}"));
    let book_arg = book.to_string_lossy().into_owned();
    builder.build(&[
        "-b",
        "pdf",
        "docs/manual",
        &book_arg,
        "-D",
        &language_arg,
        "-D",
        "html_extra_path=[]",
    ])?;
    let books = manual.join("books").join(if ed.is_empty() { "en/" } else { ed.as_str() });
    create_dir(&books)?;
    let name = format!("guidedog-manual-{language}.pdf");
    rename(&book.join(&name), &books.join(&name))
}

fn build_discussions(builder: &Builder, folders: &Folders, pdf: bool) -> Result<()> {
    builder.step("discussions, website");
    let out = folders.work.join("gds");
    let out_arg = out.to_string_lossy().into_owned();
    builder.build(&[
        "-b",
        "html",
        "docs/gds",
        &out_arg,
        "-j",
        "auto",
        "-D",
        "html_context.pdf_downloads=pdf/",
        "-D",
        "html_context.site_home=../",
    ])?;
    let discussions = folders.next.join("discussions");
    copy_tree(&out, &discussions)?;
    if !pdf {
        return Ok(());
    }
    builder.step("discussions, PDF");
    let pdfs = folders.work.join("gds-pdf");
    builder.build(&["-b", "pdf", "docs/gds", &pdfs.to_string_lossy()])?;
    let target = discussions.join("pdf");
    create_dir(&target)?;
    for file in fsutil::sorted_entries(&pdfs)? {
        if file.is_file() && file.extension().is_some_and(|e| e == "pdf") {
            rename(&file, &target.join(file.file_name().unwrap_or_default()))?;
        }
    }
    Ok(())
}

/// Keeps only the published files (no build state or hidden files besides the root's
/// .nojekyll), then swaps the new site in for the old one.
fn publish(folders: &Folders) -> Result<()> {
    remove_hidden(&folders.next, &folders.next.join(".nojekyll"))?;
    if !folders.next.join("404.html").is_file() || !folders.next.join(".nojekyll").is_file() {
        return Err(fail(
            "site-pages",
            "NO 404 PAGE",
            "The landing page build wrote no 404.html or .nojekyll.",
            "Check html_additional_pages and extensions in docs/site/conf.toml.",
        ));
    }
    remove(&folders.old)?;
    if folders.out.exists() {
        rename(&folders.out, &folders.old)?;
    }
    rename(&folders.next, &folders.out)?;
    remove(&folders.old)
}

fn remove_hidden(dir: &Path, keep: &Path) -> Result<()> {
    for entry in fsutil::sorted_entries(dir)? {
        let hidden = entry.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
        if hidden && entry != keep {
            remove(&entry)?;
        } else if entry.is_dir() {
            remove_hidden(&entry, keep)?;
        }
    }
    Ok(())
}

/// Prints the sizes as a download sees them, and the number of files.
fn print_sizes(out: &Path, root: &Path) -> Result<()> {
    for part in
        [out.to_path_buf(), out.join("manual"), out.join("manual/books"), out.join("discussions")]
    {
        let (bytes, _) = tally(&part)?;
        println!("{}: {:.1} MiB", shown(&part, root), bytes as f64 / 1_048_576.0);
    }
    println!("files: {}", tally(out)?.1);
    Ok(())
}

/// The bytes and the number of the files under a folder.
fn tally(dir: &Path) -> Result<(u64, usize)> {
    let (mut bytes, mut files) = (0, 0);
    for entry in fsutil::sorted_entries(dir)? {
        let meta = std::fs::symlink_metadata(&entry)
            .map_err(|err| fsutil::io_failure("read", &entry, &err))?;
        if meta.is_dir() {
            let (b, f) = tally(&entry)?;
            bytes += b;
            files += f;
        } else if meta.is_file() {
            bytes += meta.len();
            files += 1;
        }
    }
    Ok((bytes, files))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editions_and_links() {
        assert_eq!(edition("en"), "");
        assert_eq!(edition("zh_CN"), "zh-cn/");
        let links = landing_links("ja");
        assert!(links.contains(".. _manual-pdf: ../manual/books/ja/guidedog-manual-ja.pdf"));
        assert!(landing_links("en").contains(".. _manual: manual/index.html"));
    }
}
