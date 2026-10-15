//! The raw Graphviz C API that graphdog uses, and nothing more.
//!
//! Every signature was checked against the headers of Graphviz 16.1 and 2.42 (Debian);
//! where they differ, the note says how this binding stays correct for both. By default the
//! shared libraries a package manager installs (`apt install libgraphviz-dev`, `brew
//! install graphviz`) are linked: `-lgvc -lcgraph`. A static build (`cfg(graphviz_static)`)
//! links the archives that build.rs names instead.

use std::ffi::{c_char, c_int, c_void};

/// Opaque Graphviz context (`GVC_t`).
#[repr(C)]
pub(crate) struct Gvc {
    _private: [u8; 0],
}

/// Opaque graph or subgraph (`Agraph_t`).
#[repr(C)]
pub(crate) struct Agraph {
    _private: [u8; 0],
}

/// Opaque node (`Agnode_t`).
#[repr(C)]
pub(crate) struct Agnode {
    _private: [u8; 0],
}

/// Opaque edge (`Agedge_t`).
#[repr(C)]
pub(crate) struct Agedge {
    _private: [u8; 0],
}

/// `Agsym_s` in cgraph.h, which the header makes public. graphdog reads and sets `fixed`
/// the way dot does for -G/-N/-E. The fields up to `print` have kept this layout from 2.42
/// to 16; 16 appends `owner`, which graphdog never touches, and graphdog never allocates
/// an `Agsym` itself.
#[repr(C)]
pub(crate) struct Agsym {
    /// `Dtlink_t`: right child, then a union of hash and left child.
    pub link: [*mut c_void; 2],
    pub name: *mut c_char,
    pub defval: *mut c_char,
    pub id: c_int,
    pub kind: u8,
    /// When set, the parser ignores root-level defaults for this attribute.
    pub fixed: u8,
    pub print: u8,
}

/// agobjkind in cgraph.h.
pub(crate) const AG_GRAPH: c_int = 0;
pub(crate) const AG_NODE: c_int = 1;
pub(crate) const AG_EDGE: c_int = 2;

/// `agerrlevel_t` in cgraph.h (AGWARN, AGERR, AGMAX, AGPREV).
pub(crate) type Agerrlevel = c_int;
pub(crate) const AGWARN: Agerrlevel = 0;

/// `agusererrf` in cgraph.h: receives one formatted message.
pub(crate) type Agusererrf = Option<unsafe extern "C" fn(message: *mut c_char) -> c_int>;

// gvRenderData writes its length through `size_t *` in Graphviz 16 and `unsigned int *`
// in 2.42. Callers pass a zeroed usize: on a little-endian target a 32-bit write fills the
// low half and the zero high half keeps the value exact.
#[cfg(target_endian = "big")]
compile_error!("gd-graphdog's gvRenderData binding assumes a little-endian target");

/// `lt_symlist_t` in gvcext.h: one entry of a built-in plugin table, which ends with a
/// null entry.
#[cfg(graphviz_static)]
#[repr(C)]
pub(crate) struct LtSymlist {
    pub name: *const c_char,
    pub address: *const c_void,
}

#[cfg_attr(not(graphviz_static), link(name = "gvc"))]
unsafe extern "C" {
    #[cfg(not(graphviz_static))]
    pub(crate) fn gvContext() -> *mut Gvc;
    /// Like `gvContext`, but with the built-in plugins instead of the config file.
    #[cfg(graphviz_static)]
    pub(crate) fn gvContextPlugins(builtins: *const LtSymlist, demand_loading: c_int) -> *mut Gvc;
    pub(crate) fn gvcVersion(gvc: *mut Gvc) -> *mut c_char;
    pub(crate) fn gvLayout(gvc: *mut Gvc, g: *mut Agraph, engine: *const c_char) -> c_int;
    pub(crate) fn gvRenderData(
        gvc: *mut Gvc,
        g: *mut Agraph,
        format: *const c_char,
        result: *mut *mut c_char,
        length: *mut usize,
    ) -> c_int;
    pub(crate) fn gvFreeRenderData(data: *mut c_char);
    pub(crate) fn gvFreeLayout(gvc: *mut Gvc, g: *mut Agraph) -> c_int;
    pub(crate) fn gvFreeContext(gvc: *mut Gvc) -> c_int;
}

// 2.42 declares names and values as `char *`; neither version writes to them.
#[cfg_attr(not(graphviz_static), link(name = "cgraph"))]
unsafe extern "C" {
    pub(crate) fn agfstnode(g: *mut Agraph) -> *mut Agnode;
    pub(crate) fn agnxtnode(g: *mut Agraph, n: *mut Agnode) -> *mut Agnode;
    pub(crate) fn agfstout(g: *mut Agraph, n: *mut Agnode) -> *mut Agedge;
    pub(crate) fn agnxtout(g: *mut Agraph, e: *mut Agedge) -> *mut Agedge;
    pub(crate) fn agfstsubg(g: *mut Agraph) -> *mut Agraph;
    pub(crate) fn agnxtsubg(g: *mut Agraph) -> *mut Agraph;
    pub(crate) fn agnxtattr(g: *mut Agraph, kind: c_int, sym: *mut Agsym) -> *mut Agsym;
    pub(crate) fn agxget(obj: *mut c_void, sym: *mut Agsym) -> *mut c_char;
    pub(crate) fn agxset(obj: *mut c_void, sym: *mut Agsym, value: *const c_char) -> c_int;
    pub(crate) fn aghtmlstr(s: *const c_char) -> c_int;
    pub(crate) fn agstrdup_html(g: *mut Agraph, s: *const c_char) -> *mut c_char;
    pub(crate) fn agmemread(source: *const c_char) -> *mut Agraph;
    pub(crate) fn agclose(g: *mut Agraph) -> c_int;
    pub(crate) fn agattr(
        g: *mut Agraph,
        kind: c_int,
        name: *const c_char,
        value: *const c_char,
    ) -> *mut Agsym;
    pub(crate) fn agseterr(level: Agerrlevel) -> Agerrlevel;
    pub(crate) fn agseterrf(handler: Agusererrf) -> Agusererrf;
    pub(crate) fn agreseterrors() -> c_int;
}
