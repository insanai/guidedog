//! `BuildView`: everything a builder reads once the documents are read and resolved,
//! borrowed for the writing stage; and `OutputBuilder`, the trait each builder crate
//! implements.

use std::path::{Path, PathBuf};

use gd_conf::Config;
use gd_env::{
    BuildOptions, BuilderKind, Digest, DocId, DocInfo, Environment, LabelKind, ObjectStore,
    PluginRun, UiText,
};
use gd_host::{Budget, Problem};
use gd_outputs::Generation;
use gd_report::Report;
use guidedoc::{Limits, Policy, Registry};

use crate::intersphinx::Intersphinx;
use crate::tables::Tables;

/// The build's dates, made once from `BuildOptions::now`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dates {
    /// `|today|`: conf.toml's `today`, or the date in `today_fmt`.
    pub today: String,
    /// The date in `html_last_updated_fmt`; empty when it is unset.
    pub last_updated: String,
    /// The build's date as `YYYY-MM-DD`.
    pub date: String,
}

/// What every builder reads, borrowed from the build for the writing stage. It is
/// `Sync`: pages render on worker threads that share one view.
#[derive(Clone, Copy)]
pub struct BuildView<'b> {
    /// The build's own copy of the configuration (confined when untrusted).
    pub config: &'b Config,
    /// The build's options.
    pub options: &'b BuildOptions,
    /// The builder writing.
    pub kind: BuilderKind,
    /// What reading learned, every document sorted by docname.
    pub env: &'b Environment,
    /// The project tables.
    pub tables: &'b Tables,
    /// Other projects' inventories.
    pub intersphinx: &'b Intersphinx,
    /// The cached objects documents are loaded from.
    pub objects: &'b ObjectStore,
    /// The readers and renderers.
    pub registry: &'b Registry,
    /// What content the build lets through (raw, unsafe URLs, math).
    pub policy: &'b Policy,
    /// The limits every session reads and renders with.
    pub limits: &'b Limits,
    /// Guidedog's interface words in the build's language.
    pub ui: &'b dyn UiText,
    /// The build's dates.
    pub dates: &'b Dates,
    /// The build plugins (HTML hooks and assets).
    pub plugins: &'b PluginRun,
    /// The budget files and sessions are held within.
    pub budget: &'b Budget,
    /// The canonical folders the build may read (`gd_conf::readable_roots`).
    pub roots: &'b [PathBuf],
    /// `gd_env::output_settings`: what changes every output of this builder.
    pub settings: Digest,
}

impl std::fmt::Debug for BuildView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuildView")
            .field("kind", &self.kind)
            .field("documents", &self.env.docs.len())
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

/// A builder: writes one kind of output into a generation. gd-site implements it for
/// html, dirhtml and singlehtml, gd-book for pdf and text, gd-intl for gettext;
/// gd-project maps each `BuilderKind` to one in a single `match`.
pub trait OutputBuilder: Sync {
    /// The output it writes.
    fn kind(&self) -> BuilderKind;

    /// Writes every unit of the output into `generation`, reusing unchanged ones
    /// (`Generation::reuse`), adding what writing reports to `reports`. Fails with the
    /// problem that stops the build; pages that fail are counted in the generation.
    fn write(
        &self,
        view: &BuildView<'_>,
        generation: &mut Generation,
        reports: &mut Vec<Report>,
    ) -> Result<(), Problem>;
}

impl BuildView<'_> {
    /// The id of the document named `docname`.
    pub fn doc_named(&self, docname: &str) -> Option<DocId> {
        self.tables.doc_named(docname)
    }

    /// The document `id` names. Panics for an id of another environment, an internal
    /// invariant: ids come from the tables made from this one.
    pub fn doc(&self, id: DocId) -> &DocInfo {
        self.env.doc(id)
    }

    /// A document's anchor namespace in the book and singlehtml (`install-guide`).
    pub fn doc_key(&self, id: DocId) -> &str {
        self.tables.keys.get(id.index()).map_or("", String::as_str)
    }

    /// A document's output path below the HTML output folder: `<docname>.html`,
    /// `<docname>/index.html` in dirhtml, the root document's page in singlehtml.
    pub fn page_name(&self, docname: &str) -> String {
        let suffix = &self.config.html.file_suffix;
        match self.kind {
            BuilderKind::Singlehtml => format!("{}{suffix}", self.config.root_doc),
            BuilderKind::Dirhtml if docname != "index" && !docname.ends_with("/index") => {
                format!("{docname}/index{suffix}")
            }
            _ => format!("{docname}{suffix}"),
        }
    }

    /// The URL from one document's page to another's, with an optional anchor. From
    /// `None` it is the URL from a special page at the output root (genindex.html).
    pub fn page_url(&self, from: Option<DocId>, to: DocId, anchor: &str) -> String {
        let fragment = self.page_anchor(to, anchor);
        let to_page = self.page_name(&self.doc(to).docname);
        let mut target = to_page.clone();
        let link_suffix = &self.config.html.link_suffix;
        if !link_suffix.is_empty() && self.kind == BuilderKind::Html {
            let stem = target.strip_suffix(self.config.html.file_suffix.as_str());
            target = format!("{}{link_suffix}", stem.unwrap_or(&target));
        }
        let here = from.map_or(String::new(), |f| self.page_name(&self.doc(f).docname));
        let mut url = relative_url(&here, &target);
        if self.kind == BuilderKind::Dirhtml {
            url = url.strip_suffix("index.html").unwrap_or(&url).to_string();
        }
        if from == Some(to) || to_page == here {
            url.clear();
        } else if url.is_empty() {
            url.push_str("./"); // dirhtml: the folder a special page stands in.
        }
        if !fragment.is_empty() {
            return format!("{url}#{fragment}");
        }
        if url.is_empty() { "#".to_string() } else { url }
    }

    /// The URL from a document's page (or the output root) to a generated page.
    pub fn generated_url(&self, from: Option<DocId>, file: &str) -> String {
        match from {
            None => file.to_string(),
            Some(f) => relative_url(&self.page_name(&self.doc(f).docname), file),
        }
    }

    /// The fragment that reaches an anchor of a document in this output: the anchor
    /// itself, or in singlehtml the document's key before it.
    pub fn page_anchor(&self, doc: DocId, anchor: &str) -> String {
        if self.kind != BuilderKind::Singlehtml {
            return anchor.to_string();
        }
        if anchor.is_empty() {
            return format!("document-{}", self.doc_key(doc));
        }
        format!("{}-{anchor}", self.doc_key(doc))
    }

    /// A numbered item's number by its anchor (`"3"`, `"1.2"`); empty when it has none.
    pub fn item_number(&self, doc: DocId, anchor: &str) -> String {
        item_number(self.env, self.tables, doc, anchor).to_string()
    }

    /// The number a section's label shows, as Sphinx's `get_fignumber` finds it: the
    /// section's own, or, when it has none, its document title's; empty for none.
    pub fn section_number(&self, doc: DocId, anchor: &str) -> &str {
        let Some(numbers) = self.tables.secnumbers.get(doc.index()) else { return "" };
        let sections = &self.doc(doc).sections;
        let own = sections
            .iter()
            .zip(numbers)
            .find(|(s, n)| !n.is_empty() && s.ids.iter().any(|a| a == anchor));
        match own {
            Some((_, n)) => n,
            None => numbers.first().map_or("", String::as_str),
        }
    }

    /// The `numfig_format` text for a kind of numbered item (`"Fig. %s"`); `"%s"` when
    /// the format is empty.
    pub fn number_template(&self, kind: LabelKind) -> &str {
        let name = match kind {
            LabelKind::Table => "table",
            LabelKind::Code => "code-block",
            LabelKind::Section => "section",
            // Equations are numbered by math_numfig; anything else is formatted as a
            // figure, as Sphinx does.
            LabelKind::Figure | LabelKind::Other | LabelKind::Equation => "figure",
        };
        match self.config.numfig_format.get(name) {
            Some(format) if !format.is_empty() => format,
            _ => "%s",
        }
    }

    /// Finds the file an image a document shows names: relative to the document, or to
    /// the source folder when `uri` starts with `/`; `name.*` picks the best existing
    /// candidate for this output. The file must lie in a readable root; `None` when it
    /// does not. An image in the source folder keeps its path below `_images`, one in
    /// an include root only its name, as Sphinx keeps images from outside.
    pub fn image_file(&self, docname: &str, uri: &str) -> Option<ImageFile> {
        let dir = dir_of(docname);
        let mut rel = match uri.strip_prefix('/') {
            Some(rest) => rest.to_string(),
            None if dir.is_empty() => uri.to_string(),
            None => format!("{dir}/{uri}"),
        };
        let mut path = gd_conf::clean_path(&self.config.source_dir.join(&rel));
        if let Some(stem) = rel.strip_suffix('*').filter(|s| s.ends_with('.')).map(str::to_string)
        {
            let types: &[&str] =
                if self.kind == BuilderKind::Pdf { &BOOK_IMAGE_TYPES } else { &PAGE_IMAGE_TYPES };
            let base = gd_conf::clean_path(&self.config.source_dir.join(&stem));
            let found = types.iter().find(|ext| append_ext(&base, ext).is_file());
            if let Some(ext) = found {
                path = append_ext(&base, ext);
                rel = format!("{stem}{ext}");
            }
        }
        let roots = gd_conf::readable_roots(self.config);
        let root = gd_host::containing_root(&roots, &path)?;
        // A link may not lead out of the roots; a missing file is reported as missing.
        if path.is_file() && !gd_host::confined(self.roots, &path) {
            return None;
        }
        let target = if root == roots[0].as_path() {
            format!("_images/{}", normalize_doc("", &rel))
        } else {
            let name = path.file_name().map_or(String::new(), |n| n.to_string_lossy().into());
            format!("_images/{name}")
        };
        Some(ImageFile { path, target })
    }
}

/// Where an image a document shows comes from, and where the output keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageFile {
    /// The file.
    pub path: PathBuf,
    /// Its path below the output folder: `_images/...`.
    pub target: String,
}

/// The order in which an image written as `name.*` picks its file in pages: vector
/// first, as Sphinx's HTML builders list `supported_image_types`.
const PAGE_IMAGE_TYPES: [&str; 5] = ["svg", "png", "gif", "jpg", "jpeg"];

/// The same in the book: what Typst reads.
const BOOK_IMAGE_TYPES: [&str; 5] = ["svg", "png", "jpg", "jpeg", "gif"];

fn append_ext(base: &Path, ext: &str) -> PathBuf {
    let mut text = base.as_os_str().to_owned();
    text.push(ext);
    PathBuf::from(text)
}

/// A numbered item's number by any of its ids, borrowed from the tables.
pub(crate) fn item_number<'t>(
    env: &Environment,
    tables: &'t Tables,
    doc: DocId,
    anchor: &str,
) -> &'t str {
    let Some(numbers) = tables.fignumbers.get(doc.index()) else { return "" };
    env.doc(doc)
        .numbered
        .iter()
        .zip(numbers)
        .find(|(item, _)| item.ids.iter().any(|a| a == anchor))
        .map_or("", |(_, n)| n.as_str())
}

/// The shortest relative URL from one page to another (both below the output root,
/// `/` separators).
pub fn relative_url(from: &str, to: &str) -> String {
    let from_dir = dir_of(from);
    let a: Vec<&str> =
        if from_dir.is_empty() { Vec::new() } else { from_dir.split('/').collect() };
    let t: Vec<&str> = to.split('/').collect();
    let mut common = 0;
    while common < a.len() && common + 1 < t.len() && a[common] == t[common] {
        common += 1;
    }
    let mut parts: Vec<&str> = vec![".."; a.len() - common];
    parts.extend(&t[common..]);
    parts.join("/")
}

/// The folder part of a docname or page path; empty at the top.
pub(crate) fn dir_of(name: &str) -> &str {
    name.rfind('/').map_or("", |i| &name[..i])
}

/// Joins a relative docname onto a folder and resolves `.` and `..`.
pub fn normalize_doc(dir: &str, path: &str) -> String {
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for p in path.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(p),
        }
    }
    parts.join("/")
}

/// A docname written in a document in folder `base`, its `.rst`/`.md` suffix dropped;
/// `/name` is relative to the source folder.
pub fn doc_target(base: &str, target: &str) -> String {
    let t = target.strip_suffix(".rst").or_else(|| target.strip_suffix(".md")).unwrap_or(target);
    file_target(base, t)
}

/// A file path written in a document in folder `base`, its suffix kept.
pub fn file_target(base: &str, target: &str) -> String {
    match target.strip_prefix('/') {
        Some(rest) => normalize_doc("", rest),
        None => normalize_doc(base, target),
    }
}

/// The page a toctree names with `genindex`, `modindex` or `search` (Sphinx's virtual
/// documents), or `None` for any other name.
pub fn generated_page(docname: &str) -> Option<&'static str> {
    match docname {
        "genindex" => Some("genindex.html"),
        "modindex" => Some("py-modindex.html"),
        "search" => Some("search.html"),
        _ => None,
    }
}

/// The English title of a generated page (its toctree entry and its label).
pub(crate) fn generated_title(docname: &str) -> &'static str {
    match docname {
        "genindex" => "Index",
        "modindex" => "Module Index",
        _ => "Search Page",
    }
}

/// A document's anchor namespace in outputs that hold several documents: its name as
/// an id, `install-guide` for `install/guide`.
pub fn chapter_key(docname: &str) -> String {
    let mut out = String::with_capacity(docname.len() + 8);
    guidedoc::write_id(&mut out, docname);
    out
}

/// Fills a numfig format as Sphinx does: `{number}` and `{name}` in the new style, or
/// a single `%s` (with `%%` for a percent sign) in the old. `None` for a format that
/// names the caption of an item without one, or has no single place for the number.
pub fn format_number(template: &str, number: &str, name: &str) -> Option<String> {
    if template.contains("{name}") || template.contains("number") {
        if name.is_empty() && template.contains("{name}") {
            return None;
        }
        return Some(template.replace("{number}", number).replace("{name}", name));
    }
    let mut out = String::with_capacity(template.len() + number.len());
    let mut places = 0;
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('s') => {
                places += 1;
                out.push_str(number);
            }
            _ => return None,
        }
    }
    (places == 1).then_some(out)
}

/// Replaces each `%s` with the next value, as Python's `%` does (for translated
/// interface words such as `"(in %s %s)"`).
pub fn python_format(format: &str, values: &[&str]) -> String {
    let mut out = String::with_capacity(format.len() + 16);
    let mut rest = format;
    for value in values {
        let Some(at) = rest.find("%s") else { break };
        out.push_str(&rest[..at]);
        out.push_str(value);
        rest = &rest[at + 2..];
    }
    out.push_str(rest);
    out
}
