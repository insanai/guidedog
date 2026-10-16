//! Commands contributed through the same interface as discussions and Pages.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, SystemTime};

use gd_plugins::{
    BuildPlugin, CommandContext, CommandInfo, CommandOption, Hooks, Plugin, PluginInfo, Problem,
};

use crate::{Publication, create_post, read_metadata, transition};

/// The linked static publication commands; contributes no build hooks.
pub fn plugin() -> Plugin {
    Plugin::build(StaticSite)
}

#[derive(Debug, Clone)]
struct StaticSite;

impl BuildPlugin for StaticSite {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("ssg", "Static publication", env!("CARGO_PKG_VERSION"), Hooks::NONE)
    }

    fn commands(&self) -> Vec<CommandInfo> {
        let mut commands: Vec<_> = [
            ("new", "Create a draft article", "PATH [--title TITLE] [--template NAME]"),
            ("draft", "Withhold an article from publication", "PATH"),
            ("schedule", "Schedule an article", "PATH --at RFC3339"),
            ("publish", "Publish an article", "PATH [--date DATE]"),
            ("status", "List publication states", "[PROJECT]"),
            ("init", "Create a complete static website", "PATH --preset project|journal|research"),
            ("build", "Build and assemble every mounted project", "[PROJECT] [--out DIR]"),
        ]
        .into_iter()
        .map(|(name, summary, usage)| {
            let mut command = CommandInfo::new(&format!("ssg {name}"), summary)
                .with_help(&format!("guidedog ssg {name} {usage}\n\nDates use UTC."))
                .with_option(CommandOption::with_value(
                    "--now",
                    "RFC3339",
                    "Fixed authoring clock",
                ));
            let options: &[(&str, &str, &str)] = match name {
                "new" => {
                    &[("--title", "TITLE", "Article title"), ("--template", "NAME", "HTML layout")]
                }
                "schedule" => &[("--at", "RFC3339", "Release instant")],
                "publish" => &[("--date", "DATE", "Publication date")],
                "build" => &[("--out", "DIR", "Whole-site output directory")],
                "init" => &[
                    ("--preset", "NAME", "project, journal or research"),
                    ("--title", "TITLE", "Website title"),
                    ("--author", "NAME", "Website author"),
                ],
                _ => &[],
            };
            for &(flag, value, summary) in options {
                command = command.with_option(CommandOption::with_value(flag, value, summary));
            }
            if name == "build" {
                command = command
                    .with_option(CommandOption::switch("--no-pdf", "Skip PDF mounts"))
                    .with_option(CommandOption::switch("--preview", "Include withheld pages"));
            }
            command
        })
        .collect();
        commands.extend(crate::theme::commands());
        commands
    }

    fn run_command(
        &mut self,
        ctx: &mut CommandContext<'_>,
        name: &str,
        args: &[String],
    ) -> Result<i32, Problem> {
        let action = name.strip_prefix("ssg ").ok_or_else(|| usage("Unknown ssg command."))?;
        if let Some(action) = action.strip_prefix("theme ") {
            return crate::theme::run(ctx, action, args);
        }
        let (path, options) = parse(action, args)?;
        let now = command_time(&options)?;
        let path = ctx.dir.join(path.unwrap_or_else(|| ".".into()));
        if action == "status" {
            return status(ctx, &path, now);
        }
        if action == "build" {
            return build_site(ctx, &path, &options);
        }
        let result = match action {
            "init" => {
                let preset = options.get("--preset").map_or("project", String::as_str);
                let preset =
                    crate::Preset::parse(preset).ok_or_else(|| usage("Unknown site preset."))?;
                let title = options.get("--title").map_or("My site", String::as_str);
                let author = options.get("--author").map_or("", String::as_str);
                crate::create_site(&path, preset, title, author, &crate::lifecycle::utc_date(now))
            }
            "new" => {
                let title = options.get("--title").map(String::as_str).unwrap_or_else(|| {
                    path.file_stem().and_then(|s| s.to_str()).unwrap_or("Article")
                });
                let template = options.get("--template").map_or("", String::as_str);
                create_post(&path, title, template, &crate::lifecycle::utc_date(now))
            }
            "draft" => transition(&path, Publication::Draft, now),
            "schedule" => {
                let at = options.get("--at").ok_or_else(|| usage("schedule requires --at."))?;
                transition(&path, Publication::Schedule(at), now)
            }
            "publish" => transition(
                &path,
                Publication::Publish(options.get("--date").map(String::as_str)),
                now,
            ),
            _ => return Err(usage("Unknown ssg command.")),
        };
        result.map_err(host_problem)?;
        if !ctx.quiet {
            ctx.print(&format!("{action}: {}\n", path.display()));
        }
        Ok(0)
    }
}

fn parse(
    action: &str,
    args: &[String],
) -> Result<(Option<String>, BTreeMap<String, String>), Problem> {
    let mut options = BTreeMap::new();
    let mut path = None;
    let mut words = args.iter();
    let mut positional = false;
    while let Some(word) = words.next() {
        if word == "--" && !positional {
            positional = true;
            continue;
        }
        if !positional && word.starts_with('-') {
            let (key, inline) =
                word.split_once('=').map_or((word.as_str(), None), |(k, v)| (k, Some(v)));
            if action == "build" && matches!(key, "--no-pdf" | "--preview") && inline.is_none() {
                if options.insert(key.into(), "true".into()).is_some() {
                    return Err(usage(&format!("{key} was given twice.")));
                }
                continue;
            }
            let allowed = key == "--now"
                || match action {
                    "new" => matches!(key, "--title" | "--template"),
                    "schedule" => key == "--at",
                    "publish" => key == "--date",
                    "build" => key == "--out",
                    "init" => matches!(key, "--preset" | "--title" | "--author"),
                    _ => false,
                };
            if !allowed {
                return Err(usage(&format!("Unexpected option {key}.")));
            }
            let value = inline
                .or_else(|| words.next().map(String::as_str))
                .ok_or_else(|| usage(&format!("{key} requires a value.")))?;
            if options.insert(key.into(), value.into()).is_some() {
                return Err(usage(&format!("{key} was given twice.")));
            }
        } else if path.replace(word.clone()).is_some() {
            return Err(usage("Expected one source path or project directory."));
        }
    }
    if !matches!(action, "status" | "build") && path.is_none() {
        return Err(usage("A source path is required."));
    }
    Ok((path, options))
}

fn build_site(
    ctx: &mut CommandContext<'_>,
    root: &Path,
    values: &BTreeMap<String, String>,
) -> Result<i32, Problem> {
    let options = crate::SiteBuildOptions {
        output: values.get("--out").map(|s| ctx.dir.join(s)),
        no_pdf: values.contains_key("--no-pdf"),
        preview: values.contains_key("--preview"),
        now: values.get("--now").cloned().unwrap_or_default(),
    };
    let output = crate::build_site(root, &options, &mut |request| {
        ctx.build_project(request).map_err(|p| {
            gd_host::Problem::new(
                "ssg.mount_build",
                "MOUNT BUILD FAILED",
                &p.path,
                p.message.clone(),
                p.hint.clone(),
                p.exit,
            )
        })
    })
    .map_err(host_problem)?;
    if !ctx.quiet {
        ctx.print(&format!("Site: {}\n", output.display()));
    }
    Ok(0)
}

fn command_time(options: &BTreeMap<String, String>) -> Result<SystemTime, Problem> {
    if let Some(text) = options.get("--now") {
        return gd_clock::parse_timestamp(text).map_err(|e| usage(&e.to_string()));
    }
    let moment = gd_clock::build_time();
    if moment.unix < 0 {
        SystemTime::UNIX_EPOCH.checked_sub(Duration::from_secs(moment.unix.unsigned_abs()))
    } else {
        SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(moment.unix as u64))
    }
    .ok_or_else(|| usage("The authoring clock is outside the system's supported range."))
}

fn status(ctx: &mut CommandContext<'_>, path: &Path, now: SystemTime) -> Result<i32, Problem> {
    let loaded = gd_conf::load_config(path, &Default::default()).map_err(host_problem)?;
    for source in gd_env::discover(&loaded.config).map_err(host_problem)? {
        let page = read_metadata(&source.path).map_err(host_problem)?;
        if page.date.is_empty() && !page.draft && page.publish_at.is_empty() {
            continue;
        }
        let state = page.ineligible_reason(now).unwrap_or("published");
        let release = if page.publish_at.is_empty() { &page.date } else { &page.publish_at };
        ctx.print(&format!("{state:10} {release:25} {}\n", source.relative));
    }
    Ok(0)
}

fn usage(message: &str) -> Problem {
    Problem::new("ssg.usage", "INVALID SSG COMMAND", message)
        .hint("Run guidedog ssg --help to see the commands.")
        .exit(2)
}

pub(crate) fn host_problem(p: gd_host::Problem) -> Problem {
    Problem::new(p.code, p.title, &p.message).path(&p.path).hint(&p.hint).exit(p.exit)
}
