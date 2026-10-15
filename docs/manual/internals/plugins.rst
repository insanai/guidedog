Plugins: SDK and process protocols
==================================

``gd-plugins`` ports the Odin ``lib/plugins`` package: typed build plugins (GDS 0010)
and publishers (GDS 0009), linked into an application or run as installed programs.
This page records the Rust design and the wire contract that must stay compatible with
programs built by the Odin SDK. The crate's ``README.md`` is the API cheat sheet.

Design
------

A ``Plugin`` is either build hooks or a publisher, never both; a normal build never
invokes a publisher. Both kinds are traits whose value *is* one session's state:

``BuildPlugin``
   ``info`` (identity and ``Hooks``), ``load(&mut LoadContext)``, ``process_source``,
   ``process_html`` (both take a borrowed input and return ``Cow<'a, str>``) and
   ``collect_assets``. Default methods mean "service absent": text passes through and
   no assets are returned. ``Drop`` replaces ``unload``.

``Publisher``
   ``info`` (identity and ``Capabilities``), ``load`` (returns ``Loaded``: source and
   target), ``plan``, ``publish``, and optional ``status`` and ``recover``.

``Plugin::build(prototype)`` and ``Plugin::publisher(prototype)`` clone the unloaded
prototype for every session. ``Registry::new`` validates IDs, names, versions,
interface versions and the presence of at least one hook, so an invalid registry
cannot exist. ``PluginSession`` and ``PublisherSession`` wrap a loaded value and check
every call:

* build inputs and outputs are at most 8 MiB, names and paths bounded; a hook's
  problem must have a title, message, hint and nonzero exit, gains "Plugin ID stopped
  during STAGE." and never carries the publication statuses 6 or 7 (they become 4);
* assets: at most 256 files and 16 MiB, portable relative paths (no traversal, drive,
  backslash, control bytes, ``.git``, Windows device names, trailing dot or space),
  no case-insensitive duplicates and no file used as another's folder;
* ``LoadContext::read_config`` (1 MiB) and ``read_file`` (8 MiB) declare the file and
  append ``:`` and the SHA-256 of the bytes read to the cache key, which only grows;
* a publisher's plan that is unchanged yet lists changes, an outcome without an
  operation ID, with another generation, or with no state is ``Unknown`` (exit 7),
  never success; a plan is published only by the session (serial number) and the
  snapshot (digest) it was made from; a dry-run session never publishes or recovers.

``Snapshot`` freezes a site: regular files only, servable names, no case collisions,
bounded counts and sizes (defaults 100,000 files, 100 MiB less a byte per file, 1 GiB),
``index.html`` unless ``no_index``. Its digest is the SHA-256 of
``guidedog-snapshot 1\n`` followed, per entry in path-byte order, by the path length
(8 bytes big-endian), the path, the size (8 bytes) and the file's SHA-256. ``verify``
re-reads it with the accepted manifest's own bounds. A copied or adopted snapshot's
folder is removed on ``Drop``.

Process transport
-----------------

A host starts a program with ``--guidedog-protocol``; the two exchange one JSON object
per line over the child's standard input and output, and standard error is the
child's log for people. Two helper threads own the pipes: a writer sending request
lines and a reader framing response lines (at most 64 MiB). The host waits on their
channels with one monotonic deadline for the whole exchange (write and response) and
polls the ``Cancel`` probe every 5 ms, so a child can neither block a write nor keep a
request alive by trickling bytes. Any failure other than a refusal with a problem marks
the child broken: nothing more is sent, and stopping kills it. Stopping closes its
input and waits one second before killing it. Unload is sent with a one-second limit
and no cancellation.

Timeouts: handshake 10 s, publisher requests 5 min, build requests 60 s by default.
Messages nest at most 16 levels and repeat no key; lines are UTF-8.

Wire contract
-------------

Field names and order match the Odin SDK. The child speaks first:

.. code-block:: json

   {"protocol":"guidedog-build-plugin","versions":[{"major":1,"minor":0}],
    "api":{"major":1,"minor":0},"id":"product-badge","name":"Product badge",
    "version":"1.0.0","capabilities":[],"snapshot_schemas":[],
    "hooks":["source","html","assets"]}

A publisher's handshake uses ``"protocol":"guidedog-publisher"``, ``api`` 1.1, its
``capabilities`` (``status``, ``recover``) and ``"snapshot_schemas":[1]``. The host
selects the shared major version at the lower minor version and sends
``{"id":1,"operation":"select","version":{"major":1,"minor":0}}``. Every response
echoes ``id`` and has ``ok``; a refusal is ``{"id":N,"ok":false,"problem":{"code",
"title","path","message","hint","exit"}}`` (exit outside 1 to 125 becomes 1, an empty
code ``plugins.remote``).

Build protocol requests (IDs strictly increase; select exactly once, load before
hooks):

* ``load`` with ``"options":{"config","project","builder","language"}``, answered by
  ``"cache_key"`` and ``"dependencies"`` (at most 256 paths);
* ``source`` with ``"input":{"path","docname","format","language","text"}``
  (``format`` is ``rst`` or ``markdown``), and ``html`` with
  ``"input":{"path","docname","language","text","assets_url"}``, both answered by
  ``"text"``;
* ``assets``, answered by ``"assets":[{"path","base64"}]`` (padded standard Base64);
* ``unload``, after which the program exits.

Publisher protocol requests (lines at most 1 MiB):

* ``load`` with ``"config","dry_run","wait_seconds"``, answered by
  ``"source":{"project","site","base_url","strict"}`` (exactly one of project and
  site) and ``"target"``;
* ``plan`` with ``"snapshot":{"root","digest","schema":1,"max_files",
  "max_file_bytes","max_bytes"}``; the program re-freezes the host's folder (borrowed,
  with those bounds) and refuses a different digest; answered by
  ``"plan":{"id","generation","target","baseline","summary","unchanged","changes":
  [{"path","kind"}]}`` with kinds ``added``, ``changed``, ``removed``;
* ``publish`` with ``"plan"`` (its ID) and ``"generation"``; ``status`` with
  ``"publication"``; ``recover``; each answered by ``"result":{"state","operation",
  "generation","url","problem"}``, where state is ``not-started``, ``unchanged``,
  ``accepted``, ``pending``, ``live``, ``failed`` or ``unknown``. When a writing
  request gets no answer the host reports ``Unknown`` with the plan's generation;
* ``unload``.

Machine registry
----------------

``plugins.toml`` in ``$GUIDEDOG_CONFIG_DIR`` or the user's configuration folder
(``guidedog/``), schema 1, readable only by the user, at most 1 MiB. Each
``[[plugins]]`` table has ``id``, ``name``, ``version``, absolute ``path``,
``sha256``, ``protocol = "1.0"``, ``capabilities``, ``registered`` (UTC date) and, for
build programs only, ``protocol_name = "guidedog-build-plugin"`` and ``hooks``.
Mutations take ``plugins.toml.lock`` with ``File::try_lock`` (separate opens conflict
even within a process), then write ``plugins.toml.next``, sync it, and rename it over
the registry. ``inspect_entry`` accepts only absolute paths to ELF, Mach-O or PE files
and runs the program's handshake; ``verify_entry`` re-hashes it before use.

Not ported
----------

The arena owners and their ``release_*`` procedures, allocation-refusal accounting
(``memory_refused`` and the tests that refused each allocation in turn), Windows pipe
and handle-inheritance code (``std::process`` covers it), and the exported
``try_lock``/``unlock``, line reader and JSON field helpers. The CLI commands
(``guidedog plugin register``/``check``/``inspect``) belong to ``gd-cli``.
