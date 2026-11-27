//! The linked SSG commands work through the actual guidedog executable.

use std::collections::BTreeMap;
use std::path::Path;

use guidedog_tests::Scratch;

use super::support::{build, cli, in_order, project};

#[test]
fn publication_commands_are_linked_and_drive_real_builds() {
    let dir = project("ssg-cli");
    dir.write(
        "conf.toml",
        "project = \"SSG\"\nsource_suffix = [\".rst\", \".md\"]\n\
        extensions = [\"myst_parser\"]\n",
    );
    cli(&dir.path, 0, &["ssg", "--help"]);
    for suffix in ["rst", "md"] {
        let path = format!("post.{suffix}");
        cli(
            &dir.path,
            0,
            &["ssg", "new", &path, "--title", "出版の記録", "--now=2026-10-16T09:00:00Z"],
        );
        cli(&dir.path, 2, &["ssg", "new", &path]);
        cli(&dir.path, 0, &["ssg", "schedule", &path, "--at=2026-10-17T09:00:00Z"]);
        let status = cli(&dir.path, 0, &["ssg", "status", "--now=2026-10-16T09:00:00Z"]);
        assert!(status.out.contains("scheduled"), "{}", status.out);
        build(&dir.path, 0, &["html", "-W", "--now=2026-10-16T09:00:00Z"]);
        assert!(!dir.path.join("_build/html/post.html").exists());
        cli(&dir.path, 0, &["ssg", "publish", &path, "--now=2026-10-16T09:00:00Z"]);
        build(&dir.path, 0, &["html", "-W", "--now=2026-10-16T09:00:00Z"]);
        assert!(dir.path.join("_build/html/post.html").exists());
        cli(&dir.path, 0, &["ssg", "draft", &path]);
        build(&dir.path, 0, &["html", "-W", "--now=2026-10-16T09:00:00Z"]);
        assert!(!dir.path.join("_build/html/post.html").exists());
        std::fs::remove_file(dir.path.join(path)).unwrap();
    }
}

/// A site of two HTML mounts in English and German, the second German edition from a
/// source of its own, and a PDF book: five jobs.
fn site(name: &str, german: &str) -> Scratch {
    let dir = Scratch::new(name);
    dir.write(
        "conf.toml",
        concat!(
            "project = \"Website\"\n[site]\nbase_url = \"https://example.org/site\"\n",
            "[[site.mounts]]\nname = \"home\"\nsource = \"home\"\noutput = \"\"\n",
            "editions = [{language=\"en\",output=\"\"},{language=\"de\",output=\"de\"}]\n",
            "[[site.mounts]]\nname = \"manual\"\nsource = \"manual\"\noutput = \"manual\"\n",
            "editions = [{language=\"en\",output=\"\"},",
            "{language=\"de\",output=\"de\",source=\"german\"}]\n",
            "[[site.mounts]]\nname = \"book\"\nsource = \"manual\"\noutput = \"books\"\n",
            "builder = \"pdf\"\n",
        ),
    );
    for (source, title) in [("home", "Home"), ("manual", "Manual"), ("german", "Handbuch")] {
        dir.write(&format!("{source}/conf.toml"), &format!("project = \"{title}\"\n"));
        dir.write(&format!("{source}/index.rst"), &format!("{title}\n========\n\nText.\n"));
    }
    dir.write("german/index.rst", german);
    dir
}

/// Every file below `root`, by its path relative to `root`, with its bytes.
fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
            } else {
                let relative = path.strip_prefix(root).unwrap().to_string_lossy().into_owned();
                out.insert(relative, std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

#[test]
fn a_parallel_site_build_publishes_the_sequential_site() {
    let dir = site("ssg-parallel", "Handbuch\n========\n\nText.\n");
    let now = "--now=2026-11-27T09:00:00Z";
    let one = cli(&dir.path, 0, &["ssg", "build", "--jobs", "1", "--out", "one", now]);
    assert!(one.out.contains("one at a time: --jobs 1"), "{}", one.out);
    let many = cli(&dir.path, 0, &["ssg", "build", "--jobs", "4", "--out", "many", now]);
    assert!(many.out.contains("Built 5 editions in child processes"), "{}", many.out);
    // The children's reports come in the order of the plan, as a sequential build's.
    in_order(&many.err, &["for Home", "for Home", "for Manual", "for Handbuch", "for Manual"]);
    let (one, many) = (files(&dir.join("one")), files(&dir.join("many")));
    assert!(one.contains_key("index.html") && one.contains_key("manual/de/index.html"));
    assert!(one.keys().any(|k| k.starts_with("books/") && k.ends_with(".pdf")), "{one:?}");
    assert_eq!(one.keys().collect::<Vec<_>>(), many.keys().collect::<Vec<_>>());
    for (name, bytes) in &one {
        assert!(many[name] == *bytes, "{name} differs between the two builds");
    }
    if cfg!(target_os = "linux") {
        let record = dir.read("_build/ssg-projects/memory.toml");
        for key in ["home/en", "home/de", "manual/en", "manual/de", "book/en"] {
            assert!(record.contains(&format!("\"{key}\" = ")), "{record}");
        }
    }
}

#[test]
fn a_parallel_site_build_reports_the_first_failure_in_plan_order() {
    // `manual/de` fails (an unknown role is a warning, and mounts build with -W); so
    // does `home/de` when its source breaks too, and it comes first in the plan.
    let dir = site("ssg-parallel-failure", "Handbuch\n========\n\n:nosuchrole:`x`\n");
    let now = "--now=2026-11-27T09:00:00Z";
    let failed = cli(&dir.path, 1, &["ssg", "build", "--jobs", "4", now]);
    assert!(failed.err.contains("manual/de"), "{}", failed.err);
    dir.write("home/index.rst", "Home\n====\n\n:nosuchrole:`x`\n");
    let failed = cli(&dir.path, 1, &["ssg", "build", "--jobs", "4", now]);
    let problem = failed.err.rsplit("MOUNT BUILD FAILED").next().unwrap_or_default();
    assert!(problem.contains("home/en"), "{}", failed.err);
    assert!(!dir.exists("_build/site/index.html"), "a failed build publishes nothing");
}
