//! Private storage; its public items are re-exported at the root.

mod pixel;

pub use pixel::Pixel;

/// A grid of pixels.
pub struct Canvas {
    width: usize,
    pixels: Vec<Pixel>,
}

impl Canvas {
    /// Makes an empty canvas. Links to [`Pixel`] and [`Canvas::width`].
    pub fn new(width: usize, height: usize) -> Canvas {
        Canvas { width, pixels: vec![Pixel(0); width * height] }
    }

    /// The width.
    pub fn width(&self) -> usize {
        self.width
    }
}

impl crate::Draw for Canvas {
    type Output = usize;

    fn draw(&self, _canvas: &mut Canvas) -> Result<usize, String> {
        Ok(self.width)
    }
}

impl Clone for Canvas {
    fn clone(&self) -> Self {
        Canvas { width: self.width, pixels: self.pixels.clone() }
    }
}
