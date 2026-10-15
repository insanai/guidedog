//! `cargo xtask fetch-fonts`: fetches the Noto Serif CJK faces the Chinese, Japanese and
//! Korean PDF manuals use into `docs/shared/fonts`, each checked against its pinned
//! SHA-256. They are 25 MB each, so the repository does not keep them.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::args::{self, Spec};
use crate::fsutil::{absolute, create_dir, remove, rename, sha256_file};
use crate::{Result, fail, process, repository_root};

const HELP: &str = "\
Fetches the Noto Serif CJK fonts the Chinese, Japanese and Korean PDF books use.

Usage: cargo xtask fetch-fonts [DIR]

  DIR   where the fonts go (default: docs/shared/fonts)

A font already there with the right SHA-256 is kept; a wrong one is replaced. Each
download runs the installed curl and is checked against its pinned SHA-256.
";

/// Where the fonts are downloaded from: the tagged Serif2.003 release of noto-cjk.
const BASE: &str = "https://raw.githubusercontent.com/notofonts/noto-cjk/Serif2.003/Serif/OTF";

/// Each font's path under [`BASE`] with its SHA-256.
const FONTS: [(&str, &str); 6] = [
    (
        "Japanese/NotoSerifCJKjp-Regular.otf",
        "d9854c7a8ef170b5a7932558856fd64eb8de0b007cd823fed6f9f514ad2803d3",
    ),
    (
        "Japanese/NotoSerifCJKjp-Bold.otf",
        "861a2b2c0e24b23745c262be8c3fdef63f12628f0492fb120ee51aa55c503af8",
    ),
    (
        "Korean/NotoSerifCJKkr-Regular.otf",
        "77b4b741f864d27f15e90f275b17106dde90b2ad28f82bab72dc95805db5fb42",
    ),
    (
        "Korean/NotoSerifCJKkr-Bold.otf",
        "10cc03741178ad6d2747df8497d911e34b167d0474a826fb9d866c402cbe3d8f",
    ),
    (
        "SimplifiedChinese/NotoSerifCJKsc-Regular.otf",
        "2a2eae2628df83556c54018c41e20fa532c1b862c5256ae8b3f23feb918d12ca",
    ),
    (
        "SimplifiedChinese/NotoSerifCJKsc-Bold.otf",
        "8af07d4b6c2e82bcc72a30e066eaf295f11b9424f4aad2eaa9fe0e9c3b38fc73",
    ),
];

/// Runs `cargo xtask fetch-fonts`.
pub(crate) fn main(args: &[String]) -> Result<()> {
    let spec = Spec { command: "fetch-fonts", flags: &[], options: &[], help: HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(1, &spec)?;
    let dir = match parsed.positionals.first() {
        Some(dir) => absolute(Path::new(dir)),
        None => repository_root().join("docs/shared/fonts"),
    };
    fetch_fonts(&dir)
}

/// Fetches every font into `dir` that is not already there with the right sum.
pub(crate) fn fetch_fonts(dir: &Path) -> Result<()> {
    create_dir(dir)?;
    for (path, sum) in FONTS {
        let name = path.rsplit('/').next().unwrap_or(path);
        fetch(&format!("{BASE}/{path}"), &dir.join(name), sum)?;
    }
    Ok(())
}

fn fetch(url: &str, file: &Path, sum: &str) -> Result<()> {
    if file.is_file() && sha256_file(file)? == sum {
        return Ok(());
    }
    let name = file.file_name().unwrap_or_default().to_string_lossy().into_owned();
    println!("fetching {name}");
    let part = PathBuf::from(format!("{}.part", file.display()));
    let mut curl = Command::new("curl");
    curl.args(["-sSfL", "--retry", "3", "-o"]).arg(&part).arg(url);
    process::run(&mut curl)?;
    let got = sha256_file(&part)?;
    if got != sum {
        remove(&part)?;
        return Err(fail(
            "font-sum",
            "WRONG CHECKSUM",
            &format!("{name} has SHA-256 {got}, expected {sum}."),
            "The upstream file changed; check docs/shared/fonts/PROVENANCE.txt.",
        ));
    }
    rename(&part, file)
}
