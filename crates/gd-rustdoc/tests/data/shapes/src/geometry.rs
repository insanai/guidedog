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

/// Notes on units[^units], with a picture and links to the standard library.
///
/// ![A unit square](https://example.org/square.svg)
///
/// Sides are [`f32`] or [`prim@u8`], and names are [`str`]; see [`u8::MAX`],
/// [`Vec`], [`std::collections::HashMap`] and [`std::mem::swap`].
///
/// First line,  
/// second line.\
/// Third line.
///
/// [^units]: Points are pixels
///     at one to one.
pub fn notes() {}
