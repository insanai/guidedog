Discussion catalogs (gd-discussion)
===================================

``crates/gd-discussion`` ports ``lib/discussion`` and its command line
(``lib/discussion/command``) from the Odin version. GDS 0007 remains the specification;
this page records how the Rust design differs and why.

Shape of the API
----------------

A catalog is loaded with ``load_catalog(dir, &prefix)``; it carries its folder, prefix,
records (metadata and which required sections each has, never the text), the registry's
reservation, its labels and its process. Changes are planned by ``new_plan``,
``promote_plan``, ``state_plan`` and ``index_plan``, each returning a ``Plan`` of
``Change { path, before: Option<String>, after: Option<String> }``. Nothing is written
until ``apply(&lock, &plan)``.

The single Odin ``Options`` struct became one request type per operation: ``NewDraft``,
``Decision`` (promotion and state changes), ``BuildOptions`` and ``Init``. ``Prefix`` is
a validated newtype, so an invalid prefix cannot reach the library past ``Prefix::new``.
``replace_field`` takes ``FieldValue::Text`` or ``FieldValue::List`` instead of a quoted
Typst literal; the writer chooses the record's notation (plain text and JSON arrays in
RST, literals and tuples in Typst).

Transactions
------------

``lock_catalog`` returns a ``Lock`` released on drop; ``apply`` and ``recover`` require it,
so an unlocked write does not compile. The lock is ``std::fs::File::try_lock`` (flock). A
process that starts a child holds copies of every descriptor between fork and exec, so a
just-released lock can look taken; ``lock_catalog`` retries for up to 200 ms before
reporting ``PREFIX.busy``.

``apply`` checks every change (safe path, unchanged original, whole record or balanced
index), writes the journal ``.PREFIX-journal.json`` in the Odin JSON form, then replaces
each file through a synced stage renamed into place. ``recover`` restores in reverse
order and refuses to overwrite a later edit; ``check_recovery`` is the dry run.
``initialize`` builds the whole catalog in a staging folder, loads and checks it, and
renames it into place; the stage is removed by ``Drop`` on any failure.

Memory
------

The Odin allocator *watch* and the ``discussion.memory`` problem are gone: Rust aborts
when allocation fails. The memory budget of loading stays: each record is read through
a 64 KiB buffer, lines are bounded at 4 KiB, metadata at 64 KiB per record and 256 MiB
per catalog, and reading stops once the metadata ended and both required sections were
seen. The Odin refusal sweeps became interruption sweeps: a test stops ``apply`` after
every possible file and checks that ``recover`` restores the catalog byte for byte.

Building
--------

``build`` checks the catalog and hands each target to a ``Builder``:

.. code-block:: rust

   pub trait Builder {
       fn name(&self) -> &str;
       fn build(&self, request: &BuildRequest) -> Result<PathBuf>;
       fn build_typst_catalog(&self, catalog: &Catalog, selected: &[&Record],
           targets: &[Target]) -> Result<Vec<PathBuf>>; // default: legacy-build problem
   }

``GuidedogProgram`` runs the installed ``guidedog`` without a shell. ``dscc`` will
implement the trait over gd-project's in-process builder (and Typst for older catalogs);
until gd-project is ported it uses ``GuidedogProgram``. The builder's own program
(``--guidedog`` or ``--typst``) is given to ``Program::make_builder`` rather than carried
in every request.

Command line
------------

``command::run(&Program, &args)`` is hand-written argument parsing with Elm-style usage
reports: unknown commands, options, colors, states, labels and records get a
"Did you mean" suggestion from ``gd_report::suggest``. Reports are rendered by
``gd_report::render_text`` (or ``render_json`` with ``--diagnostics=json``); text from
records is escaped with ``escape_terminal``. ``run_with`` takes the output streams so the
command line is tested end to end.

Not ported
----------

* Building older Typst catalogs and their legacy HTML (``embedded/legacy*.odin``): they
  need the embedded Typst compiler, and belong to the dscc builder.
* The crash reporter of the Odin mains (``host.install_crash_reporter``).
