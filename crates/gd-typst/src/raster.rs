//! SVG rasterization uses the embedded renderer without compiling document code.

use std::fmt;

/// PNG dimensions and memory policy. Width and height are explicit typographic points.
pub struct RasterOptions<'a> {
    /// Positive drawing width in points.
    pub width: f64,
    /// Positive drawing height in points.
    pub height: f64,
    /// Pixels per inch, default 144.
    pub dpi: f64,
    /// Consulted before reserving the SVG, RGBA pixmap and PNG encoding allowance.
    pub reserve: Option<&'a dyn Fn(usize) -> bool>,
}

impl Default for RasterOptions<'_> {
    fn default() -> Self {
        Self { width: 0.0, height: 0.0, dpi: 144.0, reserve: None }
    }
}

impl fmt::Debug for RasterOptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RasterOptions")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("dpi", &self.dpi)
            .field("reserve", &self.reserve.is_some())
            .finish()
    }
}

/// Why a standalone SVG could not become a PNG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RasterError {
    /// The compiler and image renderer are not linked into this build.
    Disabled,
    /// A dimension or resolution is not positive and finite.
    Dimensions,
    /// A side exceeds 16384 pixels, the area exceeds 16 million pixels, or SVG exceeds 64 MiB.
    Limit,
    /// The caller refused the peak rasterization memory allowance.
    Budget,
    /// SVG decoding or PNG encoding failed.
    Data(String),
    /// The renderer failed internally.
    Internal,
}

impl fmt::Display for RasterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("PNG music requires the embedded image renderer."),
            Self::Dimensions => {
                f.write_str("PNG dimensions and resolution must be positive and finite.")
            }
            Self::Limit => f.write_str("The image exceeds the SVG or 16 million pixel limit."),
            Self::Budget => f.write_str("The memory ledger refused SVG rasterization."),
            Self::Data(message) => write!(f, "Cannot rasterize the SVG: {message}"),
            Self::Internal => f.write_str("The embedded image renderer failed internally."),
        }
    }
}

impl std::error::Error for RasterError {}

/// Borrows self-contained SVG and raster settings and returns a transparent PNG.
/// Reads no linked files, runs no document code, and fails before allocating oversized pixmaps.
pub fn rasterize_svg(svg: &[u8], options: &RasterOptions<'_>) -> Result<Vec<u8>, RasterError> {
    if ![options.width, options.height, options.dpi].iter().all(|n| n.is_finite() && *n > 0.0) {
        return Err(RasterError::Dimensions);
    }
    let scale = options.dpi / 72.0;
    if ![options.width as f32, options.height as f32, scale as f32]
        .iter()
        .all(|n| n.is_finite() && *n > 0.0)
    {
        return Err(RasterError::Dimensions);
    }
    let width = (options.width as f32 * scale as f32).round().max(1.0);
    let height = (options.height as f32 * scale as f32).round().max(1.0);
    let pixels = f64::from(width) * f64::from(height);
    if width > 16384.0 || height > 16384.0 || pixels > 16_000_000.0 || svg.len() > 64 * 1024 * 1024
    {
        return Err(RasterError::Limit);
    }
    let allowance = svg.len().saturating_add(pixels as usize * 9 + 65536);
    if options.reserve.is_some_and(|reserve| !reserve(allowance)) {
        return Err(RasterError::Budget);
    }
    render(svg, options)
}

#[cfg(feature = "embedded")]
fn render(svg: &[u8], options: &RasterOptions<'_>) -> Result<Vec<u8>, RasterError> {
    use typst::foundations::{Bytes, Content, Smart};
    use typst::layout::{Abs, Frame, FrameItem, FrameKind, Point, Sides, Size};
    use typst::syntax::Span;
    use typst::visualize::{Image, SvgImage};

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let decoded = SvgImage::new(Bytes::new(svg.to_vec()))
            .map_err(|e| RasterError::Data(format!("{e:?}")))?;
        let size = Size::new(Abs::pt(options.width), Abs::pt(options.height));
        let mut frame = Frame::new(size, FrameKind::Hard);
        frame.push(Point::zero(), FrameItem::Image(Image::plain(decoded), size, Span::detached()));
        let page = typst_layout::Page {
            frame,
            bleed: Sides::default(),
            fill: Smart::Custom(None),
            numbering: None,
            supplement: Content::empty(),
            number: 1,
        };
        let options = typst_render::RenderOptions {
            pixel_per_pt: typst::utils::Scalar::new(options.dpi / 72.0),
            ..Default::default()
        };
        typst_render::render(&page, &options)
            .encode_png()
            .map_err(|e| RasterError::Data(e.to_string()))
    }));
    typst::comemo::evict(0);
    result.map_err(|_| RasterError::Internal)?
}

#[cfg(not(feature = "embedded"))]
fn render(_svg: &[u8], _options: &RasterOptions<'_>) -> Result<Vec<u8>, RasterError> {
    Err(RasterError::Disabled)
}
