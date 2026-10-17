//! Guidedog projects end to end, through the built `guidedog` program: the manual's
//! tutorials as written, publication (all or nothing), Sphinx's constructs across a
//! project, memory budgets and limits, and builds with no Python on the machine.
//! The engine's capacity results are checked in process, through the real readers and
//! renderers.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

mod capacity;
mod directives;
mod inventories;
mod limits;
mod publication;
mod sphinx;
mod sphinx_pages;
mod ssg;
mod support;
mod tutorials;
