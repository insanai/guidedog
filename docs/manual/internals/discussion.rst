Discussion catalogs (gd-discussion)
===================================

``crates/gd-discussion`` manages discussion catalogs and holds their command line. GDS 0007 is the specification. This page records the design of the crate and the reasons for it.

Shape of the API
----------------

A catalog is loaded with ``load_catalog(dir, &prefix)``. It carries its folder, prefix,
records (metadata and which required sections each has, never the text), the registry's
reservation, its labels and its process. Changes are planned by ``new_plan``,
``promote_plan``, ``state_plan`` and ``index_plan``, each returning a ``Plan`` of
``Change { path, before: Option<String>, after: Option<String> }``. Nothing is written
until ``apply(&lock, &plan)``.

Each operation has its own request type: ``NewDraft``,
``Decision`` (promotion and state changes), ``BuildOptions`` and ``Init``. ``Prefix`` is
a validated newtype, so an invalid prefix cannot reach the library past ``Prefix::new``.
``replace_field`` takes ``FieldValue::Text`` or ``FieldValue::List``, never a literal of the record's notation. The writer chooses the record's notation (plain text and JSON arrays in
RST, literals and tuples in Typst).

Transactions
------------

``lock_catalog`` returns a ``Lock`` released on drop. ``apply`` and ``recover`` require it,
so an unlocked write does not compile. The lock is ``std::fs::File::try_lock`` (flock). A
process that starts a child holds copies of every descriptor between fork and exec, so a
just-released lock can look taken. ``lock_catalog`` retries for up to 200 ms before
reporting ``PREFIX.busy``.

``apply`` checks every change (safe path, unchanged original, whole record or balanced
index), writes the journal ``.PREFIX-journal.json``, then replaces
each file through a synced stage renamed into place. ``recover`` restores in reverse
order and refuses to overwrite a later edit. ``check_recovery`` is the dry run.
``initialize`` builds the whole catalog in a staging folder, loads and checks it, and
renames it into place. The stage is removed by ``Drop`` on any failure.

Memory
------

Allocation failure aborts the process, as everywhere in Rust, so no problem reports it. Loading has a memory budget. Each record is read through
a 64 KiB buffer. Lines are bounded at 4 KiB, metadata at 64 KiB per record and 256 MiB
per catalog. Reading stops once the metadata has ended and both required sections have been
seen. Interruption sweeps test the transactions: a test stops ``apply`` after
every possible file and checks that ``recover`` restores the catalog byte for byte.

Building
--------

``build`` checks the catalog and hands each target to a ``Builder``:

.. code-block:: rust
   :class: sketch

   pub trait Builder {
       fn build(&mut self, request: &BuildRequest) -> Result<PathBuf>;
   }

A request names the catalog, the target, the record books and, for a preview, the one record; ``BuildRequest::output()`` is the folder it goes to. The plugin's builder hands the request to its host: ``CommandContext::build_project`` builds the catalog as a Guidedog project, strict, with the books as the ``pdf_documents`` setting. The trait takes ``&mut self`` because the host's context is borrowed mutably for the build. Tests implement it with a recorder.

An older catalog of Typst records never reaches the builder. ``build`` compiles each record with gd-typst (``legacy.rs``: the embedded compiler, or the program ``--typst`` names) into the ``build`` folder beside the catalog, staged and themed.

For an RST record, the PDF composer and link pass share ``gd_link::discussion_record`` as their metadata contract. A root with a number and title selects ``discussion.typ``. The composer passes its front matter as quoted data; the link pass keeps the title heading and removes the redundant metadata field list from the body. Both full-catalog and selected-record builds follow this path. A record nested in an ordinary book remains a chapter, with its metadata in place.

Command line
------------

The command line is a plugin of guidedog (``plugin()``, ID ``dsc``). It adds commands only, no build hooks: ``guidedog dsc init``, ``new``, ``list``, ``show``, ``promote``, ``state``, ``index``, ``check``, ``build``, ``recover`` and ``config``. Their declarations (summaries, help, options) let guidedog show the help and parse the command line around them; the plugin reads its own arguments. There is one command for every catalog, and what is particular to GDS (its prefix, workflow, labels and numbering) is in ``docs/gds/discussion.toml``, as for any catalog.

The arguments are parsed by hand, with Elm-style usage reports: unknown options, colors, states, labels and records get a "Did you mean" suggestion from ``gd_report::suggest``. Reports are rendered by ``gd_report::render_text`` (or ``render_json`` with ``--diagnostics=json``) on the host's error stream; text from records is escaped with ``escape_terminal``. The tests run the commands through the plugin with a host over buffers.

The commands have no crash reporter of their own: guidedog's crash report covers them.
