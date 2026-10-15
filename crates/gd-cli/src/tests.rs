//! Command-line parsing for every command, usage problems with their suggestions, and
//! what the commands print, captured through `run_in`.

use std::path::PathBuf;

use gd_env::{BuilderKind, Jobs};
use gd_host::{Format, Problem, Target};
use guidedoc::ContentPolicy;

use super::*;

fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| a.to_string()).collect()
}

fn parse(args: &[&str]) -> Result<Command, Problem> {
    parse_command(&words(args)).command
}

fn problem(args: &[&str]) -> Problem {
    match parse(args) {
        Ok(command) => panic!("{args:?} parsed as {command:?}"),
        Err(problem) => problem,
    }
}

/// What a command line prints: exit status, standard output, standard error.
fn run_captured(args: &[&str], input: &str) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut input = input.as_bytes();
    let streams = Streams {
        out: &mut out,
        err: &mut err,
        input: &mut input,
        terminals: Terminals::default(),
    };
    let status = run_in(&words(args), &Registry::default(), streams);
    (
        status,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

#[test]
fn no_command_is_the_help_and_global_flags_are_read() {
    assert!(matches!(parse(&[]), Ok(Command::Help(None))));
    assert!(matches!(parse(&["--help"]), Ok(Command::Help(None))));
    assert!(matches!(parse(&["--licenses"]), Ok(Command::Licenses)));
    let Ok(Command::Version { typst }) = parse(&["--version", "--typst=/opt/typst"]) else {
        panic!("not --version");
    };
    assert_eq!(typst, "/opt/typst");
    let parsed = parse_command(&words(&["--color", "never", "--diagnostics=json", "-q"]));
    assert_eq!(
        parsed.output,
        Output { color: ColorMode::Never, diagnostics: Diagnostics::Json, quiet: true }
    );
}

#[test]
fn unknown_commands_and_options_suggest_the_closest() {
    let p = problem(&["biuld"]);
    assert_eq!((p.code, p.title, p.exit), ("cli.usage", "UNKNOWN COMMAND", 2));
    assert!(p.hint.ends_with("Did you mean `build`?"), "{}", p.hint);
    assert!(problem(&["--verison"]).hint.contains("Did you mean `--version`?"));
    assert!(problem(&["build", "--kepp-going"]).hint.contains("`--keep-going`"));
    assert!(problem(&["build", "htm"]).hint.contains("Did you mean `html`?"));
    assert!(problem(&["intl", "stats"]).hint.contains("Did you mean `stat`?"));
    assert!(problem(&["plugin", "lsit"]).hint.contains("Did you mean `list`?"));
    assert!(problem(&["pages", "workflo"]).hint.contains("Did you mean `workflow`?"));
    assert!(problem(&["convert", "a.rst", "--to=hmtl", "-o", "x"]).hint.contains("`html`"));
    assert_eq!(problem(&["--color=pink"]).title, "INVALID COLOR MODE");
    assert_eq!(problem(&["formats", "--format=xml"]).title, "INVALID OUTPUT FORMAT");
}

#[test]
fn convert_options_are_read_and_formats_resolved() {
    let args = [
        "convert",
        "guide.md",
        "--to",
        "pdf",
        "-o",
        "guide.pdf",
        "--emit-typst=g.typ",
        "--force",
        "--raw=omit",
        "--strict",
        "--title=T",
        "--lang",
        "de",
        "--fragment",
        "--number-sections",
        "--typst=typst",
        "--font-path=f1",
        "--font-path",
        "f2",
        "--package-cache=c",
        "--offline",
        "--embedded-fonts-only",
        "--budget=64",
        "--max-depth=9",
        "--max-nodes=99",
        "--stack-kib=1024",
    ];
    let Ok(Command::Convert(c)) = parse(&args) else { panic!("not convert") };
    assert_eq!((c.input.as_str(), c.output.as_str()), ("guide.md", "guide.pdf"));
    assert_eq!((c.from, c.to, c.raw), (Format::Commonmark, Target::Pdf, ContentPolicy::Omit));
    assert_eq!(c.emit_typst, Some(PathBuf::from("g.typ")));
    assert!(c.force && c.strict && c.fragment && c.number_sections && c.offline);
    assert!(c.embedded_fonts_only);
    assert_eq!((c.title.as_str(), c.lang.as_str()), ("T", "de"));
    assert_eq!(c.font_paths, [PathBuf::from("f1"), PathBuf::from("f2")]);
    assert_eq!(c.package_cache, Some(PathBuf::from("c")));
    assert_eq!(
        (c.budget_mib, c.max_depth, c.max_nodes, c.stack_kib),
        (Some(64), Some(9), Some(99), Some(1024))
    );
    let Ok(Command::Convert(c)) =
        parse(&["convert", "-", "--from", "rst", "-o", "-", "--to=html"])
    else {
        panic!("not convert");
    };
    assert_eq!(
        (c.from, c.to, c.input.as_str(), c.output.as_str()),
        (Format::Rst, Target::Html, "-", "-")
    );
    let Ok(Command::Convert(c)) = parse(&["convert", "a.typ", "-o", "a.pdf"]) else { panic!() };
    assert_eq!(c.from, Format::Typst);
}

#[test]
fn convert_writes_restructuredtext_from_myst_or_sphinx() {
    let Ok(Command::Convert(c)) = parse(&["convert", "a.md", "--from=myst", "-o", "a.rst"]) else {
        panic!("not convert");
    };
    assert_eq!((c.from, c.to), (Format::Myst, Target::Rst));
    let Ok(Command::Convert(c)) =
        parse(&["convert", "a.rst", "--from", "sphinx", "--to", "rst", "-o", "-"])
    else {
        panic!("not convert");
    };
    assert_eq!((c.from, c.to), (Format::Sphinx, Target::Rst));
    let refused = problem(&["convert", "a.typ", "-o", "a.rst"]);
    assert_eq!(refused.title, "TYPST CANNOT BE WRITTEN AS RESTRUCTUREDTEXT");
}

#[test]
fn convert_usage_problems_explain_themselves() {
    assert_eq!(problem(&["convert"]).title, "CONVERT NEEDS ONE INPUT");
    assert_eq!(problem(&["convert", "a", "b", "-o", "x.html"]).title, "CONVERT NEEDS ONE INPUT");
    assert_eq!(
        problem(&["convert", "a.txt", "-o", "x.html"]).title,
        "CANNOT TELL THE INPUT FORMAT"
    );
    assert_eq!(
        problem(&["convert", "a.rst", "-o", "x.doc"]).title,
        "CANNOT TELL THE OUTPUT FORMAT"
    );
    assert_eq!(problem(&["convert", "a.rst", "--to=html"]).title, "MISSING OUTPUT");
    assert_eq!(
        problem(&["convert", "a.typ", "-o", "a.pdf", "--emit-typst=x"]).title,
        "NOTHING TO EMIT"
    );
    assert_eq!(
        problem(&["convert", "a.rst", "-o", "a.html", "--raw=keep"]).title,
        "INVALID RAW-CONTENT POLICY"
    );
    assert_eq!(
        problem(&["convert", "a.rst", "-o", "a.html", "--force=yes"]).title,
        "UNEXPECTED OPTION VALUE"
    );
    assert_eq!(problem(&["convert", "a.rst", "-o"]).title, "MISSING OPTION VALUE");
    assert_eq!(problem(&["convert", "a.rst", "-o", "--force"]).title, "MISSING OPTION VALUE");
    assert_eq!(problem(&["convert", "a.rst", "-o="]).title, "EMPTY OPTION VALUE");
    assert_eq!(
        problem(&["convert", "a.rst", "-o", "a.html", "--budget=2000000"]).title,
        "BUDGET IS TOO LARGE"
    );
    assert_eq!(
        problem(&["convert", "a.rst", "-o", "a.html", "--stack-kib=99999"]).title,
        "STACK BUDGET IS TOO LARGE"
    );
    assert_eq!(
        problem(&["convert", "a.rst", "-o", "a.html", "--max-depth=0"]).title,
        "INVALID NUMBER"
    );
    assert_eq!(
        problem(&["convert", "a.rst", "-o", "a.html", "--port=80"]).title,
        "UNKNOWN OPTION"
    );
    assert!(matches!(parse(&["convert", "--help"]), Ok(Command::Help(Some("convert")))));
    // After --, a word starting with - is the input.
    let Ok(Command::Convert(c)) = parse(&["convert", "-o", "x.html", "--from=rst", "--", "-a"])
    else {
        panic!("not convert");
    };
    assert_eq!(c.input, "-a");
}

#[test]
fn build_options_fill_the_typed_command() {
    let args = [
        "build",
        "pdf",
        ".",
        "-a",
        "-E",
        "-n",
        "-W",
        "--keep-going",
        "--untrusted",
        "-j",
        "4",
        "-D",
        "language=de",
        "-D",
        "html_context.x=1",
        "-t",
        "draft",
        "-c",
        "conf",
        "-d",
        "x",
        "--budget=512",
        "--memory=ram",
        "--max-depth=10",
        "--max-nodes=20",
        "--stack-kib=64",
        "-v",
        "-v",
        "--typst",
        "/opt/typst",
    ];
    let Ok(Command::Build(b)) = parse(&args) else { panic!("not build") };
    assert_eq!((b.kind, b.dir.clone()), (BuilderKind::Pdf, PathBuf::from(".")));
    let o = &b.options;
    assert!(o.all && o.fresh && o.nitpicky && o.warnings_as_errors && o.keep_going && o.untrusted);
    assert_eq!(o.jobs, Jobs::Count(4));
    assert_eq!(b.load.overrides, ["language=de", "html_context.x=1"]);
    assert_eq!(o.tags, ["draft"]);
    assert_eq!(b.load.conf_dir, Some(PathBuf::from("conf")));
    assert_eq!(
        (o.budget, o.max_depth, o.max_nodes, o.stack_bytes),
        (Some(512 << 20), Some(10), Some(20), Some(64 << 10))
    );
    assert_eq!(
        (b.memory, b.verbose, b.typst.as_deref()),
        (MemoryMode::Stop, 2, Some("/opt/typst"))
    );
    let Ok(Command::Build(b)) = parse(&["build", "-j", "auto", "-C", "latexpdf"]) else {
        panic!()
    };
    assert_eq!(
        (b.options.jobs, b.load.no_config, b.kind),
        (Jobs::PerCore, true, BuilderKind::Pdf)
    );
    let Ok(Command::Build(b)) = parse(&["build"]) else { panic!() };
    assert_eq!((b.kind, b.dir), (BuilderKind::Html, PathBuf::from(".")));
}

#[test]
fn the_sphinx_build_forms_name_source_and_output() {
    let Ok(Command::Build(b)) = parse(&["build", "-b", "text", "docs", "out"]) else { panic!() };
    assert_eq!(b.kind, BuilderKind::Text);
    assert_eq!(
        (b.load.source.clone(), b.load.output.clone()),
        (Some("docs".into()), Some("out".into()))
    );
    assert_eq!(b.options.output_dir, Some(PathBuf::from("out")));
    let Ok(Command::Build(b)) = parse(&["build", "-M", "html", "docs", "_build"]) else {
        panic!()
    };
    assert_eq!(b.options.output_dir, Some(PathBuf::from("_build/html")));
    assert_eq!(
        problem(&["build", "-b", "html", "docs"]).title,
        "SOURCE AND OUTPUT FOLDERS ARE REQUIRED"
    );
    assert_eq!(problem(&["build", "-b", "epub", "a", "b"]).title, "UNKNOWN BUILDER");
    assert_eq!(problem(&["build", "html", ".", "more"]).title, "TOO MANY BUILD ARGUMENTS");
}

#[test]
fn build_numbers_and_modes_are_checked() {
    assert_eq!(problem(&["build", "-j", "0"]).title, "INVALID JOB COUNT");
    assert_eq!(problem(&["build", "--budget=0"]).title, "INVALID BUDGET");
    assert_eq!(problem(&["build", "--memory=swap"]).title, "INVALID MEMORY MODE");
    assert_eq!(problem(&["build", "-W=yes"]).title, "UNEXPECTED OPTION VALUE");
    assert_eq!(problem(&["build", "--port=8000"]).title, "UNKNOWN OPTION");
}

/// Earlier versions kept working memory on disk; asking for it now explains that the
/// mode is gone and what to do instead.
#[test]
fn disk_memory_is_explained_not_ignored() {
    for args in [
        &["build", "--memory=disk"][..],
        &["build", "--memory", "disk"],
        &["build", "--disk-budget=512"],
    ] {
        let p = problem(args);
        assert_eq!((p.code, p.exit), ("cli.memory-disk", 2), "{args:?}");
        assert!(p.hint.contains("--budget=MIB"), "{}", p.hint);
    }
}

#[test]
fn serve_clean_quickstart_and_migrate_read_their_options() {
    let Ok(Command::Serve(s)) = parse(&["serve", "docs", "--port", "9000", "-D", "x=1", "-W"])
    else {
        panic!("not serve");
    };
    assert_eq!(
        (s.port, s.build.kind, s.build.dir.clone()),
        (9000, BuilderKind::Html, PathBuf::from("docs"))
    );
    assert!(s.build.options.warnings_as_errors && s.build.load.overrides == ["x=1"]);
    assert_eq!(problem(&["serve", "--port=70000"]).title, "INVALID PORT");
    assert_eq!(problem(&["serve", "-b", "html"]).title, "UNKNOWN OPTION");
    let Ok(Command::Clean(c)) = parse(&["clean", "docs"]) else { panic!() };
    assert_eq!(c.dir, PathBuf::from("docs"));
    assert_eq!(problem(&["clean", "a", "b"]).title, "TOO MANY ARGUMENTS");
    let args = [
        "quickstart",
        "d",
        "-q",
        "-p",
        "P",
        "-a",
        "A",
        "-v",
        "1.0",
        "-r",
        "1.0.1",
        "-l",
        "fr",
        "--sep",
        "--suffix",
        ".md",
    ];
    let Ok(Command::Quickstart(q)) = parse(&args) else { panic!() };
    let expected = QuickstartCommand {
        dir: "d".into(),
        quiet: true,
        project: "P".into(),
        author: "A".into(),
        version: "1.0".into(),
        release: "1.0.1".into(),
        language: "fr".into(),
        suffix: ".md".into(),
        separate: Some(true),
    };
    assert_eq!(q, expected);
    let Ok(Command::Quickstart(q)) =
        parse(&["quickstart", "--no-sep", "--author", "B", "--version-name=2"])
    else {
        panic!()
    };
    assert_eq!(
        (q.separate, q.author.as_str(), q.version.as_str(), q.quiet),
        (Some(false), "B", "2", false)
    );
    let Ok(Command::Migrate(m)) = parse(&["migrate", "docs/conf.py", "-o", "-"]) else { panic!() };
    assert_eq!(
        (m.input, m.output, m.force),
        (PathBuf::from("docs/conf.py"), MigrateOutput::Stdout, false)
    );
    let Ok(Command::Migrate(m)) = parse(&["migrate", "--output=c.toml", "--force"]) else {
        panic!()
    };
    assert_eq!(
        (m.input, m.output, m.force),
        (PathBuf::from("."), MigrateOutput::File("c.toml".into()), true)
    );
    assert!(matches!(parse(&["migrate", "-h"]), Ok(Command::Help(Some("migrate")))));
}

#[test]
fn intl_reads_the_options_of_sphinx_intl() {
    let args = [
        "intl",
        "update",
        "docs",
        "-l",
        "de",
        "--language=fr",
        "-p",
        "pot",
        "-d",
        "loc",
        "-w",
        "60",
        "--no-obsolete",
        "-c",
        "conf",
        "-D",
        "x=1",
    ];
    let Ok(Command::Intl(i)) = parse(&args) else { panic!("not intl") };
    assert_eq!((i.action, i.dir.clone()), (IntlAction::Update, PathBuf::from("docs")));
    assert_eq!(i.languages, ["de", "fr"]);
    assert_eq!(
        (i.pot_dir.clone(), i.locale_dir.clone()),
        (Some("pot".into()), Some("loc".into()))
    );
    assert_eq!((i.line_width, i.obsolete), (60, false));
    assert_eq!(
        (i.load.conf_dir.clone(), i.load.overrides.clone()),
        (Some("conf".into()), vec!["x=1".to_string()])
    );
    let Ok(Command::Intl(i)) = parse(&["intl", "stat"]) else { panic!() };
    assert_eq!((i.action, i.line_width, i.obsolete), (IntlAction::Stat, 76, true));
    assert_eq!(problem(&["intl"]).title, "UNKNOWN INTL COMMAND");
    assert_eq!(problem(&["intl", "stat", "-W"]).title, "UNKNOWN OPTION");
}

#[test]
fn plugin_commands_take_their_words() {
    let Ok(Command::Plugin(p)) = parse(&[
        "plugin",
        "register",
        "badge",
        "/bin/badge",
        "--yes",
        "--project=docs",
        "--config",
        "b.toml",
        "--timeout=30",
    ]) else {
        panic!("not plugin");
    };
    assert_eq!(
        p.action,
        PluginAction::Register { id: "badge".into(), program: "/bin/badge".into() }
    );
    assert!(p.yes);
    assert_eq!(
        (p.project, p.config, p.timeout),
        (Some("docs".into()), Some("b.toml".into()), Some(30))
    );
    let Ok(Command::Plugin(p)) = parse(&["plugin", "run", "pages", "--", "auth", "--status"])
    else {
        panic!()
    };
    assert_eq!(
        p.action,
        PluginAction::Run { id: "pages".into(), args: words(&["auth", "--status"]) }
    );
    for (args, action) in [
        (&["plugin", "list"][..], PluginAction::List),
        (&["plugin", "inspect", "x"], PluginAction::Inspect { id: "x".into() }),
        (&["plugin", "unregister", "x"], PluginAction::Unregister { id: "x".into() }),
        (&["plugin", "check", "./p"], PluginAction::Check { program: "./p".into() }),
    ] {
        let Ok(Command::Plugin(p)) = parse(args) else { panic!("{args:?}") };
        assert_eq!(p.action, action);
    }
    assert!(matches!(parse(&["plugin"]), Ok(Command::Help(Some("plugin")))));
    assert_eq!(problem(&["plugin", "register", "x"]).title, "NAME AN ID AND A PROGRAM");
    assert_eq!(problem(&["plugin", "check"]).title, "NAME ONE PROGRAM TO CHECK");
    assert_eq!(problem(&["plugin", "inspect"]).title, "NAME ONE REGISTERED ID");
    assert_eq!(problem(&["plugin", "list", "--timeout=0"]).title, "NOT A POSITIVE TIME LIMIT");
    assert_eq!(problem(&["plugin", "list", "--dry-run"]).title, "UNKNOWN OPTION");
}

#[test]
fn publish_needs_a_publisher_and_its_configuration() {
    let Ok(Command::Publish(p)) = parse(&[
        "publish",
        "--publisher",
        "pages",
        "--config=pages.toml",
        "--dry-run",
        "--wait=0",
        "--timeout",
        "900",
        "--verbose",
    ]) else {
        panic!("not publish");
    };
    let expected = PublishCommand {
        publisher: "pages".into(),
        config: "pages.toml".into(),
        dry_run: true,
        wait: Some(0),
        timeout: Some(900),
        verbose: true,
    };
    assert_eq!(p, expected);
    assert_eq!(
        problem(&["publish", "--publisher=pages"]).title,
        "NAME THE PUBLISHER AND ITS CONFIGURATION"
    );
    assert_eq!(problem(&["publish", "--project=docs"]).title, "THIS OPTION IS FOR PLUGIN CHECKS");
    assert_eq!(
        problem(&["publish", "--wait=x", "--publisher=a", "--config=b"]).title,
        "NOT A NUMBER OF SECONDS"
    );
    // `publish pages` is the guided flow; `--publisher pages` is not.
    let Ok(Command::PublishPages(rest)) =
        parse(&["publish", "--config", "p.toml", "pages", "--yes"])
    else {
        panic!()
    };
    assert_eq!(rest, words(&["--config", "p.toml", "--yes"]));
    assert!(matches!(
        parse(&["publish", "--publisher", "pages", "--config=c"]),
        Ok(Command::Publish(_))
    ));
    let Ok(Command::PagesWorkflow(rest)) = parse(&["pages", "workflow", "--check"]) else {
        panic!()
    };
    assert_eq!(rest, words(&["--check"]));
    let Ok(Command::Gds(rest)) = parse(&["--color=never", "gds", "list", "--state=draft"]) else {
        panic!()
    };
    assert_eq!(rest, words(&["--color=never", "list", "--state=draft"]));
}

#[test]
fn usage_problems_are_reported_as_asked() {
    let (status, out, err) = run_captured(&["biuld"], "");
    assert_eq!((status, out.as_str()), (2, ""));
    assert!(err.starts_with("-- UNKNOWN COMMAND [cli.usage]") && err.contains("Hint:"), "{err}");
    let (status, _, err) = run_captured(&["--diagnostics=json", "biuld"], "");
    assert_eq!(status, 2);
    assert!(err.starts_with('{') && err.contains("\"code\":\"cli.usage\""), "{err}");
    // --diagnostics after the mistake still applies.
    let (_, _, err) = run_captured(&["convert", "--bogus", "--diagnostics", "json"], "");
    assert!(err.starts_with('{'), "{err}");
}

#[test]
fn help_and_versions_are_printed() {
    let (status, out, _) = run_captured(&[], "");
    assert_eq!(status, 0);
    assert!(out.contains("USAGE") && out.contains("quickstart"), "{out}");
    let (_, out, _) = run_captured(&["build", "--help"], "");
    assert!(out.contains("-b BUILDER SOURCEDIR OUTPUTDIR"), "{out}");
    let (status, out, _) = run_captured(&["--version"], "");
    assert_eq!(status, 0);
    assert!(out.starts_with(&format!("guidedog {VERSION}")) && out.contains("\ntypst "), "{out}");
    assert!(out.contains("graphviz"), "{out}");
    let (_, out, _) = run_captured(&["--licenses"], "");
    assert!(out.contains("AGPL-3.0-or-later"), "{out}");
}

#[test]
fn formats_lists_the_adapters() {
    let (status, out, _) = run_captured(&["formats", "--color=never"], "");
    assert_eq!(status, 0);
    assert!(
        out.contains("INPUT FORMATS")
            && out.contains("commonmark")
            && out.contains("TYPST BACKEND")
    );
    let (_, out, _) =
        run_captured(&["formats", "--format=json", "--typst=/nonexistent/typst"], "");
    assert!(
        out.starts_with("{\"readers\":[{\"id\":\"rst\"") && out.contains("\"typst\":\"\""),
        "{out}"
    );
}

#[test]
fn quickstart_needs_a_name_and_an_author() {
    let (status, _, err) = run_captured(&["quickstart", "-q", "-p", "P"], "");
    assert_eq!(status, 2);
    assert!(err.contains("PROJECT NAME AND AUTHOR ARE REQUIRED"), "{err}");
}
