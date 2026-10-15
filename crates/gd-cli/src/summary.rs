//! What a build's summary says it did, and only that, however far it got.

use gd_env::{BuildResult, BuildStage};

/// The summary's two lines about a build: what it did with the documents (read), and
/// with the outputs (write; `None` when there is nothing to say). A build that stopped
/// before reading read nothing, one that stopped before writing prepared nothing, and
/// outputs are written only when they were published.
pub(crate) fn build_summary(r: &BuildResult) -> (String, Option<String>) {
    if r.reached == BuildStage::Preparing && r.read == 0 && r.reused == 0 && r.unread == 0 {
        return ("nothing read: the build stopped before reading the documents".into(), None);
    }
    let mut read = format!("{} document(s) parsed, {} unchanged", r.read, r.reused);
    if r.unread > 0 {
        read.push_str(&format!(", {} could not be read", r.unread));
    }
    let write = if r.published {
        format!("{} output(s) written, {} unchanged", r.written, r.unchanged)
    } else if r.unsettled {
        format!("{} output(s) prepared; their publication is not finished", r.written)
    } else if r.written > 0 {
        format!("{} output(s) prepared, not published; {} unchanged", r.written, r.unchanged)
    } else if r.reached < BuildStage::Written {
        "nothing prepared: the build stopped before writing its output".to_string()
    } else if r.unchanged > 0 {
        format!("nothing prepared: all {} output(s) were up to date", r.unchanged)
    } else {
        "nothing prepared: no output was written".to_string()
    };
    (read, Some(write))
}

/// Where the output is: the build's own when it published, the previous build's when it
/// did not, and nothing when there is none. A publication neither finished nor undone is
/// settled by the next build.
pub(crate) fn output_note(r: &BuildResult) -> String {
    let path = gd_host::display_path(&r.output.to_string_lossy());
    if r.unsettled {
        return format!(
            " The publication in {path} is not finished; the next build finishes or undoes it."
        );
    }
    if !r.output.is_dir() {
        return if r.published { String::new() } else { " Nothing was published.".into() };
    }
    if r.published {
        format!(" Output in {path}")
    } else {
        format!(" The previous output in {path} is unchanged.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(reached: BuildStage, counts: [usize; 5]) -> BuildResult {
        let [read, reused, unread, written, unchanged] = counts;
        BuildResult { reached, read, reused, unread, written, unchanged, ..Default::default() }
    }

    type Case = (&'static str, BuildResult, &'static str, Option<&'static str>);

    /// A build's summary says what it did, and only that, however far it got.
    #[test]
    fn the_build_summary_says_only_what_happened() {
        for (name, r, read, write) in summary_cases() {
            let (got_read, got_write) = build_summary(&r);
            assert_eq!(got_read, read, "{name}");
            assert_eq!(got_write.as_deref(), write, "{name}");
        }
    }

    fn summary_cases() -> [Case; 10] {
        use BuildStage::{Preparing, Reading, Writing, Written};
        let published = BuildResult { published: true, ..result(Written, [2, 1, 0, 3, 4]) };
        let unsettled = BuildResult { unsettled: true, ..result(Written, [2, 0, 0, 3, 0]) };
        let unsettled_early = BuildResult { unsettled: true, ..result(Preparing, [0; 5]) };
        [
            (
                "published",
                published,
                "2 document(s) parsed, 1 unchanged",
                Some("3 output(s) written, 4 unchanged"),
            ),
            (
                "nothing read",
                result(Preparing, [0; 5]),
                "nothing read: the build stopped before reading the documents",
                None,
            ),
            (
                "nothing prepared",
                result(Reading, [1, 0, 0, 0, 0]),
                "1 document(s) parsed, 0 unchanged",
                Some("nothing prepared: the build stopped before writing its output"),
            ),
            (
                "stopped while writing",
                result(Writing, [1, 0, 0, 0, 0]),
                "1 document(s) parsed, 0 unchanged",
                Some("nothing prepared: the build stopped before writing its output"),
            ),
            (
                "all up to date",
                result(Written, [0, 2, 0, 0, 5]),
                "0 document(s) parsed, 2 unchanged",
                Some("nothing prepared: all 5 output(s) were up to date"),
            ),
            (
                "no output",
                result(Written, [2, 0, 0, 0, 0]),
                "2 document(s) parsed, 0 unchanged",
                Some("nothing prepared: no output was written"),
            ),
            (
                "a document not read",
                result(Written, [0, 0, 1, 0, 0]),
                "0 document(s) parsed, 0 unchanged, 1 could not be read",
                Some("nothing prepared: no output was written"),
            ),
            (
                "prepared, not published",
                result(Written, [2, 0, 0, 3, 1]),
                "2 document(s) parsed, 0 unchanged",
                Some("3 output(s) prepared, not published; 1 unchanged"),
            ),
            (
                "unsettled",
                unsettled,
                "2 document(s) parsed, 0 unchanged",
                Some("3 output(s) prepared; their publication is not finished"),
            ),
            (
                "unsettled before reading",
                unsettled_early,
                "nothing read: the build stopped before reading the documents",
                None,
            ),
        ]
    }

    #[test]
    fn the_output_note_names_only_an_existing_output() {
        let missing = BuildResult { output: "/nonexistent/out".into(), ..Default::default() };
        assert_eq!(output_note(&missing), " Nothing was published.");
        let here = BuildResult { output: ".".into(), published: true, ..Default::default() };
        assert!(output_note(&here).starts_with(" Output in"));
        let kept = BuildResult { output: ".".into(), ..Default::default() };
        assert!(output_note(&kept).contains("previous output"));
    }
}
