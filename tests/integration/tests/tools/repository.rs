//! Repository files that must agree with each other: the manual's downloadable copies,
//! the release version, the theme's words in the UI catalogs, and the public plugin
//! catalog.

use guidedog_tests::{read, repository_root, repository_text};

/// The manual's repository page offers downloads of repository documents, kept as
/// copies under `_static/repository` because a documentation project publishes only its
/// own files. Each copy must be its original, byte for byte.
#[test]
fn repository_copies_in_the_manual_match_their_originals() {
    let copies = [
        ("LICENSE", "LICENSE"),
        ("NOTICE", "NOTICE"),
        ("crates/guidedoc/README.md", "library-README.md"),
        ("docs/gds/README.md", "gds-README.md"),
        ("crates/gd-discussion/README.md", "discussion-README.md"),
        ("crates/gd-plugins/examples/publisher.rs", "publisher.rs"),
        ("crates/gd-plugins/examples/build_plugin/main.rs", "build-plugin-main.rs"),
        ("crates/gd-plugins/examples/build_plugin/badge.rs", "build-plugin-badge.rs"),
    ];
    let root = repository_root();
    for (original, copy) in copies {
        let copy_path = format!("docs/manual/_static/repository/{copy}");
        assert!(
            read(&root.join(original)) == read(&root.join(&copy_path)),
            "{copy_path} differs from {original}; run: cp {original} {copy_path}"
        );
    }
    // The theme tutorial's downloadable files are the ones the tutorial shows.
    compare_trees(
        &root.join("docs/manual/template/theme"),
        &root.join("docs/manual/_static/repository/theme"),
    );
}

/// Panics unless every file below `original` has an identical copy below `copy`.
fn compare_trees(original: &std::path::Path, copy: &std::path::Path) {
    for entry in std::fs::read_dir(original).unwrap() {
        let entry = entry.unwrap();
        let (from, to) = (entry.path(), copy.join(entry.file_name()));
        if entry.file_type().unwrap().is_dir() {
            compare_trees(&from, &to);
        } else {
            assert!(read(&from) == read(&to), "{} differs from {}", to.display(), from.display());
        }
    }
}

/// Every version a release reports is the one in VERSION: the documentation projects'
/// release and the crates' (`version.workspace`, checked by the programs' --version).
#[test]
fn release_versions_follow_the_version_file() {
    let version = repository_text("VERSION").trim().to_string();
    assert!(!version.is_empty());
    for conf_path in [
        "docs/conf.toml",
        "docs/manual/conf.toml",
        "docs/api/conf.toml",
        "docs/gds/conf.toml",
        "docs/site/conf.toml",
    ] {
        let conf = repository_text(conf_path);
        assert!(
            conf.contains(&format!("\nrelease = \"{version}\"\n")),
            "{conf_path} does not set release = \"{version}\""
        );
    }
    let cargo = repository_text("Cargo.toml");
    assert!(cargo.contains(&format!("\nversion = \"{version}\"\n")), "Cargo.toml's version");
}

/// Every word the built-in HTML templates translate with `_()` is a msgid of the UI
/// template, so each embedded catalog translates it (gd-i18n checks the catalogs
/// against the template).
#[test]
fn theme_words_are_in_the_ui_catalog_template() {
    let template = repository_text("crates/gd-i18n/locale/guidedog.pot");
    for name in ["layout.html", "article.html", "landing.html"] {
        let html = repository_text(&format!("crates/gd-theme/assets/{name}"));
        for word in translated_words(&html) {
            assert!(
                template.contains(&format!("\nmsgid \"{word}\"\n")),
                "{name} translates {word:?}; add it to crates/gd-i18n/locale/guidedog.pot \
                 and to each catalog beside it"
            );
        }
    }
}

/// The literal arguments of `_('...')` and `_("...")` in a Jinja template, outside its
/// `{# ... #}` comments.
fn translated_words(template: &str) -> Vec<&str> {
    let mut code = Vec::new();
    let mut rest = template;
    while let Some(start) = rest.find("{#") {
        code.push(&rest[..start]);
        rest = rest[start..].find("#}").map_or("", |end| &rest[start + end + 2..]);
    }
    code.push(rest);
    let mut words = Vec::new();
    for part in code {
        for quote in ['\'', '"'] {
            let open = format!("_({quote}");
            let close = format!("{quote})");
            let calls = part.split(open.as_str()).skip(1);
            words.extend(calls.filter_map(|call| call.split_once(close.as_str())).map(|w| w.0));
        }
    }
    words
}

/// The public catalog is reviewed metadata: unique ids, a known status, and for a
/// released plugin the releases that carry it, none later than this one.
#[test]
fn the_public_plugin_catalog_is_well_formed() {
    let text = repository_text("plugins/registry.toml");
    let catalog = gd_toml::parse(&text).unwrap_or_else(|e| panic!("registry.toml: {e}"));
    let entries = catalog.get("plugins").and_then(|v| v.as_array()).unwrap_or_default();
    assert!(!entries.is_empty(), "plugins/registry.toml lists no plugins");
    let current = version_numbers(repository_text("VERSION").trim());
    let mut seen = std::collections::HashSet::new();
    for entry in entries {
        let text_of = |key: &str| entry.get(key).and_then(|v| v.as_str()).unwrap_or_default();
        let id = text_of("id");
        assert!(gd_plugins::is_valid_id(id), "{id:?} is not a valid plugin id");
        assert!(seen.insert(id), "{id} is listed twice");
        let status = text_of("status");
        assert!(["planned", "released", "example"].contains(&status), "{id}: {status}");
        let releases = entry.get("releases").and_then(|v| v.as_array());
        let releases = releases.unwrap_or_else(|| panic!("{id} has no releases array"));
        if status != "released" {
            assert!(releases.is_empty(), "{id} is {status} but names releases");
            continue;
        }
        assert!(!releases.is_empty(), "{id} is released but names no release");
        for release in releases {
            let v = release.get("version").and_then(|v| v.as_str()).unwrap_or_default();
            assert!(version_numbers(v) <= current, "{id}: release {v:?} is in the future");
        }
    }
}

/// A MAJOR.MINOR.PATCH version as numbers, which compare in release order.
fn version_numbers(version: &str) -> Vec<u64> {
    version.split('.').map(|n| n.parse().unwrap_or(u64::MAX)).collect()
}
