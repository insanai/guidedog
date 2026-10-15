//! The example build plugin and a faulty one, run as programs through the protocol.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

mod support;

use std::time::Duration;

use gd_plugins::{
    BUILD_PROTOCOL_NAME, Hooks, HtmlInput, PluginOptions, PluginSession, SourceInput,
    check_plugin, inspect_entry, process_plugin, read_machine, register, verify_entry,
};
use support::Scratch;

#[test]
fn an_installed_build_plugin_transforms_pages_and_supplies_assets() {
    let root = Scratch::new("badge");
    let program = root.program("build_plugin", "product-badge");
    let entry = inspect_entry(&program).unwrap();
    assert_eq!(entry.protocol_name, BUILD_PROTOCOL_NAME);
    assert_eq!(entry.hooks, ["source", "html", "assets"]);
    root.write("docs/badge.toml", "product = \"Orbit & Co\"\n");
    let options = PluginOptions {
        config: "badge.toml".into(),
        project: root.join("docs").to_string_lossy().into(),
        builder: "html".into(),
        language: "en".into(),
        ..PluginOptions::default()
    };
    let plugin = process_plugin(&entry).unwrap();
    assert_eq!(plugin.build_info().unwrap().hooks, Hooks::ALL);
    check_plugin(&plugin, &options).unwrap();

    let mut session = PluginSession::load(&plugin, &options).unwrap();
    assert_eq!(session.dependencies().len(), 1);
    assert!(session.cache_key().starts_with(':'));
    let source =
        SourceInput { path: "index.rst", text: "Hello {{product}}.", ..Default::default() };
    assert_eq!(session.process_source(&source).unwrap(), "Hello Orbit & Co.");
    let page = HtmlInput {
        path: "index.html",
        text: "<html><head></head><body><p>x</p></body></html>",
        assets_url: "_static/plugins/product-badge/",
        ..Default::default()
    };
    let html = session.process_html(&page).unwrap();
    assert!(html.contains("href=\"_static/plugins/product-badge/banner.css\""), "{html}");
    assert!(html.contains(">Orbit &amp; Co</aside>"), "{html}");
    let assets = session.collect_assets().unwrap();
    assert_eq!(assets[0].path, "banner.css");
    drop(session);

    let registry = root.join("state/plugins.toml");
    register(&registry, &entry).unwrap();
    let saved = read_machine(&registry).unwrap();
    verify_entry(saved.find("product-badge").unwrap()).unwrap();
}

#[test]
fn a_bad_configuration_is_explained_by_the_plugin() {
    let root = Scratch::new("badge-config");
    let program = root.program("build_plugin", "product-badge");
    let plugin = process_plugin(&inspect_entry(&program).unwrap()).unwrap();
    let config = root.write("badge.toml", "product = 42\n");
    let options = PluginOptions { config: config.to_string_lossy().into(), ..Default::default() };
    let p = PluginSession::load(&plugin, &options).unwrap_err();
    assert_eq!(p.code, "badge.product");
    assert!(p.message.contains("product-badge stopped during load"), "{p}");
}

// Crashes, panics, stalls and malformed problems in a child become problems; none of
// them reaches the host's memory.
#[test]
fn failed_build_plugin_processes_do_not_crash_the_host() {
    let root = Scratch::new("broken");
    let program = root.program("protocol_fixture", "broken-build");
    let plugin = process_plugin(&inspect_entry(&program).unwrap()).unwrap();
    let config = root.join("mode.txt");
    let options = PluginOptions {
        config: config.to_string_lossy().into(),
        request_timeout: Duration::from_millis(500),
        ..PluginOptions::default()
    };
    let input = SourceInput { path: "index.rst", text: "Title\n=====\n", ..Default::default() };
    for (mode, code) in [
        ("exit", "plugins.protocol-closed"),
        ("bounds", "plugins.protocol-closed"),
        ("stall", "plugins.timeout"),
        ("bad-problem", "plugins.contract"),
    ] {
        std::fs::write(&config, mode).unwrap();
        let mut session = PluginSession::load(&plugin, &options).unwrap();
        let p = session.process_source(&input).unwrap_err();
        assert_eq!(p.code, code, "{mode}: {p}");
        assert!((1..=4).contains(&p.exit), "{mode}: {}", p.exit);
        assert!(p.message.contains("broken-build"), "{mode}: {p}");
    }
    std::fs::write(&config, "ok").unwrap();
    check_plugin(&plugin, &options).unwrap();
}

#[test]
fn a_publisher_program_is_not_a_build_plugin() {
    let root = Scratch::new("kinds");
    let program = root.program("publisher", "folder-publisher");
    let entry = inspect_entry(&program).unwrap();
    assert!(!entry.is_build());
    assert!(process_plugin(&entry).is_err());
}
