//! Command-line parsing in dot's spelling: flags and their values joined (`-Tsvg`,
//! `-Kneato`, `-Gsize=4,4`), except `-o`, which also takes its file as the next argument.

use gd_graphdog::{Attribute, AttributeKind, Engine, Format};
use gd_report::Report;

/// What the command line asks for.
#[derive(Debug, Default)]
pub struct Command {
    pub engine: Engine,
    pub format: Format,
    pub attributes: Vec<Attribute>,
    /// The files to read; `-` is standard input, and none means standard input.
    pub inputs: Vec<String>,
    /// The file to write; `None` (or `-`) is standard output.
    pub output: Option<String>,
    pub quiet: bool,
    pub help: bool,
    pub version: bool,
}

/// A usage problem as an Elm-style report, placed at "command line" (boxed: reports are
/// large and rare).
fn usage(code: &str, title: &str, message: &str, hint: &str) -> Box<Report> {
    let mut report = Report::error(code, title, message).hint(hint);
    report.path = "command line".to_string();
    Box::new(report)
}

/// Parses the arguments after the program name; a usage problem is a [`Report`].
pub fn parse_args(args: &[String]) -> Result<Command, Box<Report>> {
    let mut cmd = Command::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let arg = arg.as_str();
        match arg {
            "-h" | "--help" | "-?" => cmd.help = true,
            "-V" | "--version" => cmd.version = true,
            "-q" => cmd.quiet = true,
            "-o" => {
                let Some(file) = args.next() else {
                    return Err(usage(
                        "graphdog.missing-output",
                        "MISSING OUTPUT FILE",
                        "-o needs a file name.",
                        "Write -o graph.svg.",
                    ));
                };
                cmd.output = Some(file.clone());
            }
            _ if arg == "-" || !arg.starts_with('-') => cmd.inputs.push(arg.to_string()),
            _ if arg.starts_with("-o") => cmd.output = Some(arg[2..].to_string()),
            _ if arg.starts_with("-T") => cmd.format = format_flag(&arg[2..])?,
            _ if arg.starts_with("-K") => cmd.engine = engine_flag(&arg[2..])?,
            _ if arg.starts_with("-G") || arg.starts_with("-N") || arg.starts_with("-E") => {
                cmd.attributes.push(attribute_flag(arg)?);
            }
            _ => {
                return Err(usage(
                    "graphdog.unknown-option",
                    "UNKNOWN OPTION",
                    &format!("graphdog does not know the option {arg}."),
                    "Run graphdog --help to see the options.",
                ));
            }
        }
    }
    Ok(cmd)
}

/// The value of -T. dot also allows -Tsvg:cairo, naming a renderer; graphdog accepts
/// only the plain format.
fn format_flag(value: &str) -> Result<Format, Box<Report>> {
    Format::from_name(value).ok_or_else(|| {
        usage(
            "graphdog.unknown-format",
            "UNKNOWN FORMAT",
            &format!("graphdog cannot write the format {value:?}."),
            "Use -Tsvg (the default), -Tpng, or -Tdot for DOT with layout positions.",
        )
    })
}

fn engine_flag(value: &str) -> Result<Engine, Box<Report>> {
    Engine::from_name(value).ok_or_else(|| {
        let names: Vec<&str> = Engine::ALL.iter().map(|e| e.name()).collect();
        let mut report = usage(
            "graphdog.unknown-engine",
            "UNKNOWN ENGINE",
            &format!("graphdog has no layout engine named {value:?}."),
            &format!("Use one of: {}.", names.join(", ")),
        );
        if let Some(near) = gd_report::suggest(value, names.iter().copied()) {
            report.hints.push(format!("Did you mean `{near}`?"));
        }
        report
    })
}

/// -Gname=value, -Nname=value or -Ename=value. As in dot, a name without a value means
/// "true" (-Gsplines is -Gsplines=true).
fn attribute_flag(arg: &str) -> Result<Attribute, Box<Report>> {
    let (flag, rest) = arg.split_at(2);
    let kind = match flag {
        "-G" => AttributeKind::Graph,
        "-N" => AttributeKind::Node,
        _ => AttributeKind::Edge,
    };
    let (name, value) = rest.split_once('=').unwrap_or((rest, "true"));
    if name.is_empty() {
        return Err(usage(
            "graphdog.missing-attribute",
            "MISSING ATTRIBUTE NAME",
            &format!("{arg} names no attribute."),
            &format!("Write the attribute and its value: {flag}shape=box."),
        ));
    }
    Ok(Attribute { kind, name: name.to_string(), value: value.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn flags_parse_like_dot() {
        let cmd = parse_args(&args(&[
            "-Tdot",
            "-Kneato",
            "-Gsplines",
            "-Nshape=box",
            "-o",
            "out.dot",
            "a.dot",
            "-",
        ]))
        .expect("parses");
        assert_eq!(cmd.format, Format::Dot);
        assert_eq!(cmd.engine, Engine::Neato);
        assert_eq!(cmd.attributes[0].value, "true");
        assert_eq!(cmd.attributes[1].kind, AttributeKind::Node);
        assert_eq!(cmd.output.as_deref(), Some("out.dot"));
        assert_eq!(cmd.inputs, ["a.dot", "-"]);
    }

    #[test]
    fn bad_flags_are_usage_errors() {
        assert_eq!(parse_args(&args(&["-Tjpeg"])).expect_err("bad").title, "UNKNOWN FORMAT");
        assert_eq!(parse_args(&args(&["-Kfoo"])).expect_err("bad").title, "UNKNOWN ENGINE");
        assert_eq!(parse_args(&args(&["-o"])).expect_err("bad").title, "MISSING OUTPUT FILE");
        assert_eq!(parse_args(&args(&["-N=x"])).expect_err("bad").title, "MISSING ATTRIBUTE NAME");
        assert_eq!(parse_args(&args(&["-x"])).expect_err("bad").title, "UNKNOWN OPTION");
        let typo = parse_args(&args(&["-Kneat"])).expect_err("bad");
        assert!(typo.hints.iter().any(|h| h == "Did you mean `neato`?"), "{typo:?}");
    }
}
