//! The reStructuredText reader of Guidedoc: lines, block structure, explicit markup,
//! and the standard Docutils directives, implemented independently from the Docutils
//! markup specification. Inline markup and reference resolution live in
//! `guidedoc_rst_inline`, which this crate drives.
//!
//! ```
//! use guidedoc::{Registry, Workspace, default_request, dump_to_string, read_document};
//!
//! let mut ws = Workspace::new();
//! let mut request = default_request("x.rst", "Hello *world*.\n", "rst", "html");
//! request.read.promote_title = false;
//! let doc = read_document(&mut ws, &guidedoc_rst::reader(), &request).unwrap();
//! assert!(dump_to_string(doc).starts_with("document\n  paragraph\n"));
//! # let _ = Registry::default();
//! ```
//!
//! Dialects (the Sphinx layer) add directives and roles through [`Extension`] and read
//! with [`read_with`]; another markup (MyST) runs the directives through [`Host`],
//! [`embed_begin`], and [`Parser::run_hosted`].

#![forbid(unsafe_code)]

mod blocks;
mod csv;
mod directives;
mod directives_body;
mod directives_media;
mod directives_tables;
mod explicit;
mod extension;
mod fields;
mod host;
mod include;
mod lines;
mod lists;
pub mod messages;
mod names;
mod options;
mod paragraph;
mod parser;
mod read;
mod roles;
mod sections;
mod settings;
mod specs;
mod substitutions;
mod tables;
mod tables_grid;
mod tables_simple;
mod width;

pub use directives::{DirOption, Directive};
pub use extension::{DirectiveHandler, Extension, find_handler};
pub use host::{
    DirectiveShape, Host, HostDirective, HostItem, HostOption, HostedResult, embed_begin,
};
pub use include::path_of;
pub use lines::{
    Line, dedent, expand_views, indent_of, join_lines, trim_blank_end, trim_blank_start,
};
pub use lists::bullet_width;
pub use options::option_problem;
pub use parser::{Parser, ReaderState};
pub use read::{
    CONFORMANCE, SourceFrame, VERSION, read, read_inline, read_inline_with, read_with, reader,
};
pub use settings::{Settings, SmartQuotes, default_settings};
pub use specs::{ArgRule, ContentRule, OptSpec, OptType, choice, equal_fold, opt};
pub use tables::TableExtra;

pub use guidedoc_rst_inline::{Role, RoleBase, RoleCall, standard_role};
