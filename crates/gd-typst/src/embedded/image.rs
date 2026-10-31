//! Single-page image export. Raster dimensions are checked before allocating a pixmap.

use typst::diag::{SourceResult, bail};
use typst::syntax::Span;
use typst_layout::PagedDocument;

use crate::Target;

/// At most 16 million pixels (64 MiB of RGBA), with neither side above 16,384 pixels.
const MAX_PIXELS: f64 = 16_000_000.0;

pub(super) fn export(doc: &PagedDocument, target: Target) -> SourceResult<Vec<u8>> {
    let [page] = doc.pages() else {
        bail!(Span::detached(), "a diagram must contain exactly one page");
    };
    let size = page.frame.size();
    let (width, height) = (size.x.to_pt(), size.y.to_pt());
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        bail!(Span::detached(), "a diagram must have positive, finite dimensions");
    }
    match target {
        Target::Svg => Ok(typst_svg::svg(page, &typst_svg::SvgOptions::default()).into_bytes()),
        Target::Png => {
            let (width, height) = ((width as f32 * 2.0).round(), (height as f32 * 2.0).round());
            let pixels = f64::from(width.max(1.0)) * f64::from(height.max(1.0));
            if width > 16_384.0 || height > 16_384.0 || pixels > MAX_PIXELS {
                bail!(Span::detached(), "a PNG diagram exceeds the 16 million pixel limit");
            }
            typst_render::render(page, &typst_render::RenderOptions::default())
                .encode_png()
                .map_err(|error| {
                    typst::diag::eco_vec![typst::diag::error!(
                        Span::detached(),
                        "cannot encode PNG: {error}"
                    )]
                })
        }
        Target::Pdf | Target::Html => unreachable!("only image targets reach image export"),
    }
}
