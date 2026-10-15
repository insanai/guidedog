#import "../../shared/guide.typ": frame, record
#let gds-number = "XXXXX"
#let gds-title = "Title goes here"
#let gds-state = "prediscussion"
#let gds-type = "Standards Track"
#let gds-authors = ("Author name",)
#let gds-created = "YYYY-MM-DD"
#let gds-updated = "YYYY-MM-DD"
#let gds-discussion = "No external discussion URL assigned"
#let gds-labels = ("architecture",)
#show: frame
#record(gds-number, gds-title, gds-state, gds-type, gds-authors,
  gds-created, gds-updated, gds-discussion, labels: gds-labels)

= Abstract
State the proposed decision and its observable result.

= Motivation and scope
Explain the Guidedog project problem and the boundaries of this proposal.

= Specification
Define behavior, interfaces, ownership, limits, and errors. Give a worked example.

= Rationale and alternatives
Explain why the design fits and why meaningful alternatives were not selected.

= Security and operational considerations
State trust boundaries, failure handling, deployment, and recovery behavior.

= Backwards compatibility
Explain effects on existing projects, Sphinx migration, and published interfaces.

= Acceptance criteria and implementation status
Describe evidence required and distinguish proposed behavior from implemented code.

= Open questions
Name decisions that still need review or implementation evidence.

= Review history
Record material decisions and unresolved objections with dates.

= References
Link specifications and related discussions. Format records additionally require
mapping tables, deliberate omissions, and round-trip expectations under GDS 0001.
