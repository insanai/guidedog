//! Unit tests of the build's helpers.

use std::path::PathBuf;

use gd_conf::Config;
use gd_report::{Report, Severity};

use crate::finish::unsuppressed;

fn warning(category: &str) -> Report {
    Report { severity: Severity::Warning, category: category.into(), ..Report::default() }
}

/// suppress_warnings follows Sphinx: `type` silences the type, `type.sub` one subtype,
/// `type.*` every subtype; errors and uncategorized warnings are always kept.
#[test]
fn suppress_warnings_follows_sphinx() {
    let reports = vec![
        warning("ref.ref"),
        warning("ref.doc"),
        warning("toc.not_readable"),
        warning("autodoc"),
        warning(""),
        Report { severity: Severity::Error, category: "ref.ref".into(), ..Report::default() },
    ];
    let kept = |suppress: &[&str]| -> Vec<String> {
        let suppress: Vec<String> = suppress.iter().map(|s| s.to_string()).collect();
        unsuppressed(reports.clone(), &suppress)
            .iter()
            .map(|r| format!("{:?}:{}", r.severity, r.category))
            .collect()
    };
    assert_eq!(kept(&[]).len(), 6);
    assert_eq!(
        kept(&["ref"]),
        ["Warning:toc.not_readable", "Warning:autodoc", "Warning:", "Error:ref.ref"]
    );
    assert_eq!(kept(&["ref.doc"]).len(), 5);
    // A warning of the bare type is silenced by any of its subtypes, as in Odin.
    assert_eq!(kept(&["ref.*", "autodoc.import_object"]).len(), 3);
    assert_eq!(kept(&["toc.not_readable", "autodoc"]).len(), 4);
}

/// serve watches the source folder, the API sources, and folders outside the source
/// folder, never the build folder.
#[test]
fn the_watched_roots_are_the_project_inputs() {
    let source = PathBuf::from("/p/docs");
    let mut config = Config::new(source.clone(), source.join("_build"));
    config.html.static_path = vec!["_static".into(), "../shared".into()];
    config.templates_path = vec!["../tpl".into()];
    config.odin_api.dirs = vec!["../src".into()];
    config.rust_api.crates = vec!["../crates/demo".into()];
    config.intl.locale_dirs = vec!["locales".into()];
    config.include_roots = vec!["_build".into()];
    let roots = crate::watch_roots(&config);
    let expected: Vec<PathBuf> = ["/p/docs", "/p/src", "/p/crates/demo", "/p/shared", "/p/tpl"]
        .iter()
        .map(PathBuf::from)
        .collect();
    assert_eq!(roots, expected);
}

#[test]
fn untrusted_builds_omit_raw_content() {
    let p = crate::untrusted_policy();
    assert_eq!(p.raw, guidedoc::ContentPolicy::Omit);
    assert_eq!(p.unsafe_urls, guidedoc::ContentPolicy::Omit);
    assert!(p.keep_documents_with_errors);
    assert_eq!(crate::trusted_policy().raw, guidedoc::ContentPolicy::Allow);
}
