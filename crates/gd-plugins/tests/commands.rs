//! Contributed commands: declared, listed, and run, by a linked plugin and by the same
//! plugin served as an installed program.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

mod support;

#[path = "../examples/build_plugin/badge.rs"]
mod badge;

use std::io::Write;

use gd_plugins::{
    BuildPlugin, BuildRequest, COMMAND_PROTOCOL_VERSION, CommandContext, CommandHost, CommandInfo,
    Hooks, Plugin, PluginInfo, Problem, Registry, inspect_entry, process_plugin, read_machine,
    register,
};
use support::Scratch;

/// A host that keeps what a command writes and answers from a script.
#[derive(Default)]
struct Captured {
    out: Vec<u8>,
    err: Vec<u8>,
    answers: Vec<String>,
    builds: Vec<BuildRequest>,
}

impl CommandHost for Captured {
    fn streams(&mut self) -> (&mut dyn Write, &mut dyn Write) {
        (&mut self.out, &mut self.err)
    }

    fn read_line(&mut self) -> Option<String> {
        (!self.answers.is_empty()).then(|| self.answers.remove(0))
    }

    fn build_project(&mut self, request: &BuildRequest) -> Result<(), Problem> {
        self.builds.push(request.clone());
        Ok(())
    }
}

/// A plugin whose command asks a question and has the host build a project.
#[derive(Clone, Default)]
struct Asker;

impl BuildPlugin for Asker {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("asker", "Asker", "1.0.0", Hooks { source: true, ..Hooks::NONE })
    }

    fn commands(&self) -> Vec<CommandInfo> {
        vec![CommandInfo::new("publish asker", "Ask, then build")]
    }

    fn run_command(
        &mut self,
        ctx: &mut CommandContext<'_>,
        _name: &str,
        args: &[String],
    ) -> Result<i32, Problem> {
        if !ctx.confirm("Build?") {
            return Ok(1);
        }
        let request =
            BuildRequest { project: "docs".into(), output: "out".into(), ..Default::default() };
        ctx.build_project(&request)?;
        ctx.print(&format!("built with {}\n", args.join(" ")));
        Ok(0)
    }
}

#[test]
fn linked_commands_are_listed_and_run_with_the_hosts_services() {
    let registry = Registry::new(vec![badge::plugin(), Plugin::build(Asker)]).unwrap();
    let names: Vec<(String, String)> =
        registry.commands().map(|(p, c)| (p.id().to_string(), c.name())).collect();
    assert_eq!(
        names,
        [
            ("product-badge".to_string(), "badge preview".to_string()),
            ("asker".to_string(), "publish asker".to_string())
        ]
    );
    let asker = registry.find("asker").unwrap();
    let mut host = Captured { answers: vec!["y".into()], ..Captured::default() };
    let mut ctx = CommandContext::new(&mut host);
    ctx.interactive = true;
    let status = asker.run_command(&mut ctx, "publish asker", &["--fast".to_string()]).unwrap();
    assert_eq!(status, 0);
    assert_eq!(host.builds.len(), 1);
    assert_eq!(String::from_utf8_lossy(&host.out), "built with --fast\n");
    assert!(String::from_utf8_lossy(&host.err).contains("Build? [y/N]"));
    // Without a person to ask, the question is answered no.
    let mut host = Captured { answers: vec!["y".into()], ..Captured::default() };
    let mut ctx = CommandContext::new(&mut host);
    assert_eq!(asker.run_command(&mut ctx, "publish asker", &[]).unwrap(), 1);
    let p = asker.run_command(&mut ctx, "publish other", &[]).unwrap_err();
    assert!(host.builds.is_empty());
    assert_eq!((p.code.as_str(), p.exit), ("plugins.command", 2));
}

#[test]
fn invalid_command_declarations_make_no_registry() {
    #[derive(Clone, Default)]
    struct Bad;
    impl BuildPlugin for Bad {
        fn info(&self) -> PluginInfo {
            PluginInfo::new("bad", "Bad", "1", Hooks { source: true, ..Hooks::NONE })
        }
        fn commands(&self) -> Vec<CommandInfo> {
            vec![CommandInfo::new("Bad Words", "Upper case")]
        }
    }
    let p = Registry::new(vec![Plugin::build(Bad)]).unwrap_err();
    assert_eq!(p.code, "plugins.contract");
}

// One implementation, two deployments: the badge's command linked into this process,
// and the same command run through the installed program, write the same preview.
#[test]
fn installed_programs_advertise_and_run_their_commands() {
    let root = Scratch::new("badge-command");
    let program = root.program("build_plugin", "product-badge");
    let entry = inspect_entry(&program).unwrap();
    assert_eq!(entry.commands, badge::plugin().commands());
    assert_eq!(entry.command_protocol, COMMAND_PROTOCOL_VERSION);
    // The registry keeps the commands.
    let machine = root.join("config/plugins.toml");
    register(&machine, &entry).unwrap();
    assert_eq!(read_machine(&machine).unwrap().entries, std::slice::from_ref(&entry));
    let served = process_plugin(&entry).unwrap();
    assert_eq!(served.commands(), entry.commands.as_slice());
    let run = |plugin: &Plugin, dir: &str| {
        std::fs::create_dir_all(root.join(dir)).unwrap();
        let mut host = Captured::default();
        let mut ctx = CommandContext::new(&mut host);
        ctx.dir = root.join(dir);
        ctx.config = "badge.toml".into();
        let args = ["Orbit".to_string(), "--quiet-ok".to_string()];
        let status = plugin.run_command(&mut ctx, "badge preview", &args).unwrap();
        (status, support::text(&root.join(dir).join("badge-preview.html")))
    };
    let linked = run(&badge::plugin(), "linked");
    assert_eq!(linked, (0, "<p class=\"product-badge\">Orbit</p>\n<!-- badge.toml -->\n".into()));
    assert_eq!(run(&served, "served"), linked);
    // A failing command's problem is the program's to report; its status comes back.
    let mut host = Captured::default();
    let mut ctx = CommandContext::new(&mut host);
    ctx.dir = root.path().to_path_buf();
    assert_eq!(served.run_command(&mut ctx, "badge preview", &[]).unwrap(), 2);
    let p = badge::plugin().run_command(&mut ctx, "badge preview", &[]).unwrap_err();
    assert_eq!((p.code.as_str(), p.exit), ("badge.usage", 2));
}

#[test]
fn programs_without_commands_still_register() {
    let root = Scratch::new("no-commands");
    let program = root.program("publisher", "folder");
    let entry = inspect_entry(&program).unwrap();
    assert!(entry.commands.is_empty());
    let machine = root.join("plugins.toml");
    register(&machine, &entry).unwrap();
    let text = support::text(&machine);
    assert!(!text.contains("commands ="), "{text}");
    assert_eq!(read_machine(&machine).unwrap().entries, [entry]);
}
