use super::*;
use crate::testdir::TestDir;

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

fn run_text(words: &[&str]) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = run_with(&Program::default(), &args(words), &mut out, &mut err);
    (status, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn command_lines_name_what_they_take() {
    let (o, r) = parse(&args(&["publish", "--config", "x.toml", "--wait=30", "--dry-run"]));
    assert!(r.is_ok());
    assert_eq!(
        (o.command.as_str(), o.config.as_str(), o.wait, o.dry_run),
        ("publish", "x.toml", Some(30), true)
    );
    let (o, r) = parse(&args(&["auth", "login", "--scope=private", "--open"]));
    assert!(r.is_ok());
    assert_eq!((o.sub.as_str(), o.scope.as_str(), o.open), ("login", "private", true));
    let refused = [
        (vec!["publish", "--adopt"], "publish has no such option"),
        (vec!["publish", "--colour=x"], "Unknown option"),
        (vec!["publish", "--yes=1"], "A switch takes no value"),
        (vec!["publish", "--config"], "Missing value"),
        (vec!["publish", "extra"], "Unexpected word"),
        (vec!["deploy"], "Unknown command"),
        (vec!["auth", "whoami"], "Unknown auth command"),
        (vec!["publish", "--wait=-1"], "Not a number of seconds"),
        (vec!["publish", "--credential=file"], "Unknown credential"),
    ];
    for (words, title) in refused {
        let (_, r) = parse(&args(&words));
        let p = r.unwrap_err();
        assert_eq!(p.title, title, "{words:?}");
        assert_eq!((p.code.as_str(), p.exit), ("pages.usage", 2));
    }
}

#[test]
fn problems_are_reported_as_asked() {
    let (status, _, err) = run_text(&["deploy", "--diagnostics=json"]);
    assert_eq!(status, 2);
    let v = gd_json::parse(err.trim()).unwrap();
    assert_eq!(v.get("code").and_then(gd_json::Value::as_str), Some("pages.usage"));
    let (status, _, err) = run_text(&["deploy", "--color=never"]);
    assert_eq!(status, 2);
    assert!(err.contains("UNKNOWN COMMAND") && err.contains("Hint:"), "{err}");
    let (status, out, _) = run_text(&["--version"]);
    assert_eq!(status, 0);
    assert!(out.starts_with("guidedog pages "));
    let (status, out, _) = run_text(&[]);
    assert_eq!(status, 0);
    assert!(out.contains("Usage: guidedog pages COMMAND"));
}

#[test]
fn init_writes_a_configuration_once() {
    let dir = TestDir::new("init");
    std::fs::create_dir(dir.path().join("docs")).unwrap();
    let config = dir.path().join("pages.toml").display().to_string();
    let init = ["init", "--repository=octo/manual", "--project=docs", "--config", &config];
    let (status, out, _) = run_text(&init);
    assert_eq!(status, 0, "{out}");
    let text = std::fs::read_to_string(&config).unwrap();
    assert!(text.contains("repository = \"octo/manual\""), "{text}");
    let (status, _, err) = run_text(&init);
    assert_eq!(status, 2);
    assert!(err.contains("pages.exists"), "{err}");
    let (status, _, _) = run_text(&["init", "--repository=octo", "--project=docs"]);
    assert_eq!(status, 2);
    let (status, _, _) =
        run_text(&["init", "--repository=octo/m", "--project=docs", "--site=out"]);
    assert_eq!(status, 2);
}

/// A host that keeps what a contributed command writes.
#[derive(Default)]
struct Captured {
    out: Vec<u8>,
    err: Vec<u8>,
}

impl gd_plugins::CommandHost for Captured {
    fn streams(&mut self) -> (&mut dyn Write, &mut dyn Write) {
        (&mut self.out, &mut self.err)
    }

    fn read_line(&mut self) -> Option<String> {
        None
    }

    fn build_project(&mut self, _: &gd_plugins::BuildRequest) -> Result<(), Problem> {
        Ok(())
    }
}

#[test]
fn guidedog_runs_the_contributed_commands() {
    let plugin = crate::plugin();
    let registry = gd_plugins::Registry::new(vec![plugin.clone()]).unwrap();
    let names: Vec<String> = registry.commands().map(|(_, c)| c.name()).collect();
    assert!(names.contains(&"publish pages".into()));
    assert!(names.contains(&"pages workflow".into()));
    assert!(names.contains(&"pages init".into()));
    assert!(names.contains(&"pages recover".into()));
    let dir = TestDir::new("extension");
    let mut host = Captured::default();
    let mut ctx = gd_plugins::CommandContext::new(&mut host);
    ctx.dir = dir.path().to_path_buf();
    let offline = args(&["--offline", "--project=manual", "--color=never"]);
    let status = plugin.run_command(&mut ctx, "pages workflow", &offline).unwrap();
    // Offline, the checksum is a placeholder: the workflow is printed, and status 1.
    assert_eq!(status, 1);
    let out = String::from_utf8(host.out).unwrap();
    assert!(out.contains("manual") && out.contains("GUIDEDOG_SHA256"), "{out}");
    assert!(String::from_utf8(host.err).unwrap().contains("pages.workflow-sha256"));
    let mut host = Captured::default();
    let mut ctx = gd_plugins::CommandContext::new(&mut host);
    let p = plugin.run_command(&mut ctx, "publish pages", &args(&["--adopt"])).unwrap_err();
    assert_eq!((p.code.as_str(), p.exit), ("pages.usage", 2));
}
