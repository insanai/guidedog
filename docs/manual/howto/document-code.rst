Document a codebase
===================

A public name needs a contract. We explain its input, its result, its ownership,
and the conditions under which it fails. A signature alone is not a manual.

Static extraction architecture
------------------------------

Guidedog extracts API documentation by parsing source files into abstract
syntax trees. It never runs or imports our code. So there are no side effects,
no runtime to install, and no foreign dependencies to resolve:

.. graphviz::
   :class: graphviz-adaptive
   :caption: Guidedog static code documentation architecture.
   :alt: Code source trees are parsed into ASTs statically, transformed into virtual API documents, and integrated into cross-references and objects.inv.

   digraph code_doc_arch {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.4", ranksep="0.4"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.2,0.12"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10];

     subgraph cluster_sources {
       label="Source Trees (No Execution)";
       fontname="Helvetica-Bold";
       fontsize=12;
       fontcolor="#16382f";
       style="rounded,dashed";
       color="#216553";

       rust_src [class="gd-input", label=<<B>Rust Crates</B><BR/><FONT POINT-SIZE="11">(rust_autoapi_crates)</FONT>>, shape=note, fillcolor="#fbfaf3"];
       odin_src [class="gd-input", label=<<B>Odin Packages</B><BR/><FONT POINT-SIZE="11">(odin_autoapi_dirs)</FONT>>, shape=note, fillcolor="#fbfaf3"];
       py_src [class="gd-input", label=<<B>Python Modules</B><BR/><FONT POINT-SIZE="11">(autodoc_source_paths)</FONT>>, shape=note, fillcolor="#fbfaf3"];
     }

     subgraph cluster_parsers {
       label="Static AST Parsers";
       fontname="Helvetica-Bold";
       fontsize=12;
       fontcolor="#16382f";
       style="rounded,dashed";
       color="#216553";

       rust_parser [label=<<B>tree-sitter-rust + gd-rustdoc</B><BR/><FONT POINT-SIZE="11">Module tree + doc comments</FONT>>, fillcolor="#edf5f2"];
       odin_parser [label=<<B>gd-odindoc</B><BR/><FONT POINT-SIZE="11">Declarations + comments</FONT>>, fillcolor="#edf5f2"];
       py_parser [label=<<B>tree-sitter + pyscan</B><BR/><FONT POINT-SIZE="11">Declarations + docstrings</FONT>>, fillcolor="#edf5f2"];
     }

     virtual_docs [class="gd-input", label=<<B>Virtual API Doctrees</B><BR/><FONT POINT-SIZE="11">(api/*.rst in memory)</FONT>>, fillcolor="#fbfaf3"];
     doc_graph [label=<<B>Documentation Graph</B><BR/><FONT POINT-SIZE="11">Toctrees + references</FONT>>, fillcolor="#edf5f2"];
     inventory [label=<<B>objects.inv</B><BR/><FONT POINT-SIZE="11">Cross-project object catalog</FONT>>, shape=cylinder, fillcolor="#e0eee9"];

     rust_src -> rust_parser [label="reads"];
     odin_src -> odin_parser [label="reads"];
     py_src -> py_parser [label="reads"];
     rust_parser -> virtual_docs [label="generates"];
     odin_parser -> virtual_docs [label="generates"];
     py_parser -> virtual_docs [label="generates"];
     virtual_docs -> doc_graph [label="integrates"];
     doc_graph -> inventory [label="exports"];
   }

Rust
----

.. code-block:: toml

   rust_autoapi_crates = ["../../crates/gd-json", "../../crates/gd-toml"]
   rust_autoapi_root = "rustapi"
   rust_autoapi_member_order = "source"

Guidedog reads each crate's source with tree-sitter and follows its module tree the way
rustc does: ``Cargo.toml``'s ``[lib] path``, then ``src/lib.rs`` or ``src/main.rs``.
It does not compile, run, or expand the documented crate. Items made by macros or
build scripts are therefore absent. A ``rust_autoapi_crates`` entry may also be a single root
``.rs`` file or a folder of loose ``.rs`` files.

Rustdoc's rules decide what appears. A ``pub`` item reachable through ``pub`` modules is
documented in its module. ``pub use`` of an item from a private module documents it
under the re-exported name. ``#[doc(hidden)]`` items, ``#[cfg(test)]`` modules and
tests are left out. ``rust_autoapi_options`` adds ``private-members`` (everything, with
its visibility shown), ``hidden-members``, or ``noindex``.

Doc comments are CommonMark. Guidedog translates them into the page. ``# Examples`` and
the other headings become rubrics. Fenced code follows rustdoc's language rules.
Intra-doc links (``[`Parser`]``, ``[Parser::next]``) become ``rust`` domain references.
A link into the crate that names nothing is a ``rust.autodoc.link`` warning.

The pages are written under ``rust_autoapi_root``. There is an index page, then one page per
documented module (``my_crate.rst``, ``my_crate/parser.rst``). With ``rust_autoapi_add_toctree_entry``
(the default), the index joins the root document's toctree. With
``rust_autoapi_generate_api_docs = false``, Guidedog reads the settings without generating pages.
:doc:`../topics/domains` describes the ``rust`` directives and roles. Hand-written
pages can use them too.

Odin
----

.. code-block:: toml

   odin_autoapi_dirs = ["../../lib"]
   odin_autoapi_root = "api"
   odin_autoapi_options = ["members", "undoc-members"]

Guidedog reads Odin source by static analysis, across every build target at once.
It does not compile or run the documented package.
The generated pages join the document graph and object inventory.
See :doc:`../odin` for directives, signatures, and options.

Python
------

.. code-block:: toml

   extensions = ["sphinx.ext.autodoc"]
   autodoc_source_paths = ["../src"]

Guidedog parses Python source rather than importing it.
This avoids import side effects. It also means runtime-generated objects may be absent.
When such objects are part of the public interface, we describe them explicitly.
See :doc:`../autodoc` for the exact discovery rules.

Link across projects
--------------------

We publish the generated ``objects.inv`` with the site.
An intersphinx mapping in another project can then resolve our names.
Explicit mappings keep builds reproducible.
See :doc:`../intersphinx`.
