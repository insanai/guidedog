//! `guidedog capabilities`: what this program supports, from the code it links.

use std::fmt::Write;

use gd_plugins::Registry;

use crate::command::CapabilitiesCommand;
use crate::console::Console;

/// Writes the capability report of this program, with `linked` plugins, as JSON or as
/// a summary, and returns the exit status: 0, or the status of a project that does not
/// load.
pub(crate) fn capabilities(
    console: &mut Console<'_>,
    c: &CapabilitiesCommand,
    linked: &Registry,
) -> i32 {
    let loaded = match &c.project {
        Some(dir) => match gd_conf::load_config(dir, &gd_conf::LoadOptions::default()) {
            Ok(loaded) => Some(loaded.config),
            Err(problem) => return console.problem(&problem),
        },
        None => None,
    };
    let report = gd_capabilities::load_capabilities(linked, loaded.as_ref());
    let mut text = String::new();
    if c.json {
        gd_capabilities::write_json(&report, &mut text);
    } else {
        summary(console, &report, &mut text);
    }
    console.print(&text);
    0
}

fn summary(console: &Console<'_>, r: &gd_capabilities::Capabilities, out: &mut String) {
    let _ = write!(out, "\n  {}\n\n", console.section("DOCUMENT FORMATS"));
    for f in &r.formats {
        let reader = if f.reader.is_empty() { "-" } else { f.reader };
        let ops = if f.operations.is_empty() { "-".to_string() } else { f.operations.join(", ") };
        let _ = writeln!(out, "  {:<10} {:<8} {:<16} {ops}", f.name, reader, f.suffixes.join(" "));
    }
    let _ = write!(out, "\n  {}\n\n", console.section("BUILDERS"));
    for b in &r.builders {
        let preview = if b.preview { "preview" } else { "" };
        let _ = writeln!(out, "  {:<12} {:<6} {preview}", b.name, b.format);
    }
    let enabled = r.directives.iter().filter(|d| d.enabled).count();
    let roles = r.roles.iter().filter(|d| d.enabled).count();
    let _ = write!(out, "\n  {}\n\n", console.section("MARKUP"));
    let _ = writeln!(out, "  {enabled} of {} directives enabled", r.directives.len());
    let _ = writeln!(out, "  {roles} of {} roles enabled", r.roles.len());
    let on: Vec<&str> =
        r.markdown_extensions.iter().filter(|e| e.enabled).map(|e| e.name).collect();
    let _ = writeln!(out, "  Markdown extensions enabled: {}", on.join(", "));
    let _ = write!(out, "\n  {}\n\n", console.section("PLUGINS"));
    for p in &r.plugins {
        let _ = writeln!(out, "  {:<10} {:<10} {}", p.id, p.kind, p.commands.join(", "));
    }
    out.push_str("\n  guidedog capabilities --json writes every entry with its details.\n\n");
}
