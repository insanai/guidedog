Linking to other projects
=========================

With ``sphinx.ext.intersphinx``, references this project cannot resolve are looked up
in other projects' inventories. An inventory is the ``objects.inv`` file that every Sphinx or Guidedog site
publishes. We name the projects in ``conf.toml``:

.. code-block:: toml

   extensions = ["sphinx.ext.intersphinx"]

   [intersphinx_mapping]
   python = ["https://docs.python.org/3/", ""]
   click = ["https://click.palletsprojects.com/", ["click.inv", ""]]

Each entry is the address of the other project's pages and where its inventory is: a
URL, a file relative to the source folder, or ``""`` for the ``objects.inv`` at the
address. A list is tried in order. This is Sphinx's ``intersphinx_mapping``, with ``""``
for Python's ``None``. ``guidedog migrate`` converts it.

Then references need nothing new:

.. code-block:: rst

   :func:`len`, :class:`list`, and :ref:`tut-packages` link into Python's documentation.
   :ref:`click:testing` looks only in Click's inventory.
   :external+python:ref:`tut-packages` never looks in this project.

Links show the other project's titles for labels and documents, and carry a tooltip
such as "(in Python v3.14)". As in Sphinx, ``:doc:`` references need the project's
name (``:doc:`python:tutorial/index```), because ``intersphinx_disabled_reftypes`` is
``["std:doc"]`` by default.

Inventories are fetched when a build starts and kept in
``_build/.doctrees/__intersphinx_cache__`` for ``intersphinx_cache_limit`` days (5; a
negative number keeps them). Local files are read on every build. As in Sphinx, an
inventory may have moved: its URL redirects, as
``https://werkzeug.palletsprojects.com/objects.inv`` redirects to ``/en/stable/``. Then its new folder
becomes the base of the links. The cache remembers that base, so a build
from the cache links exactly as the build that fetched it did.
``intersphinx_timeout`` limits each download in seconds. A project that cannot be
reached is a warning, and its references stay unresolved.

Guidedog writes its own ``objects.inv`` with every page, label, term, and object, so
other projects can link here the same way.
