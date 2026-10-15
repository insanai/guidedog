//! The versions `guidedog --version` names, and the notices `--licenses` prints.

use std::fmt::Write;

/// The Guidedog release this program belongs to: the workspace version, which the file
/// `VERSION` at the repository root repeats for release tooling (a test checks both).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Build metadata appended to [`VERSION`] for development builds, from the build-time
/// environment variable `GUIDEDOG_VERSION_SUFFIX` (such as `+3fe1d8a`); empty in a
/// release.
pub const VERSION_SUFFIX: &str = match option_env!("GUIDEDOG_VERSION_SUFFIX") {
    Some(suffix) => suffix,
    None => "",
};

/// The tree-sitter crates linked in for autodoc (Python and Rust parsing), as
/// Cargo.lock pins them; a test keeps these in step with it.
pub(crate) const TREE_SITTER: [(&str, &str); 3] =
    [("tree-sitter", "0.25"), ("tree-sitter-python", "0.25"), ("tree-sitter-rust", "0.24")];

/// What the command prints as its version: [`VERSION`] and [`VERSION_SUFFIX`].
pub fn full_version() -> String {
    format!("{VERSION}{VERSION_SUFFIX}")
}

/// The versions built into the program, one per line: Guidedog, the Guidedoc engine and
/// its document schema, the tree-sitter grammars, and the Graphviz library. The crash
/// report starts with it; `--version` adds the Typst backend it finds.
pub fn version_text() -> String {
    let mut text = String::with_capacity(256);
    let _ = writeln!(text, "guidedog {}", full_version());
    let _ = writeln!(
        text,
        "guidedoc {} (document schema {})",
        guidedoc::VERSION,
        guidedoc::SCHEMA_VERSION
    );
    let grammars: Vec<String> = TREE_SITTER.iter().map(|(n, v)| format!("{n} {v}")).collect();
    let _ = writeln!(text, "{} (MIT; see --licenses)", grammars.join(", "));
    match gd_graphdog::Context::new() {
        Ok(graphviz) => {
            let linkage = if gd_graphdog::STATIC { "static" } else { "linked" };
            let _ = writeln!(text, "graphviz {} ({linkage})", graphviz.version());
        }
        Err(_) => text.push_str("graphviz: the linked library could not start\n"),
    }
    text
}

/// The Typst line of `--version`: the backend `program` selects.
pub(crate) fn typst_line(program: &str) -> String {
    match gd_typst::locate(program) {
        None => "typst not found\n".to_string(),
        Some(gd_typst::Backend::Embedded) => {
            format!("typst {} (embedded)\n", gd_typst::PINNED_VERSION)
        }
        Some(gd_typst::Backend::Program { path, version }) => format!(
            "typst {} (development backend: {})\n",
            version.as_deref().unwrap_or("of unknown version"),
            path.display()
        ),
    }
}

/// What `--licenses` prints: Guidedog's copyright and licence, then the notices of the
/// components built in (NOTICE's third-party section).
pub(crate) fn licenses_text() -> String {
    const NOTICE: &str = include_str!("../../../NOTICE");
    let third_party = NOTICE.find("Third-party components").map_or(NOTICE, |at| &NOTICE[at..]);
    format!(
        "guidedog {}\nCopyright (C) 2026 Vikrant Rathore, with assistance from Ronak Rathore and \
         Kanak Rathore.\nReleased under the GNU Affero General Public License v3.0 or later \
         (AGPL-3.0-or-later),\nwithout any warranty; see LICENSE and NOTICE in the source, at\n\
         https://github.com/insanai/guidedog. For other licensing terms, contact the \
         authors.\n\n{third_party}",
        full_version()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Release tooling reads the version from the file VERSION and checks a release tag
    /// against it, so the command must report the same version; graphdog and
    /// guidedog-pages are released under it too.
    #[test]
    fn the_version_matches_the_repository_version_file() {
        let file = include_str!("../../../VERSION").trim();
        assert_eq!(VERSION, file);
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "{VERSION} is not MAJOR.MINOR.PATCH");
        assert!(parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())));
        assert!(full_version().starts_with(VERSION));
    }

    #[test]
    fn the_version_text_names_every_component() {
        let text = version_text();
        assert!(text.starts_with(&format!("guidedog {VERSION}")), "{text}");
        assert!(text.contains("document schema"), "{text}");
        assert!(text.contains("tree-sitter-python"), "{text}");
        assert!(text.contains("graphviz"), "{text}");
        if gd_typst::EMBEDDED {
            assert_eq!(
                typst_line("typst"),
                format!("typst {} (embedded)\n", gd_typst::PINNED_VERSION)
            );
        }
        assert_eq!(typst_line("/nonexistent/typst"), "typst not found\n");
    }

    /// The tree-sitter versions `--version` names are the ones Cargo.lock pins.
    #[test]
    fn tree_sitter_versions_match_the_lock_file() {
        let lock = include_str!("../../../Cargo.lock");
        for (name, version) in TREE_SITTER {
            let entry = format!("name = \"{name}\"\nversion = \"{version}");
            assert!(lock.contains(&entry), "Cargo.lock has no {name} {version}.x");
        }
    }

    #[test]
    fn the_licenses_name_the_components() {
        let text = licenses_text();
        assert!(text.contains("AGPL-3.0-or-later") && text.contains("Typst"), "{text}");
    }
}
