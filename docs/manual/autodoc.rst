Documenting Python with autodoc
===============================

Guidedog documents Python by reading its source. It does not import a package.
Docstrings and signatures become part of the document graph. Objects created only
at runtime need an explicit description. This is a deliberate boundary.

Setting it up
-------------

We list the extension and tell Guidedog where our packages are, relative to the source
folder, as ``sys.path`` tells Python:

.. code-block:: toml

   extensions = ["sphinx.ext.autodoc"]
   autodoc_source_paths = ["../src"]

``guidedog migrate`` writes ``autodoc_source_paths`` for us. It reads the lines that
``conf.py`` files use to reach their package, such as
``sys.path.insert(0, os.path.abspath('..'))``. A package installed from elsewhere is
found only if we add the folder that holds it, for example a source checkout of a
dependency. Guidedog never looks in Python's own ``site-packages``.

The directives then work as in Sphinx:

.. code-block:: rst

   .. automodule:: shop.cart
      :members:
      :show-inheritance:

   .. autoclass:: shop.Cart
      :members: add, remove
      :inherited-members:

Every option Sphinx 9.1 accepts is accepted, with the same meaning:
``:members:``, ``:undoc-members:``, ``:private-members:``, ``:special-members:``,
``:inherited-members:``, ``:exclude-members:``, ``:member-order:``,
``:show-inheritance:``, ``:imported-members:``, ``:ignore-module-all:``,
``:class-doc-from:``, ``:no-value:``, ``:annotation:``, ``:no-index:``,
``:no-index-entry:``, ``:synopsis:``, ``:platform:``, ``:deprecated:``, and the ``no-``
forms that cancel ``autodoc_default_options``.

Settings
--------

These ``conf.toml`` settings have Sphinx's names, values, and defaults.

``autodoc_source_paths``
   Guidedog's own: the folders Python modules are found in, in order, relative to the
   source folder. Only files below them are read, and a path through a symbolic link
   that leads elsewhere is refused.

``autoclass_content``
   ``"class"`` (default), ``"init"``, or ``"both"``: which docstrings describe a class.

``autodoc_class_signature``
   ``"mixed"`` (default) or ``"separated"``, which documents ``__init__`` as a method.

``autodoc_default_options``
   A table of options every directive gets, such as ``members = true`` or
   ``member-order = "bysource"``. A value of ``false`` leaves the option out.

``autodoc_docstring_signature``
   ``true`` (default): a first docstring line like ``name(args) -> result`` is the
   signature.

``autodoc_inherit_docstrings``
   ``true`` (default): a member without a docstring takes its base class's.

``autodoc_member_order``
   ``"alphabetical"`` (default), ``"groupwise"``, or ``"bysource"``.

``autodoc_preserve_defaults``
   ``false`` (default): defaults are shown as ``repr()`` would write them. ``true``
   shows them as written.

``autodoc_typehints``
   ``"signature"`` (default), ``"description"`` (as ``:type:`` and ``:rtype:`` fields),
   ``"both"``, or ``"none"``.

``autodoc_typehints_description_target``
   ``"all"`` (default), ``"documented"``, or ``"documented_params"``.

``autodoc_typehints_format``
   ``"short"`` (default) or ``"fully-qualified"``.

``autodoc_use_type_comments``
   ``true`` (default): ``# type:`` comments count as annotations.

``strip_signature_backslash``
   Doubles backslashes in signatures, as in Sphinx.

``autodoc_mock_imports``, ``autodoc_type_aliases``, ``autodoc_warningiserror``
   Accepted. Nothing is imported, so nothing needs mocking. Type aliases are not
   applied.

How Guidedog knows what Python would do
---------------------------------------

Names are followed as Python binds them. ``from .app import Flask`` in a package's
``__init__.py`` makes ``flask.Flask`` the class defined in ``flask/app.py``.
Autodoc then shows it with ``:canonical: flask.app.Flask``. Base classes are resolved the
same way. Their method resolution order is computed as Python computes it, and
inherited members come from the bases' source. Names imported only under
``if TYPE_CHECKING:`` do not exist when the module runs, so annotations naming them stay
as written, as they do in Sphinx.

Docstrings are the strings Python would store: escapes processed and indentation
removed as Python 3.13's compiler removes it. Attribute documentation comes from the
places Sphinx's analyser reads: a ``#:`` comment after the assignment or on the lines
above it, and a string literal after it. A function's signature comes from its ``def``.
A class's signature comes from its metaclass ``__call__``, ``__new__``, or ``__init__``, as Python finds it.
A dataclass's or ``NamedTuple``'s signature comes from its fields. ``@overload`` variants replace the
implementation's signature.

What cannot be known without running Python
-------------------------------------------

Guidedog says so, naming the object, whenever the source cannot tell it what Python
would. It gives a warning when something cannot be documented. It gives a note (shown with ``-v``) when
it is documented with less than Sphinx would show.

- A compiled module (``.so``, ``.pyd``) has no source to read: its objects are not
  found.
- A value made by running code (``app = Flask(__name__)``, ``now = datetime.now()``)
  has no ``:value:``, and a computed default is shown as written.
- A function made by a decorator from outside the source paths (``@click.command()``)
  is documented as the function it decorates.
- A class from a package outside the source paths is named by the path it was imported
  from, which may differ from the module that defines it. Its members, its
  constructor's signature, and the docstrings it would pass on are unknown.
- Docstrings of builtin types are unknown, so a method that has none and would inherit
  one from ``dict`` is left out.
- Python extensions that handle autodoc's events (``autodoc-process-docstring``,
  ``autodoc-skip-member``, ...) do not run.
- A module nested far deeper than real code (Python itself refuses 200 nested brackets;
  Guidedog also refuses a ``+`` chain of a few hundred operands) is not analysed, with
  the warning ``autodoc.too_deep``.
- Work that multiplies is bounded. A name followed through imports and aliases stops
  after 48 steps along one path and 100,000 in all. A type alias or constant is expanded
  for 10,000 steps and is otherwise shown as written. A class's MRO is followed 100
  classes deep. One directive generates at most 10,000 directives (the warning
  ``autodoc.limit``); only a class that contains itself under its own name reaches this.

Adding the missing packages' source folders to ``autodoc_source_paths`` closes most of
these.

Rebuilding
----------

Every Python file an autodoc directive read is a dependency of the page. When we edit it,
the next build reads the page again. A page whose object was not found is read again
on every build, so it picks the object up as soon as the source has it.

Security
--------

Building documentation never runs the project's code. Guidedog reads Python source as
text, with the same confinement it applies to includes: only below
``autodoc_source_paths``, and not through links leading elsewhere.
