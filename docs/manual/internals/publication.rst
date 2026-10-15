Publish one complete generation
===============================

A reader should never see half an updated site.
This is a publication property. It is separate from parsing and rendering.

The invariant
-------------

Let :math:`P` be the published output tree. During preparation, :math:`P` remains
unchanged. The next tree :math:`Q` is complete before the commit is decided.
The commit journal names the output and the records that belong with it.

**Preparation can fail.** The host abandons the unfinished generation.
**A decided commit can be interrupted.** The next build finishes or restores it
from the journal. It does not guess from a staging directory's name.

.. graphviz::
   :class: graphviz-adaptive
   :caption: The durable journal separates preparation from a decided commit.
   :alt: Prepare and flush, save journal, switch output, replace records, then remove journal.

   digraph commit {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.4", ranksep="0.4"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.22,0.14"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10];

     prepare [class="gd-input", label=<<B>1. Prepare Generation Q</B><BR/><FONT POINT-SIZE="11">Render, verify, and flush files</FONT>>, fillcolor="#fbfaf3"];
     journal [label=<<B>2. Save Durable Journal</B><BR/><FONT POINT-SIZE="11">Commit is decided and durable</FONT>>, shape=cylinder, fillcolor="#edf5f2"];
     switch [label=<<B>3. Switch Generation</B><BR/><FONT POINT-SIZE="11">Journaled directory switch</FONT>>, fillcolor="#e0eee9"];
     records [label=<<B>4. Update Manifests</B><BR/><FONT POINT-SIZE="11">Replace environment cache records</FONT>>, shape=cylinder, fillcolor="#edf5f2"];
     settle [label=<<B>5. Settle Directory Names</B><BR/><FONT POINT-SIZE="11">Flush directory entries and remove journal</FONT>>, fillcolor="#eef6f2"];

     prepare -> journal -> switch -> records -> settle;
   }

Why the cache is outside the output
-----------------------------------

A directory exchange moves everything inside the publication directory.
A journal placed inside it would move during the very operation it describes.
Its location would cease to be stable when recovery needed it most.
Guidedog keeps records in a stable directory outside the publication tree.
A custom output uses a sibling cache for this reason.

The spare output and the previous output live in the build directory too, as ``.<name>.next`` and ``.<name>.old``.
For a default output this is the folder beside it: ``_build/.html.next`` sits next to ``_build/html``.
For a custom output it is the sibling cache: ``-o locales/templates`` leaves ``locales/.templates.guidedog`` beside the output, and nothing else.
The cache is a sibling of the output, so it is on the same filesystem, and the switch stays one exchange or two renames.
Earlier versions kept a custom output's ``.<name>.next`` and ``.<name>.old`` beside it.
The next build moves such a spare output into the cache, or removes the folder, and says so in a note.
It does this only when the folder is clearly Guidedog's: every file in it is a stage file, a file the manifest names, or the same file (by inode) as the output's file at that path.
A folder holding anything else, or a file with such a name, is left as it is, with a note that says so.

Durability and visibility
-------------------------

Flushing file contents is not the same as flushing directory names.
The algorithm accounts for both before removing the journal.
Platforms differ in how directories are exchanged.
The recovery protocol is designed around those differences.

The implementation is split along that line. ``gd-outputs`` owns the generation. A
``Generation`` value stages the next tree, records each output unit in the manifest,
writes the commit journal, and recovers an interrupted journal when the next build
begins. Dropping an uncommitted generation abandons it. ``gd-host`` owns the file
operations. ``Stage`` writes a file beside its target and removes it when dropped
uncommitted. ``exchange`` switches two folders in one call where the system offers it
(``renameat2`` on Linux, ``renamex_np`` on macOS). ``sync_all`` and ``sync_each``
flush file contents and folder entries. Stage names keep one prefix
(``.guidedog-stage-``), so a later build removes what a killed process left.

This protects the project publication. It is not a distributed transaction
with an external hosting provider. Copying a successful build to a server is
another operation, owned by the deployment tool.

On filesystems with directory exchange, the output path switches in one operation.
The fallback uses two renames and can briefly leave that path absent. It still
never exposes a partly assembled tree. Journal recovery finishes or restores an
interrupted switch. A client making several requests can span two generations;
the build does not promise a snapshot for an entire browsing session.
