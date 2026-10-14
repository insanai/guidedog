//! Shapes and the [`Canvas`] they are drawn on.
//!
//! # Examples
//!
//! ```
//! # use fake_shapes::Canvas;
//! let canvas = Canvas::new(4, 4);
//! assert_eq!(canvas.width(), 4);
//! ```
#![doc = include_str!("../README.md")]

pub mod geometry;
mod storage;
#[cfg(feature = "inline")]
pub mod inline {
    //! An inline module.

    /// Doubles `x`.
    pub const fn double(x: u32) -> u32 {
        x * 2
    }
}

pub use storage::Canvas;
pub use storage::Pixel as Dot;
pub use geometry::point::Point;
pub use std::collections::HashMap;

/// The largest canvas side.
pub const MAX_SIDE: usize = 4_096;

/// Every canvas drawn so far.
pub static mut DRAWN: u64 = 0;

/// A shape's kind.
///
/// See [kinds][`Kind::Circle`] and [`crate::geometry::area`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub enum Kind {
    /// A round one.
    Circle,
    /// A polygon with this many sides.
    Polygon(
        /// The side count.
        u8,
    ),
    /// A box.
    Rect {
        /// Its width.
        width: f32,
        height: f32,
    },
    Unknown = 9,
}

/// Raw bits of a color.
#[repr(C)]
pub union Bits {
    /// As one number.
    pub word: u32,
    /// As four bytes.
    pub bytes: [u8; 4],
}

/// Something that can be drawn.
pub trait Draw: Clone + Send {
    /// What drawing yields.
    type Output: Default;
    /// How many passes it takes.
    const PASSES: usize = 1;

    /// Draws `self` on `canvas`.
    ///
    /// # Errors
    ///
    /// Returns an error when the canvas is full.
    fn draw(&self, canvas: &mut Canvas) -> Result<Self::Output, String>;

    /// The name, by default `"shape"`.
    fn name(&self) -> &str {
        "shape"
    }
}

/// A shape-making macro.
#[macro_export]
macro_rules! shape {
    ($kind:ident) => { $crate::Kind::$kind };
    ($kind:ident, $($arg:expr),*) => { $crate::Kind::$kind($($arg),*) };
}

/// A result with this crate's error.
pub type Result<T> = std::result::Result<T, String>;

/// Draws everything.
///
/// # Safety
///
/// `ptr` must be valid. See [`Draw::draw`] and [`shape!`].
pub unsafe fn draw_all<T: Draw + 'static>(
    ptr: *const T,
    count: usize,
) -> Result<()>
where
    T: Sync,
{
    let _ = (ptr, count);
    Ok(())
}

/// An async helper.
#[deprecated(since = "0.2.0", note = "use draw_all")]
pub async fn draw_later() {}

/// Not shown.
#[doc(hidden)]
pub fn hidden_helper() {}

/// Private, not shown.
fn private_helper() {}

unsafe extern "C" {
    /// A C function.
    pub fn c_draw(x: i32) -> i32;
}

#[cfg(test)]
mod tests {
    #[test]
    fn works() {}
}
