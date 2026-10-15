Build and publish documentation
===============================

Publishing documentation involves turning raw reStructuredText and Markdown sources
into static artifacts, verifying their integrity, and deploying them to a hosting
environment.

Guidedog separates the process of building a site from the process of deploying it:

1. **Building:** Guidedog reads your sources, compiles templates, resolves cross-references,
   and outputs a complete, verified directory of static HTML files or a typeset PDF book.
2. **Publishing:** The resulting static files are served by your web server or deployed
   to a hosting destination, such as GitHub Pages, an S3 bucket, or an internal file server.

Because the output consists entirely of standard static files (HTML, CSS, JavaScript, and SVG),
it can be served by any static web server (such as Nginx, Apache, or Caddy) without requiring
Guidedog, a Python interpreter, or an active server process in production.

Atomic publication architecture
-------------------------------

A critical design requirement is to publish complete output generations.
The host finishes preparation before deciding a journaled commit. A failed
preparation leaves the previous site intact.

Guidedog prepares a complete generation before deciding a journaled commit:

.. graphviz::
   :class: graphviz-adaptive
   :caption: The atomic staging and generation switch process.
   :alt: A complete staged generation is journaled and switched into place. Recovery finishes or restores an interrupted commit.

   digraph atomic_publication {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.4", ranksep="0.4"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.22,0.14"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10];

     step1 [class="gd-input", label=<<B>1. Staging Generation</B><BR/><FONT POINT-SIZE="11">Render all documents into temporary<BR/>.&lt;target&gt;.next</FONT>>, fillcolor="#fbfaf3"];
     step2 [class="gd-input", label=<<B>2. Flush and Verify</B><BR/><FONT POINT-SIZE="11">Sync all written files, assets, and<BR/>manifests to disk</FONT>>, fillcolor="#fbfaf3"];
     step3 [label=<<B>3. Durable Journal</B><BR/><FONT POINT-SIZE="11">Record decided commit record outside the<BR/>target tree</FONT>>, shape=cylinder, fillcolor="#edf5f2"];
     step4 [label=<<B>4. Atomic Generation Switch</B><BR/><FONT POINT-SIZE="11">Exchange where supported; recoverable<BR/>renames otherwise</FONT>>, fillcolor="#e0eee9"];
     step5 [label=<<B>5. Settle Commit</B><BR/><FONT POINT-SIZE="11">Replace cache environment and remove<BR/>journal</FONT>>, fillcolor="#eef6f2"];

     step1 -> step2 -> step3 -> step4 -> step5;
   }

How atomic commits protect your site:

* **Staging:** All newly rendered pages are written to a hidden staging folder beside
  the destination (``.<target>.next``).
* **Isolation:** The existing live output directory is untouched while pages are being compiled.
* **Failure safety:** If any page fails to render or if warnings-as-errors (``-W``) is
  triggered, the staging folder is abandoned. The existing live site remains completely unchanged.
* **Atomic swap:** Once all files are flushed, Guidedog switches the staging directory into
  place. On supported filesystems, this is an instantaneous directory exchange.
* **Interrupted recovery:** If a crash occurs during the swap, the durable journal ensures
  that the subsequent build settles the interrupted commit rather than guessing.

Standard build workflow
-----------------------

To build both HTML and PDF outputs into the default project output directory:

.. code-block:: sh

   guidedog build html docs/manual -j auto -W
   guidedog build pdf docs/manual -W

Output locations:

* The HTML website is written to ``docs/manual/_build/html/``.
* The PDF book is written to ``docs/manual/_build/pdf/``.

The ``-j auto`` flag parallelizes document parsing across all available CPU cores.

Custom output directories
-------------------------

To direct the compiled website to an explicit target folder (for example, in a CI/CD
pipeline or deployment script), use the Sphinx-compatible command form:

.. code-block:: sh

   guidedog build -b html docs/manual build/site -W

Guidedog enforces safety constraints on output paths:

* Choose a dedicated output folder, such as ``_build/html`` inside the project or
  ``build/site`` beside it. The output must not be the source root or one of its
  ancestors, because switching it would replace the source project.
* When targeting a custom destination such as ``build/site``, Guidedog places internal
  caches in a sibling directory (such as ``build/.site.guidedog/.doctrees``), ensuring
  that the published folder contains only clean, deployable static assets.

Semantic checks and pre-flight validation
-----------------------------------------

To verify the semantic validity of a project without writing HTML files to disk, use the
``dummy`` builder:

.. code-block:: sh

   guidedog build dummy docs/manual -n -W

The dummy builder:

* Reads all documents and executes all syntax parsers.
* Resolves all toctrees, headings, cross-references, and domain objects.
* Checks document references and domain objects (``-n`` enables nitpicky
  cross-reference checking). It does not make HTTP requests to test external URLs.
* Exits with status 1 if any broken link or warning is found (``-W``).

Use the dummy builder in continuous integration pipelines for fast validation before running
the full site rendering.

Reproducible build timestamps
-----------------------------

Documentation builds often embed timestamps (for example, in "Last updated" footer notices).
For automated deployments where you want byte-for-byte reproducible outputs across multiple runs,
pin the clock using the standard ``SOURCE_DATE_EPOCH`` environment variable:

.. code-block:: sh

   SOURCE_DATE_EPOCH=1790812800 guidedog build html docs/manual -W

When this variable is set, Guidedog uses the specified Unix epoch timestamp instead of
the current system clock for all date formatting and template variables.

Deployment options
------------------

Once the build succeeds, deploy the static files to your hosting infrastructure:

1. **Static web servers (Nginx / Caddy / Apache):** Point the server's root directive
   directly to the built HTML directory.
2. **Cloud storage buckets (AWS S3 / Cloudflare R2 / Google Cloud Storage):** Sync the
   contents of ``_build/html`` to your bucket using your cloud provider's CLI tool.
3. **GitHub Pages:**

   * For direct deployments from your local machine, run ``guidedog publish pages``: it
     asks for what is still missing (the repository, a token, the Pages settings), then
     builds and publishes in one push. ``guidedog-pages`` offers the same steps one by
     one. See :doc:`github-pages`.
   * For automated deployments with GitHub Actions, ``guidedog pages workflow`` writes a
     workflow that builds and deploys the site; see :doc:`github-pages`.
   * For any other destination, write a publisher plugin as described in
     :doc:`../plugins`, and run it with ``guidedog publish --publisher=ID``.

On filesystems with directory exchange, the output path switches in one operation.
The fallback uses two renames and can briefly leave that path absent. It still
never exposes a partly assembled tree. Journal recovery finishes or restores an
interrupted switch. A client making several requests can span two generations;
the build does not promise a snapshot for an entire browsing session.
