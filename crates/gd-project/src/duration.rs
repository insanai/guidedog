//! sphinx.ext.duration: how long the documents took to read. Reading measures each
//! document it parses (`gd_read::ReadingTime`); a document kept from the previous build
//! took no time and is not listed. The summary shows the total and the slowest
//! documents, a document slower than `duration_limit` is a warning, and the output
//! folder receives `duration_write_json`, an object from document names to seconds.

use std::fmt::Write as _;

use gd_conf::Config;
use gd_env::BuildResult;
use gd_host::Problem;
use gd_outputs::{Generation, UnitRecord};
use gd_read::ReadingTime;
use gd_report::Report;

/// The unit that holds the JSON file; no document can have this name.
const DURATION_UNIT: &str = "<sphinx.ext.duration>";

/// Whether the build reports reading times.
fn listed(config: &Config) -> bool {
    config.has_extension("sphinx.ext.duration")
}

/// Adds the total and the slowest documents to the summary, and a `duration.limit`
/// warning for each document slower than `duration_limit`.
pub(crate) fn report_durations(
    config: &Config,
    times: &[ReadingTime],
    result: &mut BuildResult,
    reports: &mut Vec<Report>,
) {
    if !listed(config) {
        return;
    }
    let d = &config.ext.duration;
    if d.print_total {
        let total = times.iter().fold(0.0, |sum, t| sum + t.time.as_secs_f64());
        let text = format!("{total:.3} s reading {} document(s)", times.len());
        result.steps.push(("duration".to_owned(), text));
    }
    let slowest = slowest(times, d.n_slowest);
    if d.print_slowest {
        for t in &slowest {
            let text = format!("{:.3} s {}", t.time.as_secs_f64(), t.docname);
            result.steps.push(("slowest".to_owned(), text));
        }
    }
    let Some(limit) = d.limit else { return };
    for t in times.iter().filter(|t| t.time.as_secs_f64() > limit) {
        let message = format!(
            "{} took {:.3} seconds to read, more than duration_limit ({limit} seconds).",
            t.docname,
            t.time.as_secs_f64()
        );
        let report = Report::warning("duration.limit", "SLOW DOCUMENT", &message).hint(
            "Split the document, or move slow content (large tables, many includes) out of it.",
        );
        reports.push(Report { category: "duration".to_owned(), ..report });
    }
}

/// The `n` slowest documents, slowest first (all when `n` is 0 or less); documents
/// that took the same time keep their name order.
fn slowest(times: &[ReadingTime], n: i64) -> Vec<&ReadingTime> {
    let mut sorted: Vec<&ReadingTime> = times.iter().collect();
    sorted.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| a.docname.cmp(&b.docname)));
    if let Ok(n) = usize::try_from(n)
        && n > 0
    {
        sorted.truncate(n);
    }
    sorted
}

/// Writes `duration_write_json` into the output, unless it is empty.
pub(crate) fn write_durations_json(
    config: &Config,
    times: &[ReadingTime],
    generation: &mut Generation,
) -> Result<(), Problem> {
    let name = &config.ext.duration.write_json;
    if !listed(config) || name.is_empty() {
        return Ok(());
    }
    let mut unit = generation.unit(DURATION_UNIT);
    unit.emit_text(name, &durations_json(times))?;
    unit.finish(UnitRecord::default());
    Ok(())
}

/// The JSON object Sphinx writes: document name to seconds, in reading order.
fn durations_json(times: &[ReadingTime]) -> String {
    let mut out = String::from("{");
    for (i, t) in times.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        push_json_string(&mut out, &t.docname);
        let _ = write!(out, ": {}", t.time.as_secs_f64());
    }
    out.push('}');
    out
}

/// Appends `text` as a JSON string.
fn push_json_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn time(docname: &str, millis: u64) -> ReadingTime {
        ReadingTime { docname: docname.into(), time: Duration::from_millis(millis) }
    }

    #[test]
    fn the_slowest_come_first_and_ties_keep_name_order() {
        let times = [time("a", 5), time("b", 20), time("c", 5), time("d", 10)];
        let names: Vec<&str> = slowest(&times, 3).iter().map(|t| t.docname.as_str()).collect();
        assert_eq!(names, ["b", "d", "a"]);
        assert_eq!(slowest(&times, 0).len(), 4);
    }

    #[test]
    fn the_summary_and_the_limit_follow_the_settings() {
        let mut config = Config::new("/p", "/p/_build");
        config.extensions.push("sphinx.ext.duration".into());
        config.ext.duration.n_slowest = 1;
        config.ext.duration.limit = Some(0.015);
        let (mut result, mut reports) = (BuildResult::default(), Vec::new());
        report_durations(&config, &[time("a", 10), time("b", 20)], &mut result, &mut reports);
        assert_eq!(result.steps[0].1, "0.030 s reading 2 document(s)");
        assert_eq!(result.steps[1].1, "0.020 s b");
        assert_eq!(result.steps.len(), 2);
        assert_eq!(reports.len(), 1);
        assert!(reports[0].message.starts_with("b took 0.020 seconds"));
    }

    #[test]
    fn the_json_maps_names_to_seconds() {
        let json = durations_json(&[time("a\"b", 250), time("c", 0)]);
        assert_eq!(json, "{\"a\\\"b\": 0.25, \"c\": 0}");
    }
}
