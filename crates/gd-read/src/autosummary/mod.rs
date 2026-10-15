//! sphinx.ext.autosummary. The `autosummary` directive becomes a summary table of
//! Python objects, found by the source scan, and with `:toctree:` a hidden toctree of
//! their stub pages. The stub pages are generated documents (`stubs`), made before
//! reading from the autosummary directives of the source files, never written to the
//! source folder.

mod scan;
mod stubs;

use std::collections::BTreeMap;

use gd_autodoc::{Session, SummaryRequest, SummarySignatures};
use gd_conf::Config;
use guidedoc::Severity;
use guidedoc_sphinx::{AutodocMessage, AutodocRequest, AutodocResult};

use crate::messages::host_message;

pub(crate) use stubs::autosummary_sources;

/// Whether sphinx.ext.autosummary is on.
pub(crate) fn is_on(c: &Config) -> bool {
    c.has_extension("sphinx.ext.autosummary")
}

/// The value of option `name`, when given.
fn option<'r>(request: &'r AutodocRequest<'_>, name: &str) -> Option<&'r str> {
    request.options.iter().find(|o| o.name == name).map(|o| o.value.as_str())
}

/// `Autosummary.run`'s entry names: the first word of each content line that starts
/// with `~`, a letter or `_`.
pub(crate) fn entry_names(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with(|c: char| c == '~' || c == '_' || c.is_ascii_alphabetic()))
        .filter_map(|l| l.split_whitespace().next().map(str::to_owned))
        .collect()
}

/// The text an `autosummary` directive stands for: the table, and with `:toctree:` a
/// hidden toctree of the stub pages.
pub(crate) fn summary_text(
    session: &mut Session,
    filename_map: &BTreeMap<String, String>,
    autodoc: &gd_autodoc::Config,
    request: &AutodocRequest<'_>,
) -> AutodocResult {
    let mut out = AutodocResult { name: "<autosummary>".into(), ..AutodocResult::default() };
    let signatures = match signature_style(request) {
        Ok(style) => style,
        Err(m) => {
            out.messages.push(m);
            return out;
        }
    };
    let names = entry_names(request.content);
    let summary = gd_autodoc::summarize(
        session,
        autodoc,
        &SummaryRequest {
            names: &names,
            module: request.module,
            class: request.class,
            signatures,
        },
    );
    for m in &summary.messages {
        out.messages.push(host_message(
            to_severity(m.severity),
            &m.code,
            &m.title,
            &m.text,
            &m.hint,
        ));
    }
    let classes: Vec<String> = option(request, "class")
        .map_or_else(Vec::new, |c| c.split_whitespace().map(str::to_owned).collect());
    let mut lines = gd_autodoc::summary_table(&summary.items, &classes);
    match option(request, "toctree") {
        Some(prefix) => {
            toctree_lines(filename_map, request, prefix.trim(), &summary.items, &mut lines);
        }
        None if option(request, "caption").is_some() => out.messages.push(host_message(
            Severity::Warning,
            "autosummary.toctree",
            "CAPTION WITHOUT TOCTREE",
            "A captioned autosummary requires the :toctree: option; the caption is ignored.",
            "Add :toctree: with the folder of the stub pages, or remove :caption:.",
        )),
        None => {}
    }
    if !lines.is_empty() {
        out.text = lines.join("\n");
        out.text.push('\n');
    }
    out
}

/// `:signatures:` (or `:nosignatures:`); an unknown style is an error.
fn signature_style(request: &AutodocRequest<'_>) -> Result<SummarySignatures, AutodocMessage> {
    let Some(value) = option(request, "signatures").map(str::trim) else {
        let omitted = option(request, "nosignatures").is_some();
        return Ok(if omitted { SummarySignatures::Omitted } else { SummarySignatures::Long });
    };
    SummarySignatures::parse(value).ok_or_else(|| {
        host_message(
            Severity::Error,
            "autosummary.option",
            "UNKNOWN SIGNATURE STYLE",
            &format!("autosummary has :signatures: {value}, which is not none, short or long."),
            "Use :signatures: long (the default), short or none.",
        )
    })
}

/// The hidden toctree of the stub pages under `prefix`, named as
/// `autosummary_filename_map` says.
fn toctree_lines(
    map: &BTreeMap<String, String>,
    request: &AutodocRequest<'_>,
    prefix: &str,
    items: &[gd_autodoc::SummaryItem],
    lines: &mut Vec<String>,
) {
    if items.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push(".. toctree::".to_owned());
    lines.push("   :hidden:".to_owned());
    if let Some(caption) = option(request, "caption") {
        lines.push(format!("   :caption: {caption}"));
    }
    lines.push(String::new());
    for item in items {
        let file = map.get(&item.real_name).unwrap_or(&item.real_name);
        let entry = if prefix.is_empty() { file.clone() } else { format!("{prefix}/{file}") };
        lines.push(format!("   {}", stubs::normalize(&entry)));
    }
}

/// gd-autodoc's severity as the engine's.
pub(crate) fn to_severity(s: gd_autodoc::Severity) -> Severity {
    match s {
        gd_autodoc::Severity::Note => Severity::Note,
        gd_autodoc::Severity::Warning => Severity::Warning,
        gd_autodoc::Severity::Error => Severity::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_the_first_word_of_name_lines() {
        let content = "\n   ~pkg.Square  The class.\n:option: x\n_private\n\n2bad\n";
        assert_eq!(entry_names(content), ["~pkg.Square", "_private"]);
    }
}
