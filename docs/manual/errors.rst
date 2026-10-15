.. _errors:

Error messages
==============

A report should name the problem, show its location, and give a useful next step.
Guidedog keeps that evidence as structured data. The terminal adds color and a
source marker. JSON keeps the data without terminal decoration.

Reading a report
----------------

.. code-block:: text

   UNDEFINED SUBSTITUTION                              rst.substitution.undefined

     guide/page.rst:4: See |logo| here.
                           ^^^^^^

   The substitution "|logo|" is not defined.
   No output was written.

   Add a definition, e.g. ".. |name| replace:: text".

A report has four parts, always in this order:

1. **The title bar.** The title names the kind of problem in capitals; warnings and notes
   start with ``WARNING:`` and ``NOTE:``. At the right of the bar is the report's code,
   such as ``rst.substitution.undefined``: a stable name you can search for.
2. **Where.** The file and line, the source line itself, and a marker under the text the
   problem is about. A problem that is not about one line (a missing folder, a flag)
   shows only the path, or nothing.
3. **What happened.** The explanation, followed, when it matters, by what the problem
   means for the output: here, that nothing was written.
4. **What to do.** The hint, in green on a color terminal: a concrete direction. Where
   there are several ways to fix the problem, the hint names them, the most likely
   first.

Colors only underline what the words already say; ``--color=never`` turns them off and
``--color=always`` keeps them when the output is not a terminal.

Every hint gives directions
---------------------------

Guidedog holds its own messages to this: a hint starts with what to do ("Add", "Pass",
"Set", "Rename", "Run") and names what to do it to, such as ``--budget=MIB``,
``root_doc`` in ``conf.toml``, or the directive's option. Tests check
the messages Guidedog emits and fail when a hint is empty, repeats the message, or says
only "report this" without saying what to report with. Two forms are allowed because they name the fix themselves: a suggestion ("Did you
mean numfig?") and, for a note that needs no action, "Nothing to do:" with the reason.

When the problem is Guidedog's own fault, the hint says so and asks you to report it
with the command you ran, the input that caused it, and the output of
``guidedog --version``.

Reports for programs
--------------------

``--diagnostics=json`` writes each report as one JSON object per line on standard error,
for editors and continuous integration:

.. code-block:: json

   {"schema":1,"code":"rst.substitution.undefined","severity":"error",
    "title":"UNDEFINED SUBSTITUTION","message":"The substitution \"|logo|\" is not defined.",
    "hint":"Add a definition, e.g. \".. |name| replace:: text\".","path":"guide/page.rst",
    "line":4,"column":5,"end_line":4,"end_column":11,"source":"See |logo| here.",
    "category":""}

(shown wrapped here; each object is on one line). ``schema`` is the version of this
format, which changes only when a field does; check it before reading the rest. Columns
count Unicode scalars, not bytes or terminal cells, from 1. ``source`` is the complete
original source line; the terminal shows a bounded window around the error and aligns
its caret after escaping controls and measuring wide characters. ``category`` is the warning's category, as
``suppress_warnings`` names it, or empty.

When memory runs out
--------------------

Reaching a bound is a report, not a crash. There are two kinds:

``host.budget`` (MEMORY BUDGET REACHED)
   The build reached the memory it may hold, its budget (``--budget=MIB``, 1 GiB by
   default). The budget counts what Guidedog reads and keeps: sources, included files,
   and the PDF the embedded Typst compiler returns. The report names the step and the
   file that needed the memory, and the hint names a budget that fits as a number, for
   example::

      Run again with --budget=256, or split the work into smaller files.

   The number holds what the build held and the refused step four times over (the step
   also holds its output and the copy it writes), or twice the old budget, whichever is
   larger, rounded up to 64 MiB, and no more than the memory the machine has free when
   the step fits there. When it does not, the hint says so::

      Free memory and run again with --budget=1408: the step needs about 1408 MiB,
      more than the 900 MiB this machine has free. Or split the work into smaller files.

   ``GUIDEDOG_AVAILABLE_MIB`` states the machine's free memory when the system's own
   figure misleads, for example inside a container.

``host.memory`` (OUT OF MEMORY)
   The system could not provide a buffer the budget allowed. The command stops without
   using it: a build publishes nothing, a GDS change is applied whole or not at all, and
   ``convert`` writes a whole page or none. Free memory, or build with fewer threads
   (``-j 1``), and run the command again. Once a build has decided to publish, switching
   its output into place needs no further planning, so a refusal cannot stop a
   publication halfway; an interrupted publication is finished by the next build.

A document's own size is bounded separately, by ``--max-nodes``, ``--max-depth`` and
``--stack-kib``: an oversized document is reported with the limit it reached. See
:doc:`internals/memory` for how the budget and these limits divide the work. The Odin
implementation could also keep working memory on disk (``--memory=disk``); the Rust
implementation bounds a conversion by its limits instead, so there is no disk mode.

Both end with status 3.

Exit status
-----------

Every command ends with one of these statuses, so scripts can tell kinds of failure
apart:

======  ==============================================================================
Status  Meaning
======  ==============================================================================
0       Success. The output was written (and, for a build, published).
1       The input has a problem: an error in a document, a policy refusal (raw content
        without ``--raw``), a document that could not be read, or a warning when ``-W``
        or ``--strict`` treats warnings as errors.
2       The command was used wrongly: an unknown flag or command, a missing or invalid
        value, or an invalid setting in ``conf.toml`` or ``-D``.
3       A limit was reached: the memory budget (``--budget``), the nesting depth
        (``--max-depth``), the node count (``--max-nodes``), the stack (``--stack-kib``),
        a source too large, or memory the system could not provide.
4       A file or an outside program failed: a path that cannot be read or written, a
        file that changed while it was read, or Typst not starting or not finishing.
5       Guidedog failed internally, and the report asks you to report it.
70      Guidedog crashed. It prints ``GUIDEDOG CRASHED (internal error)`` with the command
        that ran, the fault, and what ``guidedog --version`` prints, and says where to
        report it: the issue tracker your build names, or else whoever provided your
        build. Because builds publish only at the end, a crash leaves the previous output
        in place.
130     The command was interrupted with Ctrl+C; published files were left unchanged.
======  ==============================================================================

A build that fails for any reason leaves the previous output, and the records of what it
built, as they were: fix the problem and build again.
