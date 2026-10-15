//! Repository-wide quality gates, scans of the workspace sources:
//!
//! - `dependencies`: crate dependency rules, read from `cargo metadata`;
//! - `unsafe_code`: unsafe code only in FFI modules, each block with a SAFETY comment;
//! - `hints`: every diagnostic written in the code tells the reader what to do;
//! - `ascii`: documentation prose uses ASCII wherever ASCII can say it.
//!
//! The Rust code the manual shows is compiled by the `manual_code` test binary.
//! Documentation of public items is the compiler's job (`missing_docs` in the workspace
//! lints), and the size limits are `tools/check`'s.

#![allow(clippy::unwrap_used)] // a gate failing on unexpected input is the point.

mod ascii;
mod dependencies;
mod hints;
mod source;
mod unsafe_code;
