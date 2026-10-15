//! A small build plugin served as a program, for the command-line tests of
//! `guidedog plugin`: it upper-cases `{{shout}}` markers in sources. Built by
//! `cargo test`, as examples are.

use std::borrow::Cow;

use gd_plugins::{BuildPlugin, Hooks, LoadContext, Plugin, PluginInfo, Problem, SourceInput};

#[derive(Debug, Clone, Default)]
struct Shout;

impl BuildPlugin for Shout {
    fn info(&self) -> PluginInfo {
        let hooks = Hooks { source: true, ..Hooks::NONE };
        PluginInfo::new("shout", "Shout fixture", "1.0.0", hooks)
    }

    fn load(&mut self, _ctx: &mut LoadContext<'_>) -> Result<(), Problem> {
        Ok(())
    }

    fn process_source<'a>(&mut self, input: &SourceInput<'a>) -> Result<Cow<'a, str>, Problem> {
        Ok(Cow::Owned(input.text.replace("{{shout}}", "HEY")))
    }
}

fn main() -> std::process::ExitCode {
    gd_plugins::serve_plugin(&Plugin::build(Shout))
}
