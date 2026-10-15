#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

use super::options::{parse, validate};
use super::*;
use crate::tests::TempDir;
use gd_plugins::{BuildRequest, CommandHost};
use std::io::Write;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|a| a.to_string()).collect()
}

/// Parses and validates `list`; the first problem, if any.
fn check(list: &[&str]) -> Result<Options> {
    let o = parse(&args(list))?;
    validate(&o)?;
    Ok(o)
}

/// A host over buffers that records the builds it is asked for.
#[derive(Default)]
struct Host {
    out: Vec<u8>,
    err: Vec<u8>,
    builds: Vec<BuildRequest>,
}

impl CommandHost for Host {
    fn streams(&mut self) -> (&mut dyn Write, &mut dyn Write) {
        (&mut self.out, &mut self.err)
    }

    fn read_line(&mut self) -> Option<String> {
        None
    }

    fn build_project(
        &mut self,
        request: &BuildRequest,
    ) -> std::result::Result<(), gd_plugins::Problem> {
        self.builds.push(request.clone());
        Ok(())
    }
}

#[test]
fn options_apply_only_to_their_commands() {
    let title = "Costs $(touch nope) `echo no`";
    let o = check(&["new", "--prefix=eds", "--author=Ada", "--", title]).unwrap();
    assert_eq!((o.identifier.as_str(), o.prefix.as_str()), (title, "eds"));
    check(&["promote", "cache-policy", "--reason=Reviewed", "--evidence", "Notes"])
        .expect("promote takes a reason and evidence");
    for list in [
        &["list", "--reason=no"][..],
        &["show"],
        &["recover"],
        &["list", "--prefix=../x"],
        &["list", "--author=Ada"],
        &["index", "--rollback"],
        &["bogus"],
        &["list", "--config=x.toml"], // --config starts a catalog; init alone takes it.
        &["list", "--format=yaml"],
        &["new", "a", "b"],
        &["list", "--dry-run=yes"],
        &["promote", "x", "--number=0"],
    ] {
        let p = check(list).expect_err("refused");
        assert_eq!(p.exit, 2, "{list:?}");
        assert!(!p.hint.is_empty());
    }
    let o = check(&["build", "--typst=/usr/bin/typst"]).unwrap();
    assert_eq!(o.tool, Some(PathBuf::from("/usr/bin/typst")));
}

#[test]
fn misspellings_get_a_suggestion() {
    assert_eq!(check(&["promot", "x"]).unwrap_err().suggestion.as_deref(), Some("promote"));
    assert_eq!(check(&["list", "--lavel=x"]).unwrap_err().suggestion.as_deref(), Some("--label"));
    assert_eq!(
        check(&["list", "--color=allways"]).unwrap_err().suggestion.as_deref(),
        Some("always")
    );
}

#[test]
fn a_catalog_without_a_configuration_is_named_by_its_template() {
    let root = TempDir::new("template-prefix");
    std::fs::create_dir_all(root.0.join("template")).unwrap();
    std::fs::write(root.0.join("template/rfc-template.rst"), ":sds-number: XXXXX\n").unwrap();
    assert_eq!(template_prefix(&root.0), "sds");
    assert_eq!(template_prefix(Path::new("/no/such/catalog")), "");
}

/// Runs `guidedog dsc WORDS` on `host`, returning the status, standard output, and
/// standard error of this command alone.
fn dsc_on(host: &mut Host, words: &[&str]) -> (i32, String, String) {
    let (out_at, err_at) = (host.out.len(), host.err.len());
    let status = {
        let mut ctx = CommandContext::new(host);
        let plugin = crate::plugin();
        let name = format!("dsc {}", words[0]);
        plugin.run_command(&mut ctx, &name, &args(&words[1..])).unwrap()
    };
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    (status, text(&host.out[out_at..]), text(&host.err[err_at..]))
}

/// Runs `guidedog dsc WORDS` on a host of its own.
fn dsc(words: &[&str]) -> (i32, String, String) {
    dsc_on(&mut Host::default(), words)
}

#[test]
fn the_command_line_runs_a_catalog_from_init_to_decision() {
    let root = TempDir::new("command");
    let dir = root.0.join("eds");
    let dir_arg = format!("--dir={}", dir.display());
    let d = dir_arg.as_str();
    let (status, out, err) = dsc(&["init", d, "--prefix=eds", "--author=Ada"]);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("Create EDS Discussions (eds)"), "{out}");
    let (status, _, err) = dsc(&["new", "Cache policy", d, "--labels=architecture"]);
    assert_eq!(status, 0, "{err}");
    let (status, out, _) = dsc(&["promote", "cache-policy", d, "--dry-run"]);
    assert_eq!(status, 0);
    assert!(out.contains("No files changed."), "{out}");
    assert_eq!(dsc(&["promote", "cache-policy", d]).0, 0);
    let (status, out, _) = dsc(&["list", d, "--state=discussion"]);
    assert_eq!(status, 0);
    assert!(out.contains("0001") && out.contains("Cache policy"), "{out}");
    let (status, _, err) = dsc(&["state", "1", d, "--to=acepted", "--reason=Review"]);
    assert_eq!(status, 2);
    assert!(err.contains("Did you mean `accepted`?"), "{err}");
    assert_eq!(dsc(&["state", "1", d, "--to=accepted", "--reason=Review"]).0, 0);
    let (status, out, _) = dsc(&["show", "cache-policy", d, "--format=json"]);
    assert_eq!(status, 0);
    assert!(out.contains("\"state\":\"accepted\""), "{out}");
    assert_eq!(dsc(&["check", d]).0, 0);
    assert_eq!(dsc(&["index", d, "--check"]).0, 0);
    let (status, out, _) = dsc(&["config", d]);
    assert_eq!(status, 0);
    assert!(out.starts_with("# A discussion catalog") && out.contains("prefix = \"eds\""));
    let (status, _, err) = dsc(&["list", d, "--diagnostics=json", "--state=nope"]);
    assert_eq!(status, 2);
    assert!(err.starts_with('{') && err.contains("cli.usage"), "{err}");
    assert_eq!(dsc(&["recover", d, "--rollback"]).0, 0);
    // The host builds the catalog: the site and the books, strict, into _build.
    let mut host = Host::default();
    let (status, out, err) = dsc_on(&mut host, &["build", d]);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("Built"), "{out}");
    assert_eq!(host.builds.len(), 2);
    assert_eq!(host.builds[0].output, dir.join("_build/html"));
    assert!(host.builds.iter().all(|b| b.strict && b.project == dir));
    assert_eq!(host.builds[1].builder, "pdf");
    assert!(host.builds[1].overrides[0].starts_with("pdf_documents=[{root=\"records/0001-"));
}

#[test]
fn the_plugin_declares_every_command() {
    let plugin = crate::plugin();
    let names: Vec<String> = plugin.commands().iter().map(|c| c.name()).collect();
    let expected: Vec<String> = options::COMMANDS.iter().map(|c| format!("dsc {c}")).collect();
    assert_eq!(names, expected);
    assert!(gd_plugins::Registry::new(vec![plugin]).is_ok());
}

#[test]
fn books_reach_the_build_as_quoted_data() {
    let books = vec![crate::Book {
        root: "records/0001-cache".into(),
        file: "gds-0001-cache.pdf".into(),
        title: "Quotes \" and \\ slash".into(),
        author: "René, Ada".into(),
    }];
    assert_eq!(
        host::books_setting(&books),
        "pdf_documents=[{root=\"records/0001-cache\", file=\"gds-0001-cache.pdf\", \
         title=\"Quotes \\\" and \\\\ slash\", author=\"René, Ada\"}]"
    );
}
