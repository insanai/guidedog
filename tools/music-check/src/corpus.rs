use std::path::Path;

use gd_engrave::{Engraver, FaceData, FontOptions, Options, SvgOptions};

const REGULAR: &[u8] =
    include_bytes!("../../../docs/shared/_static/data/fonts/LibertinusSerif-Regular.otf");
const BOLD: &[u8] =
    include_bytes!("../../../docs/shared/_static/data/fonts/LibertinusSerif-Bold.otf");
const ITALIC: &[u8] =
    include_bytes!("../../../docs/shared/_static/data/fonts/LibertinusSerif-Italic.otf");

pub(crate) fn fonts() -> Result<gd_engrave::Fonts<'static>, String> {
    let face = |data| FaceData { data, ..Default::default() };
    gd_engrave::load_fonts(&FontOptions {
        regular: vec![face(REGULAR)],
        bold: vec![face(BOLD)],
        italic: vec![face(ITALIC)],
        // Libertinus ships no bold italic; bold italic markup falls back to bold.
        bold_italic: Vec::new(),
    })
    .map_err(|error| error.to_string())
}

pub(crate) fn check(root: &Path, update: bool) -> Result<(), String> {
    let directory = root.join("tests/integration/data/music");
    let mut sources: Vec<_> = std::fs::read_dir(&directory)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ly"))
        .collect();
    sources.sort();
    if sources.len() < 40 {
        return Err("The reference corpus needs at least forty scores.".into());
    }
    let fonts = fonts()?;
    let mut workspace = Engraver::default();
    for source in &sources {
        let text = std::fs::read_to_string(source).map_err(|e| e.to_string())?;
        let name = source.file_name().and_then(|n| n.to_str()).unwrap_or("music.ly");
        let loaded = gd_lilypond::load_score(&text, name, &mut gd_lilypond::Options::default())
            .map_err(|e| format!("{name}: {e}"))?;
        if !loaded.warnings.is_empty() {
            return Err(format!("{name}: {:?}", loaded.warnings));
        }
        let drawing = workspace
            .engrave_score(
                &loaded.score,
                &fonts,
                &mut Options { line_width: 350.0, ..Default::default() },
            )
            .map_err(|e| format!("{name}: {e}"))?;
        audit(&drawing)
            .and_then(|()| crate::audit::check(&drawing))
            .and_then(|()| crate::audit::quants(&drawing))
            .map_err(|e| format!("{name}: {e}"))?;
        let mut svg = String::new();
        gd_engrave::write_svg(&mut svg, &drawing, &SvgOptions::default(), None)
            .map_err(|e| e.to_string())?;
        let raster_svg = svg.replacen(
            "</defs>",
            "</defs><rect width=\"100%\" height=\"100%\" fill=\"white\"/>",
            1,
        );
        let raster = gd_typst::rasterize_svg(
            raster_svg.as_bytes(),
            &gd_typst::RasterOptions {
                width: drawing.width,
                height: drawing.height,
                ..Default::default()
            },
        )
        .map_err(|e| format!("{name}: {e}"))?;
        compare(source, &svg, &raster, update)?;
    }
    println!("{} music scores: geometry, SVG and PNG pixels checked.", sources.len());
    Ok(())
}

fn compare(source: &Path, svg: &str, png: &[u8], update: bool) -> Result<(), String> {
    let svg_path = source.with_extension("svg");
    let png_path = source.with_extension("png");
    if update {
        std::fs::write(svg_path, svg).map_err(|e| e.to_string())?;
        std::fs::write(png_path, png).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let expected = std::fs::read(svg_path).map_err(|e| e.to_string())?;
    if expected != svg.as_bytes() {
        return Err(format!("{}: SVG changed.", source.display()));
    }
    let expected = std::fs::read(png_path).map_err(|e| e.to_string())?;
    crate::png::compare(&expected, png).map_err(|e| format!("{}: {e}", source.display()))
}

fn audit(drawing: &gd_engrave::Engraving) -> Result<(), String> {
    for system in &drawing.systems {
        for placed in &system.marks {
            let bounds = placed.bounds;
            if ![bounds.left, bounds.right, bounds.top, bounds.bottom]
                .iter()
                .all(|n| n.is_finite())
                || bounds.left < -0.001
                || bounds.top < -0.001
                || bounds.right > drawing.width + 0.001
                || bounds.bottom > system.height + 0.001
            {
                return Err(format!("Clipped or invalid ink: {:?}, {bounds:?}", placed.mark));
            }
        }
    }
    Ok(())
}
