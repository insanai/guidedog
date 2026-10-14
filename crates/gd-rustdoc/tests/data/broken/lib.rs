//! A crate with problems, for gd-rustdoc's tests.

mod missing;

use self::{
    // A comment inside a use list.
    a::B,
};

/// Still documented.
pub fn fine() {}

pub fn broken( {
