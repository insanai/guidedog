Internals
=========

These chapters explain the reasoning behind the implementation. They describe the
current contracts of the Rust crates. GDS records keep the decisions and review states
in a separate project. Each crate's ``README.md`` is the short API cheat sheet. The
pages here hold what does not fit there: invariants, designs, wire formats, and
conformance evidence.

.. toctree::
   :maxdepth: 1
   :caption: Engine and invariants

   engine
   algorithms
   publication
   memory
   testing
   ast

.. toctree::
   :maxdepth: 1
   :caption: Readers and conformance

   rst-conformance
   rst-ledger
   rst-inline-ledger
   commonmark-conformance
   commonmark-ledger
   cpp-declarations
   sphinx-ledger
   sphinx-comparison
   rust-domain

.. toctree::
   :maxdepth: 1
   :caption: Project builder

   project
   incremental

.. toctree::
   :maxdepth: 1
   :caption: Supporting libraries

   crates
   jinja
   gettext
   plugins
   discussion

.. toctree::
   :maxdepth: 1
   :caption: Porting reference

   engine-api-map

:doc:`engine-api-map` is kept from the port of Guidedog from Odin to Rust. It maps every
name of the Odin engine to its Rust form, for readers of the GDS records and of code
written against the Odin API.
