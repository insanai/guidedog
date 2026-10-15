//! `check_plugin`: exercise a build plugin with representative inputs.

use std::path::Path;

use crate::problem::{self, Problem};
use crate::{HtmlInput, Plugin, PluginOptions, PluginSession, SourceFormat, SourceInput};

const RST: &str = "Plugin check\n============\n\nA small document.\n";
const MARKDOWN: &str = "# Plugin check\n\nA small document.\n";
const PAGE: &str = "<!doctype html><html><head><title>Plugin check</title></head>\
                    <body><p>A small page.</p></body></html>";

/// Exercises a build plugin's declared hooks: two fresh loads, each running every
/// hook twice on representative documents and checking the results repeat, then
/// unloading. It never invokes a publisher. Passing shows interoperability, not
/// safety: a linked plugin runs in this process; check an installed program through
/// [`crate::process_plugin`] for crash isolation.
pub fn check_plugin(plugin: &Plugin, options: &PluginOptions) -> Result<(), Problem> {
    for _ in 0..2 {
        let mut session = PluginSession::load(plugin, options)?;
        check_session(&mut session)?;
    }
    Ok(())
}

fn check_session(s: &mut PluginSession) -> Result<(), Problem> {
    let project = Path::new(&s.options().project).to_path_buf();
    let language = s.options().language.clone();
    let id = s.info().id.clone();
    let hooks = s.info().hooks;
    if hooks.source {
        for (format, text, file) in [
            (SourceFormat::Rst, RST, "plugin-check.rst"),
            (SourceFormat::Markdown, MARKDOWN, "plugin-check.md"),
        ] {
            let path = project.join(file).to_string_lossy().into_owned();
            let input = SourceInput {
                path: &path,
                docname: "plugin-check",
                format,
                language: &language,
                text,
            };
            let a = s.process_source(&input)?.into_owned();
            let b = s.process_source(&input)?;
            repeatable(&id, a == b)?;
        }
    }
    if hooks.html {
        let path = project.join("_build/html/plugin-check.html").to_string_lossy().into_owned();
        let assets_url = format!("_static/plugins/{id}/");
        let input = HtmlInput {
            path: &path,
            docname: "plugin-check",
            language: &language,
            text: PAGE,
            assets_url: &assets_url,
        };
        let a = s.process_html(&input)?.into_owned();
        let b = s.process_html(&input)?;
        repeatable(&id, a == b)?;
    }
    if hooks.assets {
        let a = s.collect_assets()?;
        let b = s.collect_assets()?;
        if a != b {
            return Err(problem::contract(
                &id,
                "Keep assets in stable order with the same paths and bytes for identical \
                 loaded settings.",
            ));
        }
    }
    Ok(())
}

fn repeatable(id: &str, same: bool) -> Result<(), Problem> {
    if same {
        return Ok(());
    }
    Err(problem::contract(
        id,
        "Keep hook results repeatable; load changing values in session state before processing.",
    ))
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::{BuildPlugin, Hooks, PluginInfo};

    #[derive(Clone, Default)]
    struct Paths {
        calls: usize,
    }

    impl BuildPlugin for Paths {
        fn info(&self) -> PluginInfo {
            PluginInfo::new(
                "test-build",
                "Test",
                "1.0",
                Hooks { source: true, html: true, assets: false },
            )
        }

        fn process_source<'a>(
            &mut self,
            input: &SourceInput<'a>,
        ) -> Result<Cow<'a, str>, Problem> {
            let suffix = if input.format == SourceFormat::Rst { ".rst" } else { ".md" };
            if !input.path.ends_with(suffix) {
                return Err(problem::contract("test-build", "Pass the source suffix."));
            }
            Ok(Cow::Borrowed(input.text))
        }

        fn process_html<'a>(&mut self, input: &HtmlInput<'a>) -> Result<Cow<'a, str>, Problem> {
            self.calls += 1;
            if !input.path.ends_with(".html") {
                return Err(problem::contract("test-build", "Pass the page's path."));
            }
            Ok(Cow::Owned(format!("{}{}", input.text, self.calls)))
        }
    }

    #[test]
    fn checks_supply_paths_and_refuse_unrepeatable_hooks() {
        let options = PluginOptions { project: "docs".into(), ..PluginOptions::default() };
        let p = check_plugin(&Plugin::build(Paths::default()), &options).unwrap_err();
        assert!(p.hint.contains("repeatable"), "{p}");
    }
}
