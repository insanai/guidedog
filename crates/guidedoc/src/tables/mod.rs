//! Unicode data tables, converted mechanically from the Odin engine's generated tables
//! (Unicode 17.0.0). Each file says where its data comes from; do not edit them by hand.

pub(crate) mod casefold;
pub(crate) mod categories;
pub(crate) mod idfold;
pub(crate) mod nfd_classes;
pub(crate) mod nfd_decompositions;
pub(crate) mod nfd_runes;
