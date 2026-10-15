.. _intl:

Internationalization
====================

We translate the explanation and keep its structure. Guidedog extracts message
catalogs, merges translations, and builds for a chosen language. The source text
stays the reference. A missing translation falls back rather than inventing text.

The workflow
------------

.. graphviz::
   :class: graphviz-adaptive
   :caption: Sources become templates, templates become catalogs, and catalogs feed each language's build.
   :alt: The gettext builder writes POT templates from the sources. guidedog intl update merges them into one PO catalog per language. A translator fills the catalogs, and a build with a language reads them to make that language's site and book.

   digraph intl_workflow {
     graph [rankdir=TB, bgcolor="transparent", pad="0.22", nodesep="0.3", ranksep="0.35"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2", color="#216553",
           fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.18,0.1"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10, arrowsize=0.7];
     sources [class="gd-input", shape=note, fillcolor="#fbfaf3",
              label=<<B>Source documents</B><BR/><FONT POINT-SIZE="11">RST / Markdown</FONT>>];
     pot [label=<<B>Templates (.pot)</B><BR/><FONT POINT-SIZE="11">_build/gettext</FONT>>];
     po [class="gd-input", shape=note, fillcolor="#fbfaf3",
         label=<<B>Catalogs (.po)</B><BR/><FONT POINT-SIZE="11">locales/LANG/LC_MESSAGES</FONT>>];
     out [class="gd-output", fillcolor="#eef4fa",
          label=<<B>Site and book</B><BR/><FONT POINT-SIZE="11">in that language</FONT>>];
     sources -> pot [label="build gettext"];
     pot -> po [label="intl update"];
     po -> po [label="translate"];
     po -> out [label="build -D language"];
     sources -> out [style=dashed];
   }

1. Write the message templates with the ``gettext`` builder:

   .. code-block:: shell

      guidedog build gettext

   It writes one template (``.pot``) per text domain into ``_build/gettext``. Every
   paragraph, title, list item, table cell, caption, admonition, term, and field of the
   documents becomes a message, with the file and line it comes from.

2. Create or update the catalogs of each language, as ``sphinx-intl update`` does:

   .. code-block:: shell

      guidedog intl update -l de -l fr

   The catalogs appear in ``locales/de/LC_MESSAGES/*.po``. On later runs the command
   merges the new templates into them, as ``msgmerge`` does. Translations are kept. A
   message whose source changed gets the old translation, marked *fuzzy* for review.
   Messages no longer in the documents are kept at the end as obsolete (``#~``), so a
   translation can come back.

3. Translate: fill in each ``msgstr``. Remove the ``#, fuzzy`` line of a message once
   its translation is right. Fuzzy translations are not used.

   .. code-block:: po

      msgid "Hello *world*, see the site_."
      msgstr "Hallo *Welt*, siehe site_."

4. See how far the translation is, as ``sphinx-intl stat`` does:

   .. code-block:: shell

      guidedog intl stat -l de

5. Build in the language:

   .. code-block:: shell

      guidedog build html -D language=de
      guidedog build pdf -D language=de

   or set ``language = "de"`` in ``conf.toml``.

   Each book has a language suffix: ``manual.pdf`` becomes ``manual-de.pdf``.
   A build publishes one complete generation, so we use separate destinations to keep
   several editions. For example, we build German with
   ``guidedog build -b pdf docs build/books/de -D language=de`` and English with
   ``guidedog build -b pdf docs build/books/en -D language=en``.

Only the documents whose catalog changed are read again, as with any other input of a
document. Adding or removing a catalog reads every document again.

``guidedog intl`` takes sphinx-intl's options: ``-p DIR`` for the templates (default
``_build/gettext``), ``-l LANG``, repeatable (default: the project's language), ``-d
DIR`` for the catalogs (default: the first of ``locale_dirs``), ``-w N`` for the line
width (default 76), and ``--no-obsolete``.

Writing translations
--------------------

A message is the source text of a paragraph, title, or other element, with its markup:
reStructuredText in ``.rst`` documents, Markdown in ``.md`` ones. A translation is read
in the same markup, so it may emphasize, link, and refer as the original does.

References must stay the same. A translation keeps every reference of its message,
written the same way, and may only change the text around them and their titles:

.. code-block:: po

   msgid "See :ref:`install` and the site_."
   msgstr "Siehe :ref:`Installation <install>` und die site_."

When a translation drops a reference, the build warns (``inconsistent references in
translated message``, as Sphinx does) and uses the translation. A translation may name a
reference the original does not have. That reference could not be resolved, so the build warns and
keeps the original text. When the reader refuses a translation's markup, the build also keeps
the original, with a warning that quotes the translation.

Section anchors do not change with the language. A translated title keeps the anchor of
its original, so links into the site, from inside and outside, keep working.

Settings
--------

The standard settings have Sphinx's names and defaults. Guidedog also provides an
extraction-only exclusion setting for untranslated reference appendices.

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Setting
     - Meaning
   * - ``language``
     - The language to build in, such as ``"de"`` or ``"pt_BR"``.
   * - ``locale_dirs``
     - Where the catalogs are, relative to the source folder: each holds
       ``LANGUAGE/LC_MESSAGES/DOMAIN.po`` (or a compiled ``.mo``). Default
       ``["locales"]``. With several, the first that translates a message wins.
   * - ``gettext_compact``
     - Which catalog a document's messages go to. ``true`` (the default): a document at
       the top level has its own, and the documents of a folder share the folder's
       (``guide/usage.rst`` is in ``guide.po``). ``false``: one catalog per document. A
       name: one catalog of that name for every document.
   * - ``gettext_location``
     - Write each message's locations (``#: ../../index.rst:12``). Default ``true``.
   * - ``gettext_uuid``
     - Write an identifier per occurrence of a message. Guidedog makes them from the
       document, the message, and its occurrence, so they are the same in every build.
   * - ``gettext_auto_build``
     - Compile each ``.po`` into an ``.mo`` beside it, for other tools. Default ``true``.
       Guidedog itself reads the ``.po``.
   * - ``gettext_additional_targets``
     - More content to translate: ``index`` (index entries), ``literal-block``,
       ``doctest-block``, ``raw``, and ``image`` (alternative text).
   * - ``gettext_allow_fuzzy_translations``
     - Use fuzzy translations too. Default ``false``.
   * - ``gettext_exclude_patterns``
     - Document-path patterns omitted from POT extraction only, such as ``["api/**"]``.
       Default ``[]``. The documents remain in HTML and PDF.
   * - ``gettext_last_translator``, ``gettext_language_team``
     - The template header's ``Last-Translator`` and ``Language-Team``.
   * - ``figure_language_filename``
     - The file of an image in the build's language. Default
       ``"{root}.{language}{ext}"``: with ``language = "de"``, ``img/logo.png`` becomes
       ``img/logo.de.png`` when that file exists. The fields are ``{root}``, ``{path}``,
       ``{basename}``, ``{ext}``, ``{docpath}``, and ``{language}``.
   * - ``translation_progress_classes``
     - Give translated elements the class ``translated`` and the others
       ``untranslated`` (``true``), or only one of them (``"translated"``,
       ``"untranslated"``), so a stylesheet can show what is left.

Guidedog's own words
--------------------

Guidedog writes some words itself: admonition titles, "Added in version 2.1", "Fig. 1",
"Contents", the page layout's "Previous", "Next", "Navigation", and "Search", the search
and index pages, and the book's "Version". They come from the text domain ``guidedog``,
looked up in this order:

1. ``locales/LANGUAGE/LC_MESSAGES/guidedog.po`` of the project,
2. ``locales/LANGUAGE/LC_MESSAGES/sphinx.po`` of the project, as Sphinx reads it,
3. the catalog built into Guidedog, for German, Spanish, French, Japanese, Brazilian
   Portuguese, Russian, and Simplified Chinese.

The English words are the message identifiers. They are Sphinx's where Sphinx has
the same word, so a project's ``sphinx.po`` keeps working. A project can override any
word, or add a language, with its own ``guidedog.po``.

Templates translate with ``_()``, ``gettext()``, and ``ngettext()``, as Sphinx's
templates do:

.. code-block:: html+jinja

   <small>{{ _('Previous') }}</small>
   {{ _('Last updated on %s.')|format(last_updated) }}

``numfig_format``, ``html_title``, and ``html_short_title`` follow the language unless
``conf.toml`` sets them.

Dates
-----

``|today|``, the book's date, and a page's ``last_updated`` are formatted as Sphinx
formats them. ``today_fmt`` and ``html_last_updated_fmt`` are ``strftime`` formats.
The names of months and days follow ``language``, so ``%b %d, %Y`` is "Sep 29, 2026" in
English and "Sept. 29, 2026" in German. An empty format is ``_('%b %d, %Y')``, which a
catalog may translate. The names come from the Unicode CLDR for the languages above.
Other languages use English, as Sphinx does for a language Babel does not know.

The date is the local date, in the local time zone. We set ``SOURCE_DATE_EPOCH`` (seconds
since 1970, in UTC) to build reproducibly, with the same dates on every machine.
