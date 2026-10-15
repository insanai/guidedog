//! The commands Pages adds to guidedog's command line: `guidedog publish pages`, the
//! guided publication, and `guidedog pages workflow`. They are the `guidedog-pages`
//! commands of the same names, run with what the host offers: its streams, its
//! questions, and its own build of the project (unless `--guidedog` or
//! `$GUIDEDOG_PAGES_GUIDEDOG` names a program to build with).

use gd_plugins::{BuildRequest, CommandContext, CommandInfo, CommandOption, Problem};
use gd_report::escape_terminal;

use super::display::Console;
use super::publish::{finish, guide_options, stage};
use super::{Context, Options, parse};
use crate::credential::{Credential, ENV_PROFILE};
use crate::{GUIDEDOG_VARIABLE, Prompt, Settings, build_with_guidedog, publish_guided};

/// The guided publication's command name.
pub const PUBLISH_PAGES: &str = "publish pages";
/// The workflow command's name.
pub const PAGES_WORKFLOW: &str = "pages workflow";

const PUBLISH_HELP: &str = "\
Publishes the project's site to GitHub Pages, doing first whatever is still
missing: pages.toml (from the origin remote and the nearest conf.toml), a GitHub
token, this machine's permission to publish to the target, and the Pages source.
Each step is asked once; a configured machine publishes without a question.
guidedog builds the project itself. Without a terminal nothing is asked: a step
that needs an answer fails with a hint naming the flag or the guidedog-pages
command that does it.

Exit status: 0 published or unchanged, 1 failed (nothing published), 2 usage,
6 published but not confirmed live, 7 unknown (run guidedog-pages recover).";

const WORKFLOW_HELP: &str = "\
Prints, writes (--write), or checks (--check) .github/workflows/docs-pages.yml,
the GitHub Actions workflow that builds the site with a pinned Guidedog release
and deploys it with GitHub Pages. --offline writes a placeholder checksum and
exits 1 until --sha256 gives the real one.";

/// The commands Pages contributes.
pub(crate) fn commands() -> Vec<CommandInfo> {
    let value = CommandOption::with_value;
    let switch = CommandOption::switch;
    let publish = [
        value("--config", "FILE", "The configuration (default pages.toml)"),
        value("--repository", "OWNER/REPO", "The repository of a new configuration"),
        value("--project", "DIR", "The project of a new configuration"),
        value("--branch", "NAME", "The branch of a new configuration (default gh-pages)"),
        value("--profile", "NAME", "The credential profile (default default)"),
        value("--client-id", "ID", "The GitHub app of the device sign-in"),
        value("--credential", "env", "Use the token in GUIDEDOG_PAGES_TOKEN"),
        value("--guidedog", "PATH", "Build with this guidedog program instead"),
        value("--wait", "SECONDS", "How long to wait until it is live (0: do not wait)"),
        switch("--dry-run", "Build and plan; publish nothing"),
        switch("--yes", "Answer yes to confirmations; never asks"),
        switch("--verbose", "List every change of the plan"),
    ];
    let workflow = [
        switch("--write", "Write the workflow instead of printing it"),
        switch("--check", "Check the workflow in the repository"),
        value("--project", "DIR", "The project folder (default docs)"),
        value("--branch", "NAME", "The branch that triggers it (default main)"),
        value("--version", "X", "The Guidedog release it installs (default this one)"),
        value("--sha256", "SUM", "The release archive's SHA-256"),
        switch("--offline", "Do not download the release's checksum"),
        value("--release-repository", "OWNER/REPO", "Where releases come from"),
        switch("--force", "Replace a different workflow"),
    ];
    let mut publish_info = CommandInfo::new(
        PUBLISH_PAGES,
        "Publish to GitHub Pages, doing whatever setup is missing",
    )
    .with_help(PUBLISH_HELP);
    publish_info.options = publish.into();
    let mut workflow_info = CommandInfo::new(
        PAGES_WORKFLOW,
        "Print, write, or check a GitHub Actions workflow that deploys the site",
    )
    .with_help(WORKFLOW_HELP);
    workflow_info.options = workflow.into();
    vec![publish_info, workflow_info]
}

/// Runs the contributed command `name` with `args` and `settings`, through `host`.
pub(crate) fn run(
    settings: &Settings,
    host: &mut CommandContext<'_>,
    name: &str,
    args: &[String],
) -> Result<i32, Problem> {
    let word = match name {
        PUBLISH_PAGES => "publish",
        PAGES_WORKFLOW => "workflow",
        _ => return Err(super::usage("Unknown command", name, "Run guidedog --help.")),
    };
    let mut words = Vec::with_capacity(args.len() + 1);
    words.push(word.to_string());
    words.extend_from_slice(args);
    let (o, parsed) = parse(&words);
    parsed?;
    let mut settings = settings.clone();
    if o.credential == ENV_PROFILE {
        settings.credential = Credential::Env;
    }
    let cancel = host.cancel.clone();
    let ctx = Context { o: &o, settings: &settings, cancel: &cancel, dir: host.dir.clone() };
    Ok(match word {
        "workflow" => {
            let (out, err) = host.streams();
            super::workflow::workflow(&ctx, &mut Console::new(out, err, &o))
        }
        _ if o.dry_run => dry_run(&ctx, host),
        _ => guided(&ctx, host),
    })
}

/// `publish pages --dry-run`: builds and plans with the host, publishing nothing.
fn dry_run(ctx: &Context<'_>, host: &mut CommandContext<'_>) -> i32 {
    let staged = {
        let mut prompt = HostPrompt { host: &mut *host, o: ctx.o };
        stage(ctx, true, &mut |request| prompt.build(&program(ctx.o), request))
    };
    let (out, err) = host.streams();
    finish(ctx, &mut Console::new(out, err, ctx.o), staged, true).0
}

/// `publish pages`: the guided publication, asking through the host.
fn guided(ctx: &Context<'_>, host: &mut CommandContext<'_>) -> i32 {
    let options = guide_options(ctx, host.interactive && !ctx.o.yes);
    let result = publish_guided(&options, &mut HostPrompt { host: &mut *host, o: ctx.o });
    let (out, err) = host.streams();
    let mut console = Console::new(out, err, ctx.o);
    match result {
        Ok(r) => console.show_result(&r),
        Err(p) => console.report(&p),
    }
}

/// The program `--guidedog` or `$GUIDEDOG_PAGES_GUIDEDOG` names; empty when neither
/// does, and the host builds.
fn program(o: &Options) -> String {
    if !o.guidedog.is_empty() {
        return o.guidedog.clone();
    }
    std::env::var(GUIDEDOG_VARIABLE).unwrap_or_default()
}

/// The guided publication's questions and builds, through the host.
struct HostPrompt<'h, 'c, 'o> {
    host: &'h mut CommandContext<'c>,
    o: &'o Options,
}

impl HostPrompt<'_, '_, '_> {
    fn ask(&mut self, question: &str) -> Option<String> {
        self.host.ask(&format!("  {} ", escape_terminal(question)))
    }
}

impl Prompt for HostPrompt<'_, '_, '_> {
    fn show(&mut self, text: &str) {
        if !self.o.json {
            self.host.eprint(&format!("  {}\n", escape_terminal(text)));
        }
    }

    fn confirm(&mut self, question: &str) -> bool {
        let answer = self.ask(&format!("{question} [y/N]")).unwrap_or_default();
        matches!(answer.to_lowercase().as_str(), "y" | "yes")
    }

    fn choose(&mut self, question: &str, choices: &[&str]) -> Option<usize> {
        self.show(question);
        for (i, choice) in choices.iter().enumerate() {
            self.show(&format!("  {}. {choice}", i + 1));
        }
        let answer = self.ask(&format!("Choose 1 to {} (empty to stop):", choices.len()))?;
        let n = answer.parse::<usize>().ok()?;
        (1..=choices.len()).contains(&n).then(|| n - 1)
    }

    /// Builds with the named program, or else with the host's own engine.
    fn build(&mut self, _default: &str, request: &BuildRequest) -> Result<(), Problem> {
        let named = program(self.o);
        if !named.is_empty() {
            self.show(&format!("Building {} with {named}...", request.project.display()));
            return build_with_guidedog(&named, request);
        }
        self.show(&format!("Building {}...", request.project.display()));
        self.host.build_project(request)
    }
}
