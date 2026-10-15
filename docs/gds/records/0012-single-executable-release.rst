:gds-number: 0012
:gds-title: Single executable release
:gds-state: committed
:gds-type: Standards Track
:gds-authors: ["Vikrant Rathore"]
:gds-created: 2026-10-15
:gds-updated: 2026-10-15
:gds-discussion: No external discussion URL assigned
:gds-labels: ["architecture","release"]

Single executable release
=========================

Abstract
--------

The release contains one executable, ``guidedog``. Libraries and linked plugins provide the rest. ``gd-graphdog`` encapsulates Graphviz. The Pages plugin adds publishing and maintenance commands. The discussion plugin adds ``guidedog dsc``.

Motivation and scope
--------------------

The source tree and documentation still presented ``graphdog`` and ``guidedog-pages`` as separate programs. That contradicted the intended architecture and required users to install commands that belong inside Guidedog. We align the workspace, documentation, release archives and Homebrew formula before 1.0.0-beta.1.

Specification
-------------

.. code-block:: sh

   guidedog publish pages --repository=octo/manual --project=docs
   guidedog pages status
   guidedog pages recover
   guidedog pages workflow --write
   guidedog dsc check --dir=docs/gds

``guidedog publish pages`` builds through the host's engine. ``guidedog pages`` exposes initialization, authentication, binding, setup, diagnosis, watching, status, recovery and workflow generation. The plugin retains its state and credential formats. No registration is needed for a linked plugin.

The release workflow packages only ``guidedog``. Linux and macOS link Graphviz statically. Windows bundles the Graphviz runtime DLLs and configuration beside ``guidedog.exe``. Every archive carries notices, a source manifest and program checksums. The Homebrew formula installs only ``guidedog`` and tests publishing configuration and discussions through its plugin commands.

Rationale and alternatives
--------------------------

Libraries encapsulate implementation details. Plugins extend the one command line. The existing publisher process protocol remains supported for custom hosts and compatibility tests, with test-only adapters under ``crates/gd-pages/examples``. It does not require another release executable.

Security and operational considerations
---------------------------------------

Publishing still requires an authorized target and a token. Tokens stay in the credential store or the explicit environment credential path. The plugin keeps the publication journal and recovery behavior. The release smoke test initializes configuration offline and verifies that an unbound target cannot publish.

Backwards compatibility
-----------------------

Configuration files and publisher protocols stay compatible. Users of earlier separate programs move to ``guidedog pages`` and ``guidedog publish pages``. Graphviz rendering remains available through documentation directives and the ``gd-graphdog`` Rust API. ``guidedog dsc`` remains the discussion interface.

Acceptance criteria and implementation status
---------------------------------------------

The workspace builds with Rust 1.99. Source limits, Clippy and workspace tests must pass. Release archives must pass the HTML, PDF, discussion and Pages smoke tests on every platform. The website and discussions must build with Guidedog. Publication evidence is recorded after the release workflow finishes.

Open questions
--------------

None for the executable boundary.

Review history
--------------

On 15 October 2026 the maintainer confirmed that Pages is a plugin, Graphdog is a Graphviz library, and discussions are the ``dsc`` option of Guidedog. This record supersedes earlier descriptions of separate release programs.

References
----------

* :doc:`0008-github-pages-publishing`
* :doc:`0009-native-plugin-api`
* :doc:`0010-typed-build-plugins`

.. rubric:: Lifecycle event

::

   2026-10-15: prediscussion -> discussion. Align the release with the
   maintainer-confirmed single CLI design

.. rubric:: Lifecycle event

::

   2026-10-15: discussion -> accepted. Pages and discussions extend guidedog;
   Graphdog encapsulates Graphviz as a library

.. rubric:: Lifecycle event

::

   2026-10-15: accepted -> committed. Published one executable with linked Pages
   and discussion plugins; Graphdog remains a Graphviz library.
   https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.1 ;
   https://github.com/insanai/guidedog/actions/runs/38732558360 ;
   https://github.com/insanai/homebrew-guidedog/actions/runs/38737499042
