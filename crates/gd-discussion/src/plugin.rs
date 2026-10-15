//! The discussion plugin: `guidedog dsc` and its commands, linked into guidedog. It adds
//! commands only, no build hooks.

use gd_plugins::{BuildPlugin, CommandContext, CommandInfo, Hooks, Plugin, PluginInfo, Problem};

/// The plugin's ID, which is also the first word of its commands.
pub const ID: &str = "dsc";

/// The discussion plugin, for a program to link: `guidedog dsc init`, `new`, `list`,
/// `show`, `promote`, `state`, `index`, `check`, `build`, `recover`, and `config`.
pub fn plugin() -> Plugin {
    Plugin::build(Discussions)
}

/// The plugin's value: it keeps nothing between commands.
#[derive(Debug, Clone, Copy, Default)]
struct Discussions;

impl BuildPlugin for Discussions {
    fn info(&self) -> PluginInfo {
        PluginInfo::new(ID, "Discussion catalogs", env!("CARGO_PKG_VERSION"), Hooks::NONE)
    }

    fn commands(&self) -> Vec<CommandInfo> {
        crate::command::commands()
    }

    fn run_command(
        &mut self,
        ctx: &mut CommandContext<'_>,
        name: &str,
        args: &[String],
    ) -> Result<i32, Problem> {
        match name.strip_prefix("dsc ") {
            Some(word) => Ok(crate::command::run(ctx, word, args)),
            None => Err(Problem::new(
                "dsc.command",
                "The discussion plugin has no such command",
                &format!("There is no command {name:?}."),
            )
            .hint("Run guidedog dsc --help to see the commands.")
            .exit(2)),
        }
    }
}
