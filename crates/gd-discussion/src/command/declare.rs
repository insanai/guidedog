//! The `guidedog dsc` commands as the plugin declares them: names, summaries, help, and
//! options, for guidedog's help, parsing, and suggestions.

use gd_plugins::{CommandInfo, CommandOption};

/// One command: its word, summary, help, and the options it takes besides `--dir` and
/// `--prefix`, as (name, value placeholder or "", summary).
struct Declared {
    word: &'static str,
    summary: &'static str,
    help: &'static str,
    options: &'static [(&'static str, &'static str, &'static str)],
}

const DRY_RUN: (&str, &str, &str) = ("--dry-run", "", "Preview the change; write nothing");
const FORMAT: (&str, &str, &str) = ("--format", "table|json", "Output format (default table)");
const REASON: (&str, &str, &str) = ("--reason", "TEXT", "Why, kept in the record");
const EVIDENCE: (&str, &str, &str) =
    ("--evidence", "TEXT", "Evidence, required where the state asks for it");
const TYPST: (&str, &str, &str) =
    ("--typst", "PATH", "Typst compiler of an older Typst catalog (default embedded)");

const DECLARED: [Declared; 11] = [
    Declared {
        word: "init",
        summary: "Create a discussion catalog in DIR",
        help: "guidedog dsc init DIR --prefix=NAME [options]\n\n\
               Creates a catalog: discussion.toml, a Guidedog project (conf.toml), the\n\
               process record as a draft, a record template, and the index. --config\n\
               starts from another catalog's discussion.toml (its prefix, workflow,\n\
               numbering, and labels); --process and --template replace the process\n\
               draft and the record template.",
        options: &[
            ("--name", "TEXT", "The catalog's name (default PREFIX Discussions)"),
            ("--author", "NAME", "The default author (default Git user.name)"),
            ("--labels", "a,b", "The labels records may carry"),
            ("--config", "FILE", "Start from this discussion.toml"),
            ("--process", "FILE", "The process record's text"),
            ("--template", "FILE", "The record template's text"),
            DRY_RUN,
            FORMAT,
        ],
    },
    Declared {
        word: "new",
        summary: "Create a draft record from the catalog's template",
        help: "guidedog dsc new \"Title\" [options]\n\n\
               Writes records/XXXXX-SLUG.rst in the draft state of the catalog's\n\
               workflow, and refreshes the index.",
        options: &[
            ("--author", "NAME", "The author (default the catalog's, then Git's)"),
            ("--slug", "NAME", "The file name's slug (default from the title)"),
            ("--labels", "a,b", "The record's labels (default documentation)"),
            DRY_RUN,
            FORMAT,
        ],
    },
    Declared {
        word: "list",
        summary: "List the catalog's records",
        help: "Lists the records, active ones first by number.",
        options: &[
            ("--state", "NAME", "Only records in this state"),
            ("--label", "NAME", "Only records with this label"),
            ("--include-historical", "", "Include inherited historical records"),
            FORMAT,
        ],
    },
    Declared {
        word: "show",
        summary: "Show one record's metadata",
        help: "guidedog dsc show ID [options]\n\n\
               ID is a number, a slug, a file name, or the full title.",
        options: &[FORMAT],
    },
    Declared {
        word: "promote",
        summary: "Number a draft and open it for discussion",
        help: "guidedog dsc promote SLUG [options]\n\n\
               Gives the draft the next number (or --number), renames its file, moves\n\
               it to the workflow's open state, and refreshes the index.",
        options: &[
            ("--number", "N", "The number to give (default the next free one)"),
            ("--discussion", "URL", "Where the discussion takes place"),
            REASON,
            EVIDENCE,
            DRY_RUN,
            FORMAT,
        ],
    },
    Declared {
        word: "state",
        summary: "Record a lifecycle decision",
        help: "guidedog dsc state ID --to=STATE --reason=TEXT [options]\n\n\
               Moves the record to one of its state's next states, as the catalog's\n\
               workflow allows, and records the reason.",
        options: &[("--to", "STATE", "The new state"), REASON, EVIDENCE, DRY_RUN, FORMAT],
    },
    Declared {
        word: "index",
        summary: "Refresh the generated index",
        help: "Writes the generated index of records; --check only checks it.",
        options: &[("--check", "", "Check the index; write nothing"), DRY_RUN, FORMAT],
    },
    Declared {
        word: "check",
        summary: "Check the records and the index",
        help: "guidedog dsc check [--render] [options]\n\n\
               Checks every record's metadata and required sections, and that the\n\
               index is current. --render also builds the catalog.",
        options: &[("--render", "", "Also build the catalog"), TYPST],
    },
    Declared {
        word: "build",
        summary: "Build the catalog's website and PDF books",
        help: "guidedog dsc build [ID] [options]\n\n\
               Checks the catalog, then builds it with guidedog, warnings as errors:\n\
               the website in _build/html and a PDF book per record in _build/pdf.\n\
               A record (ID, --record, or --number) builds its PDF alone, in\n\
               _build/selected. An older Typst catalog compiles each record with Typst\n\
               into the build folder beside the catalog.",
        options: &[
            ("--target", "pdf|html|both", "What to build (default both)"),
            ("--record", "ID", "Build this record's PDF only"),
            ("--number", "N", "Build this numbered record's PDF only"),
            TYPST,
        ],
    },
    Declared {
        word: "recover",
        summary: "Roll back an interrupted change",
        help: "guidedog dsc recover --rollback [options]\n\n\
               Restores the files an interrupted change left, from the catalog's\n\
               journal. --dry-run checks whether there is anything to restore.",
        options: &[("--rollback", "", "Restore the original files"), DRY_RUN, FORMAT],
    },
    Declared {
        word: "config",
        summary: "Print the catalog's configuration as discussion.toml",
        help: "guidedog dsc config [options]\n\n\
               Prints the catalog's prefix, name, workflow, numbering, and labels.",
        options: &[],
    },
];

/// The commands the discussion plugin adds: `dsc init`, `dsc new`, and the rest.
pub(crate) fn commands() -> Vec<CommandInfo> {
    DECLARED.iter().map(declare).collect()
}

fn declare(d: &Declared) -> CommandInfo {
    let mut info = CommandInfo::new(&format!("dsc {}", d.word), d.summary).with_help(d.help);
    let catalog = [
        ("--dir", "DIR", "The catalog (default the one this folder is in)"),
        ("--prefix", "NAME", "The catalog's prefix, for one without discussion.toml"),
    ];
    for (name, value, summary) in catalog.iter().chain(d.options) {
        info = info.with_option(if value.is_empty() {
            CommandOption::switch(name, summary)
        } else {
            CommandOption::with_value(name, value, summary)
        });
    }
    info
}
