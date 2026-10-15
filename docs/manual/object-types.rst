Declaring object types
======================

A project may need names that the standard domains do not know. We declare those
object types in TOML. Guidedog can then create targets, index entries, and roles
without loading a Python extension. The declaration states a syntax, not executable behavior.

Targets: ``add_crossref_type``
------------------------------

.. code-block:: toml

   [[crossref_types]]
   directive = "setting"
   role = "setting"
   index = "pair: %s; setting"

``.. setting:: DEBUG`` then makes a target with the id ``setting-DEBUG`` and an index
entry, and ``:setting:`DEBUG``` links to it, as in Sphinx. ``index`` is Sphinx's
``indextemplate``: its type (``single``, ``pair``, ``triple``, ``see``, ``seealso``)
before the first colon, and the entry after it, with ``%s`` the name.

Descriptions: ``add_object_type``
---------------------------------

.. code-block:: toml

   [[object_types]]
   directive = "django-admin"
   role = "djadmin"
   index = "pair: %s; django-admin command"
   name = "first-word"
   display = "django-admin %s"
   program = true

``.. django-admin:: migrate [app_label]`` then describes the command like any object:
a signature, content, an id (``django-admin-migrate``), and an index entry. In Sphinx a
``parse_node`` function may read the signature. Here, fields set its usual work
instead:

``name``
   ``"whole"`` (the default): the object's name is its whole signature.
   ``"first-word"``: the name is the signature up to its first space
   (``pdb``'s ``b(reak) [lineno]`` is named ``b(reak)``).

``display``
   How the signature is shown, ``%s`` standing for it. By default, as written.

``program``
   ``true``: the name becomes the program that options described after it belong to,
   as ``.. program::`` makes it.

Other names for directives
--------------------------

An extension may register one of Sphinx's own directives under another name:

.. code-block:: toml

   [directive_aliases]
   django-admin-option = "option"
   awaitablefunction = "py:function"

A target without a domain is a standard directive, or one of the default domain's; a
target such as ``py:function`` names its domain.

Migrating
---------

``guidedog migrate`` finds each extension listed in ``conf.py`` that is a module of the
project. It reads its ``add_crossref_type``, ``add_object_type``, and ``add_directive``
calls without running them, and writes these tables. It reports what it cannot
declare: a ``parse_node`` function (set ``name``, ``display``, and ``program`` to match
it) and directives, roles, and event handlers written in Python.
