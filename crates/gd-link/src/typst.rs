//! Typst diagram policy and compilation. The compiler adapter owns image export.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use gd_env::LookupKind;
use gd_host::Stage;
use gd_typst::{Backend, Options, Target};
use guidedoc::{Document, Fail, NodeId};

use crate::figure::{Drawing, replace};
use crate::page::{Page, Warning};
use crate::rewrite::Rewriter;

const PAGE_SETUP: &str = "#set page(width: auto, height: auto, margin: 0pt)\n";
const MAX_SOURCE: usize = 1 << 20;
const MAX_FILES: usize = 4096;

pub(crate) fn replace_diagram(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
) -> Result<(), Fail> {
    let doc = rw.doc;
    let source = doc.node_text(id);
    // A diagram compiles in a confined build too: the embedded compiler runs no
    // program, reads only below the source folder, and downloads no package there.
    let failure = if source.len() > MAX_SOURCE {
        Some("The Typst diagram exceeds the 1 MiB source limit.")
    } else {
        None
    };
    if let Some(message) = failure {
        warn(page, doc, id, message.to_string());
        return rw.copy_node(page, id);
    }
    let path = match draw(page, doc, id) {
        Ok(path) => path,
        Err(message) => {
            warn(page, doc, id, message);
            return rw.copy_node(page, id);
        }
    };
    replace(
        page,
        rw,
        id,
        Drawing {
            path,
            source,
            class: "typst-diagram",
            interactive: false,
            adaptive: false,
            alt: None,
            narrow: None,
            parts: Vec::new(),
        },
    )
}

fn warn(page: &mut Page<'_>, doc: Document<'_>, id: NodeId, message: String) {
    page.warn(
        doc,
        doc.node(id).span,
        Warning {
            code: "build.typst-diagram",
            title: "TYPST DIAGRAM NOT DRAWN",
            category: "",
            message,
            hint: "Check the Typst source and pinned imports; use one page below the source root. \
               The source is shown as code.",
        },
    );
}

/// Compiles afresh when a page is rewritten. The output digest, rather than a source
/// digest, names the asset: imports and data can change without changing the directive.
fn draw(page: &mut Page<'_>, doc: Document<'_>, id: NodeId) -> Result<PathBuf, String> {
    let config = page.config();
    let extension = match doc.custom_attribute(id, "format").unwrap_or("svg") {
        "svg" => "svg",
        "png" => "png",
        other => return Err(format!("Unknown diagram format `{other}`; use svg or png.")),
    };
    let target = if extension == "png" { Target::Png } else { Target::Svg };
    let scratch = gd_env::doctrees_dir(config).join("diagrams").join(extension);
    let stage = Stage::beside(&scratch).map_err(|error| error.to_string())?;
    let origin = doc.custom_attribute(id, "typst-source").unwrap_or(&page.info.relative);
    let origin = config.source_dir.join(origin);
    let files = Mutex::new(Vec::<PathBuf>::new());
    let observe = |path: &Path| {
        let mut files = files.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if files.iter().any(|file| file == path) {
            return true;
        }
        if files.len() >= MAX_FILES || files.try_reserve(1).is_err() {
            return false;
        }
        files.push(path.to_path_buf());
        true
    };
    let reserve = |bytes| page.ledger.charge(bytes);
    let options = Options {
        target,
        source_path: Some(origin),
        observe_file: Some(&observe),
        offline: config.pdf.packages == "offline",
        embedded_fonts_only: config.pdf.fonts == "embedded",
        font_paths: config.pdf.font_paths.iter().map(|p| config.source_dir.join(p)).collect(),
        read_roots: vec![config.source_dir.clone()],
        reserve: Some(&reserve),
        ..Default::default()
    };
    let mut source = String::new();
    source
        .try_reserve(PAGE_SETUP.len() + doc.node_text(id).len())
        .map_err(|_| "Cannot reserve memory for the diagram source.")?;
    source.push_str(PAGE_SETUP);
    source.push_str(doc.node_text(id));
    let compiled = gd_typst::compile_text(
        &Backend::Embedded,
        &source,
        &config.source_dir,
        stage.path(),
        &options,
    );
    for path in files.into_inner().unwrap_or_else(|poisoned| poisoned.into_inner()) {
        page.trace.note_key(LookupKind::FileContent, &path.to_string_lossy());
    }
    let compiled = compiled.map_err(compile_error)?;
    for diagnostic in compiled.diagnostics {
        let mut report = diagnostic.report(None);
        report.path.clone_from(&page.info.relative);
        // Keep the original directive visible beside the compiler's exact position.
        report.message = format!(
            "{} ({}:{}:{})",
            report.message, diagnostic.file, diagnostic.line, diagnostic.column
        );
        crate::page::locate(doc, doc.node(id).span, &mut report);
        page.reports.push(report);
    }
    let digest = gd_env::file_digest(stage.path()).ok_or("Cannot read the generated image.")?;
    let path = scratch.with_file_name(format!("typst-{digest}.{extension}"));
    if gd_env::file_digest(&path) != Some(digest) {
        stage.commit(&path, true).map_err(|error| error.to_string())?;
    }
    Ok(path)
}

/// Keep positions in the warning even when the compiler failed before producing an image.
fn compile_error(error: gd_typst::Error) -> String {
    match &error {
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
