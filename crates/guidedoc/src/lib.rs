//! Guidedoc, the conversion engine of Guidedog: the document model, the builder readers
//! and passes write through, validation, diagnostics, the adapter registry, and the
//! pipeline that reads, transforms, and renders a document.
//!
//! The engine imports no reader, renderer, operating system, or Typst crate. Every name
//! is exported at the crate root, so ported code reads `guidedoc::begin_node`-style
//! names directly; docs/manual/internals/engine-api-map.rst maps every Odin `gd.` name to
//! its Rust form, and docs/manual/internals/ast.rst is
//! the normative node mapping.
//!
//! ```
//! use guidedoc::{convert_text, Registry, Workspace};
//! let mut ws = Workspace::new();
//! let registry = Registry::default(); // the defaults crate provides the real one
//! let failure = convert_text(&mut ws, &registry, "commonmark", "html", "# Hi\n");
//! assert_eq!(failure.map_err(|f| f.code()), Err("core.reader.unknown"));
//! ```

mod adapter;
mod anchors;
mod anchors_order;
mod builder;
mod convert;
mod document;
mod dump;
mod emit;
mod kinds;
mod media;
mod nfd;
mod object;
mod options;
mod report;
mod snapshot;
mod sources;
mod stack;
mod symbols;
mod tables;
mod text;
mod types;
mod validate;
mod workspace;

pub use adapter::{
    Conformance, ReadContext, ReadFn, ReadInlineFn, Reader, Registry, RenderContext, RenderFn,
    Renderer, SourceText, validate_registry,
};
pub use anchors::{AnchorExtra, AnchorIds, Anchors, build_anchors, id_fold, make_id, write_id};
pub use anchors_order::contents_scope;
pub use builder::{BUILDER_OPEN, Builder};
pub use convert::{
    check_render, convert, convert_text, read_document, render_document, transform_document,
};
pub use document::{Children, Document};
pub use dump::{dump_to_string, dump_tree};
pub use emit::{EmitMode, Emitter};
pub use kinds::{admonition_name, child_allowed, content_model, kind_name};
pub use media::{MediaKind, media_kind};
pub use nfd::{nfd_first, normalize_nfd, normalize_nfd_into};
pub use object::{load_object, write_object};
pub use options::{
    ContentPolicy, ENGLISH_LABELS, Highlighter, LABEL_COUNT, Label, Labels, Limits, MapEntry,
    MathStyle, Pass, PassFn, PassOutcome, Policy, ReadExtension, ReadOptions, RenderOptions,
    Request, Resource, ResourceProvider, ResourceStatus, Source, admonition_label, default_limits,
    default_policy, default_read_options, default_request, label, read_extension, split_label,
    trusted_policy,
};
pub use report::{
    Arg, CORE_CAPACITY, CORE_DEPTH, CORE_NODES, CORE_STACK, CORE_UNHANDLED_KIND, CORE_UTF8,
    CORE_WORK, Diagnostic, Fail, Message, Reports, Severity, arg_int, arg_span, arg_static,
    arg_text, content_error, diagnostic, kept_message,
};
pub use snapshot::{Snapshot, SourceEntry, SourceUse};
pub use sources::INCLUDE_MISSING;
pub use stack::{
    DEFAULT_STACK_BYTES, STACK_RESERVE, StackGuard, stack_diagnostic, stack_exhausted, stack_guard,
};
pub use symbols::{SymbolClash, SymbolTable};
pub use text::{
    TextSink, check_utf8, decode_last_rune, decode_rune, fold_rune, is_ascii_alnum,
    is_ascii_digit, is_ascii_punct, is_ascii_space, is_decimal_digit, is_letter,
    is_unicode_punctuation, is_unicode_whitespace, line_bounds, line_column, normalize_label,
    to_lower,
};
pub use types::{
    AdmonitionKind, Align, Attribute, AttributeKey, AttributeRange, CapacityNeed, CellInfo,
    Content, DocumentReading, INVALID_NODE, LineView, LinkInfo, LinkKind, ListInfo, ListStyle,
    NONE, Node, NodeFlag, NodeFlags, NodeId, NodeKind, ROOT, Region, SourceId, SourceSpan, Status,
    Text, offset_in,
};
pub use validate::validate;
pub use workspace::Workspace;

/// The engine's version: the project version every crate inherits.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Counts the incompatible changes to documents saved as objects (`write_object`).
pub const SCHEMA_VERSION: u32 = 1;
