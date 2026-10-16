//! Run with cargo test -p gd-project --test ssg_benchmark -- --ignored --nocapture.
#![allow(clippy::unwrap_used)]
mod common;

use std::time::Instant;

use common::{TempDir, assert_built, load};
use gd_project::{BuildOptions, BuilderKind};

#[test]
#[ignore = "measures a complete 1000-article site on the local machine"]
fn thousand_article_incremental_builds() {
    let dir = TempDir::new("ssg-benchmark");
    dir.put(
        "conf.toml",
        concat!(
            "project=\"Benchmark\"\ntemplates_path=[\"_templates\"]\n[site]\n",
            "base_url=\"https://example.org/journal\"\nfeeds=true\nsitemap=true\n",
            "taxonomies=[\"tags\",\"year\"]\npage_size=20\n"
        ),
    );
    dir.put("index.rst", "Journal\n=======\n\n.. postlist:: 10\n   :excerpts:\n");
    let body = "We explain one observation and show the evidence.\n\n".repeat(100);
    let article = |index: usize, body: &str| {
        format!(
            concat!(
                ":date: 2026-10-01\n:tags: rust\n:summary: A stable summary.\n\n",
                "Article {index:04}\n============\n\n{body}"
            ),
            index = index,
            body = body
        )
    };
    for index in 0..1000 {
        dir.put(&format!("posts/{index:04}.rst"), &article(index, &body));
    }
    let project = load(&dir.0);
    let options = BuildOptions {
        now: Some(gd_env::publication_time("2026-10-16").unwrap()),
        ..Default::default()
    };
    let run = |name| {
        let started = Instant::now();
        let result = gd_project::build(&project, BuilderKind::Html, &options);
        assert_built(&result);
        println!(
            "{name}: {:.3}s, read={}, written={}, reused={}",
            started.elapsed().as_secs_f64(),
            result.read,
            result.written,
            result.unchanged
        );
        result
    };
    assert_eq!(run("cold").read, 1001);
    assert_eq!(run("unchanged").written, 0);
    dir.put("posts/0500.rst", &article(500, &format!("{body}\nOne body edit.\n")));
    let edited = run("one body edit");
    assert_eq!(edited.read, 1);
    assert_eq!(edited.written, 2, "article HTML and search index only");
    dir.put("_templates/unused.html", "An unused template");
    assert_eq!(run("unused template").written, 0);
}
