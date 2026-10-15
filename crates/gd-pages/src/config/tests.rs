use super::*;
use crate::testdir::TestDir;

#[test]
fn configuration_mistakes_name_their_line() {
    let dir = TestDir::new("config");
    let path = dir.path().join(CONFIG_FILE);
    let good = "schema = 1\nrepository = \"octo/site\"\nsite = \"out\"\n";
    std::fs::write(&path, good).unwrap();
    let c = read_config(&path).unwrap();
    assert_eq!(c.branch, "gh-pages");
    assert_eq!(c.profile, "default");
    assert_eq!(c.site_url, "https://octo.github.io/site/");
    assert_eq!(c.source, SiteSource::Built(c.path.parent().unwrap().join("out")));
    assert!(c.strict);
    let cases = [
        (format!("{good}colour = \"blue\"\n"), ":4", "no setting colour"),
        ("schema = 1\nrepository = \"octo\"\nsite = \"out\"\n".into(), ":2", "OWNER/REPOSITORY"),
        (format!("{good}project = \"docs\"\n"), "", "either project or site"),
        (format!("{good}branch = \"-x\"\n"), ":4", "not a branch name"),
        (format!("{good}profile = \"env\"\n"), ":4", "not a profile name"),
        (format!("{good}site_url = \"http://x/\"\n"), ":4", "https"),
        (format!("{good}wait_seconds = -1\n"), ":4", "whole number"),
        (format!("{good}[watch]\nquiet = 3\n"), ":5", "no setting watch.quiet"),
        ("repository = \"octo/site\"\nsite = \"out\"\n".into(), "", "schema must be 1"),
        (format!("{good}project = [\n"), "", "TOML"),
    ];
    for (text, at, says) in &cases {
        std::fs::write(&path, text).unwrap();
        let p = read_config(&path).unwrap_err();
        assert_eq!(p.code, "pages.config", "{text:?}");
        assert!(p.path.contains(at), "{text:?}: {}", p.path);
        if *says != "TOML" {
            assert!(p.message.contains(says), "{text:?}: {}", p.message);
        }
        assert!(!p.hint.is_empty());
        assert_eq!(p.exit, 2);
    }
    assert_eq!(default_site_url("Octo", "octo.github.io"), "https://octo.github.io/");
}

#[test]
fn watch_settings_and_site_urls_are_read() {
    let dir = TestDir::new("config-watch");
    let path = dir.path().join(CONFIG_FILE);
    let text = "schema = 1\nrepository = \"octo/site\"\nsite = \"/srv/out\"\n\
                site_url = \"https://docs.example.org\"\nstrict = false\n\
                [watch]\ndebounce_ms = 200\nmax_wait_ms = 300\n";
    std::fs::write(&path, text).unwrap();
    let c = read_config(&path).unwrap();
    assert_eq!(c.site_url, "https://docs.example.org/");
    assert_eq!((c.debounce_ms, c.max_wait_ms), (200, 300));
    assert!(!c.strict);
    let low = text.replace("max_wait_ms = 300", "max_wait_ms = 100");
    std::fs::write(&path, low).unwrap();
    assert!(read_config(&path).unwrap_err().message.contains("from 200"));
    let missing = "schema = 1\nrepository = \"octo/site\"\nproject = \"nowhere\"\n";
    std::fs::write(&path, missing).unwrap();
    assert!(read_config(&path).unwrap_err().message.contains("no project folder"));
}

#[test]
fn rendered_configurations_read_back() {
    let dir = TestDir::new("config-render");
    std::fs::create_dir(dir.path().join("docs")).unwrap();
    let path = dir.path().join(CONFIG_FILE);
    let source = SiteSource::Project(PathBuf::from("docs"));
    std::fs::write(&path, render_config("octo", "manual", "pages", &source, "work")).unwrap();
    let c = read_config(&path).unwrap();
    assert_eq!(c.repository(), "octo/manual");
    assert_eq!(c.branch, "pages");
    assert_eq!(c.profile, "work");
    assert_eq!(c.source, SiteSource::Project(dir.path().join("docs")));
    assert_eq!(c.wait_seconds, 300);
}

#[test]
fn names_follow_github_and_git() {
    for good in ["octo", "a", "my-org", &"a".repeat(39)] {
        assert!(is_valid_owner(good), "{good}");
    }
    for bad in ["", "-x", "x-", "a--b", "a_b", &"a".repeat(40)] {
        assert!(!is_valid_owner(bad), "{bad}");
    }
    for good in ["site", "octo.github.io", "a_b-c"] {
        assert!(is_valid_repo(good), "{good}");
    }
    for bad in ["", ".", "..", "x.git", "a b", "a/b"] {
        assert!(!is_valid_repo(bad), "{bad}");
    }
    for good in ["gh-pages", "docs/site", "v1.2"] {
        assert!(is_valid_branch(good), "{good}");
    }
    for bad in ["", "-x", "/x", ".x", "x/", "x.lock", "x.", "a..b", "a//b", "a/.b", "a b"] {
        assert!(!is_valid_branch(bad), "{bad}");
    }
    assert!(is_valid_profile("work"));
    assert!(!is_valid_profile("env"));
    assert!(!is_valid_profile("a b"));
}
