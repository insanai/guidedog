use std::path::Path;

use gd_engrave::{Engraver, Options, SvgOptions};

pub(crate) fn check(root: &Path, update: bool) -> Result<(), String> {
    let directory = root.join("tests/integration/data/music");
    let mut sources: Vec<_> = std::fs::read_dir(&directory)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ly"))
        .collect();
    sources.sort();
    if sources.len() < 66 {
        return Err("The reference corpus needs at least sixty-six scores.".into());
    }
    let data = crate::fonts::load(root)?;
    let fonts = data.fonts()?;
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
        let compact = workspace
            .engrave_score(
                &loaded.score,
                &fonts,
                &mut Options { staff_size: 14.0, line_width: 240.0, ..Default::default() },
            )
            .map_err(|e| format!("{name} (14pt): {e}"))?;
        audit(&compact)
            .and_then(|()| crate::audit::check(&compact))
            .and_then(|()| crate::audit::quants(&compact))
            .map_err(|e| format!("{name} (14pt): {e}"))?;
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
        crate::png::check_dpi(&raster).map_err(|e| format!("{name}: {e}"))?;
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

pub(crate) fn audit(drawing: &gd_engrave::Engraving) -> Result<(), String> {
    if !drawing.warnings.is_empty() {
        return Err(format!("Engrave warnings: {:?}", drawing.warnings));
    }
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
