Keep a discussion catalog
=========================

A discussion catalog keeps proposals and decisions. Each discussion is one
reStructuredText record. A record gets a number when it opens for review, and its
state says where the decision stands. Guidedog's own design records, GDS, are one such
catalog. This page uses ``dsc``.

Choose a program
----------------

.. list-table:: Three ways to run the same commands
   :header-rows: 1
   :widths: 25 75

   * - Program
     - Use it when
   * - ``dsc``
     - Guidedog is installed. ``dsc`` keeps the catalog itself, and asks ``guidedog``
       to build it. It comes in the Guidedog archive.
   * - ``dscc``
     - Nothing else is installed. ``dscc`` is ``dsc`` with Guidedog built in. Each
       release has a ``dscc`` archive for every platform.
   * - ``guidedog gds``
     - You work on Guidedog. It runs the same commands on ``docs/gds``.

Graphviz, Typst, and the other tools a build needs belong to Guidedog. ``dsc`` never
looks for them: it uses whichever ``guidedog`` you have.

Create a catalog
----------------

.. code-block:: sh

   dsc init docs/eds --prefix=eds --name="Enso Discussions" --author="Ada"

The prefix names the catalog's fields, such as ``:eds-number:``. It is 2 to 16 lowercase
letters. ``init`` writes a complete catalog into a missing or empty folder:

* ``discussion.toml``, the catalog's prefix, name, default author, lifecycle, and
  numbering;
* ``conf.toml``, ``index.rst``, and ``workflow.rst``, an ordinary Guidedog project;
* ``template/rfc-template.rst``, the template of new drafts;
* ``registry.rst`` and ``bundle.rst``, the generated index;
* ``records/XXXXX-eds-process.rst``, a draft of your review process.

Edit the process draft to say how your project decides. To start from your own
documents, pass ``--config=FILE`` (a ``discussion.toml``), ``--process=FILE`` (the
process text, from its title on), and ``--template=FILE`` (a record template with
``:eds-*:`` fields). ``--labels=a,b`` limits
the labels records may carry. ``init`` checks the new catalog before it moves it into
place, and never writes into a folder that has files.

Write and review
----------------

Run the commands in the catalog folder, or in any folder inside it.

.. code-block:: sh

   dsc new "Cache policy" --labels=architecture
   dsc list
   dsc show cache-policy
   dsc promote cache-policy
   dsc state 1 --to=accepted --reason="Review converged"
   dsc state 1 --to=committed --reason="Shipped" --evidence="Tests and review"

A new draft has the number ``XXXXX``. Promotion gives it the next number and opens it
for discussion. A state change needs ``--reason``; ``committed`` also needs
``--evidence``. Add ``--dry-run`` to see a change without making it. These are the
defaults; the next section changes them.

A record starts with its metadata, one field per line:

.. code-block:: rst

   :eds-number: 0001
   :eds-title: Cache policy
   :eds-state: accepted
   :eds-type: Standards Track
   :eds-authors: ["Ada"]
   :eds-created: 2026-10-12
   :eds-updated: 2026-10-12
   :eds-discussion: https://example.org/review/1
   :eds-labels: ["architecture"]

Authors and labels are JSON lists. ``last-updated`` may stand for ``updated``. A record
may also carry ``category``, ``status``, and ``summary``; ``dsc`` keeps ``status`` in step
with the state. HTML and PDF show the fields as Number, Title, Authors, Updated, Labels,
and so on, with the lists as plain text.

Configure the lifecycle and numbering
-------------------------------------

``discussion.toml`` holds the catalog's settings. ``init`` writes every one of them,
with comments, and ``dsc config`` prints the settings in effect. A catalog without the
file follows the defaults below, which are the process of Guidedog's own GDS.

.. code-block:: sh

   dsc config > discussion.toml.new && mv discussion.toml.new discussion.toml

.. code-block:: toml

   prefix = "eds"
   name = "Enso Discussions"
   author = "Ada"
   labels = ["architecture", "process"]   # leave out to allow any label
   types = ["Standards Track", "Process", "Informational"]

   [numbering]
   width = 4      # at least four digits: 0007
   start = 1
   # limit = 99999

   [workflow]
   draft = "prediscussion"   # where new drafts start
   open = "discussion"       # where promotion moves a draft

   [workflow.states.prediscussion]
   next = ["abandoned"]
   status = "Working draft"

   [workflow.states.discussion]
   next = ["accepted", "published", "abandoned"]
   status = "Open for discussion"

   [workflow.states.committed]
   evidence = true
   status = "Implemented design"

``[workflow.states.NAME]`` defines a state: ``next`` lists the states the ``state``
command may move a record to, ``evidence = true`` asks for ``--evidence`` on a move into
it, and ``status`` is the text a record's optional ``status`` field takes there. A state
with no ``next`` is closed. Promotion is the only way from the draft state to the open
state, because it gives the number; a draft may move without one only to the states
its own ``next`` lists, such as ``abandoned``.

Numbers have at least ``width`` digits, padded with zeros. A number that needs more
digits gets them, so 10000 follows 9999, and lists sort by value. Promotion numbers from
``start``; ``limit``, when set, is the highest number it gives. Changing ``start`` or
``limit`` later is safe. Changing ``width`` is not, once records are numbered: their
numbers would no longer match it.

``dsc`` checks the file whenever it reads the catalog. A mistake names the line and what
to write instead:

.. code-block:: text

   -- THE CATALOG CONFIGURATION IS NOT VALID [discussion.config] --- discussion.toml:16:1

   There is no setting numbering.digits.

   15| [numbering]
   16| digits = 4
       ^

   Hint: Use one of: width, start, limit.

The process draft that ``init`` writes describes the lifecycle you configured, with a
table of the states and a diagram of the moves.

Build
-----

.. code-block:: sh

   dsc build
   dsc build cache-policy --target=pdf

``dsc build`` writes the website to ``_build/html`` and one PDF per record to
``_build/pdf``. Naming a record writes only its PDF, to ``_build/selected``, and leaves
the complete output alone. Warnings stop the build.

``dsc`` runs the ``guidedog`` named by ``--guidedog=PATH``, then by ``DSC_GUIDEDOG``, then
the one on ``PATH``. ``dscc`` builds with the Guidedog inside it.

When an update is interrupted
-----------------------------

Commands that change records hold the catalog's lock, ``.eds.lock``. An update of several
files first writes a journal, ``.eds-journal.json``. If it stops part way, the next
command says so:

.. code-block:: sh

   dsc recover --rollback --dry-run
   dsc recover --rollback

Recovery puts back the files as they were. It refuses to overwrite a file someone edited
since. ``dsc`` also checks each change before it writes anything: an empty or cut-short
record, or an index without its generated region, is refused.

``dsc`` keeps only each record's metadata in memory, and reads a record whole only to
change it, so checking a large catalog stays fast and small.

Use the library
---------------

The three programs share one Rust crate, ``gd-discussion``. Its ``README.md`` shows how
to load a catalog, plan a change, and apply it from your own program:

.. code-block:: rust

   use gd_discussion as ds;

   let prefix = ds::Prefix::new("eds")?;
   let lock = ds::lock_catalog(dir, &prefix)?;     // released on drop
   let catalog = ds::load_catalog(dir, &prefix)?;  // metadata only, never record text
   let plan = ds::state_plan(&catalog, &ds::Decision {
       identifier: "0003".into(),
       to: "accepted".into(),
       reason: "Converged".into(),
       ..Default::default()
   })?;                                            // changes nothing yet
   ds::apply(&lock, &plan)?;                       // journaled; ds::recover rolls back

``ds::command::run`` is the whole ``dsc`` command line, for a program of your own with
another name, prefix, or builder. The design notes are in
:doc:`../internals/discussion`.
