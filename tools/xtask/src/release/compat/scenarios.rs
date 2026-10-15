//! The operations one host runs against one plugin program, offline, each in a folder of
//! its own with machine state there too (GUIDEDOG_CONFIG_DIR, GUIDEDOG_CACHE_DIR), so the
//! user's registry is never touched. A scenario returns what went wrong as a sentence for
//! the record rather than stopping the run, so every pairing is tried.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::process;

/// The outcome of one operation: `Err` holds what went wrong.
pub(super) type Step<T = ()> = std::result::Result<T, String>;

/// One host with machine state in `dir`.
struct Session<'a> {
    host: &'a Path,
    dir: &'a Path,
}

impl Session<'_> {
    fn command(&self, program: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(program);
        command.current_dir(self.dir).args(args);
        command.env("GUIDEDOG_CONFIG_DIR", self.dir.join("state"));
        command.env("GUIDEDOG_CACHE_DIR", self.dir.join("cache"));
        command.env("NO_COLOR", "1");
        command
    }

    /// Runs the host and returns its standard output, or why it failed.
    fn host(&self, args: &[&str]) -> Step<String> {
        self.succeed(self.host, args)
    }

    /// Runs `program` and returns its standard output, or why it failed.
    fn succeed(&self, program: &Path, args: &[&str]) -> Step<String> {
        let mut command = self.command(program, args);
        let out = process::capture(&mut command).map_err(|_| unable(&command))?;
        if out.success {
            return Ok(out.stdout);
        }
        Err(format!("`{}` failed: {}", short(&command), first_line(&out.stderr)))
    }

    /// Runs the host, which must fail reporting the problem `code`.
    fn refuse(&self, args: &[&str], code: &str) -> Step {
        let mut command = self.command(self.host, args);
        command.arg("--diagnostics=json");
        let out = process::capture(&mut command).map_err(|_| unable(&command))?;
        if out.success {
            return Err(format!("`{}` succeeded; it should stop with {code}", short(&command)));
        }
        if !out.stderr.contains(&format!("\"code\":\"{code}\"")) {
            let got = first_line(&out.stderr);
            return Err(format!("`{}` did not report {code}: {got}", short(&command)));
        }
        Ok(())
    }

    /// Registers `plugin` as `id` (the ID the program declares), then checks that
    /// inspect finds the same program on disk, speaking protocol 1.0.
    fn register(&self, id: &str, plugin: &Path) -> Step {
        let path = plugin.to_string_lossy();
        self.host(&["plugin", "register", id, &path, "--yes"])?;
        let inspect = self.host(&["plugin", "inspect", id])?;
        let protocol = inspect.lines().any(|line| {
            let mut words = line.split_whitespace();
            words.next() == Some("Protocol") && words.next() == Some("1.0")
        });
        if !protocol || !inspect.contains("is the one registered") {
            return Err(String::from(
                "plugin inspect did not confirm the program and protocol 1.0",
            ));
        }
        Ok(())
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

/// Makes `dir` fresh and puts a one-page site in it, as the script did.
fn prepare(dir: &Path, page: &str) -> Step {
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| format!("cannot clear {}: {e}", dir.display()))?;
    }
    fs::create_dir_all(dir.join("site"))
        .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    write_page(dir, page)
}

fn write_page(dir: &Path, page: &str) -> Step {
    let path = dir.join("site/index.html");
    fs::write(&path, page).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// The script's matrix for the folder publisher: register (the handshake) and inspect,
/// a dry run that writes nothing, a first publication, an unchanged one, a changed
/// generation, each publication compared with the site, and unregister.
pub(super) fn folder_publisher(host: &Path, plugin: &Path, dir: &Path) -> Step {
    prepare(dir, "<p>one</p>\n")?;
    let config = "site = \"site\"\ntarget = \"public\"\n";
    fs::write(dir.join("folder.toml"), config).map_err(|e| format!("cannot write: {e}"))?;
    let session = Session { host, dir };
    session.register("folder", plugin)?;
    let publish = ["publish", "--publisher=folder", "--config=folder.toml"];
    session.host(&[publish.as_slice(), &["--dry-run"]].concat())?;
    if session.file("public").exists() {
        return Err(String::from("the dry run wrote the target folder"));
    }
    session.host(&publish)?;
    same_page(&session)?;
    session.host(&publish)?;
    write_page(dir, "<p>two</p>\n")?;
    session.host(&publish)?;
    same_page(&session)?;
    session.host(&["plugin", "unregister", "folder"])?;
    Ok(())
}

fn same_page(session: &Session<'_>) -> Step {
    let site = fs::read(session.file("site/index.html")).map_err(|e| e.to_string())?;
    let public = fs::read(session.file("public/index.html")).unwrap_or_default();
    if site != public {
        return Err(String::from("public/index.html differs from the published site"));
    }
    Ok(())
}

/// guidedog-pages offline: its own init writes pages.toml for the site, the host
/// registers and inspects it, and a dry run plans until the publisher stops safely,
/// because no binding authorizes the target on this machine.
pub(super) fn pages_publisher(host: &Path, pages: &Path, dir: &Path) -> Step {
    prepare(dir, "<p>one</p>\n")?;
    let session = Session { host, dir };
    let init = ["init", "--repository=octo/manual", "--site=site", "--color=never"];
    session.succeed(pages, &init)?;
    session.register("pages", pages)?;
    let dry_run = ["publish", "--publisher=pages", "--config=pages.toml", "--dry-run"];
    session.refuse(&dry_run, "pages.unbound")?;
    session.host(&["plugin", "unregister", "pages"])?;
    Ok(())
}

/// A host that predates build plugins refuses one at registration, as not a publisher,
/// and records nothing.
pub(super) fn build_plugin_refused(host: &Path, plugin: &Path, dir: &Path) -> Step {
    prepare(dir, "<p>one</p>\n")?;
    let session = Session { host, dir };
    let path = plugin.to_string_lossy();
    let id = "product-badge";
    session.refuse(&["plugin", "register", id, &path, "--yes"], "plugins.protocol")?;
    if session.host(&["plugin", "list"])?.contains(id) {
        return Err(String::from("the refused program was registered anyway"));
    }
    Ok(())
}

/// The new host's `plugin check` handshakes with a publisher and declines to test it as
/// a build plugin.
pub(super) fn check_declines_publisher(host: &Path, plugin: &Path, dir: &Path) -> Step {
    let session = Session { host, dir };
    session.refuse(&["plugin", "check", &plugin.to_string_lossy()], "cli.plugin-check-kind")
}

/// The first line a program prints for `--version`, or why it printed none.
pub(super) fn version_line(program: &Path) -> Step<String> {
    let mut command = Command::new(program);
    command.arg("--version");
    let out = process::capture(&mut command).map_err(|_| unable(&command))?;
    match out.stdout.lines().next() {
        Some(line) if out.success => Ok(line.trim().to_string()),
        _ => Err(format!("`{}` printed no version", short(&command))),
    }
}

fn unable(command: &Command) -> String {
    format!("could not start `{}`", short(command))
}

/// The command for a message: the program's file name and its arguments.
fn short(command: &Command) -> String {
    let program = Path::new(command.get_program());
    let mut line = program.file_name().unwrap_or_default().to_string_lossy().into_owned();
    for arg in command.get_args() {
        line.push(' ');
        line.push_str(&arg.to_string_lossy());
    }
    line
}

/// The first non-empty line of a program's messages: the title of its problem.
fn first_line(text: &str) -> &str {
    text.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or("no message")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_keep_the_title_line_and_the_program_name() {
        assert_eq!(first_line("\n  one  \ntwo\n"), "one");
        assert_eq!(first_line(""), "no message");
        let mut command = Command::new("/opt/old/guidedog");
        command.args(["plugin", "list"]);
        assert_eq!(short(&command), "guidedog plugin list");
    }
}
