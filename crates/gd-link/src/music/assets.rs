use std::path::PathBuf;

use gd_engrave::{Engraving, SvgOptions};
use gd_env::OutputFormat;
use guidedoc::{Document, NodeId};

use crate::page::Page;

pub(super) fn save(
    page: &mut Page<'_>,
    doc: Document<'_>,
    id: NodeId,
    drawing: &Engraving,
    system: Option<usize>,
    alt: &str,
    ledger: &gd_host::Ledger<'_>,
) -> Result<PathBuf, String> {
    let format = doc.custom_attribute(id, "format").unwrap_or("svg");
    if !matches!(format, "svg" | "png") {
        return Err(format!("Unknown music image format `{format}`."));
    }
    let mut svg = String::new();
    let options = SvgOptions {
        systems: system.map(|index| index..index + 1),
        title: alt,
        description: alt,
        adaptive: format == "svg" && page.format == OutputFormat::Html,
        ..Default::default()
    };
    let mut reserve = |bytes| ledger.charge(bytes);
    gd_engrave::write_svg(&mut svg, drawing, &options, Some(&mut reserve))
        .map_err(|e| e.to_string())?;
    let data = if format == "png" {
        let height = system.map(|i| drawing.systems[i].height).unwrap_or(drawing.height);
        let options = gd_typst::RasterOptions {
            width: system
                .map(|i| drawing.systems[i].width)
                .filter(|w| *w > 0.0)
                .unwrap_or(drawing.width),
            height,
            reserve: Some(&reserve),
            ..Default::default()
        };
        gd_typst::rasterize_svg(svg.as_bytes(), &options).map_err(|e| e.to_string())?
    } else {
        svg.into_bytes()
    };
    publish(page, &data, format)
}

fn publish(page: &Page<'_>, data: &[u8], extension: &str) -> Result<PathBuf, String> {
    let scratch =
        gd_env::doctrees_dir(page.config()).join("diagrams").join(format!("music.{extension}"));
    let stage = gd_host::Stage::beside(&scratch).map_err(|e| e.to_string())?;
    std::fs::write(stage.path(), data).map_err(|e| e.to_string())?;
    let digest = gd_env::file_digest(stage.path()).ok_or("Cannot read the generated music.")?;
    let path = scratch.with_file_name(format!("lilypond-{digest}.{extension}"));
    if !gd_host::same_contents(&path, data) {
        stage.commit(&path, true).map_err(|e| e.to_string())?;
    }
    Ok(path)
}
