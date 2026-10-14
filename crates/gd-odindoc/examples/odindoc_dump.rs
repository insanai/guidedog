//! Documents every package below a folder with all members, for comparing runs:
//! `cargo run --release -p gd-odindoc --example odindoc_dump -- DIR [COLLECTION] OUT`.

use gd_odindoc::{Config, DirectiveOption, Project, Request, Root, document};
use std::fmt::Write as _;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dir, collection, out) = match args.as_slice() {
        [dir, out] => (dir.clone(), String::new(), out.clone()),
        [dir, collection, out] => (dir.clone(), collection.clone(), out.clone()),
        _ => {
            eprintln!("usage: odindoc_dump DIR [COLLECTION] OUT");
            std::process::exit(2);
        }
    };
    let started = std::time::Instant::now();
    let mut project = Project::from_dirs(vec![Root { dir, collection }]);
    let config = Config::default();
    let mut text = String::new();
    for package in project.list_packages() {
        let request = Request {
            directive: "odin:autopackage".into(),
            argument: package.id.clone(),
            options: ["members", "undoc-members", "private-members"]
                .map(DirectiveOption::flag)
                .to_vec(),
            ..Request::default()
        };
        let output = document(&mut project, &config, &request);
        let _ = writeln!(text, "===== {}", package.id);
        for m in &output.messages {
            let _ = writeln!(text, "MSG {:?} {} {}", m.severity, m.code, m.text);
        }
        for line in &output.lines {
            let _ = writeln!(text, "{line}");
        }
    }
    eprintln!("{:?}", started.elapsed());
    if let Err(e) = std::fs::write(&out, text) {
        eprintln!("cannot write {out}: {e}");
    }
}
