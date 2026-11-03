//! What a build trusts, in its two parts (GDS 0018): the confinement (what it reads,
//! the network, plugins) and the content rules (raw content, unsafe URLs).

#![allow(clippy::unwrap_used)]

mod common;

use common::{TempDir, assert_built, demo, load, report_with};
use gd_project::{
    BuildOptions, BuilderKind, Confinement, ContentRule, ContentRules, PluginAccess, ReadScope,
};

const RAW: &str = "Guide\n=====\n\n.. raw:: html\n\n   <b id=\"raw\">raw</b>\n\n\
                   A `link <javascript:alert(1)>`_.\n";

fn build_with(dir: &TempDir, options: &BuildOptions) -> String {
    let r = gd_project::build(&load(&dir.0), BuilderKind::Html, options);
    assert_built(&r);
    dir.read("_build/html/guide.html")
}

fn with_conf(dir: &TempDir, extra: &str) {
    dir.put(
        "conf.toml",
        &format!(
            "project = \"Demo\"\nsource_suffix = [\".rst\", \".md\"]\n\
             extensions = [\"myst_parser\"]\n{extra}"
        ),
    );
}

/// conf.toml's rules decide unless the build imposes its own; `--untrusted` always
/// omits; a caller's defaults fill only what conf.toml leaves unset.
#[test]
fn content_rules_come_from_the_build_conf_toml_and_defaults() {
    let dir = demo("content-rules");
    dir.put("guide.rst", RAW);
    let trusted = BuildOptions { all: true, ..Default::default() };
    let page = build_with(&dir, &trusted);
    assert!(page.contains("id=\"raw\"") && page.contains("javascript:alert"), "{page}");
    with_conf(&dir, "raw_content = \"omit\"\nunsafe_urls = \"omit\"\n");
    let page = build_with(&dir, &trusted);
    assert!(!page.contains("id=\"raw\"") && !page.contains("javascript:alert"), "{page}");
    with_conf(&dir, "raw_content = \"allow\"\n");
    let untrusted = BuildOptions { untrusted: true, ..trusted.clone() };
    assert!(!build_with(&dir, &untrusted).contains("id=\"raw\""));
    let defaults = BuildOptions { content_defaults: ContentRules::OMIT, ..trusted.clone() };
    let page = build_with(&dir, &defaults);
    assert!(page.contains("id=\"raw\""), "conf.toml allows raw content: {page}");
    assert!(!page.contains("javascript:alert"), "unset in conf.toml, omitted by default");
    let imposed = ContentRules { raw: Some(ContentRule::Omit), ..Default::default() };
    let imposed = BuildOptions { content: imposed, ..trusted };
    assert!(!build_with(&dir, &imposed).contains("id=\"raw\""));
}

/// A build confined to a snapshot reads what lies inside it and nothing outside.
#[test]
fn a_confined_build_reads_only_inside_its_roots() {
    let top = TempDir::new("confined-roots");
    top.put("snapshot/shared/part.rst", "Shared text.\n");
    top.put("secret/part.rst", "Secret text.\n");
    top.put(
        "snapshot/doc/conf.toml",
        "project = \"Demo\"\ninclude_roots = [\"../shared\", \"../../secret\"]\n",
    );
    top.put(
        "snapshot/doc/index.rst",
        "Demo\n====\n\n.. include:: ../shared/part.rst\n\n.. include:: ../../secret/part.rst\n",
    );
    let project = load(&top.path("snapshot/doc"));
    let trusted = gd_project::build(&project, BuilderKind::Html, &BuildOptions::default());
    assert_built(&trusted);
    assert!(top.read("snapshot/doc/_build/html/index.html").contains("Secret text."));
    let confinement =
        Confinement { reads: ReadScope::Within(vec![top.path("snapshot")]), ..Default::default() };
    let options = BuildOptions { all: true, confinement, ..Default::default() };
    let r = gd_project::build(&project, BuilderKind::Html, &options);
    let dropped = report_with(&r, "build.untrusted").unwrap();
    assert!(dropped.contains("secret"), "{dropped}");
    let page = top.read("snapshot/doc/_build/html/index.html");
    assert!(page.contains("Shared text.") && !page.contains("Secret text."), "{page}");
}

/// Typst diagrams compile in an `--untrusted` build: the embedded compiler runs no
/// program and reads only the source folder.
#[test]
fn typst_diagrams_render_in_untrusted_builds() {
    let dir = demo("untrusted-typst");
    dir.put("guide.rst", "Guide\n=====\n\n.. typst-diagram::\n\n   #circle(radius: 6pt)\n");
    let untrusted = BuildOptions { untrusted: true, ..Default::default() };
    let r = gd_project::build(&load(&dir.0), BuilderKind::Html, &untrusted);
    assert_built(&r);
    assert!(report_with(&r, "build.typst-diagram").is_none(), "{:#?}", r.reports);
    assert!(dir.read("_build/html/guide.html").contains("_images/typst-"));
}

/// A build that starts only linked plugins drops one from the machine registry with a
/// warning; a build without the network fetches no inventory.
#[test]
fn plugins_and_the_network_follow_the_confinement() {
    let dir = demo("confined-plugins");
    dir.put(
        "conf.toml",
        "project = \"Demo\"\nsource_suffix = [\".rst\", \".md\"]\n\
         extensions = [\"myst_parser\", \"sphinx.ext.intersphinx\"]\n\
         plugins = [\"registered\"]\n\
         [intersphinx_mapping]\nother = [\"http://127.0.0.1:9/other/\", \"\"]\n",
    );
    let confinement =
        Confinement { plugins: PluginAccess::Linked, network: false, ..Default::default() };
    let options = BuildOptions { confinement, ..Default::default() };
    let r = gd_project::build(&load(&dir.0), BuilderKind::Html, &options);
    assert_built(&r);
    let dropped = report_with(&r, "build.untrusted").unwrap();
    assert!(dropped.contains("registered"), "{dropped}");
    let unfetched = r.reports.iter().find(|rep| rep.message.contains("uses no network"));
    assert!(unfetched.is_some(), "{:#?}", r.reports);
}
