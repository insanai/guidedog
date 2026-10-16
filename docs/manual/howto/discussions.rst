Keep a discussion catalog
=========================

A discussion catalog keeps proposals and decisions. Each discussion is one
reStructuredText record. A record gets a number when it opens for review, and its
state says where the decision stands. Guidedog's own design records, GDS, are one such
catalog.

The commands are ``guidedog dsc init``, ``new``, ``list``, ``show``, ``promote``, ``state``, ``index``, ``check``, ``build``, ``recover``, and ``config``. They come from the discussion plugin, which every ``guidedog`` links, so there is nothing else to install. Each command works on the catalog the current folder is in, or the one ``--dir`` names. ``guidedog dsc --help`` lists the commands, and ``guidedog dsc build --help`` shows one command's options.

Guidedog's own catalog is no exception. We run the same commands in ``docs/gds``:

.. code-block:: sh

   cd docs/gds
   guidedog dsc check
   guidedog dsc index --check

Create a catalog
----------------

.. code-block:: sh

   guidedog dsc init docs/eds --prefix=eds --name="Example Discussions" --author="Ada"

The prefix names the catalog's fields, such as ``:eds-number:``. It is 2 to 16 lowercase
letters. ``init`` writes a complete catalog into a missing or empty folder:

* ``discussion.toml``, the catalog's prefix, name, default author, lifecycle, and
  numbering;
* ``conf.toml``, ``index.rst``, and ``workflow.rst``, an ordinary Guidedog project;
* ``template/rfc-template.rst``, the template of new drafts;
* ``_templates/discussion.typ`` and ``_templates/theme.typ``, the editable PDF layout and shared theme copied from the binary;
* ``registry.rst`` and ``bundle.rst``, the generated index;
* ``records/XXXXX-eds-process.rst``, a draft of our review process.

We edit the process draft to say how our project decides. To start from our own
documents, we pass ``--config=FILE`` (a ``discussion.toml``), ``--process=FILE`` (the
process text, from its title on), and ``--template=FILE`` (a record template with
``:eds-*:`` fields). ``--labels=a,b`` limits
the labels records may carry. ``init`` checks the new catalog before it moves it into
place. It never writes into a folder that has files.

Write and review
----------------

We run the commands in the catalog folder, or in any folder inside it.

.. code-block:: sh

   guidedog dsc new "Cache policy" --labels=architecture
   guidedog dsc list
   guidedog dsc show cache-policy
   guidedog dsc promote cache-policy
   guidedog dsc state 1 --to=accepted --reason="Review converged"
   guidedog dsc state 1 --to=committed --reason="Shipped" --evidence="Tests and review"

A new draft has the number ``XXXXX``. Promotion gives it the next number and opens it
for discussion. A state change needs ``--reason``, and ``committed`` also needs
``--evidence``. ``--dry-run`` shows a change without making it. These are the
defaults. The next section changes them.

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
may also carry ``category``, ``status``, and ``summary``. ``guidedog dsc`` keeps ``status`` in step
with the state. HTML and PDF show the fields as Number, Title, Authors, Updated, Labels,
and so on, with the lists as plain text.

Configure the lifecycle and numbering
-------------------------------------

``discussion.toml`` holds the catalog's settings. ``init`` writes every one of them,
with comments. ``guidedog dsc config`` prints the settings in effect. A catalog without the
file follows the defaults below. They are the process of Guidedog's own GDS.

.. code-block:: sh

   guidedog dsc config > discussion.toml.new && mv discussion.toml.new discussion.toml

.. code-block:: toml

   prefix = "eds"
   name = "Example Discussions"
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

``[workflow.states.NAME]`` defines a state. ``next`` lists the states the ``state``
command may move a record to. ``evidence = true`` asks for ``--evidence`` on a move into
the state. ``status`` is the text a record's optional ``status`` field takes there. A state
with no ``next`` is closed. Promotion is the only way from the draft state to the open
state, because promotion gives the number. Without a number, a draft may move only to the states
its own ``next`` lists, such as ``abandoned``.

Numbers have at least ``width`` digits, padded with zeros. A number that needs more
digits gets them, so 10000 follows 9999, and lists sort by value. Promotion numbers from
``start``. ``limit``, when set, is the highest number it gives. Changing ``start`` or
``limit`` later is safe. Changing ``width`` is not, once records are numbered: their
numbers would no longer match it.

``guidedog dsc`` checks the file whenever it reads the catalog. A mistake names the line and what
to write instead:

.. code-block:: text

   -- THE CATALOG CONFIGURATION IS NOT VALID [discussion.config] --- discussion.toml:16:1

   There is no setting numbering.digits.

   15| [numbering]
   16| digits = 4
       ^

   Hint: Use one of: width, start, limit.

The process draft that ``init`` writes describes the lifecycle we configured. It has a
table of the states and a diagram of the moves.

Build
-----

.. code-block:: sh

   guidedog dsc build
   guidedog dsc build cache-policy --target=pdf

``guidedog dsc build`` writes the website to ``_build/html`` and one PDF per record to
``_build/pdf``. Naming a record writes only its PDF, to ``_build/selected``, and leaves
the complete output alone. Warnings stop the build.

The build is guidedog's own: the same build as ``guidedog build``, with the catalog as the project. Graphviz, Typst, and the other tools it needs are guidedog's too. An older catalog of Typst records compiles each record with Typst; ``--typst=PATH`` names a Typst program to use instead of the embedded compiler.

Discussion PDFs
~~~~~~~~~~~~~~~

Each RST record uses an RFC-style paper layout. The record opens with a metadata panel, state badges, a short status note and compact contents, followed by its text. Numbered sections flow continuously. Running headers identify the catalog and record, and footers show the page number and total. Tables and captioned listings can continue across pages.

The PDF builder recognizes the leading metadata, such as ``:eds-number:`` and ``:eds-title:``, for any catalog prefix. Existing catalogs without a copied discussion template use the current embedded layout. We keep existing template customizations until we replace or remove their files. Ordinary books keep their book template.

The layout is embedded in the Guidedog binary. ``guidedog dsc init`` copies ``_templates/discussion.typ`` and its shared ``_templates/theme.typ`` for customization. Existing catalogs without a copy use the embedded layout. It defines a ``book`` function with the usual PDF parameters and an additional ``record`` dictionary. ``record.id`` is the readable identifier, ``record.catalog`` is the project name, and ``record.fields`` contains pairs of readable field labels and values. All values arrive as quoted strings. The source heading supplies the title and its anchor. The template reads the catalog metadata without assuming a fixed prefix or lifecycle. Its optional ``contents`` and ``memo`` parameters control the contents and status note. :doc:`../templates` describes the shared PDF parameters.

An ``--untrusted`` build uses the built-in discussion layout instead of running a project template. A record included as a chapter of a larger book keeps its metadata in that chapter; the root document selects the book's layout.

When an update is interrupted
-----------------------------

Commands that change records hold the catalog's lock, ``.eds.lock``. An update of several
files first writes a journal, ``.eds-journal.json``. If it stops part way, the next
command says so:

.. code-block:: sh

   guidedog dsc recover --rollback --dry-run
   guidedog dsc recover --rollback

Recovery puts back the files as they were. It refuses to overwrite a file someone edited
since. ``guidedog dsc`` also checks each change before it writes anything. It refuses an empty or cut-short
record, and an index without its generated region.

``guidedog dsc`` keeps only each record's metadata in memory. It reads a record whole only to
change it. So checking a large catalog stays fast and small.

Use the library
---------------

The commands are one Rust crate, ``gd-discussion``. Its ``README.md`` shows how
to load a catalog, plan a change, and apply it from our own program:

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

``ds::plugin()`` is the whole ``guidedog dsc`` command line, as a plugin. A ``guidedog`` of our own links it beside its other plugins (see :doc:`../plugins`). The design notes are in :doc:`../internals/discussion`.
