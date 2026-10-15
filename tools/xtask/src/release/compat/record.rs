//! The qualification record: which hosts and pinned assets were used, and how each
//! pairing of a host with a plugin program fared. It is printed whatever the outcome,
//! so a failed run still says what was checked.

use std::fmt::Write as _;

use super::pins::Pin;

/// How one pairing fared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Status {
    /// Every operation behaved as the protocol requires.
    Passed,
    /// An operation did not; the text says which and how.
    Failed(String),
    /// The pairing could not run here; the text says why. It is not an incompatibility.
    Skipped(String),
}

/// One host run against one plugin program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Outcome {
    /// `old-host_new-plugin` or `new-host_old-plugin`, the script's names.
    pub direction: &'static str,
    /// The plugin program, such as `guidedog-pages 0.6.0`.
    pub plugin: String,
    /// The operations exercised, such as `register, inspect, dry run`.
    pub operations: &'static str,
    /// How it went.
    pub status: Status,
}

/// What a run checked and found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Record {
    /// The platform name of the release archive, such as `linux-amd64`.
    pub platform: &'static str,
    /// The old host's `--version` line.
    pub old_host: String,
    /// The new host's `--version` line.
    pub new_host: String,
    /// The release assets downloaded, each verified against its pin.
    pub pins: Vec<Pin>,
    /// Each pairing, in the order it ran.
    pub outcomes: Vec<Outcome>,
}

impl Record {
    /// The outcomes that failed.
    pub fn failures(&self) -> impl Iterator<Item = &Outcome> {
        self.outcomes.iter().filter(|o| matches!(o.status, Status::Failed(_)))
    }

    /// Whether every pairing that ran passed.
    pub fn is_compatible(&self) -> bool {
        self.failures().next().is_none()
    }

    /// The record as plain text for the terminal and the release evidence.
    pub fn render(&self) -> String {
        let mut out = String::from("Plugin protocol 1.0 compatibility record\n");
        let _ = writeln!(out, "  platform   {}", self.platform);
        let _ = writeln!(out, "  old host   {} (release {})", self.old_host, super::pins::OLD_TAG);
        let _ = writeln!(out, "  new host   {} (this checkout)", self.new_host);
        out.push_str("  pinned assets (SHA-256 verified)\n");
        for pin in &self.pins {
            let _ = writeln!(out, "    {}  {}", pin.sha256, pin.name);
        }
        out.push_str("  results\n");
        for outcome in &self.outcomes {
            render_outcome(&mut out, outcome);
        }
        let (mut passed, mut failed, mut skipped) = (0, 0, 0);
        for outcome in &self.outcomes {
            match outcome.status {
                Status::Passed => passed += 1,
                Status::Failed(_) => failed += 1,
                Status::Skipped(_) => skipped += 1,
            }
        }
        let _ = writeln!(out, "  {passed} passed, {failed} failed, {skipped} skipped");
        out
    }
}

fn render_outcome(out: &mut String, outcome: &Outcome) {
    let (word, why) = match &outcome.status {
        Status::Passed => ("passed", None),
        Status::Failed(why) => ("FAILED", Some(why)),
        Status::Skipped(why) => ("skipped", Some(why)),
    };
    let _ = writeln!(
        out,
        "    {:<20} {:<42} {word}: {}",
        outcome.direction, outcome.plugin, outcome.operations
    );
    if let Some(why) = why {
        let _ = writeln!(out, "      {why}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(status: Status) -> Outcome {
        Outcome {
            direction: "old-host_new-plugin",
            plugin: String::from("folder (examples/publisher)"),
            operations: "register, inspect, publish",
            status,
        }
    }

    #[test]
    fn skipped_pairings_are_not_incompatibilities() {
        let mut record = Record::default();
        record.outcomes.push(outcome(Status::Passed));
        record.outcomes.push(outcome(Status::Skipped(String::from("no odin"))));
        assert!(record.is_compatible());
        record.outcomes.push(outcome(Status::Failed(String::from("register failed"))));
        assert!(!record.is_compatible());
        assert_eq!(record.failures().count(), 1);
    }

    #[test]
    fn the_record_names_versions_pins_and_results() {
        let record = Record {
            platform: "linux-amd64",
            old_host: String::from("guidedog 0.6.0"),
            new_host: String::from("guidedog 0.7.0"),
            pins: vec![super::super::pins::SOURCE],
            outcomes: vec![
                outcome(Status::Passed),
                outcome(Status::Failed(String::from("dry run wrote public"))),
            ],
        };
        let text = record.render();
        assert!(text.contains("old host   guidedog 0.6.0 (release v0.6.0)"));
        assert!(text.contains("new host   guidedog 0.7.0"));
        assert!(text.contains("0335f7082dc7e69c88b651f961612f99c7c640f64f272c4564d8023b95ad05d8"));
        assert!(text.contains("passed: register, inspect, publish"));
        assert!(text.contains("FAILED: register, inspect, publish\n      dry run wrote public"));
        assert!(text.ends_with("1 passed, 1 failed, 0 skipped\n"));
    }
}
