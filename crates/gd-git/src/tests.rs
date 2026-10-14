use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::*;

#[test]
fn ids_match_git() {
    assert_eq!(
        object_id(Kind::Blob, b"hello\n").to_hex(),
        "ce013625030ba8dba906f756967f9e9ca394464a"
    );
    assert_eq!(object_id(Kind::Tree, b"").to_hex(), "4b825dc642cb6eb9a060e54bf8d69288fbee4904");
    let id = parse_id("CE013625030ba8dba906f756967f9e9ca394464a");
    assert_eq!(
        id.map(|i| i.to_hex()).as_deref(),
        Some("ce013625030ba8dba906f756967f9e9ca394464a")
    );
    for bad in ["", "ce01", &"g".repeat(40)] {
        assert!(parse_id(bad).is_none(), "{bad:?} accepted");
    }
    // A message spanning several SHA-1 blocks.
    let long = vec![b'a'; 1000];
    let id = object_id(Kind::Blob, &long);
    assert_eq!(id.to_hex(), "a50be72b20f0e3f078d252e8e56b11b4bec67509");
}

fn file(path: &str, mode: Mode, id: Id) -> File {
    File { path: path.to_string(), mode, id }
}

// Trees list folders as if their names ended with "/", and refuse what Git refuses.
#[test]
fn trees_follow_gits_rules() {
    let blob = object_id(Kind::Blob, b"x");
    let trees = build_trees(&[
        file("a.b", Mode::File, blob),
        file("a/c", Mode::File, blob),
        file("a/d/e", Mode::Executable, blob),
        file("_static/x.css", Mode::File, blob),
    ])
    .expect("valid tree");
    assert_eq!(trees.len(), 4);
    let root = &trees[trees.len() - 1];
    assert_eq!(root.path, "");
    let text = String::from_utf8_lossy(&root.data);
    assert!(text.find("a.b") < text.find("40000 a\0"));
    let f = |p: &str| file(p, Mode::File, blob);
    let bad: Vec<Vec<File>> = vec![
        vec![],
        vec![f("a/../b")],
        vec![f("/a")],
        vec![f(".git/x")],
        vec![f("a"), f("a")],
        vec![f("a"), f("a/b")],
        vec![f("a/b"), f("a")],
        vec![file("t", Mode::Tree, blob)],
    ];
    for files in bad {
        let err = build_trees(&files).expect_err("accepted a bad tree");
        assert_eq!(err.kind, ErrorKind::InvalidPath);
    }
}

fn packet(s: &str) -> String {
    format!("{:04x}{s}", s.len() + 4)
}

#[test]
fn packets_parse() {
    let head = "a".repeat(40);
    let first = format!("{head} refs/heads/gh-pages\0report-status object-format=sha1 agent=x\n");
    let text = format!("001f# service=git-receive-pack\n0000{}0000", packet(&first));
    let a = parse_advertisement(text.as_bytes()).expect("advertisement");
    assert_eq!(a.find("refs/heads/gh-pages").map(|i| i.to_hex()), Some(head));
    assert!(a.has_capability("object-format") && a.has_capability("report-status"));
    assert!(!a.has_capability("atomic") && !a.has_capability("object"));
    let report =
        format!("{}{}0000", packet("unpack ok\n"), packet("ng refs/heads/gh-pages fetch first\n"));
    let r = parse_report(report.as_bytes()).expect("report");
    assert!(r.is_unpacked() && r.refs.len() == 1);
    assert!(!r.refs[0].is_ok() && r.refs[0].refusal.as_deref() == Some("fetch first"));
    for bad in ["", "zzzz", "0005", "0009unpa"] {
        assert!(parse_report(bad.as_bytes()).is_err(), "accepted {bad:?}");
        assert!(parse_advertisement(bad.as_bytes()).is_err(), "accepted {bad:?}");
    }
}

#[test]
fn errors_render_in_elm_style() {
    let err = build_trees(&[]).expect_err("empty tree");
    let text = err.to_string();
    assert!(text.starts_with("-- INVALID GIT PATH [git.invalid-path] ---"), "{text}");
    assert!(text.contains("A tree needs at least one file.\n\nHint: "), "{text}");
}

#[test]
fn packs_refuse_misuse() {
    let dir = temp_dir("misuse");
    let path = dir.join("p.pack");
    let mut w = PackWriter::begin(&path, 1).expect("begin");
    assert!(PackWriter::begin(&path, 1).is_err(), "the pack must not be overwritten");
    w.add(Kind::Blob, b"x").expect("add");
    assert_eq!(w.add(Kind::Blob, b"y").map_err(|e| e.kind), Err(ErrorKind::Pack));
    drop(w);
    assert!(!path.exists(), "an unfinished pack is removed");
    let w = PackWriter::begin(&path, 2).expect("begin");
    assert_eq!(w.end().map_err(|e| e.kind), Err(ErrorKind::Pack));
    let request = dir.join("r");
    let update =
        Update { reference: "a b", old: Id::ZERO, new: Id::ZERO, pack: &path, agent: "x" };
    let bad = write_request(&request, &update);
    assert_eq!(bad.map_err(|e| e.kind), Err(ErrorKind::Protocol));
    let _ = std::fs::remove_dir_all(&dir);
}

// Each object is a header and a zlib stream that inflates to its contents.
#[test]
fn packs_hold_zlib_streams() {
    let dir = temp_dir("streams");
    let path = dir.join("p.pack");
    let page = sample(3, 5000);
    let mut w = PackWriter::begin(&path, 2).expect("begin");
    w.add(Kind::Blob, &page).expect("add");
    w.add(Kind::Tree, b"").expect("add empty");
    let size = w.end().expect("end");
    let pack = std::fs::read(&path).expect("read pack");
    assert_eq!(pack.len() as u64, size);
    assert_eq!(&pack[..12], b"PACK\0\0\0\x02\0\0\0\x02");
    let mut at = 12;
    let mut out = Vec::new();
    for (kind, data) in [(Kind::Blob, &page[..]), (Kind::Tree, &[][..])] {
        assert_eq!(pack[at] >> 4 & 7, kind as u8);
        while pack[at] & 0x80 != 0 {
            at += 1;
        }
        out.clear();
        let used = gd_flate::zlib_decompress_into(&mut out, &pack[at + 1..], 1 << 20);
        at += 1 + used.expect("a zlib stream");
        assert_eq!(out, data);
    }
    assert_eq!(&pack[at..], &gd_sha::sha1(&pack[..at])[..]);
    let _ = std::fs::remove_dir_all(&dir);
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gd-git-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn has_git() -> bool {
    Command::new("git").arg("--version").output().is_ok_and(|o| o.status.success())
}

fn run_git(dir: &Path, args: &[&str]) -> bool {
    let out = Command::new("git").current_dir(dir).args(args).output();
    match out {
        Ok(o) if o.status.success() => true,
        Ok(o) => {
            eprintln!("git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
            false
        }
        Err(e) => {
            eprintln!("git {args:?}: {e}");
            false
        }
    }
}

fn write_tree(root: &Path, files: &[(&str, Vec<u8>)]) {
    let _ = std::fs::remove_dir_all(root);
    for (path, data) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, data).expect("write");
    }
}

fn walk(root: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>) {
    for e in std::fs::read_dir(root.join(rel)).expect("read dir") {
        let e = e.expect("entry");
        let name = e.file_name().to_string_lossy().into_owned();
        let rel = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        if e.file_type().expect("type").is_dir() {
            walk(root, &rel, out);
        } else {
            out.push((rel, e.path()));
        }
    }
}

/// Packs the files of `site`, except those the branch already has (`known`), commits
/// them on top of `old`, and sends the update; returns the new commit, or ZERO when
/// receive-pack refused it.
fn push(dir: &Path, bare: &Path, site: &Path, old: Id, known: &[&str]) -> Id {
    let mut found = Vec::new();
    walk(site, "", &mut found);
    let mut files = Vec::new();
    let mut sizes = HashMap::new();
    for (rel, full) in &found {
        let (id, size) = file_id(full).expect("file id");
        files.push(file(rel, Mode::File, id));
        sizes.insert(rel.clone(), (full.clone(), size));
    }
    let trees = build_trees(&files).expect("trees");
    let who = Person {
        name: "Guidedog".into(),
        email: "guidedog@example.invalid".into(),
        time: 1_760_000_000,
        offset: 480,
    };
    let parents = if old == Id::ZERO { vec![] } else { vec![old] };
    let commit = commit_bytes(&Commit {
        tree: trees[trees.len() - 1].id,
        parents,
        author: who.clone(),
        committer: who,
        message: "Publish the site.\n".into(),
    });
    let new = object_id(Kind::Commit, &commit);
    let sending: Vec<&File> = files.iter().filter(|f| !known.contains(&f.path.as_str())).collect();
    let pack_path = dir.join(format!("{new}.pack"));
    let mut w = PackWriter::begin(&pack_path, sending.len() + trees.len() + 1).expect("begin");
    for f in sending {
        let (full, size) = &sizes[&f.path];
        w.add_file(full, *size, f.id).expect("add file");
    }
    for tree in &trees {
        w.add(Kind::Tree, &tree.data).expect("add tree");
    }
    w.add(Kind::Commit, &commit).expect("add commit");
    w.end().expect("end");
    let request_path = dir.join(format!("{new}.request"));
    let update = Update {
        reference: "refs/heads/gh-pages",
        old,
        new,
        pack: &pack_path,
        agent: "gd-test/1",
    };
    write_request(&request_path, &update).expect("request");
    let request = std::fs::read(&request_path).expect("read request");
    let out = receive_pack(bare, &request);
    let report = parse_report(&out).expect("report");
    assert!(report.is_unpacked(), "{:?}", report.unpack_error);
    if report.refs.len() != 1 || !report.refs[0].is_ok() {
        return Id::ZERO;
    }
    new
}

fn receive_pack(bare: &Path, input: &[u8]) -> Vec<u8> {
    let mut child = Command::new("git")
        .args(["receive-pack", "--stateless-rpc"])
        .arg(bare)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn git");
    let mut stdin = child.stdin.take().expect("stdin");
    let data = input.to_vec();
    let feeder = std::thread::spawn(move || {
        let _ = stdin.write_all(&data);
    });
    let out = child.wait_with_output().expect("git output");
    let _ = feeder.join();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

fn sample(seed: u32, len: usize) -> Vec<u8> {
    let mut x = seed;
    (0..len)
        .map(|i| {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            if i % 3 == 0 { (x >> 24) as u8 } else { b"abcab"[i % 5] }
        })
        .collect()
}

// Pushes to a real receive-pack: a new branch, then a change sending only new objects, and
// a stale update that receive-pack refuses. Git is the oracle; Guidedog never runs it.
#[test]
fn pushes_are_accepted_by_git() {
    if !has_git() {
        eprintln!("git is not installed: the push test is skipped");
        return;
    }
    let dir = temp_dir("push");
    let bare = dir.join("site.git");
    assert!(run_git(&dir, &["init", "--bare", "-q", bare.to_str().expect("utf-8")]));
    let site = dir.join("site");
    let big = sample(7, 200_000);
    let noise: Vec<u8> = sample(9, 70_000).iter().map(|b| b.wrapping_mul(31) ^ 0x5a).collect();
    let one = [
        ("index.html", b"<p>one</p>".to_vec()),
        ("_static/a.css", b"p{}".to_vec()),
        (".nojekyll", vec![]),
        ("_static/big.bin", big.clone()),
        ("img/noise", noise),
    ];
    write_tree(&site, &one);
    let first = push(&dir, &bare, &site, Id::ZERO, &[]);
    assert_ne!(first, Id::ZERO);
    let two = [
        ("index.html", b"<p>two</p>".to_vec()),
        ("_static/a.css", b"p{}".to_vec()),
        (".nojekyll", vec![]),
        ("_static/big.bin", big),
    ];
    write_tree(&site, &two);
    let second =
        push(&dir, &bare, &site, first, &["_static/a.css", ".nojekyll", "_static/big.bin"]);
    assert_ne!(second, Id::ZERO);
    // A stale update, from the first commit, is refused: the branch moved.
    write_tree(&site, &[("index.html", b"<p>three</p>".to_vec())]);
    assert_eq!(push(&dir, &bare, &site, first, &[]), Id::ZERO);
    let bare_text = bare.to_str().expect("utf-8");
    assert!(run_git(&dir, &["--git-dir", bare_text, "fsck", "--strict", "--no-dangling"]));
    let _ = std::fs::remove_dir_all(&dir);
}
