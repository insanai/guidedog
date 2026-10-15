//! `release package` and `release source`: the release archives of one platform, and
//! the source archive of the commit.
//!
//! The guidedog archive holds one folder, guidedog-<os>-<arch>, with guidedog,
//! guidedog-pages and graphdog. The folder also has
//! README.md, LICENSE, NOTICE, CHANGELOG.md, SOURCE.txt (where the exact source is and
//! how it was built) and NAME.build.json (version, commit, target, toolchain, and the
//! programs' SHA-256). Graphviz and Expat are linked statically, so their licences
//! (COPYING-graphviz, COPYING-expat) come along and SOURCE.txt says where their sources
//! are. The working tree must be clean, so the archives match their commit.

use std::fmt::Write;
use std::path::Path;
use std::process::Command;

use crate::args::{self, Spec};
use crate::fsutil::{self, TempDir, copy, create_dir, make_executable, read_text, sha256_file};
use crate::{Result, fail, process, repository_root};

const PACKAGE_HELP: &str = "\
Packages the programs `cargo xtask release build` put in build/ into the release
archive of one platform: dist/guidedog-PACKAGE.tar.gz.

Usage: cargo xtask release package PACKAGE

  PACKAGE   linux-amd64, linux-arm64, macos-arm64 or macos-amd64

OUT sets the output folder (default: dist). The working tree must be clean.
";

const SOURCE_HELP: &str = "\
Writes the source archive of the release, dist/guidedog-VERSION-source.tar.gz: exactly
the tracked files of the current commit (git archive), in one folder named
guidedog-VERSION. Beside it go the pinned Graphviz and Expat source tarballs that the
programs link statically, as their licences and NOTICE's additional permission ask.

Usage: cargo xtask release source

OUT sets the output folder (default: dist). The working tree must be clean.
";

/// Where the release files are downloaded from.
const RELEASES: &str = "https://github.com/insanai/guidedog/releases/download";

/// The files every archive carries beside its programs.
const DOCUMENTS: [&str; 5] = ["README.md", "LICENSE", "COPYING", "NOTICE", "CHANGELOG.md"];

/// One platform's package: its name, Rust target, and what it needs to run.
#[derive(Debug, Clone, Copy)]
struct Platform {
    package: &'static str,
    target: &'static str,
    requires: &'static str,
}

const LINUX_REQUIRES: &str = "Linux with glibc 2.35 or later";
const MACOS_REQUIRES: &str = "macOS 12 or later";

const PLATFORMS: [Platform; 4] = [
    Platform {
        package: "linux-amd64",
        target: "x86_64-unknown-linux-gnu",
        requires: LINUX_REQUIRES,
    },
    Platform {
        package: "linux-arm64",
        target: "aarch64-unknown-linux-gnu",
        requires: LINUX_REQUIRES,
    },
    Platform { package: "macos-arm64", target: "aarch64-apple-darwin", requires: MACOS_REQUIRES },
    Platform { package: "macos-amd64", target: "x86_64-apple-darwin", requires: MACOS_REQUIRES },
];

/// What every archive of one package shares.
struct Release<'a> {
    root: &'a Path,
    platform: Platform,
    version: String,
    commit: String,
    command: String,
    rustc: String,
}

/// Runs `release package`.
pub(super) fn main(args: &[String]) -> Result<()> {
    let spec = Spec { command: "release package", flags: &[], options: &[], help: PACKAGE_HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(1, &spec)?;
    let name = parsed.positional(0, "a package, such as linux-amd64", &spec)?;
    let Some(platform) = PLATFORMS.iter().find(|p| p.package == name).copied() else {
        return Err(crate::usage(
            &format!("There is no package named `{name}`."),
            "Name one of linux-amd64, linux-arm64, macos-arm64 or macos-amd64.",
        ));
    };
    package(platform)
}

fn package(platform: Platform) -> Result<()> {
    let root = repository_root();
    let mut needed: Vec<String> = super::PROGRAMS.iter().map(|p| format!("build/{p}")).collect();
    needed.extend(["build/build.command", "build/rustc.version"].map(String::from));
    needed.extend(DOCUMENTS.map(String::from));
    if let Some(missing) = needed.iter().find(|file| !root.join(file).is_file()) {
        return Err(fail(
            "release-package",
            "MISSING FILE",
            &format!("{missing} is missing."),
            "Build the programs with `cargo xtask release build` first.",
        ));
    }
    super::require_clean_tree(&root)?;
    let mut git = Command::new("git");
    git.arg("-C").arg(&root).args(["rev-parse", "HEAD"]);
    let release = Release {
        root: &root,
        platform,
        version: super::version::release_version(&root)?,
        commit: process::output(&mut git)?.trim().to_string(),
        command: read_text(&root.join("build/build.command"))?.trim_end().to_string(),
        rustc: read_text(&root.join("build/rustc.version"))?.trim_end().to_string(),
    };
    let out = super::dist_dir(&root);
    create_dir(&out)?;
    archive(&release, &out, "guidedog", &super::PROGRAMS)
}

/// Writes `out/NAME-PACKAGE.tar.gz` with the programs.
fn archive(release: &Release<'_>, out: &Path, name: &str, programs: &[&str]) -> Result<()> {
    let stage = TempDir::new("package")?;
    let folder_name = format!("{name}-{}", release.platform.package);
    let folder = stage.path().join(&folder_name);
    create_dir(&folder)?;
    let mut sums = Vec::new();
    for program in programs {
        let file = folder.join(program);
        copy(&release.root.join("build").join(program), &file)?;
        make_executable(&file)?;
        sums.push(format!("\"{program}\": \"{}\"", sha256_file(&file)?));
    }
    for document in DOCUMENTS {
        copy(&release.root.join(document), &folder.join(document))?;
    }
    let graphviz = crate::graphviz::install_dir();
    for licence in ["COPYING-graphviz", "COPYING-expat"] {
        copy(&graphviz.join(licence), &folder.join(licence))?;
    }
    fsutil::write(&folder.join("SOURCE.txt"), source_text(release, programs))?;
    let binary_sum = sha256_file(&folder.join(name))?;
    let json = build_json(release, name, &binary_sum, &sums.join(", "));
    fsutil::write(&folder.join(format!("{name}.build.json")), json)?;
    let target = out.join(super::archive_name(name, release.platform.package));
    let mut tar = Command::new("tar");
    tar.current_dir(stage.path()).args(["-cf", "-", &folder_name]);
    let mut gzip = Command::new("gzip");
    gzip.args(["-n", "-9"]);
    process::pipe_to_file(&mut tar, &mut gzip, &target)?;
    println!("{}", target.display());
    Ok(())
}

/// A JSON string literal.
fn quoted(text: &str) -> String {
    let mut out = String::new();
    gd_json::write_string(&mut out, text);
    out
}

fn build_json(release: &Release<'_>, name: &str, binary_sum: &str, sums: &str) -> String {
    let Release { platform, version, commit, rustc, command, .. } = release;
    format!(
        "{{\n  \"name\": \"{name}\",\n  \"version\": \"{version}\",\n  \
         \"commit\": \"{commit}\",\n  \
         \"source\": \"https://github.com/insanai/guidedog/tree/v{version}\",\n  \
         \"package\": \"{}\",\n  \"target\": \"{}\",\n  \"rustc\": {},\n  \"command\": {},\n  \
         \"typst_embedded\": true,\n  \"graphviz\": \"{}, linked statically\",\n  \
         \"requires\": {},\n  \"binary\": \"{name}\",\n  \"binary_sha256\": \"{binary_sum}\",\n  \
         \"programs\": {{{sums}}}\n}}\n",
        platform.package,
        platform.target,
        quoted(rustc),
        quoted(command),
        crate::graphviz::GRAPHVIZ_VERSION,
        quoted(&platform.requires.replace('\n', " ")),
    )
}

/// The SOURCE.txt of an archive: where its exact source is and how it was built.
fn source_text(release: &Release<'_>, programs: &[&str]) -> String {
    let Release { platform, version, commit, rustc, command, .. } = release;
    let mut text = String::new();
    let _ = write!(
        text,
        "Guidedog {version} for {} ({})\n\n\
         Corresponding source: https://github.com/insanai/guidedog/tree/v{version}\n\
         Commit: {commit}\n\
         Source archive: {RELEASES}/v{version}/guidedog-{version}-source.tar.gz\n\n\
         Built with {rustc} and Cargo.lock, by:\n\n    {command}\n\n\
         Programs: {}\n\
         Typst: embedded Typst 0.15.1. Graphviz: linked statically.\n\
         {}\
         Requires: {}.\n\
         Licenses and notices: LICENSE, NOTICE, COPYING-graphviz and COPYING-expat.\n",
        platform.package,
        platform.target,
        programs.join(" "),
        crate::graphviz::source_text(),
        platform.requires,
    );
    text
}

/// Runs `release source`.
pub(super) fn source_main(args: &[String]) -> Result<()> {
    let spec = Spec { command: "release source", flags: &[], options: &[], help: SOURCE_HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(0, &spec)?;
    let root = repository_root();
    let version = super::version::release_version(&root)?;
    super::require_clean_tree(&root)?;
    let out = super::dist_dir(&root);
    create_dir(&out)?;
    let archive = out.join(format!("guidedog-{version}-source.tar.gz"));
    let mut git = Command::new("git");
    git.arg("-C").arg(&root).args(["archive", "--format=tar"]);
    git.arg(format!("--prefix=guidedog-{version}/")).arg("HEAD");
    let mut gzip = Command::new("gzip");
    gzip.args(["-n", "-9"]);
    process::pipe_to_file(&mut git, &mut gzip, &archive)?;
    println!("{}", archive.display());
    for tarball in crate::graphviz::fetch_sources(&out)? {
        println!("{}", tarball.display());
    }
    Ok(())
}
