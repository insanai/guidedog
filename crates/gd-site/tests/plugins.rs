//! Build plugins in the HTML builders: every page passes through their HTML hooks,
//! and their assets are written below `_static/plugins/<id>/`.

#![allow(clippy::unwrap_used)]

mod common;

use std::borrow::Cow;

use common::{Project, section};
use gd_env::{BuilderKind, PluginRun, PluginStart};
use gd_plugins::{Asset, BuildPlugin, Hooks, HtmlInput, Plugin, PluginInfo, Problem, Registry};

/// Marks each page with its document and assets URL, and ships one stylesheet.
#[derive(Clone, Default)]
struct Badge;

impl BuildPlugin for Badge {
    fn info(&self) -> PluginInfo {
        PluginInfo::new(
            "badge",
            "Badge",
            "1.0.0",
            Hooks { html: true, assets: true, source: false },
        )
    }

    fn process_html<'a>(&mut self, input: &HtmlInput<'a>) -> Result<Cow<'a, str>, Problem> {
        let mark = format!("<!-- badge {} {} -->", input.docname, input.assets_url);
        Ok(Cow::Owned(input.text.replace("</body>", &format!("{mark}</body>"))))
    }

    fn collect_assets(&mut self) -> Result<Vec<Asset>, Problem> {
        Ok(vec![Asset::new("badge.css", "b{}")])
    }
}

#[test]
fn pages_pass_through_html_hooks_and_assets_are_written() {
    let mut p = Project::new("plugins");
    p.add("guide/intro.rst", "Intro", "Intro\n=====\n\nText.\n").sections =
        vec![section(1, "Intro", "intro", None)];
    p.add("index.rst", "Home", "Home\n====\n").sections = vec![section(1, "Home", "home", None)];
    p.doc("guide/intro").orphan = true;
    p.resolve();
    p.config.plugins.ids = vec!["badge".into()];
    let linked = Registry::new(vec![Plugin::build(Badge)]).unwrap();
    let start = PluginStart {
        config: &p.config,
        linked: &linked,
        kind: BuilderKind::Html,
        untrusted: false,
    };
    p.plugins = PluginRun::start(start).unwrap();
    p.build(BuilderKind::Html).unwrap();
    let intro = p.read(BuilderKind::Html, "guide/intro.html");
    assert!(intro.contains("<!-- badge guide/intro ../_static/plugins/badge/ -->"), "{intro}");
    let search = p.read(BuilderKind::Html, "search.html");
    assert!(search.contains("<!-- badge search.html _static/plugins/badge/ -->"), "{search}");
    assert_eq!(p.read(BuilderKind::Html, "_static/plugins/badge/badge.css"), "b{}");
}
