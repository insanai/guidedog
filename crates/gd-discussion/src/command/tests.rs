#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

use super::options::{commands_text, parse, validate};
use super::*;
use crate::tests::{TempDir, prefix};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|a| a.to_string()).collect()
}

fn gds() -> Program<'static> {
    Program {
        name: "guidedog gds",
        tool: "typst",
        prefix: Some(prefix("gds")),
        dir: Some(PathBuf::from("docs/gds")),
        ..Program::default()
    }
}

/// Parses and validates `list` as `program` would; the first problem, if any.
fn check(program: &Program<'_>, list: &[&str]) -> Result<Options> {
    let o = parse(program, &args(list))?;
    validate(program, &o)?;
    Ok(o)
}

#[test]
fn options_apply_only_to_their_commands() {
    let dsc = Program::default();
    let title = "Costs $(touch nope) `echo no`";
    let o = check(&dsc, &["new", "--prefix=eds", "--author=Ada", "--", title]).unwrap();
    assert_eq!((o.identifier.as_str(), o.prefix.as_str()), (title, "eds"));
    check(&dsc, &["promote", "cache-policy", "--reason=Reviewed", "--evidence", "Notes"])
        .expect("promote takes a reason and evidence");
    for list in [
        &["list", "--reason=no"][..],
        &["show"],
        &["recover"],
        &["list", "--prefix=../x"],
        &["list", "--author=Ada"],
        &["index", "--rollback"],
        &["bogus"],
        &["build", "--typst=/usr/bin/typst"], // dsc leaves Typst to guidedog.
        &["list", "--config=x.toml"],         // --config starts a catalog; init alone takes it.
        &["list", "--format=yaml"],
        &["new", "a", "b"],
        &["list", "--dry-run=yes"],
        &["promote", "x", "--number=0"],
    ] {
        let p = check(&dsc, list).expect_err("refused");
        assert_eq!(p.exit, 2, "{list:?}");
        assert!(!p.hint.is_empty());
    }
    // guidedog gds manages Guidedog's own catalog only.
    for list in [&["init", "x"][..], &["list", "--prefix=eds"]] {
        assert_eq!(check(&gds(), list).unwrap_err().exit, 2, "{list:?}");
    }
    let o = check(&gds(), &["build", "--typst=/usr/bin/typst"]).unwrap();
    assert_eq!(o.tool, Some(PathBuf::from("/usr/bin/typst")));
    assert!(!commands_text(&gds()).contains("init"));
}

#[test]
fn misspellings_get_a_suggestion() {
    let dsc = Program::default();
    assert_eq!(check(&dsc, &["promot", "x"]).unwrap_err().suggestion.as_deref(), Some("promote"));
    assert_eq!(
        check(&dsc, &["list", "--lavel=x"]).unwrap_err().suggestion.as_deref(),
        Some("--label")
    );
    assert_eq!(
        check(&dsc, &["list", "--color=allways"]).unwrap_err().suggestion.as_deref(),
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

/// Runs `list` as `dsc`, returning the status, standard output, and standard error.
fn dsc(list: &[&str]) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = run_with(&Program::default(), &args(list), &mut out, &mut err, (false, false));
    (status, String::from_utf8_lossy(&out).into(), String::from_utf8_lossy(&err).into())
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
}
