//! Command-line parsing in dot's spelling: flags and their values joined (`-Tsvg`,
//! `-Kneato`, `-Gsize=4,4`), except `-o`, which also takes its file as the next argument.

use gd_graphdog::{Attribute, AttributeKind, Engine, Format};

/// What the command line asks for.
#[derive(Debug, Default)]
pub struct Command {
    pub engine: Engine,
    pub format: Format,
    pub attributes: Vec<Attribute>,
    pub inputs: Vec<String>,
    pub output: String,
    pub quiet: bool,
    pub help: bool,
    pub version: bool,
}

/// A usage problem: title, message and hint for an Elm-style report.
#[derive(Debug)]
pub struct Usage {
    pub title: &'static str,
    pub message: String,
    pub hint: String,
}

fn usage(title: &'static str, message: String, hint: impl Into<String>) -> Usage {
    Usage { title, message, hint: hint.into() }
}

/// Parses the arguments after the program name.
pub fn parse_args(args: &[String]) -> Result<Command, Usage> {
    let mut cmd = Command::default();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "-h" | "--help" | "-?" => cmd.help = true,
            "-V" | "--version" => cmd.version = true,
            "-q" => cmd.quiet = true,
            "-o" => {
                i += 1;
                let Some(file) = args.get(i) else {
                    let message = "-o needs a file name.".to_string();
                    return Err(usage("MISSING OUTPUT FILE", message, "Write -o graph.svg."));
                };
                cmd.output.clone_from(file);
            }
            _ if arg == "-" || !arg.starts_with('-') => cmd.inputs.push(arg.to_string()),
            _ if arg.starts_with("-o") => cmd.output = arg[2..].to_string(),
            _ if arg.starts_with("-T") => cmd.format = parse_format_flag(&arg[2..])?,
            _ if arg.starts_with("-K") => cmd.engine = parse_engine_flag(&arg[2..])?,
            _ if arg.starts_with("-G") || arg.starts_with("-N") || arg.starts_with("-E") => {
                cmd.attributes.push(parse_attribute_flag(arg)?);
            }
            _ => {
                return Err(usage(
                    "UNKNOWN OPTION",
                    format!("graphdog does not know the option {arg}."),
                    "Run graphdog --help to see the options.",
                ));
            }
        }
        i += 1;
    }
    Ok(cmd)
}

/// The value of -T. dot also allows -Tsvg:cairo, naming a renderer; graphdog accepts
/// only the plain format.
fn parse_format_flag(value: &str) -> Result<Format, Usage> {
    gd_graphdog::parse_format(value).ok_or_else(|| {
        usage(
            "UNKNOWN FORMAT",
            format!("graphdog cannot write the format {value:?}."),
            "Use -Tsvg (the default), -Tpng, or -Tdot for DOT with layout positions.",
        )
    })
}

fn parse_engine_flag(value: &str) -> Result<Engine, Usage> {
    gd_graphdog::parse_engine(value).ok_or_else(|| {
        usage(
            "UNKNOWN ENGINE",
            format!("graphdog has no layout engine named {value:?}."),
            "Use one of: dot, neato, fdp, sfdp, circo, twopi, osage, patchwork.",
        )
    })
}

/// -Gname=value, -Nname=value or -Ename=value. As in dot, a name without a value means
/// "true" (-Gsplines is -Gsplines=true).
fn parse_attribute_flag(arg: &str) -> Result<Attribute, Usage> {
    let kind = match arg.as_bytes()[1] {
        b'G' => AttributeKind::Graph,
        b'N' => AttributeKind::Node,
        _ => AttributeKind::Edge,
    };
    let rest = &arg[2..];
    let (name, value) = rest.split_once('=').unwrap_or((rest, "true"));
    if name.is_empty() {
        return Err(usage(
            "MISSING ATTRIBUTE NAME",
            format!("{arg} names no attribute."),
            format!("Write the attribute and its value: {}shape=box.", &arg[..2]),
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
        let cmd = parse_args(&args(&["-Tdot", "-Kneato", "-Gsplines", "-Nshape=box", "-o",
            "out.dot", "a.dot", "-"]))
        .expect("parses");
        assert_eq!(cmd.format, Format::Dot);
        assert_eq!(cmd.engine, Engine::Neato);
        assert_eq!(cmd.attributes[0].value, "true");
        assert_eq!(cmd.attributes[1].kind, AttributeKind::Node);
        assert_eq!(cmd.output, "out.dot");
        assert_eq!(cmd.inputs, ["a.dot", "-"]);
    }

    #[test]
    fn bad_flags_are_usage_errors() {
        assert_eq!(parse_args(&args(&["-Tjpeg"])).expect_err("bad").title, "UNKNOWN FORMAT");
        assert_eq!(parse_args(&args(&["-Kfoo"])).expect_err("bad").title, "UNKNOWN ENGINE");
        assert_eq!(parse_args(&args(&["-o"])).expect_err("bad").title, "MISSING OUTPUT FILE");
        assert_eq!(parse_args(&args(&["-N=x"])).expect_err("bad").title,
            "MISSING ATTRIBUTE NAME");
        assert_eq!(parse_args(&args(&["-x"])).expect_err("bad").title, "UNKNOWN OPTION");
    }
}
