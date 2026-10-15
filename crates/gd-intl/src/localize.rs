//! What a build's language changes besides documents: the settings Sphinx derives in
//! English, the build's dates, and the conversions of the intl settings.

use std::time::{SystemTime, UNIX_EPOCH};

use gd_conf::{Config, FlagOrName};
use gd_env::UiText;
use gd_gettext::Error;
use gd_i18n::{Compact, Moment, Progress, Targets, format_date, parse_target};
use gd_report::{Excerpt, Report, Severity};
use gd_resolve::Dates;

/// The `numfig_format` defaults, which are translated when left as they are.
const DEFAULT_NUMFIG: [(&str, &str); 4] = [
    ("figure", "Fig. %s"),
    ("table", "Table %s"),
    ("code-block", "Listing %s"),
    ("section", "Section %s"),
];

/// The date format Sphinx uses when a format setting is empty, before translation.
const DEFAULT_DATE_FORMAT: &str = "%b %d, %Y";

/// Gives the settings Sphinx derives in the document language their translation when
/// the project left them at their English defaults: the `numfig_format` captions,
/// `html_title` ("%s %s documentation") and `html_short_title`. Changes `config`, the
/// build's own copy; the gettext builder leaves its copy in English.
pub fn localize_config(config: &mut Config, ui: &dyn UiText) {
    for (key, english) in DEFAULT_NUMFIG {
        if let Some(format) = config.numfig_format.get_mut(key)
            && format == english
        {
            *format = ui.gettext(english).into_owned();
        }
    }
    if config.html.title_derived {
        config.html.title = if config.release.is_empty() {
            python_format(&ui.gettext("%s documentation"), &[&config.project])
        } else {
            python_format(&ui.gettext("%s %s documentation"), &[&config.project, &config.release])
        };
    }
    if config.html.short_title_derived {
        config.html.short_title = config.html.title.clone();
    }
}

/// Replaces each `%s` of `format` with the next of `values`, as Python's `%` does;
/// a `%s` left without a value stays as it is.
pub fn python_format(format: &str, values: &[&str]) -> String {
    let mut out =
        String::with_capacity(format.len() + values.iter().map(|v| v.len()).sum::<usize>());
    let mut rest = format;
    let mut values = values.iter();
    while let Some(at) = rest.find("%s") {
        let Some(value) = values.next() else { break };
        out.push_str(&rest[..at]);
        out.push_str(value);
        rest = &rest[at + 2..];
    }
    out.push_str(rest);
    out
}

/// The build's dates, as Sphinx formats them: `|today|` is conf.toml's `today`, or
/// `now` in `today_fmt`; the page footer's date is `now` in `html_last_updated_fmt`
/// (empty when unset). An empty format is the translated "%b %d, %Y". Dates are UTC.
pub fn build_dates(config: &Config, ui: &dyn UiText, now: SystemTime) -> Dates {
    let moment = moment_of(now);
    let format = |f: &str| {
        let translated = ui.gettext(DEFAULT_DATE_FORMAT);
        let f = if f.is_empty() { translated.as_ref() } else { f };
        format_date(f, &moment, &config.language)
    };
    let today =
        if config.today.is_empty() { format(&config.today_fmt) } else { config.today.clone() };
    let last_updated = config.html.last_updated_fmt.as_deref().map(format).unwrap_or_default();
    Dates { today, last_updated, date: format_date("%Y-%m-%d", &moment, "en") }
}

/// `now` as a UTC moment; a time before 1970 counts back from it.
pub(crate) fn moment_of(now: SystemTime) -> Moment {
    let seconds = match now.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_secs()).unwrap_or(i64::MAX),
        Err(before) => -i64::try_from(before.duration().as_secs()).unwrap_or(i64::MAX),
    };
    Moment::from_unix_seconds(seconds)
}

/// `translation_progress_classes`: `true` marks both, a name marks one, `false` none.
/// A name gd-conf would refuse marks both, as Sphinx's `True` does.
pub fn progress_classes(setting: &FlagOrName) -> Progress {
    match setting {
        FlagOrName::Flag(false) => Progress::default(),
        FlagOrName::Name(name) if name == "translated" => {
            Progress { translated: true, untranslated: false }
        }
        FlagOrName::Name(name) if name == "untranslated" => {
            Progress { translated: false, untranslated: true }
        }
        _ => Progress::ALL,
    }
}

/// `gettext_compact`: `true` is one catalog per top-level folder, `false` one per
/// document, a name one catalog of that name.
pub fn compact_mode(setting: &FlagOrName) -> Compact {
    match setting {
        FlagOrName::Flag(true) => Compact::PerDirectory,
        FlagOrName::Flag(false) => Compact::PerDocument,
        FlagOrName::Name(name) if name.is_empty() => Compact::PerDirectory,
        FlagOrName::Name(name) => Compact::Single(name.clone()),
    }
}

/// `gettext_additional_targets`; names gd-conf refused are left out.
pub fn additional_targets(config: &Config) -> Targets {
    let mut targets = Targets::default();
    config
        .intl
        .gettext_additional_targets
        .iter()
        .filter_map(|n| parse_target(n))
        .for_each(|t| targets.insert(t));
    targets
}

/// Turns a catalog error into a build report (`i18n.catalog`) with its excerpt.
pub fn catalog_report(error: &Error, severity: Severity) -> Report {
    let mut report =
        Report::error("i18n.catalog", error.title(), &error.message).hint(&error.hint);
    report.severity = severity;
    report.path = gd_host::display_path(&error.file);
    report.line = error.line;
    report.column = error.column;
    if !error.excerpt.is_empty() {
        // The error's column counts characters; the excerpt underlines bytes.
        let start = error
            .excerpt
            .char_indices()
            .nth(error.column.saturating_sub(1))
            .map_or(error.excerpt.len(), |(i, _)| i);
        report.excerpt = Some(Excerpt {
            text: error.excerpt.clone(),
            start,
            end: start + 1,
            before: String::new(),
        });
    }
    report
}
