//! The help texts: the overview and each command's options, and the help of the
//! commands plugins add, made from what they declare.

use gd_report::ansi::{BOLD_CYAN, styled};
use gd_report::escape_terminal;

use crate::Extension;
use crate::version::full_version;

/// The first lines of every help text, styled when `color`.
fn banner(color: bool) -> String {
    format!(
        "\n  {}  {}\n  Thoughtful documentation, from source to publication.\n\n",
        styled("guidedog", BOLD_CYAN, color),
        full_version()
    )
}

/// The help of `topic` (`None` for the overview), styled when `color`. The overview
/// and `publish` list the commands plugins add (`added`).
pub(crate) fn help_text(topic: Option<&str>, color: bool, added: &[Extension]) -> String {
    let mut text = banner(color);
    text.push_str(match topic {
        None => OVERVIEW,
        Some("convert") => CONVERT,
        Some("formats") => FORMATS,
        Some("quickstart") => QUICKSTART,
        Some("migrate") => MIGRATE,
        Some("intl") => INTL,
        Some("plugin") => PLUGIN,
        Some("publish") => PUBLISH,
        Some(_) => BUILD,
    });
    match topic {
        None => {
            text.push_str(&extension_list("PLUGIN COMMANDS", added.iter()));
            text.push_str(START);
        }
        Some("publish") => {
            let targets = added.iter().filter(|e| e.info.path[0] == "publish");
            text.push_str(&extension_list("TARGETS ADDED BY PLUGINS", targets));
        }
        Some(_) => {}
    }
    text.push_str(GLOBAL);
    text
}

/// A section listing `extensions`, with each one's plugin; empty when there are none.
fn extension_list<'a>(title: &str, extensions: impl Iterator<Item = &'a Extension>) -> String {
    let mut lines = String::new();
    for e in extensions {
        let line = format!("    {:<18} {} ({})\n", e.name(), e.info.summary, e.plugin);
        lines.push_str(&escape_terminal(&line));
    }
    if lines.is_empty() {
        return String::new();
    }
    format!("  {title}\n{lines}\n")
}

/// The help of a command a plugin adds, from its declaration, styled when `color`.
pub(crate) fn extension_help(e: &Extension, color: bool) -> String {
    let mut text = banner(color);
    let origin = if e.linked { "linked into this program" } else { "registered on this machine" };
    let mut body = format!(
        "  USAGE\n    guidedog {} [options]\n\n  {}\n  Added by the plugin {} ({origin}).\n\n",
        e.name(),
        e.info.summary,
        e.plugin
    );
    if !e.info.help.is_empty() {
        for line in e.info.help.lines() {
            body.push_str(&format!("  {line}\n").replace("  \n", "\n"));
        }
        body.push('\n');
    }
    if !e.info.options.is_empty() {
        body.push_str("  OPTIONS\n");
        for o in &e.info.options {
            let name =
                if o.takes_value() { format!("{}={}", o.name, o.value) } else { o.name.clone() };
            body.push_str(&format!("    {name:<28} {}\n", o.summary));
        }
        body.push('\n');
    }
    text.push_str(&escape_terminal(&body));
    text.push_str(GLOBAL);
    text
}

/// The help of `word`, a word that begins only commands plugins add: their list.
pub(crate) fn group_help(word: &str, color: bool, added: &[Extension]) -> String {
    let mut text = banner(color);
    text.push_str(&format!("  USAGE\n    guidedog {word} COMMAND [options]\n\n"));
    let group = added.iter().filter(|e| e.info.path[0] == word);
    text.push_str(&extension_list("COMMANDS", group));
    text.push_str(&format!("  Run guidedog {word} COMMAND --help for a command's options.\n\n"));
    text.push_str(GLOBAL);
    text
}

const OVERVIEW: &str = "  USAGE
    guidedog <command> [options]

  PROJECTS
    quickstart  Create a documentation project: conf.toml, index.rst
    build       Build it: html, dirhtml, singlehtml, pdf, text, gettext
    serve       Build the website, preview it, and rebuild on change
    clean       Remove the build folder's contents
    migrate     Convert a Sphinx conf.py to conf.toml
    intl        Update translation catalogs, or count what is translated

  PUBLISHING
    publish     Build and publish with a publisher, or a target a plugin adds
    plugin      Register, list, inspect, or run plugin programs

  FILES
    convert     Convert RST, Markdown, or Typst to HTML, PDF, or RST
    formats     Show readers, renderers, and the Typst backend
    gds         Manage and build Guidedog's own discussions (as dsc does)

";

const START: &str = "  START HERE
    guidedog quickstart docs
    guidedog build html docs
    guidedog build --help

";

const GLOBAL: &str = "  GLOBAL OPTIONS
    --color=auto|always|never    Terminal styling; respects NO_COLOR
    --diagnostics=text|json      Explained reports or JSON lines
    -q, --quiet                  Suppress successful status messages
    -h, --help                   Show help
    --version                    Show the versions of Guidedog and its components
    --licenses                   Show Guidedog's licence and library notices

";

const CONVERT: &str = "  USAGE
    guidedog convert INPUT --output FILE [options]

  EXAMPLES
    guidedog convert guide.rst --output guide.html
    guidedog convert guide.md --to pdf --output guide.pdf
    guidedog convert guide.typ --output guide.pdf
    guidedog convert - --from commonmark --to html --output -
    guidedog convert page.md --from myst --output page.rst

  OPTIONS
    --from=rst|sphinx|commonmark|myst|typst
                                  Input format; the suffix decides by default
    --to=html|pdf|rst             Output format; the output suffix decides
    -o, --output=FILE|-           Destination, or - for standard output
    --emit-typst=FILE             Also write the generated Typst source
    --force                       Replace an existing output file
    --raw=refuse|omit|allow       Raw HTML/Typst content (default: refuse)
    --strict                      Treat warnings as failures
    --title=TEXT --lang=TAG       Document title and language
    --number-sections             Number section headings
    --fragment                    Write the body only, not a complete document
    --typst=PATH                  Typst program (default: the embedded compiler)
    --font-path=DIR               Extra font directory; repeat for more
    --embedded-fonts-only         Ignore system fonts, for reproducible PDFs
    --offline                     Use only Typst packages already on disk
    --package-cache=DIR           Typst package cache directory
    --budget=MIB                  Memory budget for conversion (default 256)
    --max-depth=N --max-nodes=N   Nesting and node limits
    --stack-kib=N                 Stack for nested content (default 512,
                                  at most 6144); raise it with --max-depth

  EXIT STATUS
    0 ok, 1 invalid or refused content, 2 usage, 3 limit, 4 file or Typst,
    5 internal error

";

const FORMATS: &str = "  USAGE
    guidedog formats [--format=table|json] [--typst=PATH]

  Lists the readers and renderers built in, with their versions and conformance,
  and the Typst backend this program finds.

";

const QUICKSTART: &str = "  USAGE
    guidedog quickstart [DIR] [options]

  Creates conf.toml, index.rst, _static/, _templates/, and a build folder,
  as sphinx-quickstart does. Asks questions unless -q is given.

  OPTIONS
    -q                 Quiet: take answers from the options below
    -p, --project NAME Project name
    -a, --author NAME  Author name(s)
    -v VERSION         Short version;  -r RELEASE  full release
    -l LANGUAGE        Document language (default en)
    --sep, --no-sep    Separate source/ and build/ folders, or not
    --suffix .rst      Source file suffix

";

const MIGRATE: &str = "  USAGE
    guidedog migrate [conf.py|FOLDER] [-o FILE|-] [--force]

  Converts a Sphinx conf.py (or an old guidedog.toml) to conf.toml without
  running Python. Literal settings carry over; anything Python would compute
  is reported and kept in conf.toml as a comment.

  OPTIONS
    -o FILE            Write here instead of beside conf.py; - prints it
    --force            Replace an existing conf.toml

";

const INTL: &str = "  USAGE
    guidedog intl update [-p POTDIR] -l LANG [-l LANG] [PROJECT]
    guidedog intl stat [-l LANG] [PROJECT]

  Keeps translation catalogs in step with the documents, as sphinx-intl does.
  First write the message templates with guidedog build gettext; update then
  creates locales/LANG/LC_MESSAGES/*.po, or merges new templates into them:
  translations stay, changed messages turn fuzzy, removed ones obsolete.
  Build with -D language=LANG, or language in conf.toml, to use them.

  OPTIONS
    -p, --pot-dir DIR     The templates (default _build/gettext)
    -l, --language LANG   A language; repeat it for several
    -d, --locale-dir DIR  The catalogs (default: the first of locale_dirs)
    -w, --line-width N    Wrap lines at N characters (default 76)
    --no-obsolete         Drop messages the templates no longer have
    -c DIR, -D name=value Where conf.toml is; override a setting

";

const BUILD: &str = "  USAGE
    guidedog build [BUILDER] [PROJECT] [options]
    guidedog build -b BUILDER SOURCEDIR OUTPUTDIR   (sphinx-build form)
    guidedog build -M BUILDER SOURCEDIR BUILDDIR    (sphinx-build make mode)
    guidedog clean [PROJECT]     guidedog serve [PROJECT] [--port 8000]

  BUILDERS
    html dirhtml singlehtml pdf text gettext dummy   (default: html)

  OPTIONS
    -a                 Write all outputs, not only changed ones
    -E                 Fresh build: ignore the saved environment
    -W                 Treat warnings as errors
    --keep-going       With -W, finish the build before failing
    --untrusted        Leave out raw content, fetch nothing, run no build plugin
    -n                 Nitpicky: warn about every unresolved reference
    -j N, -j auto      Read documents on N threads, or one per core
    --budget=MIB       Memory the build may hold at once (default 1024)
    --memory=ram       At the budget, stop without asking (a terminal asks)
    --max-depth=N      How deep content may nest (default 256)
    --max-nodes=N      Nodes per document (default 1000000)
    --stack-kib=N      Stack for nested content (default 512; at most what
                       the reading threads hold, 6144)
    -D name=value      Override a conf.toml setting
    -t TAG             Define a tag for only directives
    -c DIR, -C         Use conf.toml from DIR, or none
    --typst=PATH       The Typst program for PDF
    -v                 Also show notes (Docutils' info messages)
    -q                 Only warnings and errors
    --port PORT        serve: the port (default 8000)

";

const PLUGIN: &str = "  USAGE
    guidedog plugin COMMAND

  COMMANDS
    register ID PATH [--yes]   inspect a plugin program and pin its SHA-256
    check PATH                 test a build plugin without changing the registry
    list                       the programs registered here, and the linked plugins
    inspect ID                 an entry, and whether the program on disk is still it
    run ID -- ARGS             run a registered program's own commands, such as
                               guidedog plugin run pages -- auth login
    unregister ID              forget a program; it removes nothing else

  Registered programs are recorded in plugins.toml in Guidedog's configuration
  folder ($GUIDEDOG_CONFIG_DIR moves it). conf.toml's plugins list selects build
  plugins. Only guidedog publish invokes a publisher. --untrusted disables build
  plugins. A plugin may add commands to guidedog's command line: guidedog --help
  lists them, and plugin inspect shows a plugin's. Build-plugin registration
  runs checks before writing the entry: pass --project=DIR and --config=FILE
  when the plugin needs project settings. Checks exercise declared hooks,
  repeatability and cleanup in child processes; --timeout=SECONDS defaults to
  60. Passing checks is interoperability evidence, not a sandbox or proof of
  safe native code.

";

const PUBLISH: &str = "  USAGE
    guidedog publish --publisher=ID --config=FILE [--dry-run] [--wait=SECONDS]
                     [--timeout=SECONDS] [--verbose]
    guidedog publish TARGET [options]     (a target a plugin adds)

  Builds the project the publisher's configuration names, as HTML, freezes the
  site, and has the publisher plan and publish it. The publisher is looked up
  among the plugins linked into this program first, then in the machine's
  registry. --dry-run builds and plans, and writes nothing to the
  target. --timeout bounds each whole request to the publisher (default 300
  seconds); use a value longer than --wait. Ctrl+C stops the publisher; recover
  before retrying an unknown write.

  EXIT STATUS
    0 published or unchanged, 1 failed (nothing published), 6 published but not
    confirmed live, 7 unknown (run the publisher's recover)

";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_topic_has_its_usage_and_the_global_options() {
        for topic in [None, Some("convert"), Some("build"), Some("intl"), Some("plugin")] {
            let text = help_text(topic, false, &[]);
            assert!(text.contains("USAGE") && text.contains("GLOBAL OPTIONS"), "{text}");
            assert!(!text.contains('\x1b'));
        }
        assert!(help_text(None, true, &[]).contains('\x1b'));
        assert!(!help_text(Some("build"), false, &[]).contains("--memory=disk"));
    }
}
