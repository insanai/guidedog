//! -G/-N/-E defaults, applied exactly as dot applies them (lib/common/input.c,
//! global_def): the attribute is declared on cgraph's prototype graph, which every graph
//! copies when it is created, and marked `fixed`. The parser then ignores root-level
//! statements for it, such as `node [shape=box]` or `bgcolor=red`, while a value written
//! on one node, edge or subgraph still wins.
//!
//! The prototype graph is process-wide, so graphdog restores it once the source is parsed
//! (graphs keep their own copy). cgraph cannot remove an attribute: one that graphdog
//! introduced stays declared with the empty default "", which is how cgraph declares any
//! attribute that a file sets without a default, so later graphs render the same. Node
//! labels are the exception: "" would blank them, so they fall back to "\N", as in dot.

use std::ffi::{CStr, CString, c_int};
use std::ptr;

use crate::{Attribute, AttributeKind, Locked, c_string, ffi};

/// One prototype attribute as it was before graphdog changed it.
struct Saved {
    kind: c_int,
    name: CString,
    /// The previous default, or `None` if graphdog declared the attribute.
    value: Option<CString>,
    fixed: u8,
}

/// Defaults fixed on the prototype graph; dropping it restores what they replaced. It
/// borrows the lock, so the prototypes are restored before the lock is released.
pub(crate) struct Fixed<'l> {
    saved: Vec<Saved>,
    _lock: &'l Locked,
}

fn kind_code(kind: AttributeKind) -> c_int {
    match kind {
        AttributeKind::Graph => ffi::AG_GRAPH,
        AttributeKind::Node => ffi::AG_NODE,
        AttributeKind::Edge => ffi::AG_EDGE,
    }
}

/// Declares each attribute on the prototype graph, for the graph parsed next; drop the
/// result once it is parsed.
pub(crate) fn fix<'l>(lock: &'l Locked, attributes: &[Attribute]) -> Fixed<'l> {
    let mut saved = Vec::with_capacity(attributes.len());
    for attribute in attributes {
        let kind = kind_code(attribute.kind);
        let name = c_string(&attribute.name);
        let value = c_string(&attribute.value);
        // SAFETY: the Graphviz lock is held; a null graph names the prototype graph, and a
        // null value only looks the attribute up.
        let old = unsafe { ffi::agattr(ptr::null_mut(), kind, name.as_ptr(), ptr::null()) };
        let (previous, fixed) = if old.is_null() {
            (None, 0)
        } else {
            // SAFETY: `old` is a live symbol of the prototype graph; defval is a
            // NUL-terminated string cgraph owns, copied here.
            unsafe { (Some(CStr::from_ptr((*old).defval).to_owned()), (*old).fixed) }
        };
        // SAFETY: as above; cgraph copies name and value.
        let sym = unsafe { ffi::agattr(ptr::null_mut(), kind, name.as_ptr(), value.as_ptr()) };
        if !sym.is_null() {
            // SAFETY: `sym` is a live symbol of the prototype graph whose layout matches
            // ffi::Agsym up to `fixed`; no other thread can touch it under the lock.
            unsafe { (*sym).fixed = 1 };
        }
        saved.push(Saved { kind, name, value: previous, fixed });
    }
    Fixed { saved, _lock: lock }
}

/// Undoes [`fix`], newest first, so a name given twice ends up with its original default.
impl Drop for Fixed<'_> {
    fn drop(&mut self) {
        for entry in self.saved.drain(..).rev() {
            let value = entry.value.unwrap_or_else(|| {
                let label = entry.kind == ffi::AG_NODE && entry.name.as_bytes() == b"label";
                CString::from(if label { c"\\N" } else { c"" })
            });
            // SAFETY: the lock this value borrows is held; cgraph copies name and value.
            let sym = unsafe {
                ffi::agattr(ptr::null_mut(), entry.kind, entry.name.as_ptr(), value.as_ptr())
            };
            if !sym.is_null() {
                // SAFETY: `sym` is a live symbol of the prototype graph (see `fix`).
                unsafe { (*sym).fixed = entry.fixed };
            }
        }
    }
}
