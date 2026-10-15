//! `workflow`: write or check the GitHub Actions workflow of the other route.

use std::path::Path;

use gd_plugins::Problem;

use super::display::Console;
use super::{Context, usage};
use crate::problem::{io_problem, problem};
use crate::workflow::{
    RELEASE_REPOSITORY, RELEASE_ROOT, WORKFLOW_PATH, WorkflowOptions, check_workflow,
    is_safe_workflow_name, release_sha256, render_workflow,
};

pub(crate) fn workflow(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    let options = match workflow_options(ctx) {
        Ok(o) => o,
        Err(p) => return console.report(&p),
    };
    let path = ctx.dir.join(WORKFLOW_PATH);
    if ctx.o.check {
        return check(console, &path, &options);
    }
    let mut options = options;
    if !ctx.o.sha256.is_empty() {
        options.sha256 = Some(ctx.o.sha256.clone());
    } else if !ctx.o.offline {
        let repository = &options.release_repository;
        match release_sha256(RELEASE_ROOT, repository, &options.version) {
            Ok(sum) => options.sha256 = Some(sum),
            Err(p) => return console.report(&p),
        }
    }
    let text = render_workflow(&options);
    if ctx.o.write {
        if let Err(p) = write(ctx, &path, &text) {
            return console.report(&p);
        }
        console.success(&format!("Wrote {WORKFLOW_PATH} (Guidedog {})", options.version));
    } else {
        console.print(&text);
    }
    if options.sha256.is_none() {
        return console.report(&problem(
            "pages.workflow-sha256",
            "The workflow has a placeholder checksum",
            WORKFLOW_PATH,
            "It was made offline without --sha256, so its build fails until GUIDEDOG_SHA256 is \
             set.",
            &format!(
                "Copy the checksum of guidedog-linux-amd64.tar.gz from the SHA256SUMS of \
                release v{} and run the command again with --sha256.",
                options.version
            ),
        ));
    }
    0
}

fn workflow_options(ctx: &Context<'_>) -> Result<WorkflowOptions, Problem> {
    let o = ctx.o;
    let mut options = WorkflowOptions::default();
    if !o.project.is_empty() {
        options.project = o.project.trim_end_matches('/').to_string();
    }
    if o.branch_given {
        options.branch = o.branch.clone();
    }
    if !o.release.is_empty() {
        options.version = o.release.trim_start_matches('v').to_string();
    }
    if !o.release_repository.is_empty() {
        options.release_repository = o.release_repository.clone();
    }
    let fields = [
        ("--project", &options.project),
        ("--branch", &options.branch),
        ("--version", &options.version),
        ("--release-repository", &options.release_repository),
    ];
    for (flag, value) in fields {
        if !is_safe_workflow_name(value) {
            return Err(usage(
                &format!("Not a name for {flag}"),
                value,
                &format!(
                    "Use letters, \
                digits, and . _ / -, such as {flag}={}.",
                    example(flag)
                ),
            ));
        }
    }
    Ok(options)
}

fn example(flag: &str) -> &'static str {
    match flag {
        "--project" => "docs",
        "--branch" => "main",
        "--version" => crate::VERSION,
        _ => RELEASE_REPOSITORY,
    }
}

fn write(ctx: &Context<'_>, path: &Path, text: &str) -> Result<(), Problem> {
    let shown = path.display().to_string();
    if let Ok(old) = std::fs::read_to_string(path)
        && old != text
        && !ctx.o.force
    {
        return Err(problem(
            "pages.exists",
            "The workflow exists",
            &shown,
            "A different workflow is there already.",
            "Check it with guidedog pages workflow \
            --check, or add --force to replace it.",
        )
        .exit(2));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io_problem(&shown, &e))?;
    }
    std::fs::write(path, text).map_err(|e| io_problem(&shown, &e))
}

fn check(console: &mut Console<'_>, path: &Path, options: &WorkflowOptions) -> i32 {
    let shown = path.display().to_string();
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            let mut p = io_problem(&shown, &e);
            p.hint = "Write one with guidedog pages workflow --write.".to_string();
            return console.report(&p);
        }
    };
    let findings = check_workflow(&shown, &text, options);
    if findings.is_empty() {
        console.success(&format!(
            "{shown} is current (Guidedog {}, reviewed action pins)",
            options.version
        ));
        return 0;
    }
    for p in &findings {
        console.report(p);
    }
    1
}
