.. _media:

Images, video, audio, and downloads
===================================

Keep a resource near the document that explains it. Name it with a relative path,
or start the path with ``/`` to refer to the source root. Guidedog copies resources
used by the documents. A video remains a video on the web; a PDF names the recording.

Images
------

.. code-block:: rst

   .. image:: diagrams/pipeline.png
      :alt: The build pipeline
      :width: 480px

   .. figure:: /screenshots/search.png

      Searching the site.

Pages copy images into ``_images``; the book embeds them. PNG, JPEG, GIF, SVG, and WebP
work in both. A missing image is a warning: the page keeps the image's alternative
text, and the book leaves the image out.

Video and audio
---------------

An image or figure that names a video (``.mp4``, ``.webm``, ``.ogv``, ``.mov``,
``.m4v``) or an audio file (``.mp3``, ``.ogg``, ``.oga``, ``.m4a``, ``.opus``, ``.wav``,
``.flac``, ``.aac``) plays it, as Docutils does for video:

.. code-block:: rst

   .. figure:: media/tour.mp4
      :alt: A tour of the site
      :width: 640

      The two-minute tour.

   .. image:: media/pronunciation.mp3

Pages show the browser's player, with a link to the file for browsers that cannot play
it. A book cannot play media, so it names the recording in a box where the player would
be; text output writes ``[video: ...]`` or ``[audio: ...]``.

Downloads
---------

In HTML, the ``download`` role copies a file into ``_downloads`` and offers it
for saving:

.. code-block:: rst

   Get :download:`the example project <examples/project.zip>`.

For portable PDF download links, set ``html_baseurl`` to the published HTML site's
absolute HTTP or HTTPS address. The book then links to that site's copied files.
Without it, the link remains source-relative; the PDF does not embed attachments.
This setting does not upload files. Publish the HTML site before sharing the book.
This behavior is available in Guidedog 0.6.3 and later.

Theme files
-----------

``_static`` holds the files the theme uses (style sheets, scripts, fonts, a logo),
copied as they are into ``_static`` in the site. ``html_static_path`` lists it, and
``html_css_files``, ``html_js_files``, ``html_logo``, and ``html_favicon`` name files
in it. Content media belongs with the documents instead, so that moving a document
with its media keeps its links working.
