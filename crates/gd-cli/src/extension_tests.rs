//! Commands plugins add: found, parsed, listed in the help, suggested, and run with
//! guidedog's console, through a linked plugin that adds `pages workflow` and the
//! publish target `pages`, as the GitHub Pages plugin does.

use gd_plugins::{
    BuildPlugin, CommandContext, CommandInfo, CommandOption, Hooks, Plugin, PluginInfo, Problem,
    Registry,
};

use super::*;

/// A stand-in for the Pages plugin: its commands echo their arguments.
#[derive(Debug, Clone, Default)]
struct FakePages;

impl BuildPlugin for FakePages {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("pages", "Fake Pages", "1.0.0", Hooks { source: true, ..Hooks::NONE })
    }

    fn commands(&self) -> Vec<CommandInfo> {
        vec![
            CommandInfo::new("publish pages", "Publish to GitHub Pages")
                .with_help("Publishes the site.")
                .with_option(CommandOption::with_value("--repository", "OWNER/REPO", "Where")),
            CommandInfo::new("pages workflow", "Print the workflow")
                .with_option(CommandOption::switch("--check", "Check it")),
        ]
    }

    fn run_command(
        &mut self,
        ctx: &mut CommandContext<'_>,
        name: &str,
        args: &[String],
    ) -> Result<i32, Problem> {
        if args.iter().any(|a| a == "--fail") {
            return Err(Problem::new("pages.x", "It failed", "On purpose.").hint("Fix.").exit(6));
        }
        ctx.print(&format!("{name}: {}\n", args.join(" ")));
        Ok(0)
    }
}

fn linked() -> Registry {
    Registry::new(vec![Plugin::build(FakePages)]).unwrap_or_else(|p| panic!("{p}"))
}

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

fn parsed(words: &[&str]) -> Result<Command, gd_host::Problem> {
    parse_command_with(&args(words), &extensions(&linked(), false)).command
}

fn run(words: &[&str]) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut input: &[u8] = b"";
    let streams = Streams {
        out: &mut out,
        err: &mut err,
        input: &mut input,
        terminals: Terminals::default(),
    };
    let status = run_in(&args(words), &linked(), streams);
    (status, String::from_utf8_lossy(&out).into(), String::from_utf8_lossy(&err).into())
}

#[test]
fn plugin_commands_are_named_by_their_words() {
    // An option's value is never taken for the target: `--config pages` is publish's.
    let Ok(Command::Extension(e)) = parsed(&["publish", "--repository", "o/r", "pages", "--yes"])
    else {
        panic!("not the publish target");
    };
    assert_eq!(
        (e.extension.plugin.as_str(), e.extension.name()),
        ("pages", "publish pages".into())
    );
    assert_eq!(e.args, args(&["--repository", "o/r", "--yes"]));
    assert!(!e.help);
    assert!(matches!(
        parsed(&["publish", "--publisher", "pages", "--config=c"]),
        Ok(Command::Publish(_))
    ));
    let Ok(Command::Extension(e)) = parsed(&["pages", "workflow", "--check", "--help"]) else {
        panic!("not the workflow");
    };
    assert!(e.help);
    assert!(matches!(parsed(&["pages", "--help"]), Ok(Command::ExtensionHelp(w)) if w == "pages"));
}

#[test]
fn mistakes_suggest_plugin_commands_too() {
    let problem = |words: &[&str]| parsed(words).err().unwrap_or_else(|| panic!("{words:?}"));
    assert!(problem(&["pagse"]).hint.contains("Did you mean `pages`?"));
    assert!(problem(&["biuld"]).hint.contains("Did you mean `build`?"));
    let p = problem(&["pages", "workflo"]);
    assert_eq!(p.title, "UNKNOWN COMMAND");
    assert!(p.hint.contains("Did you mean `workflow`?"), "{}", p.hint);
    let p = problem(&["publish", "page"]);
    assert_eq!(p.title, "UNKNOWN PUBLISH TARGET");
    assert!(p.hint.contains("Did you mean `pages`?"), "{}", p.hint);
    assert_eq!(problem(&["pages"]).title, "NAME A COMMAND");
}

#[test]
fn plugin_commands_run_and_show_their_help() {
    let (status, out, err) = run(&["pages", "workflow", "--check"]);
    assert_eq!((status, out.as_str()), (0, "pages workflow: --check\n"), "{err}");
    let (status, out, _) = run(&["publish", "pages", "--help"]);
    assert_eq!(status, 0);
    assert!(out.contains("guidedog publish pages [options]"), "{out}");
    assert!(out.contains("Added by the plugin pages (linked into this program)"), "{out}");
    assert!(out.contains("--repository=OWNER/REPO"), "{out}");
    let (status, _, err) = run(&["pages", "workflow", "--fail"]);
    assert_eq!(status, 6);
    assert!(err.contains("pages.x") || err.contains("IT FAILED"), "{err}");
    let (_, out, _) = run(&["--help"]);
    assert!(out.contains("PLUGIN COMMANDS"), "{out}");
    assert!(out.contains("pages workflow") && out.contains("(pages)"), "{out}");
    let (_, out, _) = run(&["publish", "--help"]);
    assert!(out.contains("TARGETS ADDED BY PLUGINS") && out.contains("publish pages"), "{out}");
    let (_, out, _) = run(&["pages", "--help"]);
    assert!(out.contains("guidedog pages COMMAND") && out.contains("pages workflow"), "{out}");
}
