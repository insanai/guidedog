//! Points.

use std::fmt;

/// A point in the plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point<T = f32> {
    /// Across.
    pub x: T,
    /// Down.
    pub y: T,
    hidden: (),
}

impl<T: Copy> Point<T> {
    /// Makes a point at `(x, y)`; see [`Self::x`].
    pub fn new(x: T, y: T) -> Self {
        Point { x, y, hidden: () }
    }

    /// The point's coordinates.
    pub fn coords(&self) -> (T, T) {
        (self.x, self.y)
    }

    fn private(&mut self) {}
}

impl<T: fmt::Display> fmt::Display for Point<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

/// A tuple point.
pub struct Pair(pub i32, i32);

impl crate::Draw for Point<f32> {
    type Output = ();

    fn draw(&self, _canvas: &mut crate::Canvas) -> Result<(), String> {
        Ok(())
    }
}
