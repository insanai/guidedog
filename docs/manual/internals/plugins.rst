Plugins: SDK and process protocols
==================================

``gd-plugins`` is the plugin SDK: typed build plugins (GDS 0010) and publishers (GDS 0009), linked into an application or run as installed programs. This page records the design and the wire contract that every plugin program speaks. The crate's ``README.md`` is the API cheat sheet.

Architecture
------------

One implementation, two deployments
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

A plugin crate implements ``BuildPlugin`` (or ``Publisher``) once and exposes
``pub fn plugin() -> Plugin``. The same type then reaches a build in one of two ways:

Linked
   A custom ``guidedog`` binary compiles the crate in and passes its plugins to the CLI:
   ``gd_cli::run_with(Registry::new(vec![badge::plugin()])?)``. The gd-cli entry is ``pub fn run_with(linked: gd_plugins::Registry) -> std::process::ExitCode``, beside ``run()``, which uses an empty registry. Linked code is trusted: a panic or
   hang in a hook reaches the host.

Served
   The crate's small binary is ``fn main() -> ExitCode {
   gd_plugins::serve_plugin(&badge::plugin()) }``. Once registered with
   ``guidedog plugin register ID PATH`` (pinned by SHA-256), it runs as a child process
   per build session behind the protocol below. A crash or stall becomes a problem.

``examples/build_plugin/badge.rs`` is the library half and ``main.rs`` the served half.
A test runs the badge both ways and requires identical outputs.

Selecting plugins in a build
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

``gd-project`` and ``gd-cli`` implement these rules on top of this crate:

* ``conf.toml`` selects IDs in ``plugins`` (at most ``MAX_BUILD_PLUGINS`` = 64, valid
  and unique), their configuration files in ``[plugin_configs]`` (relative to the
  configuration folder), and ``plugin_timeout`` (1 to 86400 seconds, default 60) per
  whole program request. Hooks run in the listed order.
* Lookup order: the linked registry first, then the machine registry, read only when
  needed; a registered program is verified against its SHA-256 before it starts
  (``select_build_plugin``). An ID in neither is a configuration error.
* ``--untrusted`` refuses any selected plugin before starting code
  (``plugins.untrusted``, exit 2): "Remove plugins from this build, or review the
  project and build without --untrusted."
* Assets are collected once, at setup, and only for the HTML builders (``html``,
  ``dirhtml``, ``singlehtml``); they are staged under ``_static/plugins/ID/`` and may
  not collide with project files.
* CLI: ``guidedog plugin check|register|list|inspect|unregister|run`` and
  ``guidedog publish``; ``check`` and registration of a build program run
  ``check_plugin`` through ``process_plugin``.

Command-line extensions
~~~~~~~~~~~~~~~~~~~~~~~

A plugin of either kind may also add commands to guidedog's command line. The GitHub Pages plugin adds two:

.. code-block:: console

   $ guidedog publish pages          # the guided publication
   $ guidedog pages workflow --write # the GitHub Actions route

Neither is written in ``gd-cli``. The plugin declares them, and guidedog finds them when it meets a word it does not know.

Declaring
   ``BuildPlugin::commands`` and ``Publisher::commands`` return a list of ``CommandInfo``. Each has a ``path`` of 1 to 4 lowercase words (``["pages", "workflow"]``), a one-line ``summary``, plain-text ``help``, and the ``options`` it takes (``CommandOption``: a name such as ``--write``, a value placeholder such as ``DIR`` or nothing for a switch, and a summary). The default is no commands. A build plugin that only adds commands may offer no hooks (``Hooks::NONE``), as the discussion plugin (``guidedog dsc``) does; a session cannot load it. ``Registry::new`` checks the declarations: at most 32 commands, distinct names, bounded text. The declared options serve the help and the host's parsing; the plugin still reads its own arguments, so a command is free to have nested subcommands of its own.

Running
   ``run_command(&mut self, ctx: &mut CommandContext, name, args) -> Result<i32, Problem>`` runs the command ``name`` (its words joined by spaces) on a fresh, unloaded value. ``args`` is the command line without the command's words, global options included. ``Ok`` is the exit status; a ``Problem`` is reported by the host and its ``exit`` becomes the status.

``CommandContext``
   What the host offers, and nothing more. Its methods are ``print`` and ``eprint`` (or ``streams`` for both writers at once), ``ask`` and ``confirm`` (they return no answer when ``interactive`` is false, so a script never hangs), and ``build_project(&BuildRequest)``, which has the host build a project with its own engine and linked plugins. A ``BuildRequest`` names the project, the output folder, the builder (``html`` by default, or ``pdf``, ``dirhtml``, ``singlehtml``, ``text``), ``-D`` overrides, the base URL and ``strict``. Its public fields say where the command runs (``dir``), the plugin's configuration file when the project's ``plugin_configs`` names one (``config``), the host program (``host_program``), the reporting choices (``interactive``, ``color``, ``json``, ``quiet``) and the ``cancel`` probe. The context borrows a ``CommandHost``, a trait with ``streams``, ``read_line`` and ``build_project``: ``gd-cli`` implements it over its console, tests over buffers.

Dispatch in ``gd-cli``
   ``extensions(&linked, machine)`` lists the commands of the linked plugins, then those of the registered programs (the machine registry is read only when the command line may name one). A name guidedog already has is skipped, except targets of ``publish``; an earlier plugin wins a name. The parser matches the longest path against the command line's words, skipping the values of options (guidedog's and every declared option that takes a value), so ``publish --config pages.toml`` never names a target ``pages.toml``. ``--help`` on a contributed command prints help made from its declaration, without running the plugin. ``guidedog --help`` lists the commands under PLUGIN COMMANDS with their plugin IDs, ``guidedog publish --help`` lists the targets, and "Did you mean" draws on both guidedog's commands and the plugins'. ``guidedog plugin inspect ID`` shows the commands a plugin adds.

Which plugins are linked is the program's decision, not the command line's: ``cmd/guidedog`` passes ``Registry::new(vec![gd_pages::plugin(), gd_discussion::plugin()])`` to ``gd_cli::run_status``, and ``gd-cli`` depends only on ``gd-plugins``. The dependency gate holds that rule.

Identity and cache keys
~~~~~~~~~~~~~~~~~~~~~~~

``Plugin::identity(host_build)`` is what must change for a plugin's outputs to be
rebuilt. A served plugin's identity is its registered program's SHA-256. A linked
plugin's is the SHA-256 of its ID, product version and interface version together with
``host_build``, the host's own build identity, because
its code is part of the host. A build folds, per plugin and in order: the identity,
ID and version, the session's ``cache_key`` (which ``LoadContext::read_config`` and
``read_file`` extend with fingerprints of the exact bytes read), the language and
builder, the digest of the configuration file and of every declared dependency, and,
for HTML builders, every asset's path and bytes.

Concurrency
~~~~~~~~~~~

There is one session per selected plugin per build. Sessions are never shared between builds,
because state, cache key and dependencies belong to one build. Hooks take
``&mut self``. A host calls them from one thread or holds the sessions in a
``Mutex`` shared by reading workers (``PluginSession`` is ``Send``; a test drives one
from several threads). Plugins need no synchronization. ``Plugin`` and ``Registry``
are ``Send + Sync`` and immutable.

Evolution
~~~~~~~~~

Host-made inputs (``SourceInput``, ``HtmlInput``, ``PluginOptions``, ``LoadOptions``,
``SnapshotOptions``, ``Handshake``, ``MachineEntry``) are ``#[non_exhaustive]`` with
constructors, so a new field is not a breaking change for plugin authors. New hooks
arrive as trait methods with "absent" defaults plus a ``Hooks`` flag. Incompatible
changes bump ``PLUGIN_API_VERSION`` or ``API_VERSION`` (checked at registration and
load) and the protocol versions, which are negotiated separately.

Design
------

A ``Plugin`` is either build hooks or a publisher, never both. A normal build never
invokes a publisher. Both kinds are traits whose value *is* one session's state:

``BuildPlugin``
   ``info`` (identity and ``Hooks``), ``load(&mut LoadContext)``, ``process_source``,
   ``process_html`` (both take a borrowed input and return ``Cow<'a, str>``) and
   ``collect_assets``. Default methods mean "service absent": text passes through and
   no assets are returned. A plugin releases what it holds in ``Drop``; there is no ``unload`` hook.

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

Build hooks and publishers converse over pipes, as described next. A contributed command does not: it runs on the person's terminal (see `Command protocol`_).

A host starts a program with ``--guidedog-protocol``. The two exchange one JSON object
per line over the child's standard input and output. Standard error is the
child's log for people. Two helper threads own the pipes: a writer sending request
lines and a reader framing response lines (at most 64 MiB). The host waits on their
channels with one monotonic deadline for the whole exchange (write and response). It
polls the ``Cancel`` probe every 5 ms, so a child can neither block a write nor keep a
request alive by trickling bytes. Any failure other than a refusal with a problem marks
the child broken: nothing more is sent, and stopping kills it. Stopping closes its
input and waits one second before killing it. Unload is sent with a one-second limit
and no cancellation.

Timeouts: handshake 10 s, publisher requests 5 min, build requests 60 s by default.
Messages nest at most 16 levels and repeat no key. Lines are UTF-8.

Wire contract
-------------

Field names and order are fixed: a plugin program depends on them. The child speaks first:

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

Command protocol
~~~~~~~~~~~~~~~~

Either handshake may carry two optional fields, which older hosts ignore and older programs leave out:

.. code-block:: json

   {"command_versions":[{"major":1,"minor":0}],
    "commands":[{"path":["badge","preview"],"summary":"Write a product's label",
                 "help":"...","options":[{"name":"--quiet-ok","value":"",
                 "summary":"Print nothing on success"}]}]}

The command protocol is versioned on its own (``COMMAND_PROTOCOL_VERSION`` 1.0), so the build and publisher protocols keep their versions. Registration selects the shared major version and refuses commands in a version the host does not speak (``plugins.command-version``), or declarations that break the limits above.

To run a command, the host starts the program with two arguments, ``--guidedog-command`` and one JSON request (at most 64 KiB):

.. code-block:: json

   {"protocol":"guidedog-command","version":{"major":1,"minor":0},
    "command":["badge","preview"],"args":["Orbit"],"dir":"/home/ada/docs",
    "config":"","host":"/usr/bin/guidedog","interactive":true,"color":true,
    "json":false,"quiet":false}

The program inherits the host's standard input, output and error, in the folder ``dir``. There is no conversation: the command writes to the terminal and asks the person directly, and its exit status is the command's (a signal becomes 128 plus its number). The host flushes its own output first and verifies the program's SHA-256 before starting it. ``serve``, ``serve_publisher`` and ``serve_plugin`` answer the request by themselves, and ``serve_command(&plugin, request)`` lets a program with its own ``main`` do it. In the program, ``build_project`` starts ``host`` with ``build -b BUILDER PROJECT OUTPUT``, its output sent to standard error; a problem is reported on standard error as the request asks (text or JSON).

Why the terminal and not the pipes? A guided command asks questions, shows progress and may run for minutes. Relaying every keystroke through JSON would add a second, harder protocol for no gain; the program is already trusted to the extent of its pinned SHA-256.

Machine registry
----------------

``plugins.toml`` in ``$GUIDEDOG_CONFIG_DIR`` or the user's configuration folder
(``guidedog/``), schema 1, readable only by the user, at most 1 MiB. Each
``[[plugins]]`` table has ``id``, ``name``, ``version``, absolute ``path``,
``sha256``, ``protocol = "1.0"``, ``capabilities``, ``registered`` (UTC date) and, for
build programs only, ``protocol_name = "guidedog-build-plugin"`` and ``hooks``. A program that adds commands also has ``command_protocol = "1.0"`` and ``commands``, an array of inline tables with ``path``, ``summary``, ``help`` and ``options`` (each ``{ name, value, summary }``). Entries without them are read as adding none.
Mutations take ``plugins.toml.lock`` with ``File::try_lock`` (separate opens conflict
even within a process), then write ``plugins.toml.next``, sync it, and rename it over
the registry. ``inspect_entry`` accepts only absolute paths to ELF, Mach-O or PE files
and runs the program's handshake; ``verify_entry`` re-hashes it before use.

Alternatives considered
-----------------------

Dynamic libraries (``dlopen``, ``abi_stable``)
   Rust has no stable ABI. A plugin would have to be built by the same compiler with
   the same crate versions as the host. Every call would cross ``unsafe`` FFI, and a crash
   would take the host down. ``abi_stable`` narrows this at the cost of a large dependency and
   FFI-shaped types. Linking at compile time plus processes covers both needs safely.

WebAssembly (``wasmtime``)
   A strong sandbox and a portable binary, but a very large dependency and toolchain
   for plugin authors. It is a possible future deployment for untrusted plugins. The trait and
   protocol shapes would carry over.

Automatic registration (``inventory``, linker sections)
   Needs extra crates and hides the wiring. An explicit ``Registry::new(vec![...])``
   keeps the list of linked code visible in the host's ``main``.

What the SDK leaves out
-----------------------

Owners release what they hold through ``Drop``, so the SDK has no release procedures and no accounting of refused allocations. ``std::process`` covers pipes and handle inheritance on every platform. The CLI commands (``guidedog plugin register``/``check``/``inspect``) belong to ``gd-cli``.
