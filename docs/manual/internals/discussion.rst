Discussion catalogs (gd-discussion)
===================================

``crates/gd-discussion`` ports ``lib/discussion`` and its command line
(``lib/discussion/command``) from the Odin version. GDS 0007 remains the specification.
This page records how the Rust design differs and why.

Shape of the API
----------------

A catalog is loaded with ``load_catalog(dir, &prefix)``. It carries its folder, prefix,
records (metadata and which required sections each has, never the text), the registry's
reservation, its labels and its process. Changes are planned by ``new_plan``,
``promote_plan``, ``state_plan`` and ``index_plan``, each returning a ``Plan`` of
``Change { path, before: Option<String>, after: Option<String> }``. Nothing is written
until ``apply(&lock, &plan)``.

The single Odin ``Options`` struct became one request type per operation: ``NewDraft``,
``Decision`` (promotion and state changes), ``BuildOptions`` and ``Init``. ``Prefix`` is
a validated newtype, so an invalid prefix cannot reach the library past ``Prefix::new``.
``replace_field`` takes ``FieldValue::Text`` or ``FieldValue::List`` instead of a quoted
Typst literal. The writer chooses the record's notation (plain text and JSON arrays in
RST, literals and tuples in Typst).

Transactions
------------

``lock_catalog`` returns a ``Lock`` released on drop. ``apply`` and ``recover`` require it,
so an unlocked write does not compile. The lock is ``std::fs::File::try_lock`` (flock). A
process that starts a child holds copies of every descriptor between fork and exec, so a
just-released lock can look taken. ``lock_catalog`` retries for up to 200 ms before
reporting ``PREFIX.busy``.

``apply`` checks every change (safe path, unchanged original, whole record or balanced
index), writes the journal ``.PREFIX-journal.json`` in the Odin JSON form, then replaces
each file through a synced stage renamed into place. ``recover`` restores in reverse
order and refuses to overwrite a later edit. ``check_recovery`` is the dry run.
``initialize`` builds the whole catalog in a staging folder, loads and checks it, and
renames it into place. The stage is removed by ``Drop`` on any failure.

Memory
------

The Odin allocator *watch* and the ``discussion.memory`` problem are gone: Rust aborts
when allocation fails. The memory budget of loading stays. Each record is read through
a 64 KiB buffer. Lines are bounded at 4 KiB, metadata at 64 KiB per record and 256 MiB
per catalog. Reading stops once the metadata has ended and both required sections have been
seen. The Odin refusal sweeps became interruption sweeps: a test stops ``apply`` after
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

An older catalog of Typst records never reaches the builder. ``build`` compiles each record with gd-typst (``legacy.rs``: the embedded compiler, or the program ``--typst`` names) into the ``build`` folder beside the catalog, staged and themed as the Odin ``discussion/embedded`` did.

Command line
------------

The command line is a plugin of guidedog (``plugin()``, ID ``dsc``). It adds commands only, no build hooks: ``guidedog dsc init``, ``new``, ``list``, ``show``, ``promote``, ``state``, ``index``, ``check``, ``build``, ``recover`` and ``config``. Their declarations (summaries, help, options) let guidedog show the help and parse the command line around them; the plugin reads its own arguments. In Odin these were two programs, ``dsc`` and ``dscc``, and ``guidedog gds`` for Guidedog's own catalog with a fixed prefix and folder. Now there is one command for every catalog, and what was particular to GDS (its prefix, workflow, labels and numbering) is in ``docs/gds/discussion.toml``, as for any catalog.

The arguments are parsed by hand, with Elm-style usage reports: unknown options, colors, states, labels and records get a "Did you mean" suggestion from ``gd_report::suggest``. Reports are rendered by ``gd_report::render_text`` (or ``render_json`` with ``--diagnostics=json``) on the host's error stream; text from records is escaped with ``escape_terminal``. The tests run the commands through the plugin with a host over buffers.

Not ported
----------

* The crash reporter of the Odin mains: guidedog's own crash report covers the commands.
