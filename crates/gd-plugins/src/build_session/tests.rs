use super::*;
use crate::{Capabilities, Hooks, LoadOptions, Loaded, Outcome, Plan, Publisher, PublisherInfo};
use crate::{Snapshot, SourceFormat};

#[derive(Clone, Default)]
struct Fixture {
    loaded: String,
    hooks: Option<Hooks>,
}

impl BuildPlugin for Fixture {
    fn info(&self) -> PluginInfo {
        let hooks = self.hooks.unwrap_or(Hooks { source: true, html: false, assets: true });
        PluginInfo::new("test-build", "Test build", "1.0", hooks)
    }

    fn load(&mut self, ctx: &mut LoadContext<'_>) -> Result<(), Problem> {
        ctx.add_cache_key("loaded");
        self.loaded = ctx.cache_key().to_string();
        Ok(())
    }

    fn process_source<'a>(&mut self, input: &SourceInput<'a>) -> Result<Cow<'a, str>, Problem> {
        Ok(Cow::Owned(format!("{}:{}", self.loaded, input.text)))
    }

    fn collect_assets(&mut self) -> Result<Vec<Asset>, Problem> {
        Ok(vec![Asset::new("badge.css", "body{}")])
    }
}

fn source(text: &str) -> SourceInput<'_> {
    SourceInput { text, ..SourceInput::default() }
}

#[test]
fn build_results_own_their_text_and_assets() {
    let registry = Registry::new(vec![Plugin::build(Fixture::default())]).unwrap();
    let mut s = load_plugin(&registry, "test-build", &PluginOptions::default()).unwrap();
    assert_eq!(s.cache_key(), "loaded");
    let text = s.process_source(&source("hello")).unwrap().into_owned();
    let assets = s.collect_assets().unwrap();
    drop(s);
    assert_eq!(text, "loaded:hello");
    assert_eq!(assets[0].data, b"body{}");
}

#[test]
fn absent_hooks_pass_text_through_without_copying() {
    let plugin = Plugin::build(Fixture {
        hooks: Some(Hooks { assets: true, ..Hooks::NONE }),
        ..Fixture::default()
    });
    let mut s = PluginSession::load(&plugin, &PluginOptions::default()).unwrap();
    let text = s.process_source(&source("same")).unwrap();
    assert!(matches!(text, Cow::Borrowed("same")));
    let html = HtmlInput { text: "<p>x</p>", ..HtmlInput::default() };
    assert!(matches!(s.process_html(&html).unwrap(), Cow::Borrowed(_)));
}

#[derive(Clone)]
struct Nothing;

impl Publisher for Nothing {
    fn info(&self) -> PublisherInfo {
        PublisherInfo::new("test-build", "Test", "1.0", Capabilities::default())
    }
    fn load(&mut self, _: &LoadOptions) -> Result<Loaded, Problem> {
        Ok(Loaded::default())
    }
    fn plan(&mut self, _: &Snapshot, _: &mut Plan) -> Result<(), Problem> {
        Ok(())
    }
    fn publish(&mut self, _: &Snapshot, _: &Plan) -> Outcome {
        Outcome::default()
    }
}

#[test]
fn build_and_publisher_plugins_are_separate_services() {
    Plugin::build(Fixture::default()).validate().unwrap();
    let publisher = Plugin::publisher(Nothing);
    publisher.validate().unwrap();
    let p = PluginSession::load(&publisher, &PluginOptions::default()).unwrap_err();
    assert!(p.hint.contains("guidedog publish"));
    let no_hooks = Plugin::build(Fixture { hooks: Some(Hooks::NONE), ..Fixture::default() });
    assert!(no_hooks.validate().is_err());
    let twice = Registry::new(vec![Plugin::build(Fixture::default()), publisher]);
    assert_eq!(twice.unwrap_err().code, "plugins.contract");
}

#[test]
fn hook_problems_name_the_plugin_and_need_directions() {
    #[derive(Clone)]
    struct Failing(Problem);
    impl BuildPlugin for Failing {
        fn info(&self) -> PluginInfo {
            PluginInfo::new("failing", "Failing", "1", Hooks { source: true, ..Hooks::NONE })
        }
        fn process_source<'a>(&mut self, _: &SourceInput<'a>) -> Result<Cow<'a, str>, Problem> {
            Err(self.0.clone())
        }
    }
    let good = Problem::new("failing.x", "Bad input", "It broke.").hint("Fix it.").exit(2);
    let plugin = Plugin::build(Failing(good));
    let mut s = PluginSession::load(&plugin, &PluginOptions::default()).unwrap();
    let input = SourceInput { path: "index.rst", ..source("x") };
    let p = s.process_source(&input).unwrap_err();
    assert!(p.message.starts_with("Plugin failing stopped during source."));
    assert_eq!(p.path, "index.rst");
    let bare = Plugin::build(Failing(crate::ProblemData { exit: 2, ..Default::default() }.into()));
    let mut s = PluginSession::load(&bare, &PluginOptions::default()).unwrap();
    assert_eq!(s.process_source(&input).unwrap_err().code, "plugins.contract");
}

#[test]
fn cancellation_and_limits_stop_hooks() {
    let plugin = Plugin::build(Fixture::default());
    let options = PluginOptions { cancel: crate::Cancel::from_fn(|| true), ..Default::default() };
    assert_eq!(PluginSession::load(&plugin, &options).unwrap_err().code, "plugins.cancelled");
    let mut s = PluginSession::load(&plugin, &PluginOptions::default()).unwrap();
    let big = "x".repeat(MAX_PLUGIN_TEXT + 1);
    assert_eq!(s.process_source(&source(&big)).unwrap_err().code, "plugins.contract");
    let long = "p".repeat(MAX_PATH + 1);
    let input = SourceInput { path: &long, format: SourceFormat::Markdown, ..source("x") };
    assert!(s.process_source(&input).is_err());
}
