Domains
=======

A domain is a collection of markup constructs (directives and roles) for describing
and cross-referencing program objects belonging to a specific programming language
or domain of discourse.

Guidedog provides native, built-in implementations of the standard Sphinx domains:

* **Standard domain** (``std``): General documentation objects such as command-line options, environment variables, terms, and UI labels.
* **Python domain** (``py``): Python modules, classes, functions, methods, attributes, properties, and exceptions.
* **C domain** (``c``): C functions, types, structs, unions, enums, enumerators, variables, and macros.
* **C++ domain** (``cpp``): C++ classes, concepts, templates, namespaces, methods, and expressions.
* **JavaScript domain** (``js``): JavaScript functions, methods, classes, and properties.
* **Odin domain** (``odin``): Native Odin packages, procedures, structs, unions, and enums (see :doc:`../odin`).
* **Rust domain** (``rust``): Rust crates, modules, functions, structs, enums, traits, impls, and macros (see `Rust domain`_ below).

Setting the primary domain
--------------------------

The default domain is set with ``primary_domain`` in ``conf.toml`` (default: ``"py"``).
When a primary domain is active, its directives and roles can be written without the
domain prefix (for example, ``.. function::`` instead of ``.. py:function::``, and
``:func:`name``` instead of ``:py:func:`name```).

To change the active domain locally within a document:

.. code-block:: rst

   .. default-domain:: c

Standard domain
---------------

The standard domain describes command-line tools, environment variables, and document
cross-references.

Directives
~~~~~~~~~~

.. code-block:: rst

   .. program:: mytool

   .. option:: -c <config>, --config <config>

      Path to the configuration file.

   .. envvar:: GUIDEDOG_THEME

      Specifies the default theme stylesheet.

Roles
~~~~~

* ``:doc:`path```: Links to a project document by relative path.
* ``:ref:`label```: Links to an explicit target label (``.. _label:``).
* ``:term:`term```: Links to a term defined in a ``glossary`` directive.
* ``:option:`--config```: Links to a command-line option described by ``option``.
* ``:envvar:`VARIABLE```: Links to an environment variable described by ``envvar``.
* ``:command:`name```: Marks a system command name.
* ``:file:`path```: Marks a file or directory path, supporting ``{variable}`` placeholders.
* ``:kbd:`Ctrl+C```: Marks keyboard keystrokes.
* ``:menuselection:`File --> Save As```: Formats a software menu sequence.
* ``:guilabel:`Submit```: Formats a button or user interface element.
* ``:pep:`8```: Links to a Python Enhancement Proposal.
* ``:rfc:`7231```: Links to an IETF Request for Comments.

Python domain
-------------

The Python domain describes Python interfaces and signatures.

Directives
~~~~~~~~~~

.. code-block:: rst

   .. py:module:: pipeline.reader
      :synopsis: Document reading and normalization.

   .. py:class:: DocumentReader(source_path: str, encoding: str = "utf-8")

      Base class for document readers.

      .. py:method:: parse(content: bytes) -> Document

         Parses raw bytes into a document tree.

         :param content: Raw source content bytes.
         :return: Parsed document tree instance.
         :raises ValueError: If the source format is unrecognized.

      .. py:attribute:: encoding
         :type: str

         The character encoding used for text decoding.

Directives include: ``py:module``, ``py:currentmodule``, ``py:function``, ``py:data``,
``py:class``, ``py:method``, ``py:staticmethod``, ``py:classmethod``, ``py:attribute``,
``py:property``, ``py:exception``, ``py:decorator``, and ``py:type``.

Roles
~~~~~

* ``:py:func:`name```: Links to a Python function.
* ``:py:class:`name```: Links to a Python class.
* ``:py:meth:`name```: Links to a Python class or instance method.
* ``:py:attr:`name```: Links to a Python attribute.
* ``:py:data:`name```: Links to Python module-level data.
* ``:py:exc:`name```: Links to a Python exception class.
* ``:py:mod:`name```: Links to a Python module.

C domain
--------

The C domain describes C language libraries and headers.

Directives
~~~~~~~~~~

.. code-block:: rst

   .. c:namespace:: gd

   .. c:struct:: Workspace

      Reusable conversion state; its buffers are kept between conversions.

      .. c:member:: size_t capacity

         Total byte size of the output buffer.

   .. c:function:: int gd_convert(Workspace *ws, const char *input, char *output, size_t out_len)

      Converts source text into HTML.

      :param ws: Pointer to the initialized workspace.
      :param input: Null-terminated input string.
      :param output: Destination buffer.
      :param out_len: Size of destination buffer.
      :returns: 0 on success, or an error code.

Roles
~~~~~

* ``:c:func:`name```: Links to a C function.
* ``:c:member:`name```: Links to a C struct or union member.
* ``:c:data:`name``` / ``:c:var:`name```: Links to a C variable.
* ``:c:type:`name```: Links to a C typedef or type.
* ``:c:struct:`name```: Links to a C struct.
* ``:c:union:`name```: Links to a C union.
* ``:c:enum:`name```: Links to a C enum.
* ``:c:macro:`name```: Links to a C preprocessor macro.

C++ domain
----------

The C++ domain supports modern C++ constructs, concepts, namespaces, and scopes.

Directives
~~~~~~~~~~

.. code-block:: rst

   .. cpp:namespace:: guidedog

   .. cpp:concept:: template<typename T> Reader

      Specifies requirements for document reader types.

   .. cpp:class:: template<typename Allocator> Builder

      Constructs AST document nodes.

      .. cpp:function:: NodeId add_text(std::string_view text)

         Appends text to the active node.

Roles
~~~~~

* ``:cpp:class:`name```: Links to a C++ class or struct.
* ``:cpp:func:`name```: Links to a C++ function or method.
* ``:cpp:member:`name``` / ``:cpp:var:`name```: Links to a member or variable.
* ``:cpp:type:`name```: Links to a C++ type alias or typedef.
* ``:cpp:concept:`name```: Links to a C++20 concept.
* ``:cpp:enum:`name```: Links to a C++ enum.

JavaScript domain
-----------------

The JavaScript domain describes client-side scripts, libraries, and Node.js modules.

Directives
~~~~~~~~~~

.. code-block:: rst

   .. js:module:: theme

   .. js:class:: ThemeController(options)

      Manages color themes and local storage persistence.

      .. js:method:: toggleTheme()

         Toggles between light and dark modes.

Roles
~~~~~

* ``:js:func:`name```: Links to a JavaScript function.
* ``:js:meth:`name```: Links to a JavaScript method.
* ``:js:class:`name```: Links to a JavaScript class.
* ``:js:data:`name```: Links to a JavaScript variable.
* ``:js:attr:`name```: Links to a JavaScript object attribute.
* ``:js:mod:`name```: Links to a JavaScript module.

Rust domain
-----------

The Rust domain describes crates as rustdoc presents them. Each directive takes the
declaration as Rust writes it, on one line, visibility and qualifiers included. Targets
are full paths with ``::``, the crate's name first.

Directives
~~~~~~~~~~

.. code-block:: rst

   .. rust:module:: my_crate::geometry
      :synopsis: Points and shapes.

   .. rust:struct:: pub struct Point<T = f32>

      A point in the plane.

      .. rust:field:: pub x: T

         The horizontal coordinate.

      .. rust:impl:: impl<T: Copy> Point<T>

         .. rust:method:: pub fn new(x: T, y: T) -> Self

            Makes a point.

   .. rust:enum:: pub enum Shape

      .. rust:variant:: Circle(f32)

   .. rust:trait:: pub trait Draw: Clone + Send

      .. rust:method:: fn draw(&self) -> u8
         :provided:

Directives include: ``rust:crate``, ``rust:module``, ``rust:function``, ``rust:method``,
``rust:struct``, ``rust:enum``, ``rust:variant``, ``rust:union``, ``rust:trait``,
``rust:impl``, ``rust:type``, ``rust:const``, ``rust:static``, ``rust:macro``,
``rust:field``, ``rust:associatedtype``, ``rust:associatedconst``, and ``rust:use``.
Options include ``:visibility:``, ``:cfg:``, ``:unsafe:``, ``:async:``, ``:const:``,
``:provided:``, ``:attributes:``, ``:deprecated:``, ``:for:``, ``:trait:``, ``:target:``,
and ``:noindex:``. An ``impl`` is never a target itself; its members belong to the type.

Guidedog can also generate these pages from a crate's source, without compiling it;
see :doc:`../howto/document-code` and ``rust_autoapi_crates`` in
:doc:`../reference/configuration`.

Roles
~~~~~

* ``:rust:mod:`path```: Links to a crate or module.
* ``:rust:fn:`path```: Links to a function or method.
* ``:rust:struct:`path```, ``:rust:enum:`path```, ``:rust:union:`path```,
  ``:rust:trait:`path```: Link to those types.
* ``:rust:variant:`path```, ``:rust:field:`path```: Link to a variant or a field.
* ``:rust:type:`path```, ``:rust:const:`path```, ``:rust:static:`path```,
  ``:rust:macro:`path```: Link to aliases and associated types, constants, statics, and
  macros.
* ``:rust:any:`path```: Links to any of them, or through an inventory to another
  project (``std::vec::Vec``); an unresolved target is shown as code.

Cross-referencing conventions
-----------------------------

Cross-references in domains support several helpful modifiers:

* **Hide prefix**: Prefixing the target with a tilde (``~``) displays only the final component of the target name. For example, ``:py:func:`~pipeline.reader.DocumentReader.parse``` renders as ``parse()``.
* **Current scope resolution**: Relative references look up names within the active module or namespace before falling back to global lookup.
* **Exact match**: Prefixing with a dot (``.``) searches from the current module or enclosing object.

The Rust domain follows the same conventions with ``::`` paths:
``:rust:struct:`~my_crate::geometry::Point``` renders as ``Point``.
