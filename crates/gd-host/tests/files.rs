//! Reading, confinement, publication, and folder operations against a real file system.

mod common;

use common::TempDir;
use gd_host::*;
use guidedoc::{ResourceProvider, ResourceStatus};
use std::fs;
use std::path::PathBuf;

fn status(files: &Files<'_>, from: &str, name: &str) -> Result<String, ResourceStatus> {
    files.provide(from, name).map(|r| r.text.to_string())
}

#[test]
fn provider_confines_includes() {
    let root = TempDir::new("confine");
    let docs = root.join("docs");
    root.write("docs/sub/a.rst", "A");
    root.write("secret.rst", "S");
    let files = Files::new(&[&docs]);
    let from = docs.join("main.rst");
    let from = from.to_str().unwrap_or_default();
    let found = files.provide(from, "sub/a.rst").map_err(|_| "refused");
    assert_eq!(found.as_ref().map(|r| &*r.text), Ok("A"));
    let canonical = docs.join("sub/a.rst");
    assert_eq!(found.map(|r| r.canonical), Ok(canonical.to_string_lossy().into_owned()));
    assert_eq!(status(&files, from, "../secret.rst"), Err(ResourceStatus::Refused));
    let absolute = root.join("secret.rst");
    let absolute = absolute.to_str().unwrap_or_default();
    assert_eq!(status(&files, from, absolute), Err(ResourceStatus::Refused));
    assert_eq!(status(&files, from, "<isonum.txt>"), Err(ResourceStatus::Refused));
    assert!(status(&files, from, "missing.rst").is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(root.join("secret.rst"), docs.join("link.rst")).expect("link");
        assert_eq!(status(&files, from, "link.rst"), Err(ResourceStatus::Refused));
        // A link that stays inside the root is refused too: the rule is about links.
        symlink(docs.join("sub/a.rst"), docs.join("inner.rst")).expect("link");
        assert_eq!(status(&files, from, "inner.rst"), Err(ResourceStatus::Refused));
        symlink(docs.join("sub"), docs.join("subdir")).expect("link");
        assert_eq!(status(&files, from, "subdir/a.rst"), Err(ResourceStatus::Refused));
        let roots = canonical_roots(&[&docs]);
        assert_eq!(confinement(&roots, &docs.join("sub/a.rst")), Confinement::Inside);
        assert_eq!(confinement(&roots, &docs.join("link.rst")), Confinement::ThroughLink);
        assert_eq!(confinement(&roots, &root.join("secret.rst")), Confinement::Outside);
        assert!(confined(&roots, &docs.join("inner.rst")));
    }
}

#[test]
fn project_rooted_names_and_byte_order_marks() {
    let root = TempDir::new("project");
    root.write("docs/deep/main.rst", "");
    root.write("docs/shared.rst", "\u{feff}shared");
    let files = Files::new(&[root.join("docs")]).with_project(root.join("docs"));
    let from = root.join("docs/deep/main.rst");
    let text = status(&files, from.to_str().unwrap_or_default(), "/shared.rst");
    assert_eq!(text.as_deref(), Ok("shared"));
}

// Files are charged to a ledger before they are read: an include the budget cannot hold
// is refused as over the budget, and read_file says so.
#[test]
fn file_reads_are_charged() {
    let root = TempDir::new("charged");
    let big = root.write("big.rst", &"x".repeat(4096));
    let budget = Budget::new(1024);
    let ledger = Ledger::new(&budget);
    let files = Files::new(&[&root.0]).with_ledger(&ledger);
    let from = root.join("main.rst");
    let from = from.to_str().unwrap_or_default();
    assert_eq!(status(&files, from, "big.rst"), Err(ResourceStatus::Limit));
    let refused = read_file(&big, Some(&ledger), MAX_SOURCE_BYTES).map(|d| d.len());
    assert_eq!(refused.map_err(|p| p.code), Err("host.budget"));
    budget.set_limit(8192);
    let data = read_file(&big, Some(&ledger), MAX_SOURCE_BYTES).map(|d| d.len());
    assert_eq!(data, Ok(4096));
    assert_eq!(budget.used(), 4096);
    ledger.settle();
    assert_eq!(budget.used(), 0);
    let small = read_file(&big, None, 16).map(|d| d.len());
    assert_eq!(small.map_err(|p| p.code), Err("host.limit.source"));
    let folder = read_file(&root.0, None, 16).map(|d| d.len());
    assert_eq!(folder.map_err(|p| p.code), Err("host.file-type"));
}

#[test]
fn streams_stop_at_the_input_limit() {
    let mut input: &[u8] = &[b'x'; 100_000];
    let refused = read_stream(&mut input, "pipe", 16);
    assert_eq!(refused.map_err(|p| p.code), Err("host.limit.source"));
    let mut exact: &[u8] = b"0123456789abcdef";
    assert_eq!(read_stream(&mut exact, "pipe", 16).map(|d| d.len()), Ok(16));
}

#[test]
fn sources_are_utf8_without_a_byte_order_mark() {
    let root = TempDir::new("source");
    let good = root.write("good.rst", "\u{feff}Title\n");
    let source = read_source(good.to_str().unwrap_or_default(), MAX_SOURCE_BYTES);
    let source = source.unwrap_or_default();
    assert_eq!(source.text, "Title\n");
    assert_eq!(PathBuf::from(&source.canonical), good);
    let bad = root.join("bad.rst");
    fs::write(&bad, b"ok\nab\xffc").expect("written");
    let problem = read_source(bad.to_str().unwrap_or_default(), MAX_SOURCE_BYTES).err();
    let problem = problem.unwrap_or_default();
    assert_eq!(problem.code, "host.utf8");
    assert!(problem.message.contains("line 2, column 3"), "{}", problem.message);
}

#[test]
fn contents_are_compared_in_chunks() {
    let root = TempDir::new("compare");
    let data = vec![7u8; 40_000];
    let path = root.join("data.bin");
    fs::write(&path, &data).expect("written");
    assert!(same_contents(&path, &data));
    assert!(!same_contents(&path, &data[1..]));
    assert!(!same_contents(&root.join("missing"), b""));
}

#[test]
fn publish_refuses_and_replaces() {
    let root = TempDir::new("publish");
    let out = root.join("nested/out.html");
    assert_eq!(publish(&out, b"one", false), Ok(()));
    assert_eq!(publish(&out, b"two", false).map_err(|p| p.code), Err("host.output.exists"));
    assert_eq!(fs::read_to_string(&out).ok().as_deref(), Some("one"));
    assert_eq!(publish(&out, b"two", true), Ok(()));
    assert_eq!(fs::read_to_string(&out).ok().as_deref(), Some("two"));
    assert_eq!(fs::read_dir(root.join("nested")).map(|d| d.count()).ok(), Some(1));
    // A stage that is dropped uncommitted leaves nothing.
    let stage = Stage::beside(&out).expect("stage");
    let staged = stage.path().to_path_buf();
    assert!(staged.exists());
    drop(stage);
    assert!(!staged.exists());
}

#[test]
fn unsynced_files_are_flushed_in_one_pass() {
    let root = TempDir::new("unsynced");
    let mut files = Vec::new();
    for i in 0..40 {
        let sub = if i % 2 == 0 { "site/sub" } else { "site" };
        let path = root.join(&format!("{sub}/{i}"));
        let text = path.to_string_lossy().into_owned();
        assert_eq!(write_unsynced(&path, text.as_bytes()), Ok(()));
        files.push(path);
    }
    for path in &files {
        assert_eq!(fs::read_to_string(path).ok(), Some(path.to_string_lossy().into_owned()));
    }
    assert_eq!(sync_all(&files), Ok(()));
    let folders = folders_of(&files, &root.0);
    assert_eq!(folders, vec![root.0.clone(), root.join("site"), root.join("site/sub")]);
    assert_eq!(sync_all(&folders), Ok(()));
    assert_eq!(sync_each(&folders), Ok(()));
    assert_eq!(sync_all(&[root.join("gone")]), Ok(()));
    for entry in fs::read_dir(root.join("site")).expect("listed").flatten() {
        assert!(!entry.file_name().to_string_lossy().starts_with(STAGE_PREFIX));
    }
}

// A process killed while writing leaves its empty stage file behind. remove_stages
// removes those, at any depth, and only names a stage is given.
#[test]
fn stages_left_by_a_killed_process_are_removed() {
    let root = TempDir::new("stages");
    let made = Stage::beside(&root.join("sub/record.json.next")).expect("stage");
    let made_path = made.path().to_path_buf();
    std::mem::forget(made); // abandoned as a killed process abandons it.
    let stale = [
        ".guidedog-stage-0123456789.next",
        ".guidedog-stage-9876543210",
        "sub/deeper/.guidedog-stage-5555555555.svg",
    ];
    let mine = [
        ".guidedog-stage-notes.txt",
        "notes.next",
        ".guidedog-stage-012345678.next",
        ".guidedog-stage-0123456789x",
        ".guidedog-stage-0123456789.tar.gz",
        "sub/.guidedog-stage-",
    ];
    stale.iter().for_each(|f| drop(root.write(f, "")));
    mine.iter().for_each(|f| drop(root.write(f, "x")));
    let folder = root.join(".guidedog-stage-1111111111");
    fs::create_dir(&folder).expect("folder");
    remove_stages(&root.0);
    assert!(!made_path.exists());
    stale.iter().for_each(|f| assert!(!root.join(f).exists(), "{f} is left"));
    mine.iter().for_each(|f| assert!(root.join(f).is_file(), "{f} was removed"));
    assert!(folder.is_dir());
    remove_stages(&root.join("missing")); // nothing to list: nothing happens.
}

// exchange swaps two folders in one step where the system can, and changes nothing
// where it cannot.
#[test]
fn exchange_swaps_two_folders() {
    let dir = TempDir::new("exchange");
    let (a, b) = (dir.join("a"), dir.join("b"));
    dir.write("a/f", "in a");
    dir.write("b/f", "in b");
    let id_a = identity(&a).ok().flatten();
    let swapped = exchange(&a, &b).expect("exchange");
    let text = |p: PathBuf| fs::read_to_string(p).unwrap_or_default();
    if swapped {
        assert_eq!(text(a.join("f")), "in b");
        assert_eq!(text(b.join("f")), "in a");
        assert_eq!(identity(&b).ok().flatten(), id_a);
    } else {
        assert_eq!(text(a.join("f")), "in a");
    }
    if cfg!(any(all(target_os = "linux", target_env = "gnu"), target_os = "macos")) {
        assert!(exchange(&a, &dir.join("missing")).is_err());
    }
}

// link_file links a file (the same file, not a copy), makes a symbolic link again, and
// never replaces a file already there.
#[test]
fn link_file_links_without_copying() {
    let dir = TempDir::new("link");
    let (from, to) = (dir.write("from", "bytes"), dir.join("to"));
    assert!(matches!(link_file(&from, &to, PathKind::File), Ok(false)));
    if cfg!(unix) {
        assert_eq!(identity(&to).ok().flatten(), identity(&from).ok().flatten());
    }
    assert_eq!(fs::read_to_string(&to).ok().as_deref(), Some("bytes"));
    assert!(link_file(&from, &to, PathKind::File).is_err());
    #[cfg(unix)]
    {
        let (link, again) = (dir.join("link"), dir.join("again"));
        std::os::unix::fs::symlink("from", &link).expect("link");
        assert!(matches!(link_file(&link, &again, PathKind::Link), Ok(false)));
        assert_eq!(fs::read_link(&again).ok(), Some(PathBuf::from("from")));
        assert_eq!(path_type(&again).ok().flatten(), Some(PathKind::Link));
    }
    assert_eq!(identity(&dir.join("missing")).ok(), Some(None));
    assert_eq!(path_type(&dir.join("missing/below")).ok(), Some(None));
    let names = list_folder(&dir.0).expect("listed");
    assert_eq!(names.len(), if cfg!(unix) { 4 } else { 2 });
    assert!(names.iter().all(|e| e.kind != PathKind::Folder));
    assert_eq!(remove_path(&to).ok(), Some(()));
    assert_eq!(path_type(&to).ok(), Some(None));
}
