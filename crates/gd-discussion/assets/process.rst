@NAME@ Process
@PROCESS_UNDERLINE@

Abstract
--------

This draft says how @NAME@ proposes, reviews, and records decisions.
It is a starting point for the maintainers to edit. It is not yet accepted.

Lifecycle and numbering
-----------------------

@LIFECYCLE@
Closed records are kept, and numbers are never reused. The tool records decisions;
the maintainers' review makes them. ``discussion.toml`` sets these states and moves.

Writing and review
------------------

Start from ``template/rfc-template.rst``. Explain the problem before the solution.
Name the terms, the behavior a reader will see, the alternatives, compatibility,
security, and operations. Keep the implementation status separate from the proposal.
Every record needs an Abstract and a References section.
A number is not an approval.

Operations and recovery
-----------------------

Commands that change records hold a lock. Each file is replaced whole, and a journal
makes an update of several files recoverable. ``--dry-run`` shows a change without
making it. After an interruption, run ``recover --rollback``; it refuses to overwrite
a later edit. Keep the records in version control.

References
----------

* :doc:`../workflow`, the commands of this catalog.
* Guidedog, which builds the HTML and PDF editions.
