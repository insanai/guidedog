//! Running a command a plugin adds: guidedog finds the plugin (linked into this program,
//! or registered on this machine and checked against its SHA-256), lends it a
//! `CommandContext` over this command's console, and builds projects for it with its
//! own engine.

use std::io::Write;
use std::path::{Path, PathBuf};

use gd_env::BuilderKind;
use gd_plugins::{BuildRequest, CommandContext, CommandHost, Plugin, Problem, Registry};

use crate::command::{BuildCommand, ExtensionCommand, MemoryMode};
use crate::console::Console;
use crate::plugin::{plugin_problem, registered};

/// Runs `command` with the plugin that contributes it and returns its exit status.
pub(crate) fn run_extension(
    console: &mut Console<'_>,
    command: &ExtensionCommand,
    linked: &Registry,
) -> i32 {
    let e = &command.extension;
    let plugin = match find_plugin(&e.plugin, linked) {
        Ok(plugin) => plugin,
        Err(p) => return plugin_problem(console, &p),
    };
    gd_host::catch_interrupts();
    let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let (interactive, color, json, quiet) =
        (console.interactive(), console.color_err, console.json, console.quiet);
    let result = {
        let mut host = CliHost { console: &mut *console, linked };
        let mut ctx = CommandContext::new(&mut host);
        ctx.config = plugin_config(&dir, &e.plugin);
        ctx.dir = dir;
        ctx.host_program =
            std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        (ctx.interactive, ctx.color, ctx.json, ctx.quiet) = (interactive, color, json, quiet);
        ctx.cancel = gd_plugins::Cancel::from_fn(gd_host::was_interrupted);
        let result = plugin.run_command(&mut ctx, &e.name(), &command.args);
        ctx.flush();
        result
    };
    match result {
        Ok(status) => status,
        Err(p) => plugin_problem(console, &p),
    }
}

/// The plugin `id`: linked into this program, or registered on this machine with its
/// program unchanged since.
fn find_plugin(id: &str, linked: &Registry) -> Result<Plugin, Problem> {
    if let Some(plugin) = linked.find(id) {
        return Ok(plugin.clone());
    }
    let entry = registered(&gd_plugins::machine_path()?, id)?;
    gd_plugins::verify_entry(&entry)?;
    if entry.is_build() {
        gd_plugins::process_plugin(&entry)
    } else {
        gd_plugins::process_publisher(&entry)
    }
}

/// The plugin's configuration file, when the project around `dir` names one in
/// conf.toml's `plugin_configs`; empty otherwise (a project that does not load names
/// none: the plugin's own default applies).
fn plugin_config(dir: &Path, id: &str) -> String {
    let Some((source, _)) = gd_conf::find_project(dir) else {
        return String::new();
    };
    let options = gd_conf::LoadOptions { source: Some(source.clone()), ..Default::default() };
    let Ok(loaded) = gd_conf::load_config(dir, &options) else {
        return String::new();
    };
    match loaded.config.plugins.configs.get(id) {
        Some(file) => source.join(file).to_string_lossy().into_owned(),
        None => String::new(),
    }
}

/// What guidedog offers a plugin's command: this command's streams and input, and
/// builds with this program's engine and linked plugins.
struct CliHost<'c, 'a, 'r> {
    console: &'c mut Console<'a>,
    linked: &'r Registry,
}

impl CommandHost for CliHost<'_, '_, '_> {
    fn streams(&mut self) -> (&mut dyn Write, &mut dyn Write) {
        (&mut *self.console.out, &mut *self.console.err)
    }

    fn read_line(&mut self) -> Option<String> {
        let mut line = String::new();
        match self.console.input.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim_end_matches(['\n', '\r']).to_string()),
        }
    }

    fn build_project(&mut self, request: &BuildRequest) -> Result<(), Problem> {
        let build = build_command(request)?;
        let status = crate::project::build(self.console, &build, self.linked);
        if status == 0 {
            return Ok(());
        }
        Err(Problem::new(
            "cli.plugin-build",
            "The project did not build",
            "The build the plugin asked for failed; its report is above.",
        )
        .hint("Fix what the build reported, then run the command again.")
        .path(&request.project.to_string_lossy())
        .exit(status))
    }
}

/// The `guidedog build` that carries out `request`.
fn build_command(request: &BuildRequest) -> Result<BuildCommand, Problem> {
    let name = if request.builder.is_empty() { "html" } else { request.builder.as_str() };
    let Some(kind) = BuilderKind::from_name(name) else {
        return Err(Problem::new(
            "cli.plugin-builder",
            "The plugin asked for an unknown builder",
            &format!("guidedog has no builder {name:?}."),
        )
        .hint("Ask for html, dirhtml, singlehtml, pdf, or text.")
        .exit(2));
    };
    let mut overrides = request.overrides.clone();
    if !request.base_url.is_empty() {
        overrides.push(format!("html_baseurl={}", request.base_url));
    }
    Ok(BuildCommand {
        dir: PathBuf::from("."),
        kind,
        load: gd_conf::LoadOptions {
            source: Some(request.project.clone()),
            output: Some(request.output.clone()),
            overrides,
            ..Default::default()
        },
        options: gd_env::BuildOptions {
            warnings_as_errors: request.strict,
            now: if request.now.is_empty() {
                None
            } else {
                Some(gd_clock::parse_timestamp(&request.now).map_err(|e| {
                    Problem::new("cli.plugin-clock", "Invalid publication clock", &e.to_string())
                        .hint("Use an RFC 3339 timestamp with an explicit time zone.")
                        .exit(2)
                })?)
            },
            preview: request.preview,
            output_dir: Some(request.output.clone()),
            ..Default::default()
        },
        memory: MemoryMode::Ask,
        verbose: 0,
        typst: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_become_builds() {
        let request = BuildRequest {
            project: "docs".into(),
            output: "out".into(),
            builder: "pdf".into(),
            base_url: "https://example.org/".into(),
            strict: true,
            ..BuildRequest::default()
        };
        let build = build_command(&request).unwrap_or_else(|p| panic!("{p}"));
        assert_eq!(build.kind, BuilderKind::Pdf);
        assert!(build.options.warnings_as_errors);
        assert_eq!(build.load.overrides, ["html_baseurl=https://example.org/"]);
        let unknown = BuildRequest { builder: "epub".into(), ..request };
        assert!(build_command(&unknown).is_err());
    }
}
