//! Chooses how Graphviz is linked.
//!
//! By default the shared system libraries are linked (`-lgvc -lcgraph`, declared in
//! `src/ffi.rs`), which suits development with a Graphviz package installed. A static
//! build links the archives that `cargo xtask graphviz` builds instead, so the program
//! needs no Graphviz at run time. It is chosen by either
//!
//! - `GUIDEDOG_GRAPHVIZ_STATIC=<install dir>`, the folder `cargo xtask graphviz` prints, or
//! - the `static-graphviz` feature, which uses the task's default folder,
//!   `target/graphviz/<version>` in the repository.
//!
//! A static build sets `cfg(graphviz_static)`: the crate then hands Graphviz its built-in
//! plugins (src/builtin.rs) instead of reading a plugin config file.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// The Graphviz version the static build pins. tools/xtask/src/graphviz.rs builds it, and
/// src/builtin.rs reads its `textspan_t`, so the three change together.
const VERSION: &str = "16.1.0";

/// The archives in link order: each one only uses the ones after it.
const ARCHIVES: [&str; 11] = [
    "gvplugin_dot_layout",
    "gvplugin_neato_layout",
    "gvplugin_core",
    "gvc",
    "pathplan",
    "xdot",
    "cgraph",
    "cdt",
    "util",
    "vpsc",
    "expat",
];

fn main() {
    println!("cargo:rustc-check-cfg=cfg(graphviz_static)");
    println!("cargo:rerun-if-env-changed=GUIDEDOG_GRAPHVIZ_STATIC");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=GUIDEDOG_GRAPHVIZ_WINDOWS");
    if let Some(install) = env::var_os("GUIDEDOG_GRAPHVIZ_WINDOWS") {
        println!("cargo:rustc-link-search=native={}", Path::new(&install).join("lib").display());
    }
    if let Some(install) = static_install() {
        link_static(&install);
    }
}

/// The install folder of a static build, or `None` for the shared system libraries.
fn static_install() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("GUIDEDOG_GRAPHVIZ_STATIC").filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    env::var_os("CARGO_FEATURE_STATIC_GRAPHVIZ")?;
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?);
    Some(manifest.join("../../target/graphviz").join(VERSION))
}

fn link_static(install: &Path) {
    let stamp = install.join("VERSION");
    println!("cargo:rerun-if-changed={}", stamp.display());
    let found = fs::read_to_string(&stamp).unwrap_or_default();
    if found.trim() != VERSION {
        // A build script reports a misconfiguration by failing; there is no caller to
        // return an error to.
        panic!(
            "\n\nNo static Graphviz {VERSION} build is in {}.\n\
             Hint: run `cargo xtask graphviz` and set GUIDEDOG_GRAPHVIZ_STATIC to the folder \
             it prints, or unset GUIDEDOG_GRAPHVIZ_STATIC to link the system's Graphviz.\n",
            install.display()
        );
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        panic!(
            "\n\nGraphviz cannot be linked statically on Windows; unset \
                GUIDEDOG_GRAPHVIZ_STATIC.\n"
        );
    }
    println!("cargo:rustc-cfg=graphviz_static");
    println!("cargo:rustc-link-search=native={}", install.join("lib").display());
    for archive in ARCHIVES {
        println!("cargo:rustc-link-lib=static={archive}");
    }
    // Graphviz's vpsc is C++. On Linux the task installs libstdc++.a beside the archives
    // so the program does not depend on the system's libstdc++; macOS always has libc++.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else {
        println!("cargo:rustc-link-lib=static=stdc++");
        println!("cargo:rustc-link-lib=dylib=m");
    }
}
