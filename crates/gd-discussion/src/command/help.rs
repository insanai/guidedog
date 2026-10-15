//! `--help`.

use super::{Program, VERSION};
use std::fmt::Write;

/// The help text of `program`, styled when `color` is set.
pub(crate) fn help_text(program: &Program<'_>, color: bool) -> String {
    let mut out = String::with_capacity(4096);
    let name = if color { format!("\x1b[1;36m{}\x1b[0m", program.name) } else {
        program.name.to_string()
    };
    let _ = writeln!(out, "\n  {name}  {VERSION}");
    out.push_str("  Proposals and decisions, written down and kept in order.\n\n");
    let _ = writeln!(out, "  USAGE\n    {} <command> [title or record] [options]\n", program.name);
    if program.prefix.is_none() {
        out.push_str(START);
    }
    out.push_str(COMMANDS);
    out.push_str("  OPTIONS\n");
    match &program.dir {
        None => out.push_str(
            "    --dir=DIR             The catalog (default: the one you are in)\n    \
             --prefix=NAME         The catalog's prefix, for one without a configuration\n"),
        Some(dir) => {
            let _ = writeln!(out, "    --dir=DIR             Discussion root (default: {})",
                dir.display());
        }
    }
    out.push_str(OPTIONS);
    if program.tool == "guidedog" {
        out.push_str("    --guidedog=PATH       Guidedog program (default: $DSC_GUIDEDOG,\n");
        out.push_str("                          then guidedog on PATH)\n");
    } else {
        let _ = writeln!(out, "    --{}=PATH          The builder's program, when it needs one",
            program.tool);
    }
    out.push_str(GLOBAL);
    out
}

const START: &str = "  START
    init DIR      Create a catalog: --prefix=eds --name=\"Enso Discussions\"
                  --config=FILE, --process=FILE and --template=FILE choose
                  its workflow, process draft and record template

";

const COMMANDS: &str = "  WRITE & REVIEW
    new \"Title\"   Create a placeholder draft from the local template
    list          Browse records; filter with --state and --label
    show ID       Inspect a number, slug, filename, or full title
    promote SLUG  Assign a number and open the draft for discussion
    state ID      Record a lifecycle decision with --to and --reason

  MAINTAIN & BUILD
    index         Refresh generated indexes; --check is read-only
    check         Validate records and index; --render also builds artifacts
    build [ID]    Publish the catalog into its _build directory
                  HTML is a complete site; a record selects one PDF
    recover       Roll back an interrupted update with --rollback
    config        Print the workflow, numbering, and labels as discussion.toml

";

const OPTIONS: &str = "    --author=NAME         Draft author; defaults to Git user.name
    --slug=NAME           Explicit lowercase draft slug
    --labels=a,b          Draft labels (default: documentation)
    --number=N            Promotion number, or build a numbered record
    --discussion=URL      Link to the review when promoting
    --reason=TEXT         Explain a state change or promotion
    --evidence=TEXT       Required when the destination state asks for it
    --dry-run             Preview a change without writing files
    --include-historical  Include inherited records in listings
    --target=pdf|html|both  Output targets (default: both)
    --format=table|json   List/show output format
";

const GLOBAL: &str = "
  GLOBAL OPTIONS
    --color=auto|always|never    Terminal styling; respects NO_COLOR
    --diagnostics=text|json      Explained reports or JSON lines
    -q, --quiet                  Suppress successful status messages
    -h, --help                   Show help
    --version                    Show the version

";
