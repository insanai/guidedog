//! `release plugin-compatibility`: qualifies plugin protocol 1.0 against the released
//! 0.6.0 host and SDK in both directions. The old host runs the new plugin programs, and
//! the new host runs the old ones, with release assets downloaded and pinned by SHA-256.

mod pins;
mod record;
mod scenarios;

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::args::{self, Spec};
use crate::fsutil::{TempDir, absolute, create_dir};
use crate::{Result, fail, process, repository_root};
use pins::Pin;
use record::{Outcome, Record, Status};
use scenarios::Step;

const HELP: &str = "\
Qualifies plugin protocol 1.0 against the released Guidedog 0.6.0 in both directions,
offline once the pinned release assets are downloaded:

  old host, new plugins   the 0.6.0 guidedog runs this checkout's guidedog-pages and
                          the folder publisher example; it must refuse the build-plugin
                          example cleanly, since 0.6.0 has no build plugins
  new host, old plugins   this checkout's guidedog runs the 0.6.0 guidedog-pages and
                          the 0.6.0 folder publisher example (built with odin, when it
                          is installed)

Each pairing registers (the handshake) and inspects the program, plans a dry run that
writes nothing, and the folder publisher also publishes, republishes unchanged and
publishes a changed generation. A record of versions, pins and results is printed;
any incompatibility exits 1.

Usage: cargo xtask release plugin-compatibility [--work DIR]

  --work DIR   keep downloads and test folders in DIR (default: a temporary folder,
               removed afterwards); a download with the pinned SHA-256 is reused

Runs on Linux x86-64 and Apple silicon, the platforms the pins cover.
";

/// The new programs the pairings run, built from this checkout.
struct NewPrograms {
    host: PathBuf,
    pages: PathBuf,
    publisher: PathBuf,
    build_plugin: PathBuf,
}

/// The old programs, from the 0.6.0 release.
struct OldPrograms {
    host: PathBuf,
    pages: PathBuf,
    /// The 0.6.0 publisher example, or why it could not be built.
    publisher: Step<PathBuf>,
}

/// Runs `release plugin-compatibility`.
pub(super) fn main(args: &[String]) -> Result<()> {
    let spec = Spec {
        command: "release plugin-compatibility",
        flags: &[],
        options: &["--work"],
        help: HELP,
    };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(0, &spec)?;
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let Some((platform, program_pin)) = pins::program_pin(os, arch) else {
        return Err(fail(
            "compat-platform",
            "NO PINNED RELEASE FOR THIS MACHINE",
            &format!(
                "The 0.6.0 release assets are pinned for linux-amd64 and macos-arm64, \
                      not {os}-{arch}."
            ),
            "Run plugin compatibility qualification on Linux x86-64 or Apple silicon.",
        ));
    };
    let temp;
    let work = match parsed.option("--work") {
        Some(dir) => absolute(Path::new(dir)),
        None => {
            temp = TempDir::new("compat")?;
            temp.path().to_path_buf()
        }
    };
    create_dir(&work)?;
    let new = build_new_programs()?;
    let old = prepare_old_programs(&work, platform, &program_pin)?;
    let mut record = Record {
        platform,
        old_host: version(&old.host)?,
        new_host: version(&new.host)?,
        pins: vec![pins::SOURCE, program_pin],
        outcomes: Vec::new(),
    };
    run_old_host(&mut record, &old, &new, &work.join("old-host_new-plugin"))?;
    run_new_host(&mut record, &old, &new, &work.join("new-host_old-plugin"));
    print!("{}", record.render());
    verdict(&record)
}

/// Builds guidedog, guidedog-pages and the gd-plugins examples (debug), and finds them.
fn build_new_programs() -> Result<NewPrograms> {
    let root = repository_root();
    let mut cargo = Command::new("cargo");
    cargo.current_dir(&root).args(["build", "--locked", "-p", "guidedog-cmd"]);
    cargo.args(["-p", "guidedog-pages-cmd", "-p", "gd-plugins"]);
    cargo.args(["--example", "publisher", "--example", "build_plugin"]);
    println!("{}", process::describe(&cargo));
    process::run(&mut cargo)?;
    let out = super::build::target_dir(&root).join("debug");
    let exe = std::env::consts::EXE_SUFFIX;
    Ok(NewPrograms {
        host: out.join(format!("guidedog{exe}")),
        pages: out.join(format!("guidedog-pages{exe}")),
        publisher: out.join(format!("examples/publisher{exe}")),
        build_plugin: out.join(format!("examples/build_plugin{exe}")),
    })
}

/// Downloads and verifies the pinned assets, unpacks the programs, and builds the old
/// publisher example from the source archive when odin is installed.
fn prepare_old_programs(work: &Path, platform: &str, program_pin: &Pin) -> Result<OldPrograms> {
    let downloads = work.join("downloads");
    create_dir(&downloads)?;
    let source = pins::fetch(&pins::SOURCE, &downloads)?;
    let programs = pins::fetch(program_pin, &downloads)?;
    let release = work.join("release");
    create_dir(&release)?;
    untar(&programs, &release, false)?;
    let dir = release.join(format!("guidedog-{platform}"));
    let publisher = build_old_publisher(work, &source);
    Ok(OldPrograms { host: dir.join("guidedog"), pages: dir.join("guidedog-pages"), publisher })
}

fn untar(archive: &Path, into: &Path, strip: bool) -> Result<()> {
    let mut tar = Command::new("tar");
    tar.arg("-xzf").arg(archive).arg("-C").arg(into);
    if strip {
        tar.arg("--strip-components=1");
    }
    process::run(&mut tar)
}

/// Builds `examples/publisher` of the 0.6.0 source, a publisher made with the 0.6.0 SDK,
/// exactly as the Odin script did; without odin the pairing is skipped.
fn build_old_publisher(work: &Path, archive: &Path) -> Step<PathBuf> {
    let installed = Command::new("odin").arg("version").output().is_ok();
    if !installed {
        return Err(String::from(
            "odin is not installed, so the 0.6.0 publisher example cannot be built; the \
             0.6.0 guidedog-pages still covers an old publisher",
        ));
    }
    let source = work.join("source");
    let out = work.join("old-publisher");
    let unpacked = create_dir(&source).and_then(|()| untar(archive, &source, true));
    unpacked.map_err(|_| String::from("the 0.6.0 source archive could not be unpacked"))?;
    let mut odin = Command::new("odin");
    odin.arg("build").arg(source.join("examples/publisher"));
    odin.args(["-vet", "-strict-style"]).arg(format!("-out:{}", out.display()));
    let built = process::capture(&mut odin);
    match built {
        Ok(result) if result.success => Ok(out),
        _ => Err(String::from("odin could not build the 0.6.0 examples/publisher")),
    }
}

fn version(program: &Path) -> Result<String> {
    scenarios::version_line(program).map_err(|why| {
        fail(
            "compat-program",
            "PROGRAM DOES NOT RUN",
            &format!("{}: {why}.", program.display()),
            "Check that the program was built or unpacked for this machine.",
        )
    })
}

/// The old host against the new plugin programs.
fn run_old_host(
    record: &mut Record,
    old: &OldPrograms,
    new: &NewPrograms,
    dir: &Path,
) -> Result<()> {
    const DIRECTION: &str = "old-host_new-plugin";
    let host = &old.host;
    let outcome = scenarios::folder_publisher(host, &new.publisher, &dir.join("folder"));
    push(record, DIRECTION, "folder publisher (gd-plugins example)", FOLDER_OPS, outcome);
    let pages = version(&new.pages)?;
    let pages =
        pages.split_whitespace().take(2).collect::<Vec<_>>().join(" ") + " (this checkout)";
    let outcome = scenarios::pages_publisher(host, &new.pages, &dir.join("pages"));
    push(record, DIRECTION, &pages, PAGES_OPS, outcome);
    let outcome = scenarios::build_plugin_refused(host, &new.build_plugin, &dir.join("build"));
    let ops = "register refused cleanly (0.6.0 has no build plugins)";
    push(record, DIRECTION, "product-badge build plugin (serve_plugin)", ops, outcome);
    Ok(())
}

/// The new host against the old plugin programs.
fn run_new_host(record: &mut Record, old: &OldPrograms, new: &NewPrograms, dir: &Path) {
    const DIRECTION: &str = "new-host_old-plugin";
    let host = &new.host;
    let outcome = scenarios::pages_publisher(host, &old.pages, &dir.join("pages"))
        .and_then(|()| scenarios::check_declines_publisher(host, &old.pages, &dir.join("pages")));
    let ops = "init, register, inspect, dry run, unregister, check declines";
    push(record, DIRECTION, "guidedog-pages 0.6.0 (release asset)", ops, outcome);
    let plugin = "folder publisher (0.6.0 example, Odin)";
    match &old.publisher {
        Ok(program) => {
            let outcome = scenarios::folder_publisher(host, program, &dir.join("folder"));
            push(record, DIRECTION, plugin, FOLDER_OPS, outcome);
        }
        Err(why) => skip(record, DIRECTION, plugin, FOLDER_OPS, why),
    }
    let why = "0.6.0 has no build plugins: its host and SDK speak only the publisher half of \
               protocol 1.0, and no release ships a build-plugin program, so this direction \
               is covered for publishers only";
    skip(record, DIRECTION, "build plugin from 0.6.0", "register, check", why);
}

const FOLDER_OPS: &str = "register, inspect, dry run, publish, unchanged, changed";
const PAGES_OPS: &str = "init, register, inspect, dry run (stops unbound), unregister";

fn push(
    record: &mut Record,
    direction: &'static str,
    plugin: &str,
    ops: &'static str,
    step: Step,
) {
    let status = match step {
        Ok(()) => Status::Passed,
        Err(why) => Status::Failed(why),
    };
    record.outcomes.push(Outcome {
        direction,
        plugin: plugin.to_string(),
        operations: ops,
        status,
    });
}

fn skip(record: &mut Record, direction: &'static str, plugin: &str, ops: &'static str, why: &str) {
    let status = Status::Skipped(why.to_string());
    record.outcomes.push(Outcome {
        direction,
        plugin: plugin.to_string(),
        operations: ops,
        status,
    });
}

/// Fails, naming each failed pairing, unless every pairing that ran passed.
fn verdict(record: &Record) -> Result<()> {
    if record.is_compatible() {
        println!("plugin protocol 1.0 is compatible with Guidedog 0.6.0 in both directions");
        return Ok(());
    }
    let mut message = String::from("These pairings of a host and a plugin program failed:\n");
    for outcome in record.failures() {
        let why = match &outcome.status {
            Status::Failed(why) => why.as_str(),
            Status::Passed | Status::Skipped(_) => "",
        };
        message.push_str(&format!("\n    {} {}: {why}", outcome.direction, outcome.plugin));
    }
    Err(fail(
        "compat",
        "PLUGIN PROTOCOL INCOMPATIBILITY",
        &message,
        "Protocol 1.0 must keep working with the 0.6.0 host and SDK. Run the failing \
         command by hand with --work DIR and GUIDEDOG_CONFIG_DIR=DIR/<pairing>/state, and \
         fix the new side; never change the released one.",
    ))
}
