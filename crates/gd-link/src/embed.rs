//! Pictures embedded in one converted document.
//!
//! A build draws a graph, a Typst diagram and a score into files and copies them
//! beside the pages. A single conversion (`guidedog convert`) has no folder to copy
//! to, so this pass draws them the same way and writes each drawing into the document
//! itself, as an image whose address is a `data:` URL. The page or fragment then needs
//! no file.
//!
//! The drawing is confined like an `--untrusted` build: a graph opens no file, a score
//! reads no file, and a Typst diagram reads only below the folder the caller names
//! (nothing, when it names none) and downloads no package when the caller says so.
//! The count and the total size of the pictures are bounded. A picture that is not
//! drawn, or would pass a bound, stays a code block, with a finding.

use std::cell::{Cell, OnceCell, RefCell};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use gd_graphdog::{Context, Engine, FileDecision, Format};
use guidedoc::{
    Arg, AttributeKey, Builder, Document, Fail, LinkInfo, LinkKind, Message, NodeId, NodeKind,
    PassOutcome, Severity, Status, arg_text,
};

use crate::page::flag;
use crate::rewrite::{Action, Rewriter, Rule, rewrite_document};

/// A graph that was not drawn; `{0}` says why.
pub const EMBED_GRAPH: Message = Message {
    code: "embed.graphviz",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "GRAPH NOT DRAWN",
    message: "{0}",
    hint: "Fix the DOT source. The graph is shown as code.",
};

/// A Typst diagram that was not drawn; `{0}` says why.
pub const EMBED_TYPST: Message = Message {
    code: "embed.typst-diagram",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "TYPST DIAGRAM NOT DRAWN",
    message: "{0}",
    hint: "Check the Typst source and its imports; use one page. The source is shown as code.",
};

/// A score that was not engraved; `{0}` says why.
pub const EMBED_MUSIC: Message = Message {
    code: "embed.lilypond",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "MUSIC NOT ENGRAVED",
    message: "{0}",
    hint: "Check the supported notation, and write the music in the directive. The source \
           is shown as code.",
};

/// A picture left out because a bound was reached; `{0}` says which.
pub const EMBED_LIMIT: Message = Message {
    code: "embed.limit",
    severity: Severity::Warning,
    status: Status::Limit,
    title: "PICTURE LIMIT REACHED",
    message: "This picture is not embedded because {0}. Its source is shown as code.",
    hint: "Raise --max-pictures or --max-picture-kib, or build a project, which writes \
           pictures as files.",
};

/// The longest Typst diagram source, as in a build.
const MAX_TYPST_SOURCE: usize = 1 << 20;
/// How many files one Typst diagram may look at, as in a build.
const MAX_TYPST_FILES: usize = 4096;
const PAGE_SETUP: &str = "#set page(width: auto, height: auto, margin: 0pt)\n";

/// How the pictures of a document are drawn and bounded. `Default` reads no file, may
/// download Typst packages, and embeds at most 32 pictures of 4 MiB in total.
#[derive(Debug, Clone)]
pub struct EmbedOptions {
    /// The folder a Typst diagram may read below; `None`: it reads no file.
    pub root: Option<PathBuf>,
    /// The document is Markdown: a `dot` or `graphviz` fence is a graph, as in a build.
    pub markdown: bool,
    /// Typst uses only packages already on disk.
    pub offline: bool,
    /// Typst and the engraver ignore system fonts.
    pub embedded_fonts_only: bool,
    /// Extra font folders, searched first.
    pub font_paths: Vec<PathBuf>,
    /// The Typst package cache; `None` is the system cache.
    pub package_cache: Option<PathBuf>,
    /// The most pictures embedded in one document.
    pub max_pictures: usize,
    /// The most bytes all pictures of one document may have, before encoding.
    pub max_bytes: usize,
}

impl Default for EmbedOptions {
    fn default() -> Self {
        EmbedOptions {
            root: None,
            markdown: false,
            offline: false,
            embedded_fonts_only: false,
            font_paths: Vec::new(),
            package_cache: None,
            max_pictures: 32,
            max_bytes: 4 << 20,
        }
    }
}

/// What one thread draws pictures with: its Graphviz context and its engraver, each
/// started on first use. Reuse it between documents.
#[derive(Default)]
pub struct Embedder {
    graphs: OnceCell<Option<Context>>,
    engraver: RefCell<gd_engrave::Engraver>,
    music: OnceCell<crate::MusicSource>,
    pictures: Cell<usize>,
    bytes: Cell<usize>,
    /// A drawing was refused because it would pass the size bound.
    over: Cell<bool>,
}

impl std::fmt::Debug for Embedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Embedder")
            .field("pictures", &self.pictures.get())
            .field("bytes", &self.bytes.get())
            .finish_non_exhaustive()
    }
}

/// A picture as drawn: its bytes and its media type.
struct Picture {
    data: Vec<u8>,
    media: &'static str,
}

/// Why a picture was not drawn: the finding and its text.
type Refused = (Message, String);

impl Embedder {
    /// Makes an embedder that has drawn nothing yet.
    pub fn new() -> Embedder {
        Embedder::default()
    }

    /// The body of a pass: rewrites `doc` into `b` with every graph, Typst diagram and
    /// score drawn and embedded as a `data:` URL image, in a figure when it has a
    /// caption. The bounds of `options` count from the start of this document.
    pub fn embed(
        &self,
        doc: Document<'_>,
        b: &mut Builder<'_>,
        options: &EmbedOptions,
    ) -> PassOutcome {
        self.pictures.set(0);
        self.bytes.set(0);
        let mut rule = Embed { tools: self, options };
        let wanted = (0..doc.node_count()).any(|i| rule.visits(doc, NodeId::from_index(i)));
        if !wanted {
            return PassOutcome::Unchanged;
        }
        rewrite_document(doc, b, &mut rule)
    }

    /// How many pictures the last document embedded, and their bytes before encoding.
    pub fn embedded(&self) -> (usize, usize) {
        (self.pictures.get(), self.bytes.get())
    }
}

/// The kind of picture a code block asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Graph,
    Typst,
    Music,
}

impl Kind {
    fn class(self) -> &'static str {
        match self {
            Kind::Graph => "graphviz",
            Kind::Typst => "typst-diagram",
            Kind::Music => "lilypond-music",
        }
    }
}

struct Embed<'a> {
    tools: &'a Embedder,
    options: &'a EmbedOptions,
}

impl Embed<'_> {
    fn kind(&self, doc: Document<'_>, id: NodeId) -> Option<Kind> {
        if doc.kind(id) != NodeKind::CodeBlock {
            return None;
        }
        if flag(doc, id, "typst-diagram") {
            return Some(Kind::Typst);
        }
        if flag(doc, id, "lilypond") {
            return Some(Kind::Music);
        }
        let fence = self.options.markdown
            && matches!(doc.attribute(id, AttributeKey::Language), Some("dot" | "graphviz"));
        (flag(doc, id, "graphviz") || fence).then_some(Kind::Graph)
    }

    /// Draws the picture of `id` within the bounds.
    fn draw(&self, doc: Document<'_>, id: NodeId, kind: Kind) -> Result<Picture, Refused> {
        if self.tools.pictures.get() >= self.options.max_pictures {
            let why = format!("the document has more than {} pictures", self.options.max_pictures);
            return Err((EMBED_LIMIT, why));
        }
        let left = self.options.max_bytes.saturating_sub(self.tools.bytes.get());
        self.tools.over.set(false);
        let drawn = match kind {
            Kind::Graph => self.graph(doc, id, left).map_err(|m| (EMBED_GRAPH, m)),
            Kind::Typst => self.typst(doc, id, left).map_err(|m| (EMBED_TYPST, m)),
            Kind::Music => self.music(doc, id).map_err(|m| (EMBED_MUSIC, m)),
        };
        let over = self.tools.over.get() || drawn.as_ref().is_ok_and(|p| p.data.len() > left);
        if over {
            let kib = self.options.max_bytes >> 10;
            return Err((EMBED_LIMIT, format!("the pictures together pass {kib} KiB")));
        }
        let picture = drawn?;
        self.tools.pictures.set(self.tools.pictures.get() + 1);
        self.tools.bytes.set(self.tools.bytes.get() + picture.data.len());
        Ok(picture)
    }

    /// Whether a drawing of `bytes` fits in what is `left` of the size bound; a
    /// refusal is remembered, so the finding names the bound and not the tool.
    fn fits(&self, bytes: usize, left: usize) -> bool {
        let fits = bytes <= left;
        self.tools.over.set(self.tools.over.get() || !fits);
        fits
    }

    /// Lays a graph out with Graphviz. The graph opens no file.
    fn graph(&self, doc: Document<'_>, id: NodeId, left: usize) -> Result<Picture, String> {
        let layout = doc.custom_attribute(id, "layout").unwrap_or("");
        let engine = if layout.is_empty() { Some(Engine::Dot) } else { Engine::from_name(layout) };
        let Some(engine) = engine else {
            return Err(format!("Graphviz has no layout engine named \"{layout}\"."));
        };
        let context = self.tools.graphs.get_or_init(|| Context::new().ok());
        let Some(context) = context else {
            return Err("Graphviz could not load its layout plugins.".to_string());
        };
        let mut named = false;
        let mut decide = |_: &str, _: &str| {
            named = true;
            FileDecision::Refuse
        };
        let mut reserve = |bytes: usize| self.fits(bytes, left);
        let options = gd_graphdog::Options {
            engine,
            format: Format::Svg,
            attributes: &[],
            files: Some(&mut decide),
            reserve: Some(&mut reserve),
        };
        match context.render(doc.node_text(id), options) {
            Ok(rendered) => Ok(Picture { data: rendered.output, media: "image/svg+xml" }),
            Err(_) if named => {
                Err("An embedded graph reads no file; leave the attribute out.".to_string())
            }
            Err(e) if e.line > 0 => Err(format!("{} (line {} of the graph)", e.message, e.line)),
            Err(e) => Err(e.message),
        }
    }

    /// Compiles a Typst diagram with the embedded compiler, confined to the root.
    fn typst(&self, doc: Document<'_>, id: NodeId, left: usize) -> Result<Picture, String> {
        let text = doc.node_text(id);
        if text.len() > MAX_TYPST_SOURCE {
            return Err("The Typst diagram exceeds the 1 MiB source limit.".to_string());
        }
        let (target, media) = match doc.custom_attribute(id, "format").unwrap_or("svg") {
            "svg" => (gd_typst::Target::Svg, "image/svg+xml"),
            "png" => (gd_typst::Target::Png, "image/png"),
            other => return Err(format!("Unknown diagram format `{other}`; use svg or png.")),
        };
        let o = self.options;
        let looked = AtomicUsize::new(0);
        let observe = |_: &Path| {
            o.root.is_some() && looked.fetch_add(1, Ordering::Relaxed) < MAX_TYPST_FILES
        };
        let reserve = |bytes: usize| self.fits(bytes, left);
        let root = o.root.clone().unwrap_or_else(|| PathBuf::from("."));
        let options = gd_typst::Options {
            target,
            observe_file: Some(&observe),
            offline: o.offline,
            package_cache: o.package_cache.clone(),
            embedded_fonts_only: o.embedded_fonts_only,
            font_paths: o.font_paths.clone(),
            read_roots: o.root.iter().cloned().collect(),
            reserve: Some(&reserve),
            ..Default::default()
        };
        let mut source = String::with_capacity(PAGE_SETUP.len() + text.len());
        source.push_str(PAGE_SETUP);
        source.push_str(text);
        match gd_typst::compile_bytes(&source, &root, &options) {
            Ok((data, _)) => Ok(Picture { data, media }),
            Err(error) => Err(typst_failure(&error)),
        }
    }

    /// Engraves a score written in the directive. A score file is not read.
    fn music(&self, doc: Document<'_>, id: NodeId) -> Result<Picture, String> {
        if doc.custom_attribute(id, guidedoc::SCORE_FILE_ATTRIBUTE).is_some() {
            return Err("An embedded score reads no music file.".to_string());
        }
        if doc.custom_attribute(id, "format").is_some_and(|format| format != "svg") {
            return Err("An embedded score is an SVG image; leave :format: out.".to_string());
        }
        let o = self.options;
        let source = self.tools.music.get_or_init(|| {
            crate::music::load_music_fonts(&gd_typst::Options {
                embedded_fonts_only: o.embedded_fonts_only,
                font_paths: o.font_paths.clone(),
                ..Default::default()
            })
        });
        let fonts = source.load_fonts();
        let fonts = fonts.loaded.as_ref().map_err(ToString::to_string)?;
        let mut read = gd_lilypond::Options::default();
        let loaded = gd_lilypond::load_score(doc.node_text(id), "score.ly", &mut read)
            .map_err(|failed| failed.to_string())?;
        let mut options = gd_engrave::Options { line_width: 564.0, ..Default::default() };
        let engraving = self
            .tools
            .engraver
            .borrow_mut()
            .engrave_score(&loaded.score, fonts, &mut options)
            .map_err(|e| e.to_string())?;
        let title = doc.custom_attribute(id, "alt").unwrap_or("");
        let svg_options = gd_engrave::SvgOptions {
            title,
            description: title,
            adaptive: true,
            ..Default::default()
        };
        let mut svg = String::new();
        gd_engrave::write_svg(&mut svg, &engraving, &svg_options, None)
            .map_err(|e| e.to_string())?;
        Ok(Picture { data: svg.into_bytes(), media: "image/svg+xml" })
    }

    /// Writes the image that stands for code block `id`, in a figure when it has a
    /// caption; the block's ids, name and classes move to it.
    fn write(
        &mut self,
        rw: &mut Rewriter<'_, '_, '_>,
        id: NodeId,
        kind: Kind,
        picture: &Picture,
    ) -> Result<(), Fail> {
        let doc = rw.doc;
        let span = doc.node(id).span;
        let caption = doc.custom_attribute(id, "caption");
        let outer =
            rw.open(if caption.is_some() { NodeKind::Figure } else { NodeKind::Image }, span)?;
        rw.moved_to(id, outer);
        rw.copy_attributes(id, outer, |key, name| {
            matches!(key, AttributeKey::Id | AttributeKey::Class)
                || (key == AttributeKey::Custom && name == "name")
        })?;
        let image = if caption.is_some() { rw.open(NodeKind::Image, span)? } else { outer };
        let alt = doc.custom_attribute(id, "alt").unwrap_or_else(|| doc.node_text(id).trim());
        rw.b.add_attribute_string(image, AttributeKey::Alt, alt)?;
        if let Some(align) = doc.custom_attribute(id, "align") {
            rw.b.add_attribute_string(image, AttributeKey::Align, align)?;
        }
        rw.add_class(image, kind.class())?;
        let mut url = format!("data:{};base64,", picture.media);
        gd_json::base64::encode_into(&mut url, &picture.data);
        let destination = rw.b.add_text(&url)?;
        rw.b.set_link(
            image,
            LinkInfo { destination, kind: LinkKind::External, ..LinkInfo::default() },
        )?;
        let Some(text) = caption else { return rw.close(outer) };
        rw.close(image)?;
        match doc.code_caption(id) {
            Some(parsed) => rw.copy_node(self, parsed)?,
            None => {
                let c = rw.open(NodeKind::Caption, span)?;
                rw.add_plain(text, span)?;
                rw.close(c)?;
            }
        }
        rw.close(outer)
    }
}

/// What a failed Typst compile said, with positions inside the diagram.
fn typst_failure(error: &gd_typst::Error) -> String {
    match error {
        gd_typst::Error::Compile { diagnostics, .. } if !diagnostics.is_empty() => diagnostics
            .iter()
            .map(|d| {
                let line = if d.stdin { d.line.saturating_sub(1) } else { d.line };
                let location = if d.stdin { "diagram" } else { &d.file };
                format!("{location}:{line}:{}: {} {}", d.column, d.message, d.hints.join(" "))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => error.message().into_owned(),
    }
}

impl Rule for Embed<'_> {
    fn visits(&self, doc: Document<'_>, id: NodeId) -> bool {
        self.kind(doc, id).is_some()
    }

    fn decide(&mut self, _: Document<'_>, _: NodeId) -> Action {
        Action::Replace
    }

    fn replace(&mut self, rw: &mut Rewriter<'_, '_, '_>, id: NodeId) -> Result<(), Fail> {
        let Some(kind) = self.kind(rw.doc, id) else { return rw.copy_node(self, id) };
        match self.draw(rw.doc, id, kind) {
            Ok(picture) => self.write(rw, id, kind, &picture),
            Err((message, text)) => {
                rw.b.report(message, rw.doc.node(id).span, arg_text(text.trim()), Arg::None);
                rw.copy_node(self, id)
            }
        }
    }
}

#[cfg(test)]
mod tests;
