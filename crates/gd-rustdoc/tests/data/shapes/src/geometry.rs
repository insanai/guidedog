//! Geometry: points and areas.

pub mod point;

/// The area of a `w` by `h` box.
///
/// - one
/// - two, with *emphasis*
///   continued
///
/// 1. first
/// 2. second
///
/// | a | b |
/// |---|---|
/// | 1 | `2` |
///
/// ```text
/// not rust
/// ```
///
/// > quoted
pub fn area(w: f32, h: f32) -> f32 {
    w * h
}

/// Broken link to [`crate::nowhere`] and private [`helper`].
pub fn links() {}

fn helper() {}
