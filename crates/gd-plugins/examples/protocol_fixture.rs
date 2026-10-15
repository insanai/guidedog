//! A deliberately faulty plugin program for the crate's process-boundary tests. Its
//! file name selects what it is: a name holding `silent` never sends a handshake, one
//! holding `build` is a build plugin that fails as its configuration file says, and
//! any other is a publisher that fails as its site folder's name says.

use std::borrow::Cow;
use std::io::{BufRead, Write};
use std::process::ExitCode;
use std::time::Duration;

use gd_plugins::{BuildPlugin, Hooks, LoadContext, PluginInfo, Problem, ProblemData, SourceInput};

fn stall() -> ! {
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn main() -> ExitCode {
    let name = std::env::args().next().unwrap_or_default();
    let name = std::path::Path::new(&name).file_name().unwrap_or_default().to_string_lossy();
    if name.contains("silent") {
        stall();
    }
    if name.contains("build") {
        return gd_plugins::serve(Broken::default());
    }
    serve_faulty_publisher();
    ExitCode::SUCCESS
}

/// A build plugin that fails in its source hook as its configuration says: `exit`,
/// `bounds` (a panic), `stall`, `bad-problem`, `markup`, or `ok`.
#[derive(Clone, Default)]
struct Broken {
    mode: String,
}

impl BuildPlugin for Broken {
    fn info(&self) -> PluginInfo {
        PluginInfo::new(
            "broken-build",
            "Fault fixture",
            "1",
            Hooks { source: true, ..Hooks::NONE },
        )
    }

    fn load(&mut self, ctx: &mut LoadContext<'_>) -> Result<(), Problem> {
        self.mode = ctx.read_config()?.trim().to_string();
        Ok(())
    }

    fn process_source<'a>(&mut self, input: &SourceInput<'a>) -> Result<Cow<'a, str>, Problem> {
        match self.mode.as_str() {
            "exit" => std::process::exit(37),
            "bounds" => {
                let bytes = [0u8; 1];
                let index = std::env::args().count();
                Ok(Cow::Owned(bytes[index].to_string()))
            }
            "stall" => stall(),
            "bad-problem" => Err(ProblemData { exit: 2, ..ProblemData::default() }.into()),
            "markup" => Ok(Cow::Borrowed("Title\n=====\n\n.. not-a-directive::\n")),
            _ => Ok(Cow::Borrowed(input.text)),
        }
    }
}

const HANDSHAKE: &str = concat!(
    r#"{"protocol":"guidedog-publisher","versions":[{"major":1,"minor":0}],"#,
    r#""api":{"major":1,"minor":1},"id":"fixture","name":"Fixture","version":"1","#,
    r#""capabilities":[],"snapshot_schemas":[1],"hooks":[]}"#
);

/// A publisher speaking the protocol by hand, failing as its site folder's name says:
/// `malformed` and `refused` answer load wrongly, `write` writes then stalls in
/// publish, and `stall-unload` stalls when unloaded.
fn serve_faulty_publisher() {
    let mut out = std::io::stdout().lock();
    let mut send = |line: String| {
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    };
    send(HANDSHAKE.to_string());
    let (mut mode, mut site) = (String::new(), String::new());
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { return };
        let Ok(request) = gd_json::parse(&line) else { return };
        let id = request.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let field =
            |key: &str| request.get(key).and_then(|v| v.as_str()).unwrap_or_default().to_string();
        match field("operation").as_str() {
            "load" => {
                site = field("config");
                mode = site.rsplit('/').next().unwrap_or_default().to_string();
                let mut quoted = String::new();
                gd_json::write_string(&mut quoted, &site);
                send(match mode.as_str() {
                    "malformed" => format!(r#"{{"id":{id}}}"#),
                    "refused" => format!(r#"{{"id":{id},"ok":false}}"#),
                    _ => {
                        let source = format!(r#"{{"site":{quoted}}}"#);
                        format!(r#"{{"id":{id},"ok":true,"source":{source},"target":"fixture"}}"#)
                    }
                });
                continue;
            }
            "plan" => {
                let digest = request
                    .get("snapshot")
                    .and_then(|s| s.get("digest"))
                    .and_then(|d| d.as_str())
                    .unwrap_or_default();
                let plan = format!(
                    r#"{{"id":"plan-1","generation":"{digest}","target":"fixture",{}"#,
                    r#""unchanged":false,"changes":[]}"#
                );
                send(format!(r#"{{"id":{id},"ok":true,"plan":{plan}}}"#));
                continue;
            }
            "publish" => {
                let _ = std::fs::write(format!("{site}/written"), "a write happened");
                stall();
            }
            "unload" if mode == "stall-unload" => stall(),
            _ => {}
        }
        send(format!(r#"{{"id":{id},"ok":true}}"#));
        if field("operation") == "unload" {
            return;
        }
    }
}
