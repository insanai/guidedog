//! `guidedog convert` and `guidedog formats`: the commands that need no project.

use std::fmt::Write;
use std::path::Path;

use gd_host::{ArtifactRequest, Delivered};
use guidedoc::{Builder, Conformance, Document, Pass, PassOutcome, Registry};

use crate::command::{ConvertCommand, FormatsCommand, MathOutput};
use crate::console::Console;

/// Converts one file as `c` says, reports what happened, and returns the exit status.
pub(crate) fn convert(console: &mut Console<'_>, c: &ConvertCommand) -> i32 {
    gd_host::catch_interrupts();
    let today = gd_clock::build_time().local().date.to_string();
    let limits = conversion_limits(c);
    let mut policy = guidedoc::default_policy();
    policy.raw = c.raw;
    policy.strict = c.strict;
    policy.keep_documents_with_errors = c.keep_going;
    // Writing source again keeps the quotes and dashes as written (no smart quotes);
    // MyST is read with the Sphinx layer, so its roles and directives are understood.
    let settings = if c.to == gd_host::Target::Rst {
        guidedoc_defaults::ReaderSettings::for_source()
    } else {
        guidedoc_defaults::ReaderSettings::default()
    };
    let reader = c.from.reader().unwrap_or("rst");
    let (ui, _) = gd_i18n::ui_translator(&c.lang, &[], false);
    let labels = gd_i18n::fill_labels(&ui);
    let mathml = (c.math == MathOutput::Mathml).then(gd_mathml::MathMl::new);
    let embed = c.embed_pictures.then(|| (gd_link::Embedder::new(), embed_options(c)));
    let draw = |doc: Document<'_>, b: &mut Builder<'_>| match &embed {
        Some((embedder, options)) => embedder.embed(doc, b, options),
        None => PassOutcome::Unchanged,
    };
    let typeset = |doc: Document<'_>, b: &mut Builder<'_>| match &mathml {
        Some(math) => math.prepare(doc, b),
        None => PassOutcome::Unchanged,
    };
    let passes =
        [Pass { name: "embed-pictures", run: &draw }, Pass { name: "mathml", run: &typeset }];
    let request = ArtifactRequest {
        input: &c.input,
        output: &c.output,
        from: c.from,
        to: c.to,
        emit_typst: c.emit_typst.as_deref(),
        force: c.force,
        policy,
        limits,
        read: guidedoc::ReadOptions { today: &today, ..settings.options(reader) },
        render: guidedoc::RenderOptions {
            title: &c.title,
            labels: Some(&labels),
            language: &c.lang,
            section_numbers: c.number_sections,
            standalone: !c.fragment,
            math_writer: mathml.as_ref().map(|math| math as &dyn guidedoc::MathWriter),
            // A complete page carries the rules Typst's MathML needs; a fragment's host
            // adds them to its own style sheet.
            stylesheet: if mathml.is_some() { gd_mathml::MATHML_CSS } else { "" },
            ..guidedoc::RenderOptions::default()
        },
        passes: if embed.is_some() || mathml.is_some() { &passes } else { &[] },
        typst_program: &c.typst,
        typst_options: gd_typst::Options {
            offline: c.offline,
            package_cache: c.package_cache.clone(),
            embedded_fonts_only: c.embedded_fonts_only,
            font_paths: c.font_paths.clone(),
            ..gd_typst::Options::default()
        },
        budget: c.budget_mib.map_or(256 << 20, |mib| mib << 20),
        ..ArtifactRequest::default()
    };
    let result = gd_host::convert_artifact(&guidedoc_defaults::registry(), &request);
    report_conversion(console, result)
}

/// Prints the reports and the outcome of a conversion; returns the exit status.
fn report_conversion(console: &mut Console<'_>, result: gd_host::ArtifactResult) -> i32 {
    let consequence = if result.exit != 0 { "No output was written." } else { "" };
    for report in &result.reports {
        let error = report.severity == gd_report::Severity::Error;
        console.report(report, if error { consequence } else { "" });
    }
    if result.exit != 0 {
        if let Some(problem) = &result.problem {
            console.problem(problem);
        }
        return result.exit;
    }
    match result.delivered {
        Delivered::Stdout(bytes) => {
            let _ = console.out.write_all(&bytes);
        }
        Delivered::File(path) => console.success(&format!("Wrote {}", path.display())),
        Delivered::Nothing => {}
    }
    0
}

/// How `--embed-pictures` draws: a Typst diagram reads below the input's folder, and
/// nothing when the input is standard input; fonts and packages follow the Typst options.
fn embed_options(c: &ConvertCommand) -> gd_link::EmbedOptions {
    let defaults = gd_link::EmbedOptions::default();
    let folder = Path::new(&c.input).parent().unwrap_or(Path::new(""));
    let folder = if folder.as_os_str().is_empty() { Path::new(".") } else { folder };
    gd_link::EmbedOptions {
        root: (c.input != "-").then(|| folder.canonicalize().ok()).flatten(),
        markdown: matches!(c.from, gd_host::Format::Commonmark | gd_host::Format::Myst),
        offline: c.offline,
        embedded_fonts_only: c.embedded_fonts_only,
        font_paths: c.font_paths.clone(),
        package_cache: c.package_cache.clone(),
        max_pictures: c.max_pictures.unwrap_or(defaults.max_pictures),
        max_bytes: c.max_picture_kib.map_or(defaults.max_bytes, |kib| kib << 10),
    }
}

fn conversion_limits(c: &ConvertCommand) -> guidedoc::Limits {
    let mut limits = guidedoc::default_limits();
    if let Some(n) = c.max_depth {
        limits.max_depth = n;
    }
    if let Some(n) = c.max_nodes {
        limits.max_nodes = n;
    }
    limits.stack_bytes = match c.stack_kib {
        Some(kib) => kib << 10,
        None => gd_host::default_stack_budget(gd_host::WORKER_STACK_BYTES),
    };
    limits
}

fn conformance(c: Conformance) -> &'static str {
    match c {
        Conformance::Planned => "planned",
        Conformance::Partial => "partial",
        Conformance::Conformant => "conformant",
    }
}

/// Lists the adapters this program actually has and the Typst backend it finds.
pub(crate) fn formats(console: &mut Console<'_>, f: &FormatsCommand) -> i32 {
    let registry = guidedoc_defaults::registry();
    let backend = gd_typst::locate(&f.typst);
    let text = if f.json {
        formats_json(&registry, backend.as_ref())
    } else {
        formats_table(console, &registry, backend.as_ref(), &f.typst)
    };
    console.print(&text);
    0
}

fn formats_json(registry: &Registry, backend: Option<&gd_typst::Backend>) -> String {
    let mut out = String::from("{\"readers\":[");
    let readers = registry.readers.iter().map(|r| (r.id, r.name, r.version, r.conformance));
    adapters_json(&mut out, readers);
    out.push_str("],\"renderers\":[");
    let renderers = registry.renderers.iter().map(|r| (r.id, r.name, r.version, r.conformance));
    adapters_json(&mut out, renderers);
    out.push_str("],\"typst\":");
    gd_report::escape_json(&mut out, backend.and_then(|b| b.version()).unwrap_or(""));
    let pinned = backend.is_some_and(|b| b.is_pinned());
    let _ = writeln!(out, ",\"typst_pinned\":{pinned}}}");
    out
}

fn adapters_json<'r>(
    out: &mut String,
    adapters: impl Iterator<Item = (&'r str, &'r str, &'r str, Conformance)>,
) {
    for (i, (id, name, version, c)) in adapters.enumerate() {
        if i > 0 {
            out.push(',');
        }
        for (key, value) in [("id", id), ("name", name), ("version", version)] {
            let lead = if key == "id" { "{" } else { "," };
            let _ = write!(out, "{lead}\"{key}\":");
            gd_report::escape_json(out, value);
        }
        let _ = write!(out, ",\"conformance\":\"{}\"}}", conformance(c));
    }
}

fn formats_table(
    console: &Console<'_>,
    registry: &Registry,
    backend: Option<&gd_typst::Backend>,
    program: &str,
) -> String {
    let mut out = format!("\n  {}\n\n", console.section("INPUT FORMATS"));
    for r in &registry.readers {
        let c = conformance(r.conformance);
        let _ = writeln!(out, "  {:<12} {:<18} {:<10} {c}", r.id, r.name, r.version);
    }
    let _ = writeln!(
        out,
        "  {:<12} {:<18} {:<10} via the Typst backend",
        "typst", "Typst (native)", "-"
    );
    let _ = write!(out, "\n  {}\n\n", console.section("OUTPUT FORMATS"));
    out.push_str("  html         direct renderer; native Typst uses Typst HTML export\n");
    out.push_str("  pdf          generated or native Typst, compiled by the backend\n");
    out.push_str("  rst          reStructuredText source, written by the rst renderer\n");
    out.push_str("  typst        intermediate source through --emit-typst\n");
    let _ = write!(out, "\n  {}\n\n", console.section("TYPST BACKEND"));
    let Some(backend) = backend else {
        let _ =
            writeln!(out, "  not found ({program}); PDF output and .typ input are unavailable\n");
        return out;
    };
    let version = backend.version().unwrap_or("(version unknown)");
    let note = if backend.is_pinned() {
        "pinned".to_string()
    } else {
        format!("expected {}", gd_typst::PINNED_VERSION)
    };
    let (name, kind) = match backend {
        gd_typst::Backend::Embedded => ("typst".to_string(), "embedded in this executable"),
        gd_typst::Backend::Program { path, .. } => {
            (path.display().to_string(), "development backend")
        }
    };
    let _ = writeln!(out, "  {name} {version} ({note}, {kind})");
    out.push_str("  Packages: @preview downloads into the Typst cache unless --offline;\n");
    out.push_str("  fonts: --font-path, then system fonts, then embedded fonts.\n\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_registry_is_valid_and_has_what_convert_names() {
        let registry = guidedoc_defaults::registry();
        assert!(registry.validate().is_ok());
        for id in ["rst", "commonmark"] {
            assert!(registry.find_reader(id).is_some(), "{id}");
        }
        for id in ["html", "typst"] {
            assert!(registry.find_renderer(id).is_some(), "{id}");
        }
    }

    #[test]
    fn formats_json_lists_the_adapters() {
        let json = formats_json(&guidedoc_defaults::registry(), None);
        assert!(json.starts_with("{\"readers\":[{\"id\":\"rst\""), "{json}");
        assert!(json.contains("\"typst_pinned\":false"), "{json}");
    }
}
