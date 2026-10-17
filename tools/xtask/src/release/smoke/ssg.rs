//! Complete SSG qualification runs from every unpacked archive with an empty PATH.

use std::path::Path;

use super::{Runner, check_pdf, smoke_failure};
use crate::Result;
use crate::fsutil::{create_dir, read_text, sha256_file, write};

pub(super) fn run(runner: &Runner, root: &Path) -> Result<()> {
    for preset in ["project", "journal", "research"] {
        runner.run(
            root,
            &[
                "ssg",
                "init",
                preset,
                "--preset",
                preset,
                "--title",
                "Archive smoke",
                "--now=2026-10-16T00:00:00Z",
            ],
        )?;
    }
    let conf = root.join("journal/conf.toml");
    let config = read_text(&conf)?.replace("page_size = 10", "page_size = 1");
    write(&conf, config.replace("[site]", "[site]\ntheme = true"))?;
    write(
        &root.join("journal/posts/second.rst"),
        concat!(
            ":date: 2026-10-16\n:tags: journal\n\nSecond observation\n==================\n",
            "\nWe test **rich archive excerpts**. See `the guide <mount://home/guide.html>`_.\n",
            "\n.. more::\n\nArchive-only tail.\n"
        ),
    )?;
    install_theme(runner, root)?;
    write(
        &root.join("conf.toml"),
        concat!(
            "project=\"Mounted website\"\n[site]\nbase_url=\"https://example.org/site\"\n",
            "[[site.mounts]]\nname=\"home\"\nsource=\"project\"\noutput=\"\"\n",
            "[[site.mounts]]\nname=\"journal\"\nsource=\"journal\"\noutput=\"journal\"\n",
            "[[site.mounts]]\nname=\"research\"\nsource=\"research\"\noutput=\"research\"\n"
        ),
    )?;
    runner.run(root, &["ssg", "build", "--now=2026-10-16T00:00:00Z"])?;
    let site = root.join("_build/site");
    for file in [
        "index.html",
        "guide.html",
        "journal/journal/page/2/index.html",
        "journal/tags/journal/page/2/index.html",
        "journal/year/2026/index.html",
        "journal/journal/atom.xml",
        "research/papers/index.html",
    ] {
        if !site.join(file).is_file() {
            return Err(smoke_failure("ssg-complete", &format!("missing {file}")));
        }
    }
    let html = read_text(&site.join("journal/index.html"))?;
    if !html.contains("Pinned theme") || html.contains("landing.html") {
        return Err(smoke_failure(
            "ssg-complete",
            "theme missing or publication metadata rendered as body",
        ));
    }
    check_pdf(&site.join("research/papers/first-paper.pdf"), "ssg-complete")?;
    check_excerpts(&site)?;
    println!(
        "ssg smoke passed: presets, mounts, taxonomy, pagination, excerpts, feeds, theme and PDF"
    );
    Ok(())
}

fn check_excerpts(site: &Path) -> Result<()> {
    for (file, guide) in [
        ("journal/index.html", "../guide.html"),
        ("journal/journal/index.html", "../../guide.html"),
    ] {
        let html = read_text(&site.join(file))?;
        if !html.contains("<strong>rich archive excerpts</strong>")
            || html.contains("Archive-only tail.")
            || !html.contains(&format!("href=\"{guide}\""))
        {
            return Err(smoke_failure("ssg-excerpts", "rich introduction missing or tail leaked"));
        }
    }
    let full = read_text(&site.join("journal/posts/second.html"))?;
    if !full.contains("Archive-only tail.") || full.contains("more::") {
        return Err(smoke_failure("ssg-excerpts", "full article lost content or rendered marker"));
    }
    Ok(())
}

fn install_theme(runner: &Runner, root: &Path) -> Result<()> {
    let templates = root.join("theme-source/_templates");
    create_dir(&templates)?;
    write(
        &templates.join("landing.html"),
        concat!(
            "{% extends 'layout.html' %}{% block body %}",
            "<p>Pinned theme</p>{{ body }}{% endblock %}"
        ),
    )?;
    runner.run(root, &["ssg", "theme", "pack", "theme-source", "--out=theme.gdt"])?;
    let hash = sha256_file(&root.join("theme.gdt"))?;
    runner.run(
        root,
        &[
            "ssg",
            "theme",
            "install",
            "theme.gdt",
            "--project=journal",
            &format!("--sha256={hash}"),
        ],
    )?;
    runner.run(root, &["ssg", "theme", "verify", "journal"])
}
