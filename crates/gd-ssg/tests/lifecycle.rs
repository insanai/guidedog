//! Authoring edits use real readers and real atomic source replacement.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use gd_ssg::{Publication, create_post, read_metadata, transition};

struct TempDir(PathBuf);
impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("gd-ssg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn rst_and_markdown_follow_the_same_publication_transitions() {
    let dir = TempDir::new("lifecycle");
    let now = gd_clock::parse_timestamp("2026-10-16T09:00:00Z").unwrap();
    for suffix in ["rst", "md"] {
        let path = dir.0.join(format!("post.{suffix}"));
        create_post(&path, "An article", "article.html", "2026-10-16").unwrap();
        assert!(read_metadata(&path).unwrap().draft);
        assert!(create_post(&path, "Replacement", "", "2026-10-16").is_err());
        transition(&path, Publication::Schedule("2026-10-20T09:00:00Z"), now).unwrap();
        let page = read_metadata(&path).unwrap();
        assert!(!page.draft);
        assert_eq!(page.ineligible_reason(now), Some("scheduled"));
        transition(&path, Publication::Publish(None), now).unwrap();
        let page = read_metadata(&path).unwrap();
        assert!(page.publish_at.is_empty());
        assert_eq!(page.ineligible_reason(now), None);
        transition(&path, Publication::Draft, now).unwrap();
        assert!(read_metadata(&path).unwrap().draft);
        let before = std::fs::read(&path).unwrap();
        assert!(transition(&path, Publication::Schedule("2026-02-30T00:00:00Z"), now).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(transition(&path, Publication::Publish(Some("2027-01-01")), now).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}

#[test]
fn publishing_a_future_dated_expired_source_takes_effect_immediately() {
    let dir = TempDir::new("publish-boundary");
    let path = dir.0.join("post.rst");
    std::fs::write(
        &path,
        concat!(
            ":draft: true\n:date: 2027-01-01\n",
            ":expires-at: 2026-01-01T00:00:00Z\n\nPaper\n=====\n\nBody unchanged.\n"
        ),
    )
    .unwrap();
    let now = gd_clock::parse_timestamp("2026-10-16T09:00:00Z").unwrap();
    transition(&path, Publication::Publish(None), now).unwrap();
    let page = read_metadata(&path).unwrap();
    assert_eq!(page.date, "2026-10-16");
    assert_eq!(page.ineligible_reason(now), None);
    assert!(
        std::fs::read_to_string(path).unwrap().ends_with("\nPaper\n=====\n\nBody unchanged.\n")
    );
}

#[cfg(unix)]
#[test]
fn edits_retain_permissions_and_refuse_source_symlinks() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = TempDir::new("permissions");
    let path = dir.0.join("post.rst");
    create_post(&path, "Article", "", "2026-10-16").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let now = gd_clock::parse_timestamp("2026-10-16T09:00:00Z").unwrap();
    transition(&path, Publication::Publish(None), now).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o640);
    let link = dir.0.join("link.rst");
    symlink(&path, &link).unwrap();
    assert!(transition(&link, Publication::Draft, now).is_err());
    assert!(!read_metadata(&path).unwrap().draft);
}
