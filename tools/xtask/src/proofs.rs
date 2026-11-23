//! Checks the Lean proofs of the design records (`docs/gds/proofs`): they build, no
//! proof is left open, and no theorem depends on an axiom beyond Lean's standard three.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::args::{self, Spec};
use crate::fsutil::{read_text, sorted_entries};
use crate::process::{capture, run};
use crate::{Failure, Result, fail, repository_root};

const HELP: &str = "\
Checks the Lean proofs of the design records in docs/gds/proofs.

Usage: cargo xtask proofs

Builds the proofs with `lake build`, refuses a proof left open (`sorry`, `admit`) or a
declared `axiom`, and compiles every `Audit.lean`, which prints the axioms each theorem
depends on: only propext, Classical.choice and Quot.sound may appear.

Needs Lean's `lake` on PATH or in ~/.elan/bin; docs/gds/proofs/lean-toolchain names the
version. The proofs use Lean's core library alone, so nothing is downloaded but Lean.
";

/// The axioms of Lean's core library that a theorem may depend on.
const STANDARD_AXIOMS: [&str; 3] = ["propext", "Classical.choice", "Quot.sound"];

pub(crate) fn main(args: &[String]) -> Result<()> {
    let spec = Spec { command: "proofs", flags: &[], options: &[], help: HELP };
    let Some(parsed) = args::parse(args, &spec)? else { return Ok(()) };
    parsed.at_most(0, &spec)?;
    let root = repository_root().join("docs/gds/proofs");
    let mut sources = Vec::new();
    collect_lean(&root, &mut sources)?;
    for source in &sources {
        refuse_open_proofs(source)?;
    }
    let lake = find_lake()?;
    run(Command::new(&lake).arg("build").current_dir(&root))?;
    let mut audited = 0;
    for audit in sources.iter().filter(|s| s.ends_with("Audit.lean")) {
        let mut command = Command::new(&lake);
        command.args(["env", "lean"]).arg(audit).current_dir(&root);
        let printed = capture(&mut command)?;
        if !printed.success {
            let message = format!("{} does not compile:\n{}", shown(audit), printed.stderr);
            return Err(invalid(&message, "Run `lake build` in docs/gds/proofs and fix it."));
        }
        audited += check_axioms(audit, &printed.stdout)?;
    }
    if audited == 0 {
        return Err(invalid(
            "No Audit.lean printed the axioms of a theorem.",
            "Add `#print axioms <theorem>` lines to the record's Audit.lean.",
        ));
    }
    println!("Verified {audited} theorems in docs/gds/proofs: built, no open proof,");
    println!("and no axiom beyond {}.", STANDARD_AXIOMS.join(", "));
    Ok(())
}

/// Lists the Lean sources below `dir`, skipping Lake's build output.
fn collect_lean(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for path in sorted_entries(dir)? {
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != ".lake" {
                collect_lean(&path, out)?;
            }
        } else if name.ends_with(".lean") {
            out.push(path);
        }
    }
    Ok(())
}

/// Fails when a source leaves a proof open or declares an axiom. Comments count too:
/// a proof file has no reason to say these words.
fn refuse_open_proofs(source: &Path) -> Result<()> {
    let text = read_text(source)?;
    for (number, line) in text.lines().enumerate() {
        let words = || line.split(|c: char| !c.is_alphanumeric() && c != '_');
        let open = words().find(|w| matches!(*w, "sorry" | "admit" | "sorryAx"));
        let declared = line.trim_start().starts_with("axiom ").then_some("axiom");
        if let Some(word) = open.or(declared).filter(|_| !source.ends_with("Audit.lean")) {
            let message = format!("{}:{} says `{word}`.", shown(source), number + 1);
            return Err(invalid(&message, "Finish the proof; a theorem must rest on no gap."));
        }
    }
    Ok(())
}

/// Reads what `#print axioms` printed for `audit` and returns how many theorems it
/// covers; fails when one depends on an axiom outside [`STANDARD_AXIOMS`].
fn check_axioms(audit: &Path, printed: &str) -> Result<usize> {
    let mut theorems = 0;
    for line in printed.lines() {
        if line.ends_with("does not depend on any axioms") {
            theorems += 1;
            continue;
        }
        let Some((theorem, axioms)) = line.split_once(" depends on axioms: [") else {
            continue;
        };
        theorems += 1;
        let axioms = axioms.trim_end_matches(']');
        for axiom in axioms.split(',').map(str::trim) {
            if !STANDARD_AXIOMS.contains(&axiom) {
                let message = format!("{theorem} depends on the axiom `{axiom}`.");
                let hint = format!("Prove it without the axiom; see {}.", shown(audit));
                return Err(invalid(&message, &hint));
            }
        }
    }
    Ok(theorems)
}

/// Finds `lake`: on PATH, or where elan installs it.
fn find_lake() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    let installed = home.map(|h| PathBuf::from(h).join(".elan/bin/lake"));
    let candidates = [Some(PathBuf::from("lake")), installed];
    for lake in candidates.into_iter().flatten() {
        if Command::new(&lake).arg("--version").output().is_ok_and(|o| o.status.success()) {
            return Ok(lake);
        }
    }
    Err(fail(
        "proofs",
        "LEAN NOT INSTALLED",
        "The program `lake` is not on PATH nor in ~/.elan/bin.",
        "Install Lean with elan (https://lean-lang.org); the proofs name their version.",
    ))
}

fn shown(path: &Path) -> String {
    let root = repository_root();
    path.strip_prefix(&root).unwrap_or(path).display().to_string()
}

fn invalid(message: &str, hint: &str) -> Failure {
    fail("proofs", "PROOFS NOT VERIFIED", message, hint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_axioms_pass_and_others_fail() {
        let audit = Path::new("Audit.lean");
        let good = "'A.b' depends on axioms: [propext, Quot.sound]\n\
                    'A.c' does not depend on any axioms\n";
        assert_eq!(check_axioms(audit, good).ok(), Some(2));
        let open = "'A.b' depends on axioms: [propext, sorryAx]\n";
        assert!(check_axioms(audit, open).is_err());
    }
}
