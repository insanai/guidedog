//! Older Typst discussion catalogs: each selected record compiles directly to the
//! `build` folder beside the catalog, as PDF, HTML, or both, with an HTML index of the
//! catalog. Publication is staged, so a compiler failure cannot replace a previously
//! good artifact.

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use gd_discussion::{Catalog, Problem, Record, Result, Target, artifact_name};

const CSS: &str = include_str!("../assets/discussions.css");
const JS: &str = include_str!("../assets/discussions.js");

/// The largest compiled page read back for theming (64 MiB).
const MAX_PAGE_BYTES: u64 = 64 << 20;

/// What a staged file in the build folder starts with.
const STAGE: &str = ".gds-stage-";

/// Builds `selected` records of the Typst catalog for `targets` with the Typst
/// compiler `program` names (the embedded one when `None` or `typst`). Returns the
/// files written, the index last.
pub(crate) fn build(
    catalog: &Catalog,
    selected: &[&Record],
    targets: &[Target],
    program: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    let engine = program.map_or_else(|| "typst".to_string(), |p| p.to_string_lossy().into_owned());
    let Some(backend) = gd_typst::locate(&engine) else {
        return Err(Problem::new(
            "typst.missing",
            "Typst was not found",
            engine,
            "This program has no embedded Typst compiler, and no typst program was found.",
            "Install Typst or pass --typst=/absolute/path/to/typst.",
        )
        .exit(4));
    };
    // Records may import files of the repository the catalog's folder is two levels in.
    let dir = catalog.dir.canonicalize().map_err(|e| io_problem(&catalog.dir, &e))?;
    let root = dir.ancestors().nth(2).unwrap_or(&dir).to_path_buf();
    let outdir = catalog.dir.parent().unwrap_or(&catalog.dir).join("build");
    fs::create_dir_all(&outdir).map_err(|e| io_problem(&outdir, &e))?;
    let mut outputs = Vec::new();
    for record in selected.iter().filter(|r| !r.historical) {
        let name = artifact_name(&catalog.prefix, record);
        for (target, suffix) in [(Target::Pdf, "pdf"), (Target::Html, "html")] {
            if !targets.contains(&target) {
                continue;
            }
            let output = outdir.join(format!("{name}.{suffix}"));
            compile_artifact(&backend, &root, &record.path, &output, target)?;
            outputs.push(output);
        }
    }
    if targets.contains(&Target::Html) {
        let index = outdir.join("index.html");
        write_staged(&index, &theme_html(&index_page(&outdir, catalog), &index)?)?;
        outputs.push(index);
    }
    remove_obsolete(&outdir, catalog)?;
    Ok(outputs)
}

/// Compiles one record to `output` through a staged file beside it.
fn compile_artifact(
    backend: &gd_typst::Backend,
    root: &Path,
    source: &Path,
    output: &Path,
    target: Target,
) -> Result<()> {
    let suffix = if target == Target::Html { "html" } else { "pdf" };
    let stage = Stage::beside(output, suffix);
    let options = gd_typst::Options {
        target: if target == Target::Html {
            gd_typst::Target::Html
        } else {
            gd_typst::Target::Pdf
        },
        ..gd_typst::Options::default()
    };
    let source = source.canonicalize().map_err(|e| io_problem(source, &e))?;
    let shown = source.display().to_string();
    if let Err(error) = gd_typst::compile_file(backend, &source, root, &stage.path, &options) {
        return Err(compile_problem(&error, &shown));
    }
    if target == Target::Html {
        let page = read_page(&stage.path)?;
        let themed = theme_html(&page, output)?;
        fs::write(&stage.path, themed).map_err(|e| io_problem(&stage.path, &e))?;
    }
    let size = fs::metadata(&stage.path).map_err(|e| io_problem(&stage.path, &e))?.len();
    if size == 0 {
        return Err(Problem::new(
            "typst.empty",
            "Typst produced no artifact",
            shown,
            "The compiler returned an empty artifact.",
            "Check the selected Typst executable.",
        )
        .exit(4));
    }
    stage.commit(output)
}

fn compile_problem(error: &gd_typst::Error, source: &str) -> Problem {
    match error {
        gd_typst::Error::Launch { program, message } => Problem::new(
            "typst.launch",
            "Cannot start Typst",
            program.as_str(),
            message.as_str(),
            "Check --typst points to an executable compiler.",
        )
        .exit(4),
        gd_typst::Error::Compile { detail, .. } => {
            let mut detail = detail.clone();
            if detail.len() > 8192 {
                let cut = gd_report::floor_boundary(&detail, 8192);
                detail.truncate(cut);
                detail.push_str("\n[diagnostics truncated]");
            }
            Problem::new(
                "typst.compile",
                "Typst could not build this discussion",
                source,
                detail,
                "Fix the reported source location and rebuild. The previous artifact is intact.",
            )
            .exit(4)
        }
        other => {
            let report = other.report();
            Problem::new(
                &*report.code,
                &report.title,
                source,
                &*report.message,
                report.hints.join(" "),
            )
            .exit(4)
        }
    }
}

/// A file written beside its destination and renamed onto it; removed on drop when it
/// was not committed.
struct Stage {
    path: PathBuf,
}

impl Stage {
    fn beside(output: &Path, suffix: &str) -> Stage {
        let dir = output.parent().unwrap_or(Path::new("."));
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let name = format!("{STAGE}{}-{nanos:08x}.{suffix}", std::process::id());
        Stage { path: dir.join(name) }
    }

    fn commit(self, output: &Path) -> Result<()> {
        fs::rename(&self.path, output).map_err(|e| io_problem(output, &e))
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Writes `text` to `path` through a stage.
fn write_staged(path: &Path, text: &str) -> Result<()> {
    let stage = Stage::beside(path, "html");
    fs::write(&stage.path, text).map_err(|e| io_problem(&stage.path, &e))?;
    stage.commit(path)
}

fn read_page(path: &Path) -> Result<String> {
    let size = fs::metadata(path).map_err(|e| io_problem(path, &e))?.len();
    if size > MAX_PAGE_BYTES {
        return Err(Problem::new(
            "typst.size",
            "Typst output is too large",
            path.display().to_string(),
            "The compiled page exceeds the 64 MiB limit.",
            "Split the discussion into smaller records.",
        )
        .exit(3));
    }
    fs::read_to_string(path).map_err(|e| io_problem(path, &e))
}

fn io_problem(path: &Path, error: &std::io::Error) -> Problem {
    Problem::new(
        "gds.io",
        "Cannot access this path",
        path.display().to_string(),
        error.to_string(),
        "Check the path and its permissions, then retry.",
    )
    .exit(4)
}

/// Deletes the pages and PDFs of records the catalog no longer has, and stages an
/// interrupted build left behind. It touches only files named as this module names them.
fn remove_obsolete(outdir: &Path, catalog: &Catalog) -> Result<()> {
    let entries = fs::read_dir(outdir).map_err(|e| io_problem(outdir, &e))?;
    let lead = format!("{}-", catalog.prefix);
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let stem = name.strip_suffix(".pdf").or_else(|| name.strip_suffix(".html"));
        let artifact = name.starts_with(&lead) && stem.is_some();
        if !name.starts_with(STAGE) && !artifact {
            continue;
        }
        let current = stem.is_some_and(|stem| {
            catalog
                .records
                .iter()
                .any(|r| !r.historical && artifact_name(&catalog.prefix, r) == stem)
        });
        if artifact && current {
            continue;
        }
        fs::remove_file(entry.path()).map_err(|e| io_problem(&entry.path(), &e))?;
    }
    Ok(())
}

/// The catalog's index page: every record, drafts and historical ones included, with
/// links only to the artifacts that exist, so a selected build has no broken links.
fn index_page(outdir: &Path, catalog: &Catalog) -> String {
    let mut page = String::from(
        "<!doctype html><html lang=\"en\"><head>\n<meta charset=\"utf-8\"><meta \
         name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>Guidedog \
         Discussions</title></head><body>\n<h1>Guidedog Discussions</h1>\n<p>Proposals, \
         decisions, and their reasoning for Guidedog and the Guidedoc library.</p>\n<p>Read \
         each record's status before treating a proposal as an agreed design.</p>\n<div \
         class=\"discussion-list\">",
    );
    for record in &catalog.records {
        index_card(&mut page, outdir, catalog, record);
    }
    page.push_str("</div></body></html>");
    page
}

fn index_card(page: &mut String, outdir: &Path, catalog: &Catalog, r: &Record) {
    let prefix = catalog.prefix.to_string().to_uppercase();
    let _ = write!(
        page,
        "<section class=\"discussion-card\"><p class=\"eyebrow\">{} {} | {} <span class=\"chip \
         state-{}\">{}</span></p><h2>{}</h2><p class=\"labels\">{}</p>",
        escape(&prefix),
        escape(&r.number),
        escape(&r.kind),
        escape(&r.state),
        escape(&r.state),
        escape(&r.title),
        escape(&r.labels.join(", "))
    );
    if r.historical {
        page.push_str("<p>Historical source retained in records; not built.</p>");
    } else {
        let mut available = false;
        for (suffix, label) in [("html", "Read discussion"), ("pdf", "Download PDF")] {
            let name = format!("{}.{suffix}", artifact_name(&catalog.prefix, r));
            if outdir.join(&name).exists() {
                let _ = write!(
                    page,
                    "<a class=\"artifact-link\" href=\"{}\">{label}</a> ",
                    escape(&name)
                );
                available = true;
            }
        }
        if !available {
            page.push_str("<p>This record has not been built yet.</p>");
        }
    }
    page.push_str("</section>");
}

/// `text` with the HTML special characters escaped.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// Adds stable IDs to the `h1` and `h2` headings Typst's HTML export writes (keeping
/// existing ones), and returns the body with the navigation links to them.
fn heading_navigation(body: &str) -> (String, String) {
    let (mut out, mut nav) = (String::with_capacity(body.len()), String::new());
    let mut rest = body;
    let mut count = 0;
    while let Some(at) = rest.find("<h") {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 2..];
        let level = tail.as_bytes().first().copied();
        if level != Some(b'1') && level != Some(b'2') {
            out.push_str("<h");
            rest = tail;
            continue;
        }
        let ending = if level == Some(b'1') { "</h1>" } else { "</h2>" };
        let Some((tag, content)) = tail.split_once('>') else { break };
        let Some((title, after)) = content.split_once(ending) else { break };
        let id = match tag.split_once("id=\"") {
            Some((_, id_rest)) => id_rest.split('"').next().unwrap_or_default().to_string(),
            None => format!("gds-section-{count}"),
        };
        let tag =
            if tag.contains("id=\"") { tag.to_string() } else { format!("{tag} id=\"{id}\"") };
        count += 1;
        let _ = write!(out, "<h{tag}>{title}{ending}");
        let class = if level == Some(b'1') { "nav-title" } else { "nav-section" };
        let _ = write!(
            nav,
            "<a class=\"{class}\" href=\"#{}\">{}</a>",
            escape(&id),
            strip_tags(title)
        );
        rest = after;
    }
    out.push_str(rest);
    (out, nav)
}

fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// Themes a standalone Typst HTML page (`source`) for `output`: the discussions'
/// styles, navigation, and a PDF link when the PDF beside it exists.
fn theme_html(source: &str, output: &Path) -> Result<String> {
    let shape = source.split_once("</head>").and_then(|(head, rest)| {
        let (_, tail) = rest.split_once("<body>")?;
        let (body, _) = tail.split_once("</body>")?;
        Some((head, body))
    });
    let Some((head, body)) = shape else {
        return Err(Problem::new(
            "typst.html-shape",
            "Unexpected Typst HTML document",
            output.display().to_string(),
            "The compiler output does not contain the expected standalone document structure.",
            "Use the supported Typst 0.15.1 compiler; the previous HTML is unchanged.",
        )
        .exit(4));
    };
    let (article, navigation) = heading_navigation(body);
    let article = article
        .replace("<table", "<div class=\"table-wrap\"><table")
        .replace("</table>", "</table></div>");
    let mut page = String::with_capacity(source.len() + CSS.len() + JS.len() + 2048);
    let _ = write!(page, "{head}<style>{CSS}</style></head><body>{HEADER}{navigation}{MAIN}");
    page.push_str(&article);
    page.push_str(FOOTER);
    let pdf = output.with_extension("pdf");
    if pdf.exists() {
        let name = pdf.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let _ = write!(page, "<a href=\"{}\">Download PDF</a>", escape(&name));
    }
    let _ = write!(page, "</div></aside></div><script>{JS}</script></body></html>");
    Ok(page)
}

const HEADER: &str = "<a class=\"skip\" href=\"#content\">Skip to content</a>
<header class=\"topbar\"><a class=\"brand\" href=\"index.html\">
<span class=\"brand-mark\" aria-hidden=\"true\">g.</span>guidedog</a>
<small>Discussions &amp; decisions</small>
<button class=\"theme-toggle\" type=\"button\">Theme: auto</button></header>
<div class=\"layout\"><aside class=\"sidebar\" aria-label=\"Document navigation\">
<a href=\"index.html\">All discussions</a>
<details open><summary>IN THIS DOCUMENT</summary>
<input id=\"section-filter\" type=\"search\" placeholder=\"Find a section...\" \
aria-label=\"Find a section\">
<nav aria-label=\"Table of contents\">";

const MAIN: &str = "</nav></details></aside><main id=\"content\" tabindex=\"-1\">
<p class=\"breadcrumb\">GUIDEDOG / ENGINEERING / DISCUSSIONS</p>";

const FOOTER: &str = "<footer><p>Guidedog Discussions. Written to be understood,
    built to be maintained.</p>
</footer></main><aside class=\"rail\" aria-label=\"About these discussions\"><div \
class=\"rail-inner\">
<strong>DESIGN IN THE OPEN</strong>
<p>Read the proposal.<br>Understand the tradeoffs.<br>Follow the decision.</p>
<p>Each discussion records its own status. A proposal is not an implementation.</p>";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_get_stable_ids_and_navigation() {
        let (body, nav) =
            heading_navigation("<h1>Title</h1><p>x</p><h2 id=\"own\">A <em>b</em></h2><h3>c</h3>");
        assert_eq!(
            body,
            "<h1 id=\"gds-section-0\">Title</h1><p>x</p><h2 id=\"own\">A <em>b</em></h2><h3>c</h3>"
        );
        assert_eq!(
            nav,
            "<a class=\"nav-title\" href=\"#gds-section-0\">Title</a><a class=\"nav-section\" \
             href=\"#own\">A b</a>"
        );
    }

    #[test]
    fn pages_are_themed_or_refused() {
        let page =
            "<html><head><title>t</title></head><body><h1>T</h1><table></table></body></html>";
        let themed =
            theme_html(page, Path::new("/nonexistent/gds-0001-x.html")).unwrap_or_default();
        assert!(themed.contains("<style>") && themed.contains("table-wrap"), "{themed}");
        assert!(themed.contains("href=\"#gds-section-0\""));
        let refused = theme_html("<p>no document</p>", Path::new("x.html"));
        assert!(refused.is_err_and(|p| p.code == "typst.html-shape"));
        assert_eq!(escape("<a & \"b\">"), "&lt;a &amp; &quot;b&quot;&gt;");
    }

    /// A Typst catalog's records compile with the embedded compiler, beside an index
    /// that links only what exists; files of records no longer in it are removed.
    #[test]
    fn typst_catalogs_compile_record_by_record() {
        if !gd_typst::EMBEDDED {
            return;
        }
        let root = std::env::temp_dir().join(format!("gd-cli-legacy-{}", std::process::id()));
        let dir = root.join("docs").join("gds");
        let records = dir.join("records");
        let _ = fs::create_dir_all(&records);
        let record = records.join("0001-cache.typ");
        let _ = fs::write(&record, "= Cache policy\n\nKeep it small.\n");
        let outdir = root.join("docs").join("build");
        let _ = fs::create_dir_all(&outdir);
        let _ = fs::write(outdir.join("gds-0009-gone.pdf"), "old");
        let prefix = gd_discussion::Prefix::new("gds").unwrap_or_else(|p| panic!("{p}"));
        let r = Record {
            number: "0001".into(),
            title: "Cache".into(),
            path: record,
            ..Record::default()
        };
        let catalog =
            Catalog { dir: dir.clone(), records: vec![r.clone()], ..Catalog::new(prefix) };
        let written = build(&catalog, &[&r], &[Target::Html, Target::Pdf], None);
        let written = written.unwrap_or_else(|p| panic!("{p}"));
        let names: Vec<String> = written
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["gds-0001-cache.pdf", "gds-0001-cache.html", "index.html"]);
        let index = fs::read_to_string(outdir.join("index.html")).unwrap_or_default();
        assert!(index.contains("href=\"gds-0001-cache.html\"") && index.contains("Download PDF"));
        let page = fs::read_to_string(outdir.join("gds-0001-cache.html")).unwrap_or_default();
        assert!(page.contains("gds-0001-cache.pdf") && page.contains("nav-title"), "{page}");
        assert!(!outdir.join("gds-0009-gone.pdf").exists());
        let _ = fs::remove_dir_all(&root);
    }
}
